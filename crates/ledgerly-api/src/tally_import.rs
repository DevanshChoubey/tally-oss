//! Import from a running Tally into the open company file (spec 6.7, flagship import). **Card W3**
//! implements this module.
//!
//! [`Api::import_tally_http`] pulls the whole voucher history of a Tally company over HTTP,
//! maps it with `ledgerly_tally_bridge::import`, and commits the events in one change attributed to
//! an import job (`Actor::Import`). The raw Tally responses are kept first, in a folder beside the
//! company file named after it with `.tally-capture` appended.
//!
//! # Target company
//!
//! The import only goes into a company that holds exactly what [`Api::create`] seeds: the reserved
//! groups, the default voucher types and Main Location, and nothing else, with no period locked.
//! Any other company is refused with `Outcome::Unsupported` before Tally is contacted. Merging into existing books
//! is not offered: an import into a company with data would need an id base and a merge rule that
//! the spec does not give.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use ledgerly_core::{CancelToken, Date, Outcome, Period, Unsupported};
use ledgerly_model::seed_godowns::default_godowns;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{Books, Godown, Group, VoucherType};
use ledgerly_store::Actor;
use ledgerly_tally_bridge::TallyTransport;
use ledgerly_tally_bridge::import::{ImportCounts, ImportedTally, import_company, import_range};
use ledgerly_tally_bridge::transport::HttpTransport;

use crate::ApiError;
use crate::facade::Api;

/// Suffix of the folder that keeps the raw Tally responses, beside the company file.
const CAPTURE_SUFFIX: &str = ".tally-capture";

/// What a Tally import brought in, for the user to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// Name of the Tally company that was read.
    pub company: String,
    /// The voucher period asked of Tally (inclusive). Vouchers outside it are not in the import.
    pub period: Period,
    /// Masters read from Tally, by kind (`group`, `ledger`, `voucher type`, ...).
    pub masters: BTreeMap<&'static str, usize>,
    /// Vouchers read from Tally, by Tally voucher type name.
    pub vouchers: BTreeMap<String, usize>,
    /// Voucher date windows accepted from Tally.
    pub voucher_windows: usize,
    /// Events committed to the company file.
    pub events: usize,
    /// Each thing in Tally's export that was not imported, as `place: message`.
    pub warnings: Vec<String>,
}

impl fmt::Display for ImportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Imported from Tally company \"{}\": {} events committed.",
            self.company, self.events
        )?;
        writeln!(
            f,
            "Vouchers were requested for {} to {} in {} window(s). \
             Vouchers dated outside that period are not in this import.",
            self.period.from(),
            self.period.to(),
            self.voucher_windows
        )?;
        writeln!(f, "Read from Tally:")?;
        for (kind, count) in &self.masters {
            writeln!(f, "  {count} {kind}(s)")?;
        }
        for (voucher_type, count) in &self.vouchers {
            writeln!(f, "  {count} {voucher_type} voucher(s)")?;
        }
        if self.warnings.is_empty() {
            writeln!(f, "No warnings.")?;
        } else {
            writeln!(f, "{} not imported:", self.warnings.len())?;
            for warning in &self.warnings {
                writeln!(f, "  {warning}")?;
            }
        }
        Ok(())
    }
}

impl Api {
    /// Imports the whole voucher history of Tally company `company`, read from Tally on
    /// `host` and `port` (Tally's HTTP server, port 9000 by default).
    ///
    /// Nothing is contacted or written unless the company is empty (see the module docs). On
    /// success the events are committed as one change and the books are reloaded.
    ///
    /// # Errors
    /// - [`ApiError::Import`] when Tally cannot be read, a response is not valid Tally XML, or a
    ///   master or voucher cannot be mapped. Nothing is committed.
    /// - [`ApiError::Store`] when the commit fails (nothing is written), or when the commit
    ///   succeeded but the books could not be reloaded.
    pub fn import_tally_http(
        &mut self,
        host: &str,
        port: u16,
        company: &str,
    ) -> Result<Outcome<ImportSummary>, ApiError> {
        let transport = HttpTransport::new(host, port);
        self.import_tally_with(&transport, company, &CancelToken::new())
    }

