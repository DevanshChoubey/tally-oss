use ledgerly_core::{Date, MoneyError, Paise};
use ledgerly_model::{ReservedGroup, ReservedLedger, VoucherError};
use thiserror::Error;

/// Why a command was rejected. Nothing is written when a command errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    /// The voucher breaks a store-free invariant.
    #[error(transparent)]
    Voucher(#[from] VoucherError),
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// Reserved groups cannot be deleted.
    #[error("{0:?} is a reserved group and cannot be deleted")]
    ReservedGroup(ReservedGroup),
    /// Reserved ledgers cannot be deleted.
    #[error("{0:?} is a reserved ledger and cannot be deleted")]
    ReservedLedger(ReservedLedger),
    /// The master is used by vouchers.
    #[error("in use by {vouchers} voucher(s); remove those first")]
    InUse {
        /// Number of vouchers using it.
        vouchers: usize,
    },
    /// The change would make a group its own ancestor.
    #[error("a group cannot be placed under itself or its own sub-group")]
    GroupCycle,
    /// Something referenced does not exist.
    #[error("{what} does not exist")]
    NotFound {
        /// What was looked up.
        what: String,
    },
    /// A name or number is already taken.
    #[error("{what} already exists")]
    Duplicate {
        /// What clashes.
        what: String,
    },
    /// The date is inside a locked period.
    #[error("books are locked up to {until}; unlock the period to change this")]
    PeriodLocked {
        /// Last locked date.
        until: Date,
    },
    /// Sales-side and GST document series are never deleted or renumbered; cancel instead.
    #[error(
        "vouchers in a sales or GST document series cannot be deleted; cancel the voucher instead"
    )]
    SalesSeriesNoDelete,
    /// A settlement exceeds the open amount of a bill.
    #[error("bill '{bill}' has {open:?} open; {settling:?} would over-settle it")]
    OverSettled {
        /// Bill name.
        bill: String,
        /// Open amount.
        open: Paise,
        /// Amount being settled.
        settling: Paise,
    },
    /// The voucher number is too long for the document rules.
    #[error("number '{number}' is longer than {max} characters")]
    NumberTooLong {
        /// The number.
        number: String,
        /// Maximum length.
        max: usize,
    },
    /// Any other rejection, with the field and the reason.
    #[error("{field}: {reason}")]
    Invalid {
        /// Field or object.
        field: String,
        /// What is wrong and how to fix it.
        reason: String,
    },
}
