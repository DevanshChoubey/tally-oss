use ledgerly_core::{Date, LedgerId, MoneyError, Paise, VoucherTypeId};
use thiserror::Error;

/// Why a voucher fails the store-free invariants (spec 5).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VoucherError {
    /// Debits and credits differ.
    #[error("voucher does not balance: debits {dr:?}, credits {cr:?}")]
    Unbalanced {
        /// Total debits.
        dr: Paise,
        /// Total credits.
        cr: Paise,
    },
    /// Bill allocations on a line do not add up to the line.
    #[error("bill allocations on line {line} total {allocated:?}, the line is {expected:?}")]
    BillAllocationMismatch {
        /// Line index (0-based).
        line: usize,
        /// Line amount.
        expected: Paise,
        /// Sum of allocations.
        allocated: Paise,
    },
    /// Voucher dated before books begin.
    #[error("voucher date {date} is before books begin on {books_begin}")]
    BeforeBooksBegin {
        /// Voucher date.
        date: Date,
        /// Books-begin date.
        books_begin: Date,
    },
    /// An Against reference names a bill of another ledger.
    #[error("bill '{bill}' does not belong to this ledger")]
    BillOnWrongLedger {
        /// Bill name.
        bill: String,
        /// Ledger on the line.
        ledger: LedgerId,
    },
    /// The voucher kind needs a party.
    #[error("this voucher needs a party")]
    MissingParty,
    /// A line uses a ledger that does not exist.
    #[error("ledger {0:?} does not exist")]
    UnknownLedger(LedgerId),
    /// The voucher type does not exist.
    #[error("voucher type {0:?} does not exist")]
    UnknownVoucherType(VoucherTypeId),
    /// No lines.
    #[error("voucher has no lines")]
    Empty,
    /// A line amount is negative.
    #[error("line {line} has a negative amount")]
    NegativeAmount {
        /// Line index (0-based).
        line: usize,
    },
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// Any other invariant, with the field and the reason.
    #[error("{field}: {reason}")]
    Invalid {
        /// Field or line.
        field: String,
        /// What is wrong and how to fix it.
        reason: String,
    },
}
