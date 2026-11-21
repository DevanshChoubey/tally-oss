//! Banking over the open company file (spec 6.5, card W5): statement import, match suggestions,
//! confirmation of a match, and the bank reconciliation statement (BRS).
//!
//! Frozen interface (contracts.md, W5):
//!
//! ```text
//! impl Api { pub fn brs(&self, ledger: LedgerId, as_of: Date) -> Result<Outcome<Brs>, ApiError>; }
//! ```
//!
//! # Where statements are held
//!
//! The store's `StatementImported` event carries only the statement id, so the rows and the file
//! hash of an imported statement are not in the company file. This session holds them in
//! [`Statements`], which [`Api`] keeps. After the company file is reopened:
//!
//! - the confirmed matches remain, because `MatchConfirmed` events are applied when the books load;
//! - the statements are gone, so the BRS statement side is `Unsupported` until the statement is
//!   imported again, and a file imported before the reopen is not recognised as a duplicate;
//! - ids are not reused: the next id is one more than the largest `StatementImported` id in the
//!   audit log.
//!
//! Interface request: a statement payload (hash and rows) in the store, in
//! `docs/interface-requests.md`.
//!
//! # Confirmation
//!
//! Only a user-confirmed match sets the bank date (spec 6.5). [`Api::confirm_bank_match`] takes a
//! [`Candidate`] from [`Api::bank_suggestions`]. It accepts the candidate only when the suggestion
//! engine lists the same match for the same statement now, so a stale or invented candidate is
//! refused. Then it runs `Command::ConfirmBankMatch`, which the period lock applies to.
//!
//! # Import
//!
//! A statement is parsed with its format from the built-in registry and checked by
//! `check_statement` (ledgerly-bank). A failed check keeps nothing. CSV formats are parsed but
//! refused for now: the CSV parser does not state an opening or closing balance (see
//! `docs/interface-requests.md`), so the balance check cannot pass.

use std::collections::BTreeMap;

use ledgerly_bank::BankError;
use ledgerly_bank::brs::Brs;
use ledgerly_bank::matcher::{Candidate, MatchRule, suggest_matches};
use ledgerly_bank::statement::{BankStatement, check_statement};
use ledgerly_bankfmt::csv::parse_csv_with_layout;
use ledgerly_bankfmt::mt940::parse_mt940;
use ledgerly_bankfmt::registry::{FileKind, builtin_registry};
use ledgerly_bankfmt::xlsx::parse_xlsx;
use ledgerly_bankfmt::{BankFmtError, RawStatement};
use ledgerly_core::{BankStatementId, Date, LedgerId, Outcome};
use ledgerly_model::{BankMatch, Books, MatchKind};
use ledgerly_store::Event;

use crate::command::{Command, CommandResult};
use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The bank statements this session holds, by id. See the module docs for why they are not in the
/// company file.
#[derive(Debug, Default)]
pub(crate) struct Statements {
    /// Statements by id, in id order.
    by_id: BTreeMap<BankStatementId, BankStatement>,
}

impl Statements {
    /// The statement with this id.
    ///
    /// # Errors
    /// [`ApiError::UnknownBankStatement`] when this session does not hold it.
    fn get(&self, id: BankStatementId) -> Result<&BankStatement, ApiError> {
        self.by_id
            .get(&id)
            .ok_or(ApiError::UnknownBankStatement { id: id.0 })
    }

    /// Stores a verified statement under its id.
    fn insert(&mut self, id: BankStatementId, statement: BankStatement) {
        self.by_id.insert(id, statement);
    }

    /// The lower-case hex SHA-256 of every held statement, for the duplicate check.
    fn hashes(&self) -> Vec<String> {
        self.by_id
            .values()
            .map(|statement| statement.file_hash().to_owned())
            .collect()
    }

    /// The largest id held, if any.
    fn largest_id(&self) -> Option<u64> {
        self.by_id.keys().map(|id| id.0).max()
    }

    /// Copies of the statements of `ledger`, in id order.
    fn for_ledger(&self, ledger: LedgerId) -> Vec<BankStatement> {
        self.by_id
            .values()
            .filter(|statement| statement.ledger() == ledger)
            .cloned()
            .collect()
    }
}

