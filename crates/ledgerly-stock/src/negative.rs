//! Negative stock detection and policy. **Card T5** implements this module.
//!
//! Frozen interface (plan card T5):
//!
//! ```text
//! pub fn first_negative(item: StockItemId, moves: &[StockMove]) -> Option<Date>;
//! pub fn check_policy(v: &Voucher, books: &Books, policy: NegativeStockPolicy) -> Result<Vec<StockWarning>, StockError>;
//! ```
//!
//! # Detection
//!
//! The running quantity of an item is its opening quantity plus every receipt minus every issue,
//! taken in `(date, voucher)` order, across all godowns and batches (negative stock is checked per
//! item, not per godown). It is negative when it is strictly below zero; exactly zero is fine.
//! The first date at which it is negative after a movement is the date reported.
//!
//! [`first_negative`] has the frozen signature and no item master, so it starts from zero opening
//! stock. Callers that know the item use [`first_negative_with_opening`], which starts from the
//! item's opening stock and stops at a date. [`first_negative`] cannot report an arithmetic
//! overflow; it returns `None` in that case, so valuation code uses the checked variant.
//!
//! # Policy
//!
//! [`check_policy`] answers "would saving this voucher take stock below zero?" for the items the
//! voucher issues:
//!
//! * [`NegativeStockPolicy::Block`]: the first such item (lowest id) is an error,
//!   [`StockError::NegativeStockBlocked`].
//! * [`NegativeStockPolicy::Warn`]: one [`StockWarning`] per such item, lowest id first.
//! * [`NegativeStockPolicy::Allow`]: no warnings; valuations over the period are still marked
//!   provisional by the valuation methods.
//!
//! The voucher's own movements are derived from its inventory lines (base units, direction from
//! the voucher type's base kind, as the movement ledger is built) and replace any movements the
//! books already hold for the same voucher id, so editing a voucher is checked correctly. Only
//! vouchers that issue stock are checked: a voucher that only receives stock cannot lower an
//! item's quantity. A stock effect that cannot be decided (stock journal, physical stock, or an
//! accounting voucher kind that carries inventory lines) is refused with
//! [`StockError::Invalid`], never guessed. Optional and cancelled vouchers are not in the books
//! and are not checked; regular and post-dated vouchers are.
//!
//! An item is reported when, with the voucher included, its running quantity is negative after
//! some movement dated on or after the voucher's date. Negative stretches that end before the
//! voucher's date are not this voucher's doing.

use std::collections::BTreeMap;

use ledgerly_core::{Date, Decimal, MoneyError, Qty, StockItemId, VoucherId};
use ledgerly_model::{
    BaseVoucherKind, Books, NegativeStockPolicy, StockItem, StockMove, Voucher, VoucherStatus,
};

use crate::StockError;

/// A voucher would leave an item below zero, and the policy let it through with a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockWarning {
    /// The item that goes below zero.
    pub item: StockItemId,
    /// First date on or after the voucher's date at which the running quantity is negative.
    pub date: Date,
    /// The voucher being checked.
    pub voucher: VoucherId,
}

/// One quantity step of an item's history.
#[derive(Debug, Clone, Copy)]
struct Step {
    date: Date,
    voucher: VoucherId,
    qty_in: Decimal,
    qty_out: Decimal,
}

fn invalid(field: &str, reason: impl Into<String>) -> StockError {
    StockError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn overflow() -> StockError {
    StockError::Money(MoneyError::Overflow)
}

/// Sum of the item's opening quantities.
///
/// # Errors
/// [`StockError::Invalid`] for a negative opening quantity; [`StockError::Money`] on overflow.
pub(crate) fn opening_qty(item: &StockItem) -> Result<Decimal, StockError> {
    let mut total = Decimal::ZERO;
    for o in &item.opening {
        if o.qty.0 < Decimal::ZERO {
            return Err(invalid("opening", "negative opening quantity"));
        }
        total = total.checked_add(o.qty.0).ok_or_else(overflow)?;
    }
    Ok(total)
}

/// Walks the sorted steps; returns the first date at or after `from` where the running quantity
/// is negative after a step.
fn scan(opening: Decimal, steps: &[Step], from: Option<Date>) -> Result<Option<Date>, StockError> {
    let mut running = opening;
    for s in steps {
        running = running
            .checked_add(s.qty_in)
            .and_then(|r| r.checked_sub(s.qty_out))
            .ok_or_else(overflow)?;
        if running < Decimal::ZERO && from.is_none_or(|f| s.date >= f) {
            return Ok(Some(s.date));
        }
    }
    Ok(None)
}

fn sorted_steps(item: StockItemId, moves: &[StockMove], through: Option<Date>) -> Vec<Step> {
    let mut steps: Vec<Step> = moves
        .iter()
        .filter(|m| m.item == item && through.is_none_or(|t| m.date <= t))
        .map(|m| Step {
            date: m.date,
            voucher: m.voucher,
            qty_in: m.qty_in.0,
            qty_out: m.qty_out.0,
        })
        .collect();
    steps.sort_by_key(|s| (s.date, s.voucher));
    steps
}

/// First date on which the running quantity of `item` is below zero, counting from zero opening
/// stock and ignoring other items' moves. The moves need not be sorted.
///
/// Returns `None` when the quantity never goes below zero, and also when the quantity
/// arithmetic overflows; use [`first_negative_with_opening`] to see that error.
pub fn first_negative(item: StockItemId, moves: &[StockMove]) -> Option<Date> {
    let steps = sorted_steps(item, moves, None);
    scan(Decimal::ZERO, &steps, None).ok().flatten()
}

/// First date on or before `through` on which the running quantity of `item` is below zero,
/// starting from the item's opening stock. Moves of other items and moves after `through` are
/// ignored; the moves need not be sorted.
///
/// # Errors
/// [`StockError::Invalid`] for a negative opening quantity; [`StockError::Money`] on overflow.
pub fn first_negative_with_opening(
    item: &StockItem,
    moves: &[StockMove],
    through: Date,
) -> Result<Option<Date>, StockError> {
    let steps = sorted_steps(item.id, moves, Some(through));
    scan(opening_qty(item)?, &steps, None)
}

/// How a voucher kind moves stock.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StockEffect {
    Receipt,
    Issue,
    NoMovement,
    Undecided,
}