    /// [`Api::import_tally_http`] over any transport, with a cancel token that stops the pull
    /// between requests (a cancelled import commits nothing). The tests serve recorded fixtures
    /// through this; the app passes a token its cancel button sets.
    ///
    /// # Errors
    /// As [`Api::import_tally_http`]; a cancel is [`ApiError::Import`] wrapping a cancelled pull.
    pub fn import_tally_with(
        &mut self,
        transport: &dyn TallyTransport,
        company: &str,
        cancel: &CancelToken,
    ) -> Result<Outcome<ImportSummary>, ApiError> {
        let books_begin = match self.import_target()? {
            Outcome::Value(begin) => begin,
            Outcome::Unsupported(refusal) => return Ok(Outcome::Unsupported(refusal)),
        };
        let today = self.clock().now().date();
        let period = import_range(books_begin, today)?;
        let capture = self.capture_dir();
        let imported = match import_company(transport, company, period, &capture, cancel)? {
            Outcome::Value(imported) => imported,
            Outcome::Unsupported(refusal) => return Ok(Outcome::Unsupported(refusal)),
        };
        let actor = import_actor(self.actor(), today);
        self.commit_as(&imported.events, &actor)?;
        Ok(Outcome::Value(summary(company, period, imported)))
    }

    /// The beginning of the books, when the company is empty enough to import into.
    fn import_target(&mut self) -> Result<Outcome<Date>, ApiError> {
        let locked = self.lock().is_some();
        let books = match self.loaded_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(refusal) => return Ok(Outcome::Unsupported(refusal)),
        };
        if locked || !is_seed_only(books) {
            return Ok(Outcome::Unsupported(Unsupported {
                what: "Tally import into this company".to_owned(),
                reason: "the company already holds data, or a period is locked. An import needs \
                         a new company file, so that nothing is merged or overwritten. Nothing \
                         was imported"
                    .to_owned(),
            }));
        }
        Ok(Outcome::Value(books.company.books_begin))
    }

    /// The folder for the raw Tally responses: `<company file>.tally-capture`, beside the file.
    fn capture_dir(&self) -> PathBuf {
        let path = self.store().path();
        let mut name: OsString = path
            .file_name()
            .map_or_else(|| OsString::from("company"), std::ffi::OsStr::to_os_string);
        name.push(CAPTURE_SUFFIX);
        path.parent().unwrap_or_else(|| Path::new("")).join(name)
    }
}

/// True when `books` holds only what [`Api::create`] seeds: the reserved groups, the default
/// voucher types, Main Location, and no other master, posting or voucher.
fn is_seed_only(books: &Books) -> bool {
    let masters = &books.masters;
    let groups: Vec<Group> = masters.groups.values().cloned().collect();
    let voucher_types: Vec<VoucherType> = masters.voucher_types.values().cloned().collect();
    let godowns: Vec<Godown> = masters.godowns.values().cloned().collect();
    groups == reserved_groups()
        && voucher_types == default_voucher_types()
        && godowns == default_godowns()
        && masters.ledgers.is_empty()
        && masters.units.is_empty()
        && masters.stock_groups.is_empty()
        && masters.stock_items.is_empty()
        && masters.batches.is_empty()
        && masters.cost_categories.is_empty()
        && masters.cost_centres.is_empty()
        && books.vouchers.is_empty()
        && books.postings.is_empty()
        && books.stock_moves.is_empty()
}

/// The actor the import commits as: the user the session acts for, with a job description that
/// names the day and not the company.
fn import_actor(actor: &Actor, today: Date) -> Actor {
    let on_behalf_of = match actor.clone() {
        Actor::User(user) => user,
        Actor::Import { on_behalf_of, .. } | Actor::Assistant { on_behalf_of, .. } => on_behalf_of,
    };
    Actor::Import {
        job: format!("tally-import {today}"),
        on_behalf_of,
    }
}

/// The summary of an import, from what the bridge returned.
fn summary(company: &str, period: Period, imported: ImportedTally) -> ImportSummary {
    let ImportCounts {
        masters,
        vouchers,
        voucher_windows,
    } = imported.counts;
    ImportSummary {
        company: company.to_owned(),
        period,
        masters,
        vouchers,
        voucher_windows,
        events: imported.events.len(),
        warnings: imported
            .warnings
            .into_iter()
            .map(|w| format!("{}: {}", w.place, w.message))
            .collect(),
    }
}