impl Api {
    /// Imports a bank statement file for the bank ledger `ledger` (spec 6.5).
    ///
    /// The file is parsed with the format `format_id` from the built-in registry, then checked by
    /// `check_statement`: the file hash must be new, the opening and closing balances must be
    /// stated, and the running balances must agree. A failed check keeps nothing. On success the
    /// `StatementImported` event is committed and the statement is held by this session.
    ///
    /// # Errors
    /// - [`ApiError::UnknownBankFormat`] when `format_id` is not in the registry.
    /// - [`ApiError::BankFormat`] when the file cannot be read in that format.
    /// - [`ApiError::StatementImport`] when a check fails (duplicate, missing balance, bad line,
    ///   balance mismatch, running balance).
    /// - [`ApiError::Bank`] when `ledger` does not exist or is not a bank ledger.
    /// - [`ApiError::Store`] when the event cannot be committed.
    ///
    /// Returns `Outcome::Unsupported` when the books cannot be read.
    pub fn import_bank_statement(
        &mut self,
        file: &[u8],
        format_id: &str,
        ledger: LedgerId,
    ) -> Result<Outcome<BankStatementId>, ApiError> {
        let raw = parse_statement(file, format_id)?;
        let books = match self.loaded_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        require_bank_ledger(books, ledger)?;
        let statement = check_statement(raw, file, ledger, &self.bank.hashes())?;
        let id = self.next_statement_id()?;
        let actor = self.actor().clone();
        self.commit_as(&[Event::StatementImported(id)], &actor)?;
        self.bank.insert(id, statement);
        Ok(Outcome::Value(id))
    }

    /// The statements of `ledger` that this session holds, in id order.
    pub fn bank_statements(&self, ledger: LedgerId) -> Vec<(BankStatementId, &BankStatement)> {
        self.bank
            .by_id
            .iter()
            .filter(|(_, statement)| statement.ledger() == ledger)
            .map(|(id, statement)| (*id, statement))
            .collect()
    }

    /// Suggests matches between the lines of a held statement and the vouchers in the books, plus
    /// the rule suggestions for lines that no voucher explains (spec 6.5). Nothing is confirmed.
    ///
    /// # Errors
    /// [`ApiError::UnknownBankStatement`] when this session does not hold `statement`;
    /// [`ApiError::Stale`] after a saved change whose reload failed.
    ///
    /// Returns `Outcome::Unsupported` when the books cannot be read.
    pub fn bank_suggestions(
        &self,
        statement: BankStatementId,
        rules: &[MatchRule],
    ) -> Result<Outcome<Vec<Candidate>>, ApiError> {
        let held = self.bank.get(statement)?;
        match self.cached_books()? {
            Outcome::Value(books) => Ok(Outcome::Value(suggest_matches(held, books, rules))),
            Outcome::Unsupported(unsupported) => Ok(Outcome::Unsupported(unsupported)),
        }
    }

    /// Confirms a suggested match: its bank date is set on the bank lines of its vouchers, as an
    /// audited `MatchConfirmed` event. The bank date is the date of the statement line (the latest
    /// one when several lines are matched).
    ///
    /// The candidate is accepted only when [`Api::bank_suggestions`] lists the same match for the
    /// same statement now. A rule suggestion has no voucher yet, so it cannot be confirmed.
    ///
    /// # Errors
    /// - [`ApiError::UnknownBankStatement`] when this session does not hold `statement`.
    /// - [`ApiError::MatchNotSuggested`] when the current suggestions do not list this match.
    /// - [`ApiError::Engine`] when the period lock covers a voucher, or a voucher is not a
    ///   matchable regular or post-dated voucher, or it is already cleared by the bank. Nothing is
    ///   written.
    /// - [`ApiError::Store`] when the commit fails.
    ///
    /// Returns `Outcome::Unsupported` for a rule suggestion.
    pub fn confirm_bank_match(
        &mut self,
        statement: BankStatementId,
        candidate: &Candidate,
    ) -> Result<Outcome<CommandResult>, ApiError> {
        let matched = match self.confirmable_match(statement, candidate)? {
            Outcome::Value(matched) => matched,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        self.execute(Command::ConfirmBankMatch(matched))
    }

    /// The bank match that confirming `candidate` would record, when it is still suggested.
    fn confirmable_match(
        &self,
        statement: BankStatementId,
        candidate: &Candidate,
    ) -> Result<Outcome<BankMatch>, ApiError> {
        if candidate.kind != MatchKind::Exact || candidate.vouchers.is_empty() {
            return Ok(Outcome::unsupported(
                "bank match of a rule suggestion",
                "a rule suggests a ledger for this statement line, but no voucher exists to date yet; post the voucher, then match it",
            ));
        }
        let held = self.bank.get(statement)?;
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        let listed = suggest_matches(held, books, &[]).iter().any(|s| {
            s.kind == candidate.kind
                && s.statement_lines == candidate.statement_lines
                && s.vouchers == candidate.vouchers
                && s.proposed_bank_date == candidate.proposed_bank_date
        });
        if !listed {
            return Err(ApiError::MatchNotSuggested {
                lines: candidate.statement_lines.clone(),
            });
        }
        Ok(Outcome::Value(BankMatch {
            statement,
            statement_lines: candidate.statement_lines.clone(),
            vouchers: candidate.vouchers.clone(),
            kind: candidate.kind,
            bank_date: candidate.proposed_bank_date,
        }))
    }

    /// The bank reconciliation statement of `ledger` as of `as_of` (spec 6.5, K3 rules).
    ///
    /// The books side (balance per books, uncleared deposits and payments, balance per bank) is
    /// always a value. The statement side is `Unsupported` unless an imported statement covers
    /// `as_of` exactly; see `ledgerly_bank::brs` for the coverage rule.
    ///
    /// # Errors
    /// - [`ApiError::Bank`] when `ledger` does not exist, is not a bank ledger, or a statement held
    ///   for it cannot be used.
    /// - [`ApiError::Stale`] after a saved change whose reload failed.
    ///
    /// Returns `Outcome::Unsupported` when the books cannot be read.
    pub fn brs(&self, ledger: LedgerId, as_of: Date) -> Result<Outcome<Brs>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        let statements = self.bank.for_ledger(ledger);
        Ok(ledgerly_bank::brs::brs(
            books,
            ledger,
            as_of,
            &statements,
        )?)
    }

