//! Import masters or transactions from an XML file in the other program's import form into the
//! open company (S22).
//!
//! [`Api::import_tally_xml_file`] follows the import screen of
//! <https://help.tallysolutions.com/import-data-from-xml-or-json/> (read 2026-11-27): masters or
//! transactions, a file path, and for masters the behaviour when a master already exists. Every
//! object is one change run through the engine and committed on its own, attributed to an import
//! job (`Actor::Import`), so the period lock and every voucher check apply. An object the engine
//! refuses, or that cannot be read or resolved, is an exception: it is listed with its reason and
//! the import goes on with the next object (the getting-started page's "Record Exceptions and
//! Import", <https://help.tallysolutions.com/getting-started-with-importing-data-into-tallyprime/>).
//!
//! Groups are imported before ledgers, parents first, so a file may list a ledger before its
//! group. Vouchers are posted in file order. A voucher whose `GUID` is already in the books
//! follows "Overwrite voucher when a voucher with same GUID exists"
//! (<https://help.tallysolutions.com/import-data-faq/>, read 2026-11-27; [`VoucherBehaviour`]):
//! - Yes: "the existing voucher gets overwritten" (the latest one with that GUID, through the
//!   engine's alter, so the lock and every check apply; an automatic number is kept);
//! - No, the default: "a duplicate voucher gets created. The subsequent import of the same voucher
//!   will overwrite the previously duplicated/created voucher". So with one voucher of that GUID in
//!   the books it is posted again, and with two or more the latest is overwritten.

use std::fmt;
use std::path::{Path, PathBuf};

use ledgerly_core::{Outcome, VoucherId};
use ledgerly_model::Voucher;
use ledgerly_store::Actor;
use ledgerly_tally_bridge::file_import::{
    Exception, ImportMaster, ImportMasterKind, MasterBehaviour, MasterStep, master_label,
    master_step, read_masters, read_vouchers, voucher_object, voucher_step,
};

use crate::facade::Api;
use crate::{ApiError, Command};

/// What to import from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlImport {
    /// Masters (groups, ledgers), treating a master that exists as the behaviour says.
    Masters(MasterBehaviour),
    /// Transactions (vouchers); their masters must be in the company already.
    Transactions(VoucherBehaviour),
}

impl XmlImport {
    /// Transactions with the default configuration: overwrite off.
    #[must_use]
    pub const fn default_transactions() -> Self {
        Self::Transactions(VoucherBehaviour {
            overwrite_same_guid: false,
        })
    }
}

/// The import configuration for transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VoucherBehaviour {
    /// "Overwrite voucher when a voucher with same GUID exists"
    /// (<https://help.tallysolutions.com/import-data-faq/>, read 2026-11-27). Yes: the voucher in
    /// the books with the file voucher's GUID is overwritten. No (the default): it is posted again
    /// as a duplicate, and a later import of it overwrites that duplicate.
    pub overwrite_same_guid: bool,
}

/// The folder, beside the company file, that backups go into when no backup folder is stored.
pub const BACKUP_FOLDER: &str = "Backups";

/// One object that was not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportException {
    /// The object, such as `ledger 'Kumar Textiles'`.
    pub object: String,
    /// Why it was not imported.
    pub reason: String,
}

/// A note on one object of the file: an object left as it was, or a field read but not imported.
/// The object and the note are kept apart so a screen can show them in two columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportNote {
    /// The object or place, such as `ledger 'Kumar Textiles'`,
    /// `Sales voucher SO-12 dated 2026-10-05` or `offset 120`.
    pub object: String,
    /// What happened to it.
    pub note: String,
}

impl ImportNote {
    fn new(object: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            object: object.into(),
            note: note.into(),
        }
    }
}

impl fmt::Display for ImportNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.object, self.note)
    }
}

