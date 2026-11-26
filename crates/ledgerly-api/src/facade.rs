//! Api facade (card W1, Wave 2). The app, the CLI and the MCP server use only this module.
//!
//! Frozen interface (plan card W1):
//!
//! ```text
//! pub struct Api { .. }
//! impl Api { pub fn execute(&mut self, cmd: Command) -> Result<Outcome<CommandResult>, ApiError>; pub fn report(&self, p: &ReportParams) -> Result<Outcome<Report>, ApiError>; pub fn search(&self, q: &str) -> Vec<SearchHit>; }
//! ```
//!
//! # Construction
//!
//! [`Api::create`] makes a company file and seeds it with the reserved groups and the default
//! voucher types, as audited master events. [`Api::open`] opens one. Both load the books into a
//! cache, which every successful commit refreshes.
//!
//! # Changes
//!
//! [`Api::execute`] runs the engine on the cached books. The engine's events are committed in one
//! store transaction, and the cache is reloaded from the store only after that commit. A refused
//! command or a failed commit leaves the store and the cache unchanged. An unsupported command
//! commits nothing.
//!
//! Changes are attributed to the actor set by [`Api::set_actor`]. Until it is called, the actor
//! is user 1, the local user of a single-user company file. [`Api::open`] stamps audit records
//! with the system clock until [`Api::set_clock`] is called.
//!
//! # Period lock
//!
//! The lock date is the last `PeriodLocked` event in the audit log (spec section 5, invariant 8).
//! Every voucher command uses the lock-aware engine functions with it.
//!
//! # Unsupported input
//!
//! When the store cannot project a voucher (`StoreError::Unsupported`), the books cannot be
//! read. [`Api::report`] and [`Api::execute`] then return `Outcome::Unsupported`, naming the
//! voucher, and never an empty result.

use std::path::Path;

use ledgerly_assist::provider::AiProvider;
use ledgerly_core::{Clock, Date, Outcome, Unsupported, UserId, VoucherId};
use ledgerly_model::seed_godowns::default_godowns;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{Books, Company, MasterSnapshot};
use ledgerly_reports::{Report, ReportParams};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};

use crate::bank::Statements;
use crate::command::{self, Command, CommandResult};
use crate::error::ApiError;
use crate::events::{lock_date, read_all};
use crate::report::{self, ExportFormat};
use crate::search::{SearchHit, search_books, search_catalogue};

/// The app version written to every audit record.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The command and query facade over one company file.
#[derive(Debug)]
pub struct Api {
    /// The company file.
    store: SqliteStore,
    /// Source of the audit timestamps.
    clock: Clock,
    /// Who is making the changes.
    actor: Actor,
    /// The loaded books, or why they cannot be loaded.
    cache: Cache,
    /// The lock date in force, from the audit log.
    lock: Option<Date>,
    /// Bank statements imported in this session (card W5). Not kept in the company file, which
    /// records only the `StatementImported` id; see `bank.rs`.
    pub(crate) bank: Statements,
    /// The assistant's provider, when one is attached ([`Api::attach_assistant`]). Opening a
    /// company attaches none, so it touches no network.
    assistant: Option<Box<dyn AiProvider>>,
}

/// The state of the books cache.
#[derive(Debug)]
enum Cache {
    /// The books, loaded from the store.
    Ready(Box<Books>),
    /// The store holds a voucher this build cannot project. Nothing is computed from the books.
    Blocked(Unsupported),
    /// A change was saved but the books could not be reloaded. The next command reloads them.
    Stale,
}