    /// The id for a new statement: one more than the largest id in the audit log or held here, so
    /// an id is not reused after the file is reopened.
    fn next_statement_id(&self) -> Result<BankStatementId, ApiError> {
        let records = read_all(self.store())?;
        let logged = records.iter().filter_map(|record| {
            if let Event::StatementImported(id) = &record.event {
                Some(id.0)
            } else {
                None
            }
        });
        let largest = logged.chain(self.bank.largest_id()).max().unwrap_or(0);
        let next = largest.checked_add(1).ok_or_else(|| BankError::Invalid {
            field: "bank statement id".to_owned(),
            reason: "no bank statement id is left in this company file".to_owned(),
        })?;
        Ok(BankStatementId(next))
    }
}

/// Parses `file` with the registry format `format_id`. CSV is parsed with its layout; XLSX and
/// MT940 use their own parsers.
///
/// # Errors
/// [`ApiError::UnknownBankFormat`] when the id is not registered; [`ApiError::BankFormat`] when
/// the registry or the file cannot be read in that format.
fn parse_statement(file: &[u8], format_id: &str) -> Result<RawStatement, ApiError> {
    let registry = builtin_registry()?;
    let spec = registry
        .get(format_id)
        .ok_or_else(|| ApiError::UnknownBankFormat(format_id.to_owned()))?;
    let raw = match spec.kind {
        FileKind::Csv => parse_csv_with_layout(file, &spec.csv_layout()?)?,
        FileKind::Xlsx => parse_xlsx(file, spec)?,
        FileKind::Mt940 => parse_mt940(mt940_text(file)?)?,
    };
    Ok(raw)
}

/// The text of an MT940 file.
///
/// # Errors
/// [`BankFmtError::Format`] when the bytes are not UTF-8.
fn mt940_text(file: &[u8]) -> Result<&str, BankFmtError> {
    std::str::from_utf8(file).map_err(|_| BankFmtError::Format {
        reason: "an MT940 statement must be UTF-8 text".to_owned(),
    })
}

/// Checks that `ledger` exists in `books` and is a bank ledger.
///
/// # Errors
/// [`BankError::NotABankLedger`] for a ledger without bank details; [`BankError::Invalid`] when
/// the ledger does not exist.
fn require_bank_ledger(books: &Books, ledger: LedgerId) -> Result<(), ApiError> {
    match books.masters.ledgers.get(&ledger) {
        Some(master) if master.bank.is_some() => Ok(()),
        Some(_) => Err(BankError::NotABankLedger(ledger).into()),
        None => Err(BankError::Invalid {
            field: "ledger".to_owned(),
            reason: format!("ledger {} does not exist in this company", ledger.0),
        }
        .into()),
    }
}
