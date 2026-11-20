use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, Date, GodownId, LedgerId, Paise, Qty, SignedAmount, StockItemId, VoucherId,
    VoucherTypeId,
};
use serde::{Deserialize, Serialize};

use crate::company::Company;
use crate::masters::Masters;
use crate::voucher::{Voucher, VoucherStatus};

/// One ledger posting derived from a voucher line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    /// Source voucher.
    pub voucher: VoucherId,
    /// Voucher date.
    pub date: Date,
    /// Ledger.
    pub ledger: LedgerId,
    /// Amount and side.
    pub amount: SignedAmount,
    /// Voucher status (scenarios filter on it).
    pub status: VoucherStatus,
    /// Voucher type.
    pub type_id: VoucherTypeId,
}

/// One stock movement derived from an inventory line allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockMove {
    /// Source voucher.
    pub voucher: VoucherId,
    /// Voucher date.
    pub date: Date,
    /// Item.
    pub item: StockItemId,
    /// Godown.
    pub godown: GodownId,
    /// Batch.
    pub batch: Option<BatchId>,
    /// Quantity in, in base units.
    pub qty_in: Qty,
    /// Quantity out, in base units.
    pub qty_out: Qty,
    /// Value of the movement at voucher rate.
    pub value: Paise,
}

/// The read model every pure domain function consumes.
///
/// `postings` and `stock_moves` are sorted by `(date, voucher)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Books {
    /// The company.
    pub company: Company,
    /// All masters.
    pub masters: Masters,
    /// All vouchers, including cancelled ones.
    pub vouchers: BTreeMap<VoucherId, Voucher>,
    /// Ledger postings.
    pub postings: Vec<Posting>,
    /// Stock movements.
    pub stock_moves: Vec<StockMove>,
}

/// Which vouchers a computation includes (spec invariant 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scenario {
    /// Regular vouchers, plus post-dated ones on or before the as-of date.
    Actual,
    /// [`Scenario::Actual`] plus vouchers with these statuses.
    Including(Vec<VoucherStatus>),
}
