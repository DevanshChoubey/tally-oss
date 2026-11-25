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
//! Each import commits one `Event::BankStatementStored` event: the verified statement (its lines,
//! balances, end date and file hash) is in the company file. [`Statements`] is a cache of those
//! events, read from the audit log the first time a statement is needed after the file is opened
//! (`Api::open` is not owned by this card, so the read is lazy). After a reopen the statements,
//! the duplicate-file check and the BRS statement side are all as they were. Every stored
//! statement is verified again when it is read back; one that no longer balances is an error, not
//! a quiet omission. A file written before this change holds only `StatementImported` ids: those
//! statements have no rows to rebuild, and their ids are never reused.
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
//! `check_statement_with_end` (ledgerly-bank). A failed check keeps nothing. A CSV statement gets
//! its opening and closing balance from its balance column (ledgerly-bankfmt); a CSV without one
//! is refused with `MissingBalance`. An MT940 statement keeps its `:62F:` date as the end date.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use ledgerly_bank::BankError;
use ledgerly_bank::brs::Brs;
use ledgerly_bank::matcher::{Candidate, MatchRule, suggest_matches};
use ledgerly_bank::statement::{
    BankStatement, StatementError, check_statement_with_end, restore_statement,
};
use ledgerly_bankfmt::csv::parse_csv_with_layout;
use ledgerly_bankfmt::mt940::parse_mt940_with_end;
use ledgerly_bankfmt::registry::{FileKind, builtin_registry};
use ledgerly_bankfmt::xlsx::parse_xlsx;
use ledgerly_bankfmt::{BankFmtError, ParsedStatement, RawLine, RawStatement};
use ledgerly_core::{BankStatementId, Date, LedgerId, Outcome, Paise};
use ledgerly_model::{BankMatch, Books, MatchKind};
use ledgerly_store::{AuditRecord, Event, StoredStatementLine};

use crate::command::{Command, CommandResult};
use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The bank statements of the open company file, by id, read from the audit log on first use.
#[derive(Debug, Default)]
pub(crate) struct Statements {
    /// Statements by id, in id order. Unset until the audit log has been read.
    by_id: OnceLock<BTreeMap<BankStatementId, BankStatement>>,
}

impl Statements {
    /// The statements, reading them from the audit log the first time.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the log cannot be read; [`ApiError::StatementImport`] when a
    /// stored statement no longer passes its checks.
    fn all(
        &self,
        records: impl FnOnce() -> Result<Vec<AuditRecord>, ApiError>,
    ) -> Result<&BTreeMap<BankStatementId, BankStatement>, ApiError> {
        if let Some(loaded) = self.by_id.get() {
            return Ok(loaded);
        }
        let mut by_id = BTreeMap::new();
        for record in records()? {
            if matches!(record.event, Event::BankStatementStored { .. }) {
                let (id, statement) = rebuild(record.event)?;
                by_id.insert(id, statement);
            }
        }
        Ok(self.by_id.get_or_init(|| by_id))
    }

    /// Stores a verified, just-committed statement. The cache must already be loaded.
    fn insert(&mut self, id: BankStatementId, statement: BankStatement) {
        if let Some(loaded) = self.by_id.get_mut() {
            loaded.insert(id, statement);
        }
    }
}

/// The statement an `Event::BankStatementStored` holds, verified again.
///
/// # Errors
/// [`ApiError::StatementImport`] when the stored statement fails its checks.
fn rebuild(event: Event) -> Result<(BankStatementId, BankStatement), ApiError> {
    let Event::BankStatementStored {
        id,
        ledger,
        file_hash,
        format_id,
        account_hint,
        opening,
        closing,
        end_date,
        lines,
    } = event
    else {
        return Err(BankError::Invalid {
            field: "audit log".to_owned(),
            reason: "expected a stored bank statement event".to_owned(),
        }
        .into());
    };
    let raw = RawStatement {
        format_id,
        account_hint,
        opening: Some(opening),
        closing: Some(closing),
        lines: lines.into_iter().map(raw_line).collect(),
    };
    Ok((id, restore_statement(raw, end_date, file_hash, ledger)?))
}

fn raw_line(line: StoredStatementLine) -> RawLine {
    RawLine {
        row: line.row,
        date: line.date,
        value_date: line.value_date,
        description: line.description,
        reference: line.reference,
        debit: line.debit,
        credit: line.credit,
        balance: line.balance,
        raw: line.raw,
    }
}

fn stored_line(line: &RawLine) -> StoredStatementLine {
    StoredStatementLine {
        row: line.row,
        date: line.date,
        value_date: line.value_date,
        description: line.description.clone(),
        reference: line.reference.clone(),
        debit: line.debit,
        credit: line.credit,
        balance: line.balance,
        raw: line.raw.clone(),
    }
}

