use ledgerly_core::{Date, Paise};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A statement as parsed from a bank file, before any checks or matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawStatement {
    /// Format registry id used.
    pub format_id: String,
    /// Account number or name found in the file, if any.
    pub account_hint: Option<String>,
    /// Opening balance stated in the file (debit-positive means overdrawn is negative; see `RawLine`).
    pub opening: Option<Paise>,
    /// Closing balance stated in the file.
    pub closing: Option<Paise>,
    /// Lines in file order.
    pub lines: Vec<RawLine>,
}

/// One statement line. Balances are from the account holder's view: credit balance positive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLine {
    /// Row or record number in the file (1-based).
    pub row: u32,
    /// Transaction date.
    pub date: Date,
    /// Value date.
    pub value_date: Option<Date>,
    /// Description.
    pub description: String,
    /// Cheque or reference number.
    pub reference: Option<String>,
    /// Money out (withdrawal); zero when none.
    pub debit: Paise,
    /// Money in (deposit); zero when none.
    pub credit: Paise,
    /// Running balance after the line, when the file has one.
    pub balance: Option<Paise>,
    /// The raw cells, kept for audit.
    pub raw: Vec<String>,
}

/// Errors parsing bank files.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BankFmtError {
    /// A row could not be parsed.
    #[error("row {row}: {reason}")]
    Row {
        /// Row number (1-based).
        row: u32,
        /// What is wrong.
        reason: String,
    },
    /// The file does not match the format description.
    #[error("format error: {reason}")]
    Format {
        /// What is wrong.
        reason: String,
    },
    /// The format registry entry is invalid.
    #[error("format description '{id}': {reason}")]
    Registry {
        /// Format id.
        id: String,
        /// What is wrong.
        reason: String,
    },
}
