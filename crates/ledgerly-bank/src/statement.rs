//! Imported statements and the balance check (card K1, spec 6.5).
//!
//! A [`BankStatement`] is an immutable imported fact: it can only be built by
//! [`check_statement`], which rejects the whole file unless every check passes. Nothing is
//! corrected, defaulted or partially accepted (rule A1).
//!
//! Frozen interface (plan card K1):
//!
//! ```text
//! pub struct BankStatement { .. }   pub enum StatementError { BalanceMismatch { .. }, RunningBalance { row: u32, .. }, Duplicate { file_hash: String }, .. }
//! pub fn check_statement(raw: RawStatement, file_bytes: &[u8], ledger: LedgerId, known_hashes: &[String]) -> Result<BankStatement, StatementError>;
//! ```
//!
//! Balance convention (from [`RawLine`]): balances are from the account holder's view, credit
//! balance positive. The balance after a line is `previous + credit - debit`, and the file's
//! closing balance must equal `opening + Σ credit - Σ debit`.

use ledgerly_bankfmt::{RawLine, RawStatement};
use ledgerly_core::{Date, LedgerId, MoneyError, Paise};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Why a statement file was rejected. Every variant means nothing was stored.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StatementError {
    /// Opening balance plus the net of all lines is not the stated closing balance.
    #[error(
        "statement does not balance: opening {} + net of lines {} = {}, but the file states closing {}",
        .opening.to_rupees(), .lines_net.to_rupees(), .computed_closing.to_rupees(), .stated_closing.to_rupees()
    )]
    BalanceMismatch {
        /// Opening balance stated in the file.
        opening: Paise,
        /// Σ credit - Σ debit over all lines.
        lines_net: Paise,
        /// `opening + lines_net`.
        computed_closing: Paise,
        /// Closing balance stated in the file.
        stated_closing: Paise,
    },
    /// A line's running balance disagrees with the balance carried forward from the rows before it.
    #[error(
        "row {row}: running balance is {} but the previous balance and this row's amounts give {}",
        .found.to_rupees(), .expected.to_rupees()
    )]
    RunningBalance {
        /// File row number of the first bad line.
        row: u32,
        /// Balance implied by the opening balance and the lines up to and including this row.
        expected: Paise,
        /// Balance the file states on this row.
        found: Paise,
    },
    /// A file with the same hash was already imported.
    #[error("this file was already imported (SHA-256 {file_hash})")]
    Duplicate {
        /// Lower-case hex SHA-256 of the file.
        file_hash: String,
    },
    /// The file does not state an opening or closing balance, so it cannot be verified.
    #[error("the file states no {which} balance, so the statement cannot be verified")]
    MissingBalance {
        /// `"opening"` or `"closing"`.
        which: &'static str,
    },
    /// A line has an amount that cannot be right.
    #[error("row {row}: {reason}")]
    InvalidLine {
        /// File row number.
        row: u32,
        /// What is wrong.
        reason: String,
    },
    /// Arithmetic overflow while summing.
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// A verified, immutable imported statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankStatement {
    ledger: LedgerId,
    file_hash: String,
    format_id: String,
    account_hint: Option<String>,
    opening: Paise,
    closing: Paise,
    period: Option<(Date, Date)>,
    lines: Vec<RawLine>,
}

impl BankStatement {
    /// The bank ledger this statement was imported for.
    pub fn ledger(&self) -> LedgerId {
        self.ledger
    }

    /// Lower-case hex SHA-256 of the imported file.
    pub fn file_hash(&self) -> &str {
        &self.file_hash
    }

    /// Format registry id the file was parsed with.
    pub fn format_id(&self) -> &str {
        &self.format_id
    }

    /// Account number or name found in the file, if any.
    pub fn account_hint(&self) -> Option<&str> {
        self.account_hint.as_deref()
    }

    /// Opening balance as printed in the file (credit positive).
    pub fn opening(&self) -> Paise {
        self.opening
    }

    /// Closing balance as printed in the file (credit positive).
    pub fn closing(&self) -> Paise {
        self.closing
    }

    /// Earliest and latest transaction date, or `None` for a statement with no lines.
    pub fn period(&self) -> Option<(Date, Date)> {
        self.period
    }

    /// The lines in file order, exactly as parsed.
    pub fn lines(&self) -> &[RawLine] {
        &self.lines
    }
}

/// Verifies a parsed statement and, only if every check passes, returns it as an immutable fact.
///
/// Checks, in order: the file hash is not in `known_hashes` (hex compared ignoring case); opening
/// and closing balances are present; every line has non-negative amounts and is not both a debit
/// and a credit; each stated running balance equals the balance carried forward; and
/// `opening + Σ credit - Σ debit = closing`.
///
/// # Errors
/// A [`StatementError`] naming the first problem found; no partial result is ever returned.
pub fn check_statement(
    raw: RawStatement,
    file_bytes: &[u8],
    ledger: LedgerId,
    known_hashes: &[String],
) -> Result<BankStatement, StatementError> {
    let file_hash = hex::encode(Sha256::digest(file_bytes));
    if known_hashes
        .iter()
        .any(|known| known.eq_ignore_ascii_case(&file_hash))
    {
        return Err(StatementError::Duplicate { file_hash });
    }
    let opening = raw
        .opening
        .ok_or(StatementError::MissingBalance { which: "opening" })?;
    let stated_closing = raw
        .closing
        .ok_or(StatementError::MissingBalance { which: "closing" })?;

    let mut running = opening;
    for line in &raw.lines {
        check_amounts(line)?;
        running = running.checked_add(line.credit)?.checked_sub(line.debit)?;
        if let Some(found) = line.balance
            && found != running
        {
            return Err(StatementError::RunningBalance {
                row: line.row,
                expected: running,
                found,
            });
        }
    }
    if running != stated_closing {
        return Err(StatementError::BalanceMismatch {
            opening,
            lines_net: running.checked_sub(opening)?,
            computed_closing: running,
            stated_closing,
        });
    }

    let period = raw.lines.iter().fold(None, |acc, l| match acc {
        None => Some((l.date, l.date)),
        Some((lo, hi)) => Some((lo.min(l.date), hi.max(l.date))),
    });
    Ok(BankStatement {
        ledger,
        file_hash,
        format_id: raw.format_id,
        account_hint: raw.account_hint,
        opening,
        closing: stated_closing,
        period,
        lines: raw.lines,
    })
}

fn check_amounts(line: &RawLine) -> Result<(), StatementError> {
    if line.debit.is_negative() || line.credit.is_negative() {
        return Err(StatementError::InvalidLine {
            row: line.row,
            reason: "debit and credit amounts must not be negative".to_string(),
        });
    }
    if !line.debit.is_zero() && !line.credit.is_zero() {
        return Err(StatementError::InvalidLine {
            row: line.row,
            reason: "a line cannot be both a debit and a credit".to_string(),
        });
    }
    Ok(())
}
