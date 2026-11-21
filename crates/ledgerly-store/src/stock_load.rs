//! Stock movement projection (**card S5**, spec 6.4), revised in round 3 to documented behaviour.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn stock_moves(store: &SqliteStore, period: Option<Period>) -> Result<Vec<StockMove>, StoreError>;
//! ```
//!
//! Round 3 added [`moves_from_vouchers`] (called by `load_books`) and the `status` field of
//! [`StockMove`]. `stock_moves` returns the carried movements, actual and optional; `load_books`
//! returns the actual ones only.
//!
//! Like postings (card S4), stock movements are derived from the vouchers that the event log
//! replays to, not read from the `stock_moves` projection table (nothing fills it in this wave).
//!
//! Rules. "Documented" means stated by a TallyPrime help page read on 2026-11-21 (URLs in the
//! tests and in the round-3 report). Anything a page does not state is refused, not guessed.
//! - Direction follows the voucher kind. Purchase, receipt note and rejection-in bring goods in;
//!   sales, delivery note and rejection-out send them out. A credit note with items is a sales
//!   return, which increases stock by the returned quantity (sales-return page). Orders never move
//!   stock.
//! - Actual stock is the `Regular` movements only. An `Optional` voucher is carried with status
//!   `Optional` when its direction is known, and never counts as actual stock (the optional-voucher
//!   page says it does not affect the books; it is silent on stock). Memorandum and reversing
//!   journal vouchers are neither carried nor counted: their direction is not decided.
//! - Physical stock (regular): the count becomes the book quantity. The book quantity at the voucher
//!   date is the item's opening stock plus the regular movements dated before it, per item, godown
//!   and batch (physical-stock page). A count equal to the book quantity gives no movement. A
//!   different count would give an adjustment whose value the page does not state, so it is
//!   refused. A count sharing item and godown with any other movement on the same date is refused,
//!   because the page does not say which is applied first. An optional physical-stock voucher gives
//!   no movement.
//! - Typed `StoreError::Unsupported` (the voucher is valid input whose effect is not handled): a
//!   debit note with items; a stock journal (an inventory line records no source or destination
//!   side); a non-stock kind with items; a post-dated voucher with inventory lines; a cancelled
//!   voucher with inventory lines; a physical count that differs from the book quantity; a physical
//!   count that shares a key with another movement on the same date.
//! - `StoreError::Encoding` (stored data does not fit the model): an unknown item, godown, batch,
//!   unit or voucher type; allocation quantities or amounts that do not add up to the line; a unit
//!   that cannot be converted to the item's base unit.

use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, Date, Decimal, GodownId, Paise, Period, Qty, StockItemId, UnitId, VoucherId,
};
use ledgerly_model::{
    BaseVoucherKind, InventoryLine, Masters, StockItem, StockMove, UnitKind, Voucher, VoucherStatus,
};

use crate::load::{in_period, load_all};
use crate::{SqliteStore, StoreError};

/// Carried stock movements of the company (actual and optional), limited to movements dated
/// inside `period` when given, sorted by `(date, voucher)`.
///
/// `load_books(period)?.stock_moves` is the subset with status [`VoucherStatus::Regular`].
///
/// # Errors
/// [`StoreError`] on a read failure; [`StoreError::Unsupported`] for a voucher whose stock effect is
/// not handled; [`StoreError::Encoding`] for stored data that does not fit the model (see the module
/// documentation).
pub fn stock_moves(
    store: &SqliteStore,
    period: Option<Period>,
) -> Result<Vec<StockMove>, StoreError> {
    let (_company, replay) = load_all(store)?;
    let moves = moves_from_vouchers(&replay.masters, &replay.vouchers)?;
    Ok(moves
        .into_iter()
        .filter(|m| in_period(m.date, period))
        .collect())
}

/// The pure part of [`stock_moves`]: the carried movements of `vouchers`, sorted by
/// `(date, voucher)`, after checking every physical count against the book quantity.
///
/// Every voucher is needed, not only those in a period, because a book quantity depends on earlier
/// movements.
///
/// # Errors
/// [`StoreError::Unsupported`] and [`StoreError::Encoding`] as in the module documentation.
pub fn moves_from_vouchers(
    masters: &Masters,
    vouchers: &BTreeMap<VoucherId, Voucher>,
) -> Result<Vec<StockMove>, StoreError> {
    let mut carried = Vec::new();
    let mut counts = Vec::new();
    for voucher in vouchers.values() {
        match project(masters, voucher) {
            Ok(Plan::Nothing) => {}
            Ok(Plan::Moves(moves)) => carried.extend(moves),
            Ok(Plan::Count(count)) => counts.push(count),
            Err(Reject::Unsupported(reason)) => {
                return Err(StoreError::Unsupported {
                    voucher: voucher.id,
                    reason,
                });
            }
            Err(Reject::Invalid(message)) => {
                return Err(StoreError::Encoding(format!("stock moves: {message}")));
            }
        }
    }
    // Stable sort: moves of one voucher keep line and allocation order.
    carried.sort_by_key(|m: &StockMove| (m.date, m.voucher));
    check_counts(masters, &carried, counts)?;
    Ok(carried)
}

