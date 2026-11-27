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
//! group. Vouchers are posted in file order. A voucher whose `GUID` is already in the books is
//! posted again, as the import FAQ describes when overwriting vouchers with the same GUID is
//! turned off (<https://help.tallysolutions.com/import-data-faq/>); overwriting is not offered.

use std::fmt;
use std::path::{Path, PathBuf};

use ledgerly_core::{Outcome, VoucherId};
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
    Transactions,
}

/// One object that was not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportException {
    /// The object, such as `ledger 'Kumar Textiles'`.
    pub object: String,
    /// Why it was not imported.
    pub reason: String,
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
    /// Existing masters altered (Modify with New Data).
    pub altered: usize,
    /// Existing ledgers whose opening balance was added to (Combine Opening Balance).
    pub combined: usize,
    /// Objects left as they were, each with its reason in `ignored_notes`.
    pub ignored: usize,
    /// Objects not imported: the length of `exceptions`.
    pub errors: usize,
    /// Every object not imported, in the order met.
    pub exceptions: Vec<ImportException>,
    /// Every object ignored, as `object: reason`.
    pub ignored_notes: Vec<String>,
    /// Fields that were read but not imported, and the reader's warnings, as `place: message`.
    pub warnings: Vec<String>,
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
                    summary.warnings.extend(c.warnings);
                    self.import_masters(&c.items, behaviour, &mut summary)
                })
            }
            XmlImport::Transactions => {
                let contents = read_vouchers(&bytes).map_err(failed);
                contents.and_then(|c| {
                    summary.warnings.extend(c.warnings);
                    self.import_vouchers(&c.items, &mut summary)
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
                summary.ignored_notes.push(format!("{object}: {why}"));
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
            summary
                .warnings
                .extend(plan.dropped.into_iter().map(|d| format!("{object}: {d}")));
        }
        Ok(())
    }

    fn import_vouchers(
        &mut self,
        vouchers: &[ledgerly_tally_bridge::file_import::TVoucher],
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
            let next = books
                .vouchers
                .keys()
                .next_back()
                .map_or(Some(1), |id| id.0.checked_add(1));
            let Some(next) = next else {
                summary.exception(object, "too many vouchers to number".to_owned());
                continue;
            };
            let (voucher, dropped) = match voucher_step(v, &books, VoucherId(next)) {
                Ok(step) => step,
                Err(e) => {
                    summary.bridge_exception(e);
                    continue;
                }
            };
            if self.run(
                Command::PostVoucher(voucher),
                object.clone(),
                Applied::Created,
                summary,
            )? {
                summary
                    .warnings
                    .extend(dropped.into_iter().map(|d| format!("{object}: {d}")));
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
