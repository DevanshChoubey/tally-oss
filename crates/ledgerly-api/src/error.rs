//! Errors of the facade (card W1). Each variant wraps one crate's typed error.
//!
//! A refused change is `Err(ApiError::Engine(..))`. Input this build cannot compute is not an
//! error: it is `Ok(Outcome::Unsupported(..))` (rule A1).

use ledgerly_bank::BankError;
use ledgerly_bank::statement::StatementError;
use ledgerly_bankfmt::BankFmtError;
use ledgerly_engine::EngineError;
use ledgerly_reports::ReportError;
use ledgerly_store::StoreError;
use thiserror::Error;

/// Why a facade call could not complete.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The engine refused the command. Nothing was written.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// The store failed. When a commit failed, nothing was written.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A report engine rejected its parameters or its input.
    #[error(transparent)]
    Report(#[from] ReportError),
    /// A bank request was refused: the ledger is not a bank ledger, or a bank rule failed.
    #[error(transparent)]
    Bank(#[from] BankError),
    /// A statement file failed its checks (duplicate file, missing balance, bad line, balance
    /// mismatch). Nothing was kept.
    #[error(transparent)]
    StatementImport(#[from] StatementError),
    /// A statement file could not be read in the chosen format.
    #[error(transparent)]
    BankFormat(#[from] BankFmtError),
    /// The format id is not in the built-in format registry.
    #[error("bank format '{0}' is not in the format registry")]
    UnknownBankFormat(String),
    /// This session does not hold the statement. Statements are not kept in the company file, so
    /// a statement imported before the file was reopened must be imported again.
    #[error("bank statement {id} is not held by this session; import the statement file again")]
    UnknownBankStatement {
        /// The statement id.
        id: u64,
    },
    /// The current suggestions for the statement do not list this match: it was already matched,
    /// or the statement or the books changed. Run the suggestions again.
    #[error(
        "the match on statement line(s) {lines:?} is not among the current suggestions; run the suggestions again"
    )]
    MatchNotSuggested {
        /// The statement lines of the refused match.
        lines: Vec<u32>,
    },
    /// A change was saved, but the books could not be reloaded afterwards, so figures are not
    /// computed until a command or [`crate::Api::open`] reloads them.
    #[error(
        "the books could not be reloaded after a saved change; run a command or reopen the company file"
    )]
    Stale,
}