/// What a file import did, for the user to read. Every exception is listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileImportSummary {
    /// What was imported.
    pub what: XmlImport,
    /// The file read.
    pub path: PathBuf,
    /// Objects created.
    pub created: usize,
    /// Existing masters altered (Modify with New Data), or vouchers overwritten (same GUID).
    pub altered: usize,
    /// Existing ledgers whose opening balance was added to (Combine Opening Balance).
    pub combined: usize,
    /// Objects left as they were, each with its reason in `ignored_notes`.
    pub ignored: usize,
    /// Objects not imported: the length of `exceptions`.
    pub errors: usize,
    /// Every object not imported, in the order met.
    pub exceptions: Vec<ImportException>,
    /// Every object ignored, with why.
    pub ignored_notes: Vec<ImportNote>,
    /// Fields that were read but not imported, and the reader's warnings, each with its object
    /// or place in the file.
    pub warnings: Vec<ImportNote>,
    /// The backup of the company taken before the import, when one was asked for
    /// ([`Api::back_up_and_import_tally_xml_file`]).
    pub backup: Option<PathBuf>,
}

impl fmt::Display for FileImportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: created {}, altered {}, combined {}, ignored {}, errors {}",
            self.path.display(),
            self.created,
            self.altered,
            self.combined,
            self.ignored,
            self.errors
        )?;
        for e in &self.exceptions {
            writeln!(f, "  exception: {}: {}", e.object, e.reason)?;
        }
        for note in &self.ignored_notes {
            writeln!(f, "  ignored: {note}")?;
        }
        Ok(())
    }
}

impl FileImportSummary {
    fn new(what: XmlImport, path: &Path) -> Self {
        Self {
            what,
            path: path.to_owned(),
            created: 0,
            altered: 0,
            combined: 0,
            ignored: 0,
            errors: 0,
            exceptions: Vec::new(),
            ignored_notes: Vec::new(),
            warnings: Vec::new(),
            backup: None,
        }
    }

    fn exception(&mut self, object: String, reason: String) {
        self.errors = self.errors.saturating_add(1);
        self.exceptions.push(ImportException { object, reason });
    }

    fn bridge_exception(&mut self, e: Exception) {
        self.exception(e.object, e.reason);
    }
}

/// What one change came to.
#[derive(Clone, Copy)]
enum Applied {
    Created,
    Altered,
    Combined,
}

impl Api {
    /// Takes a backup of the company, then imports the file as
    /// [`Api::import_tally_xml_file`] does. The import screen asks whether to take one first:
    /// "Select Yes to take a backup of the company data before importing"
    /// (<https://help.tallysolutions.com/import-data-from-xml-or-json/>, read 2026-11-27). The
    /// backup goes into the company's backup folder ([`Api::backup_folder`]: the one stored on
    /// the backup screen, else the `Backups` folder beside the company file, [`BACKUP_FOLDER`];
    /// the help names no folder), never collides with another backup's name, and its path is in
    /// [`FileImportSummary::backup`].
    ///
    /// # Errors
    /// [`ApiError::Store`] when the backup cannot be written or does not verify: nothing is
    /// imported. Then as [`Api::import_tally_xml_file`].
    pub fn back_up_and_import_tally_xml_file(
        &mut self,
        what: XmlImport,
        path: &Path,
    ) -> Result<Outcome<FileImportSummary>, ApiError> {
        let backup = self.back_up_now()?;
        Ok(self.import_tally_xml_file(what, path)?.map(|mut summary| {
            summary.backup = Some(backup);
            summary
        }))
    }

