//! Shared vocabulary for Ledgerly.
//!
//! Everything here is pure data and arithmetic with no I/O. Money is [`Paise`] (`i64`), never a
//! float (engineering rule A3). Every computation elsewhere returns an [`Outcome`]: a value or an
//! explicit [`Unsupported`] (rule A1).

mod cancel;
mod clock;
mod dates;
mod gstin;
mod hsn;
mod ids;
mod money;
mod outcome;
mod pan;
mod qty;
mod state;
mod tally_ref;

pub mod date_entry;
pub mod format;
pub mod money_ops;

pub use cancel::CancelToken;
pub use clock::Clock;
pub use dates::{FinancialYear, Period, PeriodError};
pub use gstin::{Gstin, GstinError, gstin_check_character};
pub use hsn::{Hsn, HsnError};
pub use ids::{
    BankStatementId, BatchId, CompanyId, CostCategoryId, CostCentreId, EventSeq, GodownId, GroupId,
    LedgerId, StockGroupId, StockItemId, UnitId, UserId, VoucherId, VoucherTypeId,
};
pub use money::{DrCr, MoneyError, Paise, RoundingRule, SignedAmount, round_rupees_to_paise};
pub use outcome::{Outcome, Unsupported};
pub use pan::{Pan, PanError};
pub use qty::{Qty, Rate};
pub use state::{StateCode, StateCodeError};
pub use tally_ref::TallyRef;

/// Re-exported so every crate uses the same decimal and date types.
pub use rust_decimal::Decimal;
/// Re-exported so every crate uses the same date type.
pub use time::{Date, Month, OffsetDateTime};
