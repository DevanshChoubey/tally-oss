use ledgerly_core::{LedgerId, MoneyError};
use thiserror::Error;

/// Banking errors (shared by cards K1-K5).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BankError {
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// The ledger is not a bank ledger.
    #[error("ledger {0:?} is not a bank account")]
    NotABankLedger(LedgerId),
    /// A cheque leaf is already used or outside the book.
    #[error("cheque {number}: {reason}")]
    ChequeLeaf {
        /// Cheque number.
        number: String,
        /// What is wrong.
        reason: String,
    },
    /// Any other problem.
    #[error("{field}: {reason}")]
    Invalid {
        /// Field.
        field: String,
        /// Reason.
        reason: String,
    },
}