    /// Imports the masters or transactions in the XML file at `path` into the open company. See
    /// the module docs for the rules.
    ///
    /// # Errors
    /// - [`ApiError::FileImport`] when the file cannot be read or is not a readable XML document;
    ///   nothing is imported.
    /// - [`ApiError::Store`] when a commit fails or the books cannot be reloaded. The objects
    ///   committed before it stay imported.
    pub fn import_tally_xml_file(
        &mut self,
        what: XmlImport,
        path: &Path,
    ) -> Result<Outcome<FileImportSummary>, ApiError> {
        let failed = |reason: String| ApiError::FileImport {
            path: path.to_owned(),
            reason,
        };
        let bytes = std::fs::read(path).map_err(|e| failed(e.to_string()))?;
        if let Outcome::Unsupported(refusal) = self.books()? {
            return Ok(Outcome::Unsupported(refusal));
        }
        let mut summary = FileImportSummary::new(what, path);
        let saved = self.actor().clone();
        let job = import_actor(&saved, self.clock_date());
        self.set_actor(job);
        let result = match what {
            XmlImport::Masters(behaviour) => {
                let contents = read_masters(&bytes).map_err(failed);
                contents.and_then(|c| {
                    summary.warnings.extend(
                        c.warnings
                            .into_iter()
                            .map(|w| ImportNote::new(format!("offset {}", w.offset), w.message)),
                    );
                    self.import_masters(&c.items, behaviour, &mut summary)
                })
            }
            XmlImport::Transactions(behaviour) => {
                let contents = read_vouchers(&bytes).map_err(failed);
                contents.and_then(|c| {
                    summary.warnings.extend(
                        c.warnings
                            .into_iter()
                            .map(|w| ImportNote::new(format!("offset {}", w.offset), w.message)),
                    );
                    self.import_vouchers(&c.items, behaviour, &mut summary)
                })
            }
        };
        self.set_actor(saved);
        result.map(|()| Outcome::Value(summary))
    }

    fn import_masters(
        &mut self,
        masters: &[ImportMaster],
        behaviour: MasterBehaviour,
        summary: &mut FileImportSummary,
    ) -> Result<(), ApiError> {
        let (groups, rest): (Vec<&ImportMaster>, Vec<&ImportMaster>) = masters
            .iter()
            .partition(|m| m.kind == ImportMasterKind::Group);
        // Groups, parents first: a group whose parent is not there yet waits for the next pass.
        let mut pending = groups;
        loop {
            let before = pending.len();
            let mut waiting = Vec::new();
            for m in pending {
                let parent_known = self.loaded_masters_has_parent(m)?;
                if parent_known {
                    self.import_master(m, behaviour, summary)?;
                } else {
                    waiting.push(m);
                }
            }
            if waiting.is_empty() {
                break;
            }
            if waiting.len() == before {
                // No progress: the rest have unknown parents and become exceptions.
                for m in waiting {
                    self.import_master(m, behaviour, summary)?;
                }
                break;
            }
            pending = waiting;
        }
        for m in rest {
            self.import_master(m, behaviour, summary)?;
        }
        Ok(())
    }