/// Listed in full so that a new voucher kind must be decided here.
fn stock_effect(kind: BaseVoucherKind) -> StockEffect {
    match kind {
        BaseVoucherKind::Purchase
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::CreditNote => StockEffect::Receipt,
        BaseVoucherKind::Sales
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::DebitNote => StockEffect::Issue,
        BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder
        | BaseVoucherKind::Memorandum => StockEffect::NoMovement,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock => StockEffect::Undecided,
    }
}

/// The voucher's issues, per item, in base units.
fn voucher_issues(
    v: &Voucher,
    books: &Books,
) -> Result<BTreeMap<StockItemId, Vec<Step>>, StockError> {
    let mut issues: BTreeMap<StockItemId, Vec<Step>> = BTreeMap::new();
    for line in &v.inventory_lines {
        let item = books.masters.stock_items.get(&line.item).ok_or_else(|| {
            invalid(
                "inventory_lines",
                format!(
                    "voucher {:?} names stock item {:?}, which does not exist",
                    v.id, line.item
                ),
            )
        })?;
        if line.qty.0 < Decimal::ZERO {
            return Err(invalid("inventory_lines", "negative line quantity"));
        }
        // Compound units (1 box = 12 pcs) convert through the unit chain, as in every other
        // quantity conversion; an unrelated unit or an overflow is an error, never a guess.
        let base = crate::units::to_base_units(
            Qty(line.qty.0),
            line.unit,
            item,
            &books.masters,
        )?
        .0;
        issues.entry(item.id).or_default().push(Step {
            date: v.date,
            voucher: v.id,
            qty_in: Decimal::ZERO,
            qty_out: base,
        });
    }
    Ok(issues)
}

/// First negative date for one item once the voucher's steps replace its own movements.
fn first_negative_with_voucher(
    item: &StockItem,
    v: &Voucher,
    books: &Books,
    new_steps: &[Step],
) -> Result<Option<Date>, StockError> {
    let others: Vec<StockMove> = books
        .stock_moves
        .iter()
        .filter(|m| m.voucher != v.id)
        .copied()
        .collect();
    let mut steps = sorted_steps(item.id, &others, None);
    steps.extend_from_slice(new_steps);
    steps.sort_by_key(|s| (s.date, s.voucher));
    scan(opening_qty(item)?, &steps, Some(v.date))
}

/// Applies the company's negative stock policy to a voucher about to be saved.
///
/// See the module docs for what is checked.
///
/// # Errors
/// [`StockError::NegativeStockBlocked`] under [`NegativeStockPolicy::Block`];
/// [`StockError::Invalid`] when the voucher type or an item is unknown or the stock effect of the
/// voucher cannot be decided; [`StockError::Unit`] for an unconvertible unit;
/// [`StockError::Money`] on overflow.
pub fn check_policy(
    v: &Voucher,
    books: &Books,
    policy: NegativeStockPolicy,
) -> Result<Vec<StockWarning>, StockError> {
    match policy {
        NegativeStockPolicy::Allow => return Ok(Vec::new()),
        NegativeStockPolicy::Block | NegativeStockPolicy::Warn => {}
    }
    match v.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => {}
        VoucherStatus::Optional | VoucherStatus::Cancelled => return Ok(Vec::new()),
    }
    if v.inventory_lines.is_empty() {
        return Ok(Vec::new());
    }
    let kind = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| {
            invalid(
                "type_id",
                format!("voucher {:?} has an unknown voucher type", v.id),
            )
        })?
        .base;
    match stock_effect(kind) {
        StockEffect::Receipt | StockEffect::NoMovement => return Ok(Vec::new()),
        StockEffect::Issue => {}
        StockEffect::Undecided => {
            return Err(invalid(
                "inventory_lines",
                format!(
                    "voucher {:?} of kind {kind:?} carries inventory lines whose stock effect is not supported",
                    v.id
                ),
            ));
        }
    }
    let mut warnings = Vec::new();
    for (item_id, steps) in voucher_issues(v, books)? {
        let item = books.masters.stock_items.get(&item_id).ok_or_else(|| {
            invalid(
                "inventory_lines",
                format!("stock item {item_id:?} does not exist"),
            )
        })?;
        let Some(date) = first_negative_with_voucher(item, v, books, &steps)? else {
            continue;
        };
        match policy {
            NegativeStockPolicy::Block => {
                return Err(StockError::NegativeStockBlocked {
                    item: item_id,
                    date,
                });
            }
            NegativeStockPolicy::Warn => warnings.push(StockWarning {
                item: item_id,
                date,
                voucher: v.id,
            }),
            NegativeStockPolicy::Allow => {}
        }
    }
    Ok(warnings)
}