impl Api {
    /// Creates a company file at `path` holding `company`, and seeds it with the reserved groups,
    /// the default voucher types and the default godown, Main Location. Projection maintenance is
    /// switched on. The file must not exist.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the file cannot be created or seeded. A file left half-created by
    /// a failed seed is removed.
    pub fn create(path: &Path, company: &Company, clock: Clock) -> Result<Self, ApiError> {
        let mut store = SqliteStore::create(path, company, clock)?;
        let actor = default_actor();
        let meta = audit_meta(&actor, clock);
        // Every file the API creates keeps its projection tables current from the first event
        // (owner decision 2026-11-21), so `rebuild_check` can report a match on it.
        let seeded = store
            .enable_projections()
            .and_then(|_| store.commit(&seed_events(), &meta));
        if let Err(e) = seeded {
            drop(store);
            if let Err(remove) = std::fs::remove_file(path) {
                tracing::warn!(error = %remove, "could not remove a half-created company file");
            }
            return Err(e.into());
        }
        let mut api = Self {
            store,
            clock,
            actor,
            cache: Cache::Stale,
            lock: None,
            bank: Statements::default(),
            assistant: None,
        };
        api.reload()?;
        Ok(api)
    }

    /// Opens the company file at `path`.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the file cannot be opened or read.
    pub fn open(path: &Path) -> Result<Self, ApiError> {
        let store = SqliteStore::open(path)?;
        let mut api = Self {
            store,
            clock: Clock::System,
            actor: default_actor(),
            cache: Cache::Stale,
            lock: None,
            bank: Statements::default(),
            assistant: None,
        };
        api.reload()?;
        Ok(api)
    }

    /// The lock date in force: the date of the last `PeriodLocked` event in the audit log
    /// (spec section 5, invariant 8), or `None` when the books were never locked. Vouchers dated
    /// on or before it are refused.
    #[must_use]
    pub fn lock_date(&self) -> Option<Date> {
        self.lock
    }

    /// Sets the actor recorded on later changes (the signed-in user).
    pub fn set_actor(&mut self, actor: Actor) {
        self.actor = actor;
    }

    /// Attaches the assistant's provider for this session. The app attaches the local model or
    /// the user's cloud provider; tests and captures attach the fixed-answer test provider.
    pub fn attach_assistant(&mut self, provider: Box<dyn AiProvider>) {
        self.assistant = Some(provider);
    }

    /// The attached provider, if any.
    #[must_use]
    pub fn assistant(&self) -> Option<&dyn AiProvider> {
        self.assistant.as_deref()
    }

    /// Whether a provider is attached.
    #[must_use]
    pub fn assistant_attached(&self) -> bool {
        self.assistant.is_some()
    }

    /// Sets the clock that stamps later audit records.
    pub fn set_clock(&mut self, clock: Clock) {
        self.clock = clock;
    }

    /// Today's date by the company's clock (UTC), for calls that need a day and were not given one.
    pub(crate) fn clock_date(&self) -> Date {
        self.clock.now().date()
    }

    /// Runs a command: the engine computes its events on the cached books, the events are
    /// committed, and the cache is reloaded.
    ///
    /// # Errors
    /// - [`ApiError::Engine`] when the engine refuses the command (nothing is written).
    /// - [`ApiError::Store`] when the commit fails (nothing is written), or when the commit
    ///   succeeded but the books could not be reloaded. In the second case the change is saved
    ///   and the next command reloads the books.
    pub fn execute(&mut self, cmd: Command) -> Result<Outcome<CommandResult>, ApiError> {
        self.execute_with(cmd, |_| Vec::new())
    }