/// The event that stores `statement` under `id`.
fn stored_event(id: BankStatementId, statement: &BankStatement) -> Event {
    Event::BankStatementStored {
        id,
        ledger: statement.ledger(),
        file_hash: statement.file_hash().to_owned(),
        format_id: statement.format_id().to_owned(),
        account_hint: statement.account_hint().map(str::to_owned),
        opening: statement.opening(),
        closing: statement.closing(),
        end_date: statement.end_date(),
        lines: statement.lines().iter().map(stored_line).collect(),
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
        let parsed = parse_statement(file, format_id)?;
        let books = match self.loaded_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        require_bank_ledger(books, ledger)?;
        let hashes = self.known_hashes()?;
        let statement =
            check_statement_with_end(parsed.raw, parsed.end_date, file, ledger, &hashes)?;
        let id = self.next_statement_id()?;
        let actor = self.actor().clone();
        self.commit_as(&[stored_event(id, &statement)], &actor)?;
        self.bank.insert(id, statement);
        Ok(Outcome::Value(id))
    }

    /// The statements of `ledger` stored in the company file, in id order. They are there after a
    /// reopen too. The list is empty when the stored statements cannot be read; use
    /// [`Api::stored_bank_statements`] to see why.
    pub fn bank_statements(&self, ledger: LedgerId) -> Vec<(BankStatementId, &BankStatement)> {
        self.stored_bank_statements(ledger).unwrap_or_default()
    }

    /// The statements of `ledger` stored in the company file, in id order.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read; [`ApiError::StatementImport`] when a
    /// stored statement no longer passes its checks.
    pub fn stored_bank_statements(
        &self,
        ledger: LedgerId,
    ) -> Result<Vec<(BankStatementId, &BankStatement)>, ApiError> {
        Ok(self
            .held()?
            .iter()
            .filter(|(_, statement)| statement.ledger() == ledger)
            .map(|(id, statement)| (*id, statement))
            .collect())
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
        let held = self.held_statement(statement)?;
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
        let held = self.held_statement(statement)?;
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
        let statements: Vec<BankStatement> = self
            .held()?
            .values()
            .filter(|statement| statement.ledger() == ledger)
            .cloned()
            .collect();
        Ok(ledgerly_bank::brs::brs(books, ledger, as_of, &statements)?)
    }

    /// The statements in the company file, read from the audit log on first use.
    pub(crate) fn held(&self) -> Result<&BTreeMap<BankStatementId, BankStatement>, ApiError> {
        self.bank
            .all(|| read_all(self.store()).map_err(ApiError::from))
    }

    /// The stored statement `id`.
    ///
    /// # Errors
    /// [`ApiError::UnknownBankStatement`] when the company file holds no such statement.
    fn held_statement(&self, id: BankStatementId) -> Result<&BankStatement, ApiError> {
        self.held()?
            .get(&id)
            .ok_or(ApiError::UnknownBankStatement { id: id.0 })
    }

    /// The file hash of every stored statement, for the duplicate check.
    fn known_hashes(&self) -> Result<Vec<String>, ApiError> {
        Ok(self
            .held()?
            .values()
            .map(|statement| statement.file_hash().to_owned())
            .collect())
    }

    /// The id for a new statement: one more than the largest id stored, or logged by an older
    /// build as `StatementImported`, so an id is never reused.
    fn next_statement_id(&self) -> Result<BankStatementId, ApiError> {
        let records = read_all(self.store())?;
        let logged = records.iter().filter_map(|record| {
            if let Event::StatementImported(id) = &record.event {
                Some(id.0)
            } else {
                None
            }
        });
        let stored = self.held()?.keys().map(|id| id.0);
        let largest = logged.chain(stored).max().unwrap_or(0);
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
fn parse_statement(file: &[u8], format_id: &str) -> Result<ParsedStatement, ApiError> {
    let registry = builtin_registry()?;
    let spec = registry
        .get(format_id)
        .ok_or_else(|| ApiError::UnknownBankFormat(format_id.to_owned()))?;
    let parsed = match spec.kind {
        FileKind::Csv => ParsedStatement {
            raw: parse_csv_with_layout(file, &spec.csv_layout()?)?,
            end_date: None,
        },
        FileKind::Xlsx => ParsedStatement {
            raw: parse_xlsx(file, spec)?,
            end_date: None,
        },
        FileKind::Mt940 => parse_mt940_with_end(mt940_text(file)?)?,
    };
    Ok(parsed)
}

/// Why a statement file was refused for not adding up, with every figure of the working: what
/// the file opens with, what its lines add, what that makes, what the file prints, and the gap.
/// The screen shows these as they are; it adds nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceRefusal {
    /// Opening balance stated in the file.
    pub opening: Paise,
    /// How many lines the file has.
    pub lines: usize,
    /// Deposits less withdrawals over all lines.
    pub lines_net: Paise,
    /// The opening balance plus the net of the lines.
    pub computed_closing: Paise,
    /// The closing balance the file prints.
    pub stated_closing: Paise,
    /// The printed closing balance less the computed one.
    pub difference: Paise,
}

/// The working behind a refusal of `file` for a balance that does not add up, or `None` when
/// `error` is some other refusal (a duplicate, a missing balance, a bad line, a break in the
/// running balance), or the file cannot be read again in `format_id`.
#[must_use]
pub fn balance_refusal(file: &[u8], format_id: &str, error: &ApiError) -> Option<BalanceRefusal> {
    let ApiError::StatementImport(StatementError::BalanceMismatch {
        opening,
        lines_net,
        computed_closing,
        stated_closing,
    }) = error
    else {
        return None;
    };
    let lines = parse_statement(file, format_id).ok()?.raw.lines.len();
    let difference = stated_closing.checked_sub(*computed_closing).ok()?;
    Some(BalanceRefusal {
        opening: *opening,
        lines,
        lines_net: *lines_net,
        computed_closing: *computed_closing,
        stated_closing: *stated_closing,
        difference,
    })
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
