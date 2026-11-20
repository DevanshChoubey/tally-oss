use ledgerly_core::{MoneyError, PeriodError};
use thiserror::Error;

/// GST computation errors. Unsupported cases are `Outcome::Unsupported`, not errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GstError {
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// Bad period.
    #[error(transparent)]
    Period(#[from] PeriodError),
    /// A rule pack is malformed.
    #[error("rule pack error: {reason}")]
    Pack {
        /// What is wrong.
        reason: String,
    },
    /// Something referenced does not exist.
    #[error("{what} does not exist")]
    NotFound {
        /// What was looked up.
        what: String,
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