/// Which way a voucher kind moves goods.
#[derive(Clone, Copy)]
enum Direction {
    In,
    Out,
}

/// What inventory lines mean on a voucher of this kind.
enum Effect {
    /// Goods move in the given direction.
    Moves(Direction),
    /// The lines are a physical count of the stock (regular vouchers only).
    Physical,
    /// Inventory lines do not move stock (orders).
    NoMovement,
    /// Out of the books and actual stock, and not carried: the direction is not decided.
    Outside,
    /// Cannot be projected; the text says why.
    Unsupported(&'static str),
}

fn effect_of(kind: BaseVoucherKind) -> Effect {
    match kind {
        BaseVoucherKind::Purchase
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::CreditNote => Effect::Moves(Direction::In),
        BaseVoucherKind::Sales | BaseVoucherKind::DeliveryNote | BaseVoucherKind::RejectionOut => {
            Effect::Moves(Direction::Out)
        }
        BaseVoucherKind::SalesOrder | BaseVoucherKind::PurchaseOrder => Effect::NoMovement,
        BaseVoucherKind::PhysicalStock => Effect::Physical,
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => Effect::Outside,
        BaseVoucherKind::DebitNote => Effect::Unsupported(
            "a debit note with items: its stock effect (purchase return or price adjustment) is not documented",
        ),
        BaseVoucherKind::StockJournal => Effect::Unsupported(
            "a stock journal needs a source and a destination side for each line, and an inventory line records neither",
        ),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal => Effect::Unsupported("this voucher kind does not move stock"),
    }
}

/// What one voucher contributes.
enum Plan {
    /// Nothing to carry.
    Nothing,
    /// Movements, each with the voucher's status.
    Moves(Vec<StockMove>),
    /// A physical count, checked against the book quantity by [`check_counts`].
    Count(PhysicalCount),
}

/// Why a voucher cannot be projected.
enum Reject {
    /// Valid input whose effect is not handled; becomes [`StoreError::Unsupported`].
    Unsupported(String),
    /// Stored data that does not fit the model; becomes [`StoreError::Encoding`].
    Invalid(String),
}

/// One physical count (a regular physical-stock voucher).
struct PhysicalCount {
    voucher: VoucherId,
    date: Date,
    lines: Vec<CountLine>,
}

/// One counted allocation, in base units.
struct CountLine {
    item: StockItemId,
    godown: GodownId,
    batch: Option<BatchId>,
    count: Decimal,
}

/// Book quantity key: item, godown and batch.
type BookKey = (StockItemId, GodownId, Option<BatchId>);

fn project(masters: &Masters, voucher: &Voucher) -> Result<Plan, Reject> {
    if voucher.inventory_lines.is_empty() {
        return Ok(Plan::Nothing);
    }
    let id = voucher.id;
    let kind = masters
        .voucher_types
        .get(&voucher.type_id)
        .ok_or_else(|| {
            Reject::Invalid(format!(
                "voucher {id:?} has inventory lines but its voucher type {:?} does not exist",
                voucher.type_id
            ))
        })?
        .base;
    let effect = effect_of(kind);
    if matches!(effect, Effect::NoMovement) {
        return Ok(Plan::Nothing);
    }
    match voucher.status {
        VoucherStatus::Cancelled => {
            return Err(Reject::Unsupported(
                "the stock effect of a cancelled voucher is not documented".to_owned(),
            ));
        }
        VoucherStatus::PostDated => {
            return Err(Reject::Unsupported(
                "the stock effect of a post-dated voucher is not documented".to_owned(),
            ));
        }
        VoucherStatus::Optional => {
            // Carried only when its direction is known; never an error, and never actual stock.
            return match effect {
                Effect::Moves(direction) => Ok(Plan::Moves(moves_of(masters, voucher, direction)?)),
                Effect::Physical
                | Effect::NoMovement
                | Effect::Outside
                | Effect::Unsupported(_) => Ok(Plan::Nothing),
            };
        }
        VoucherStatus::Regular => {}
    }
    match effect {
        Effect::Moves(direction) => Ok(Plan::Moves(moves_of(masters, voucher, direction)?)),
        Effect::Physical => Ok(Plan::Count(count_of(masters, voucher)?)),
        Effect::NoMovement | Effect::Outside => Ok(Plan::Nothing),
        Effect::Unsupported(reason) => Err(Reject::Unsupported(reason.to_owned())),
    }
}

/// Movements of a voucher whose lines move goods in `direction`, one per allocation.
fn moves_of(
    masters: &Masters,
    voucher: &Voucher,
    direction: Direction,
) -> Result<Vec<StockMove>, Reject> {
    let mut out = Vec::new();
    for (index, line) in voucher.inventory_lines.iter().enumerate() {
        for alloc in allocations(masters, line).map_err(|m| line_error(voucher, index, &m))? {
            let (qty_in, qty_out) = match direction {
                Direction::In => (Qty(alloc.qty), Qty::ZERO),
                Direction::Out => (Qty::ZERO, Qty(alloc.qty)),
            };
            out.push(StockMove {
                voucher: voucher.id,
                date: voucher.date,
                item: alloc.item,
                godown: alloc.godown,
                batch: alloc.batch,
                qty_in,
                qty_out,
                value: alloc.amount,
                status: voucher.status,
            });
        }
    }
    Ok(out)
}

/// The counted allocations of a physical-stock voucher.
fn count_of(masters: &Masters, voucher: &Voucher) -> Result<PhysicalCount, Reject> {
    let mut lines = Vec::new();
    for (index, line) in voucher.inventory_lines.iter().enumerate() {
        for alloc in allocations(masters, line).map_err(|m| line_error(voucher, index, &m))? {
            lines.push(CountLine {
                item: alloc.item,
                godown: alloc.godown,
                batch: alloc.batch,
                count: alloc.qty,
            });
        }
    }
    Ok(PhysicalCount {
        voucher: voucher.id,
        date: voucher.date,
        lines,
    })
}

fn line_error(voucher: &Voucher, index: usize, message: &str) -> Reject {
    Reject::Invalid(format!(
        "voucher {:?} inventory line {}: {message}",
        voucher.id,
        index.saturating_add(1)
    ))
}

/// Checks every physical count against the book quantity at its date.
///
/// The book quantity is opening stock plus the regular movements dated before the count, per item,
/// godown and batch. Each movement and each count line is counted per (date, item, godown); a count
/// whose key is counted more than once that day is refused.
fn check_counts(
    masters: &Masters,
    carried: &[StockMove],
    mut counts: Vec<PhysicalCount>,
) -> Result<(), StoreError> {
    counts.sort_by_key(|c| (c.date, c.voucher));
    let mut per_day: BTreeMap<(Date, StockItemId, GodownId), usize> = BTreeMap::new();
    for m in carried {
        bump(&mut per_day, (m.date, m.item, m.godown));
    }
    for count in &counts {
        for line in &count.lines {
            bump(&mut per_day, (count.date, line.item, line.godown));
        }
    }

    // Opening stock is the book quantity at books-begin.
    let mut book: BTreeMap<BookKey, Decimal> = BTreeMap::new();
    for item in masters.stock_items.values() {
        for opening in &item.opening {
            add_to(
                &mut book,
                (item.id, opening.godown, opening.batch),
                opening.qty.0,
            )?;
        }
    }

    // Only regular movements change the book quantity. `carried` is sorted by date.
    let regular: Vec<&StockMove> = carried
        .iter()
        .filter(|m| m.status == VoucherStatus::Regular)
        .collect();
    let mut next = 0_usize;
    for count in &counts {
        while let Some(m) = regular.get(next) {
            if m.date >= count.date {
                break;
            }
            let delta = m.qty_in.0.checked_sub(m.qty_out.0).ok_or_else(overflow)?;
            add_to(&mut book, (m.item, m.godown, m.batch), delta)?;
            next = next.saturating_add(1);
        }
        for line in &count.lines {
            let shared = per_day
                .get(&(count.date, line.item, line.godown))
                .copied()
                .unwrap_or(0);
            if shared > 1 {
                return Err(StoreError::Unsupported {
                    voucher: count.voucher,
                    reason: "the count shares item and godown with another movement on the same date, and the page does not say which is applied first".to_owned(),
                });
            }
            let book_qty = book
                .get(&(line.item, line.godown, line.batch))
                .copied()
                .unwrap_or(Decimal::ZERO);
            if line.count != book_qty {
                return Err(StoreError::Unsupported {
                    voucher: count.voucher,
                    reason: format!(
                        "the physical count {} differs from the book quantity {}; the value of an adjustment is not documented",
                        line.count, book_qty
                    ),
                });
            }
        }
    }
    Ok(())
}

fn bump<K: Ord>(counter: &mut BTreeMap<K, usize>, key: K) {
    let slot = counter.entry(key).or_insert(0);
    *slot = slot.saturating_add(1);
}

fn add_to(
    book: &mut BTreeMap<BookKey, Decimal>,
    key: BookKey,
    delta: Decimal,
) -> Result<(), StoreError> {
    let slot = book.entry(key).or_insert(Decimal::ZERO);
    *slot = slot.checked_add(delta).ok_or_else(overflow)?;
    Ok(())
}

fn overflow() -> StoreError {
    StoreError::Encoding("stock moves: quantity overflow".to_owned())
}

/// One allocation of an inventory line, with its quantity in base units.
struct Alloc {
    item: StockItemId,
    godown: GodownId,
    batch: Option<BatchId>,
    qty: Decimal,
    amount: Paise,
}

/// Validates the allocations of `line` and converts their quantities to the base unit.
fn allocations(masters: &Masters, line: &InventoryLine) -> Result<Vec<Alloc>, String> {
    let item = masters
        .stock_items
        .get(&line.item)
        .ok_or_else(|| format!("stock item {:?} does not exist", line.item))?;
    let factor = base_factor(masters, item, line.unit)?;
    if line.godowns.is_empty() {
        return Err("no godown allocation, so the stock location is unknown".to_owned());
    }
    let mut qty_sum = Decimal::ZERO;
    let mut amount_sum = Paise::new(0);
    let mut out = Vec::with_capacity(line.godowns.len());
    for allocation in &line.godowns {
        if allocation.qty.0.is_sign_negative() || allocation.amount.is_negative() {
            return Err("an allocation has a negative quantity or amount".to_owned());
        }
        if !masters.godowns.contains_key(&allocation.godown) {
            return Err(format!("godown {:?} does not exist", allocation.godown));
        }
        if let Some(batch) = allocation
            .batch
            .filter(|batch| !masters.batches.contains_key(batch))
        {
            return Err(format!("batch {batch:?} does not exist"));
        }
        qty_sum = qty_sum
            .checked_add(allocation.qty.0)
            .ok_or_else(|| "quantity overflow".to_owned())?;
        amount_sum = amount_sum
            .checked_add(allocation.amount)
            .map_err(|e| format!("amount: {e}"))?;
        let qty = allocation
            .qty
            .0
            .checked_mul(factor)
            .ok_or_else(|| "quantity overflow converting to the base unit".to_owned())?;
        out.push(Alloc {
            item: line.item,
            godown: allocation.godown,
            batch: allocation.batch,
            qty,
            amount: allocation.amount,
        });
    }
    if qty_sum != line.qty.0 {
        return Err(format!(
            "allocation quantities sum to {qty_sum} but the line quantity is {}",
            line.qty.0
        ));
    }
    if amount_sum != line.amount {
        return Err(format!(
            "allocation amounts sum to {} paise but the line amount is {} paise",
            amount_sum.get(),
            line.amount.get()
        ));
    }
    Ok(out)
}

/// Base units in one `unit` of `item`.
fn base_factor(masters: &Masters, item: &StockItem, unit: UnitId) -> Result<Decimal, String> {
    let factor = if unit == item.unit {
        Decimal::ONE
    } else if let Some(alt) = item.alt_unit.filter(|alt| alt.unit == unit) {
        alt.base_per_alt
    } else {
        let master = masters
            .units
            .get(&unit)
            .ok_or_else(|| format!("unit {unit:?} does not exist"))?;
        match &master.kind {
            UnitKind::Compound {
                first,
                factor,
                second,
            } if *first == unit && *second == item.unit => *factor,
            UnitKind::Compound { .. } | UnitKind::Simple => {
                return Err(format!(
                    "unit {unit:?} is neither the base unit {:?} of item {:?}, its alternate unit, \
                     nor a compound unit of the base unit, so the quantity cannot be converted",
                    item.unit, item.id
                ));
            }
        }
    };
    if factor <= Decimal::ZERO {
        return Err(format!(
            "unit {unit:?} has a conversion factor that is not positive"
        ));
    }
    Ok(factor)
}