    /// Whether `m`'s parent group is in the books (or `m` has none, or exists already).
    fn loaded_masters_has_parent(&self, m: &ImportMaster) -> Result<bool, ApiError> {
        let books = match self.books()? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(_) => return Ok(true),
        };
        let groups = &books.masters.groups;
        if groups.values().any(|g| g.name == m.name) {
            return Ok(true);
        }
        let parent = m
            .parent
            .as_deref()
            .map(|p| p.chars().filter(|c| !c.is_control()).collect::<String>());
        Ok(match parent.as_deref().map(str::trim) {
            None | Some("" | "Primary") => true,
            Some(p) => groups.values().any(|g| g.name == p),
        })
    }

    fn import_master(
        &mut self,
        m: &ImportMaster,
        behaviour: MasterBehaviour,
        summary: &mut FileImportSummary,
    ) -> Result<(), ApiError> {
        let object = master_label(m);
        let books = match self.books()? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(u) => {
                summary.exception(object, format!("not supported: {}: {}", u.what, u.reason));
                return Ok(());
            }
        };
        let plan = match master_step(m, &books.masters, behaviour) {
            Ok(plan) => plan,
            Err(e) => {
                summary.bridge_exception(e);
                return Ok(());
            }
        };
        let (command, applied) = match plan.step {
            MasterStep::Ignore(why) => {
                summary.ignored = summary.ignored.saturating_add(1);
                summary.ignored_notes.push(ImportNote::new(object, why));
                return Ok(());
            }
            MasterStep::CreateGroup(g) => (Command::CreateGroup(g), Applied::Created),
            MasterStep::AlterGroup(g) => (Command::AlterGroup(g), Applied::Altered),
            MasterStep::CreateLedger(l) => (Command::CreateLedger(l), Applied::Created),
            MasterStep::AlterLedger { ledger, combined } => (
                Command::AlterLedger(ledger),
                if combined {
                    Applied::Combined
                } else {
                    Applied::Altered
                },
            ),
        };
        if self.run(command, object.clone(), applied, summary)? {
            summary.warnings.extend(
                plan.dropped
                    .into_iter()
                    .map(|d| ImportNote::new(object.clone(), d)),
            );
        }
        Ok(())
    }

    fn import_vouchers(
        &mut self,
        vouchers: &[ledgerly_tally_bridge::file_import::TVoucher],
        behaviour: VoucherBehaviour,
        summary: &mut FileImportSummary,
    ) -> Result<(), ApiError> {
        for v in vouchers {
            let object = voucher_object(v);
            let books = match self.books()? {
                Outcome::Value(b) => b,
                Outcome::Unsupported(u) => {
                    summary.exception(object, format!("not supported: {}: {}", u.what, u.reason));
                    continue;
                }
            };
            let same_guid: Vec<&Voucher> = match v.guid.as_deref() {
                Some(guid) => books
                    .vouchers
                    .values()
                    .filter(|b| b.tally.as_ref().is_some_and(|t| t.guid == guid))
                    .collect(),
                None => Vec::new(),
            };
            let target = match (behaviour.overwrite_same_guid, same_guid.as_slice()) {
                (_, []) | (false, [_]) => None,
                (true, [.., last]) | (false, [_, .., last]) => Some(*last),
            };
            if target.is_none() && !same_guid.is_empty() {
                summary.warnings.push(ImportNote::new(
                    object.clone(),
                    "a voucher with the same GUID is in the books; with overwrite off it is \
                     posted again as a duplicate",
                ));
            }
            let id = if let Some(existing) = target {
                existing.id
            } else {
                let next = books
                    .vouchers
                    .keys()
                    .next_back()
                    .map_or(Some(1), |id| id.0.checked_add(1));
                let Some(next) = next else {
                    summary.exception(object, "too many vouchers to number".to_owned());
                    continue;
                };
                VoucherId(next)
            };
            let (mut voucher, dropped) = match voucher_step(v, &books, id) {
                Ok(step) => step,
                Err(e) => {
                    summary.bridge_exception(e);
                    continue;
                }
            };
            let (command, applied) = match target {
                Some(existing) => {
                    // An automatic number is the books' own; the overwrite keeps it.
                    if voucher.number.is_none() {
                        voucher.number.clone_from(&existing.number);
                        voucher.series.clone_from(&existing.series);
                    }
                    (Command::AlterVoucher(voucher), Applied::Altered)
                }
                None => (Command::PostVoucher(voucher), Applied::Created),
            };
            if self.run(command, object.clone(), applied, summary)? {
                summary.warnings.extend(
                    dropped
                        .into_iter()
                        .map(|d| ImportNote::new(object.clone(), d)),
                );
            }
        }
        Ok(())
    }

    /// Runs one change and counts it; a refusal becomes an exception. Returns whether it was
    /// committed.
    fn run(
        &mut self,
        command: Command,
        object: String,
        applied: Applied,
        summary: &mut FileImportSummary,
    ) -> Result<bool, ApiError> {
        match self.execute(command) {
            Ok(Outcome::Value(_)) => {
                let counter = match applied {
                    Applied::Created => &mut summary.created,
                    Applied::Altered => &mut summary.altered,
                    Applied::Combined => &mut summary.combined,
                };
                *counter = counter.saturating_add(1);
                Ok(true)
            }
            Ok(Outcome::Unsupported(u)) => {
                summary.exception(object, format!("not supported: {}: {}", u.what, u.reason));
                Ok(false)
            }
            Err(ApiError::Engine(e)) => {
                summary.exception(object, e.to_string());
                Ok(false)
            }
            Err(other) => Err(other),
        }
    }
}

/// The import job acting for whoever runs the import.
fn import_actor(actor: &Actor, today: ledgerly_core::Date) -> Actor {
    let on_behalf_of = match actor.clone() {
        Actor::User(user) => user,
        Actor::Import { on_behalf_of, .. } | Actor::Assistant { on_behalf_of, .. } => on_behalf_of,
    };
    Actor::Import {
        job: format!("xml-file-import {today}"),
        on_behalf_of,
    }
}