    /// [`Api::execute`], committing `extra(engine events)` in the same transaction after the
    /// engine's events (for example the assistant event that records an accepted draft, which
    /// names the voucher the engine posted). Nothing extra is written when the engine writes
    /// nothing.
    ///
    /// # Errors
    /// As [`Api::execute`].
    pub(crate) fn execute_with(
        &mut self,
        cmd: Command,
        extra: impl FnOnce(&[Event]) -> Vec<Event>,
    ) -> Result<Outcome<CommandResult>, ApiError> {
        if matches!(self.cache, Cache::Stale) {
            self.reload()?;
        }
        let books = match &self.cache {
            Cache::Ready(books) => books,
            Cache::Blocked(unsupported) => return Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => return Err(ApiError::Stale),
        };
        let cmd = command::assign_ids(cmd, books)?;
        let mut events = match command::run(cmd, books, self.lock)? {
            Outcome::Value(events) => events,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        if events.is_empty() {
            return Ok(Outcome::Value(CommandResult::unchanged()));
        }
        let more = extra(&events);
        events.extend(more);
        let meta = audit_meta(&self.actor, self.clock);
        let receipt = self.store.commit(&events, &meta)?;
        let result = CommandResult::committed(&events, receipt);
        self.reload()?;
        Ok(Outcome::Value(result))
    }

    /// Runs a report over the cached books.
    ///
    /// # Errors
    /// - [`ApiError::Report`] when the report engine rejects `p`.
    /// - [`ApiError::Store`] when the audit log cannot be read for an audit report.
    /// - [`ApiError::Stale`] after a saved change whose reload failed; run a command to reload.
    pub fn report(&self, p: &ReportParams) -> Result<Outcome<Report>, ApiError> {
        match &self.cache {
            Cache::Ready(books) => report::run(&self.store, books, p),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// Runs a report over the cached books and writes it to `path` as CSV or XLSX, atomically
    /// (the file is written whole or not at all). The file holds the same report [`Api::report`]
    /// returns for `p`.
    ///
    /// # Errors
    /// The errors of [`Api::report`], and [`ApiError::Report`] when the file cannot be written.
    pub fn export_report(
        &self,
        p: &ReportParams,
        format: ExportFormat,
        path: &Path,
    ) -> Result<Outcome<()>, ApiError> {
        match &self.cache {
            Cache::Ready(books) => report::export(&self.store, books, p, format, path),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// Exports the balance sheet as the screen shows it by default: each group with its
    /// sub-groups and no ledgers. Any other report is `Unsupported`.
    ///
    /// # Errors
    /// The errors of [`Api::export_report`].
    pub fn export_report_by_group(
        &self,
        p: &ReportParams,
        format: ExportFormat,
        path: &Path,
    ) -> Result<Outcome<()>, ApiError> {
        match &self.cache {
            Cache::Ready(books) => report::export_by_group(&self.store, books, p, format, path),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// An owned copy of the loaded books, for a caller that draws masters and vouchers on another
    /// thread (the app's worker sends it back to the UI thread after each change).
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed; run a command to reload.
    pub fn books(&self) -> Result<Outcome<Books>, ApiError> {
        match &self.cache {
            Cache::Ready(books) => Ok(Outcome::Value(Books::clone(books))),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// Searches ledgers, vouchers, reports and settings. The matching rules and hit order are
    /// described in `search.rs`. An empty query gives no hits.
    ///
    /// When the books cannot be loaded, the report and setting hits are still returned, followed
    /// by one [`SearchHit::Unavailable`] that says why ledgers and vouchers are missing.
    pub fn search(&self, q: &str) -> Vec<SearchHit> {
        if q.trim().is_empty() {
            return Vec::new();
        }
        let mut hits = match &self.cache {
            Cache::Ready(books) => search_books(books, q),
            Cache::Blocked(_) | Cache::Stale => search_catalogue(q),
        };
        if let Some(reason) = self.unavailable_reason() {
            hits.push(SearchHit::Unavailable { reason });
        }
        hits
    }

    /// Why the books are not searchable, or `None` when they are.
    fn unavailable_reason(&self) -> Option<String> {
        match &self.cache {
            Cache::Ready(_) => None,
            Cache::Blocked(unsupported) => {
                Some(format!("{}: {}", unsupported.what, unsupported.reason))
            }
            Cache::Stale => Some(
                "the books need reloading after a saved change; run a command to reload them"
                    .to_owned(),
            ),
        }
    }

    /// Crate-internal access for the Wave 2 extensions in other modules of this crate (bridge
    /// import, GST, banking, maintenance). Reloads first when a saved change left the cache stale.
    ///
    /// Returns the cached books, or `Unsupported` when the store holds a voucher this build cannot
    /// project.
    ///
    /// # Errors
    /// [`ApiError::Store`] when a needed reload fails.
    pub(crate) fn loaded_books(&mut self) -> Result<Outcome<&Books>, ApiError> {
        if matches!(self.cache, Cache::Stale) {
            self.reload()?;
        }
        match &self.cache {
            Cache::Ready(books) => Ok(Outcome::Value(books)),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// The cached books without reloading, or `Unsupported` / [`ApiError::Stale`] as in
    /// [`Api::report`]. For read-only extensions that take `&self`.
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed.
    pub(crate) fn cached_books(&self) -> Result<Outcome<&Books>, ApiError> {
        match &self.cache {
            Cache::Ready(books) => Ok(Outcome::Value(books)),
            Cache::Blocked(unsupported) => Ok(Outcome::Unsupported(unsupported.clone())),
            Cache::Stale => Err(ApiError::Stale),
        }
    }

    /// The company file, for read-only store calls (verify, backup, audit reads).
    pub(crate) fn store(&self) -> &SqliteStore {
        &self.store
    }

    /// The lock date in force, from the audit log.
    pub(crate) fn lock(&self) -> Option<Date> {
        self.lock
    }

    /// Commits `events` attributed to `actor` with the current clock, then reloads the books.
    /// An empty list commits nothing and returns `None`.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the commit fails (nothing is written) or the reload fails (the
    /// change is saved and the cache is stale).
    pub(crate) fn commit_as(
        &mut self,
        events: &[Event],
        actor: &Actor,
    ) -> Result<Option<ledgerly_store::CommitReceipt>, ApiError> {
        if events.is_empty() {
            return Ok(None);
        }
        let meta = audit_meta(actor, self.clock);
        let receipt = self.store.commit(events, &meta)?;
        self.reload()?;
        Ok(Some(receipt))
    }

    /// The actor changes are attributed to.
    pub(crate) fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The clock that stamps audit records. Used by the Tally import to find the current
    /// financial year without reading the system clock itself.
    pub(crate) fn clock(&self) -> Clock {
        self.clock
    }

    /// Reloads the cache and the lock date from the store.
    ///
    /// # Errors
    /// [`ApiError::Store`] on a read failure. The cache is then `Stale`.
    fn reload(&mut self) -> Result<(), ApiError> {
        let records = match read_all(&self.store) {
            Ok(records) => records,
            Err(e) => {
                self.cache = Cache::Stale;
                return Err(e.into());
            }
        };
        let cache = match self.store.load_books(None) {
            Ok(books) => Cache::Ready(Box::new(books)),
            Err(StoreError::Unsupported { voucher, reason }) => {
                Cache::Blocked(blocked(voucher, &reason))
            }
            Err(e) => {
                self.cache = Cache::Stale;
                return Err(e.into());
            }
        };
        self.cache = cache;
        self.lock = lock_date(&records);
        Ok(())
    }
}

/// Who changes are attributed to until [`Api::set_actor`] is called.
fn default_actor() -> Actor {
    Actor::User(UserId(1))
}

/// The audit metadata of a change made now.
fn audit_meta(actor: &Actor, clock: Clock) -> AuditMeta {
    AuditMeta {
        actor: actor.clone(),
        at: clock.now(),
        reason: None,
        app_version: APP_VERSION.to_owned(),
    }
}

/// The reserved groups and default voucher types of a new company, as master creations.
fn seed_events() -> Vec<Event> {
    reserved_groups()
        .into_iter()
        .map(|group| Event::MasterCreated(MasterSnapshot::Group(group)))
        .chain(
            default_voucher_types()
                .into_iter()
                .map(|kind| Event::MasterCreated(MasterSnapshot::VoucherType(kind))),
        )
        .chain(
            default_godowns()
                .into_iter()
                .map(|godown| Event::MasterCreated(MasterSnapshot::Godown(godown))),
        )
        .collect()
}

/// The unsupported state of a store that holds a voucher it cannot project.
fn blocked(voucher: VoucherId, reason: &str) -> Unsupported {
    Unsupported {
        what: format!("voucher {}", voucher.0),
        reason: format!(
            "{reason}; no figures or changes are computed from the books until this is resolved"
        ),
    }
}
