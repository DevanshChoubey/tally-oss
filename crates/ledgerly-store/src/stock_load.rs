//! Stock movement projection (**card S5**, spec 6.4).
//!
//! Frozen interface:
//!
//! ```text
//! pub fn stock_moves(store: &SqliteStore, period: Option<Period>) -> Result<Vec<StockMove>, StoreError>;
//! ```
//!
//! Like postings (card S4), stock movements are derived from the vouchers that the event log
//! replays to, not read from the `stock_moves` projection table (nothing fills it in this wave).
//!
//! Rules, all derived from the model's doc comments and from the meaning of the voucher kinds:
//! - One [`StockMove`] per godown/batch allocation of each inventory line, in line order; the
//!   result is sorted by `(date, voucher)`.
//! - Direction comes from the voucher type's [`BaseVoucherKind`]: `Purchase`, `ReceiptNote` and
//!   `RejectionIn` bring goods in (`qty_in`); `Sales`, `DeliveryNote` and `RejectionOut` send
//!   goods out (`qty_out`). Orders (`SalesOrder`, `PurchaseOrder`) never move stock.
//! - Quantities are converted from the line's unit to the item's base unit: the base unit itself,
//!   the item's alternate unit (`base_per_alt`), or a compound unit whose `second` is the base
//!   unit (`factor`). Any other unit is an error, not a guess.
//! - `value` is the allocation amount ("value of the movement at voucher rate").
//! - Vouchers that cannot be projected without guessing make the call fail naming the voucher and
//!   the reason (rule A1: no silent default). These are: inventory lines on a credit or debit
//!   note (the direction depends on whether the note is a return or a price adjustment), on a
//!   stock journal or physical stock voucher (needs source and destination, or an absolute
//!   count, which `StockMove` cannot express), on a `Contra`, `Payment`, `Receipt`, `Journal`,
//!   `Memorandum` or `ReversingJournal` voucher, and any inventory line of an `Optional` or
//!   `PostDated` voucher (`StockMove` carries no status, so such a move could not be told apart
//!   from a real one). Cancelled vouchers have no inventory lines and give no moves.
//!
//! [`StockMove`]: ledgerly_model::StockMove
//! [`BaseVoucherKind`]: ledgerly_model::BaseVoucherKind

use std::collections::BTreeMap;

use ledgerly_core::{Decimal, Period, Qty, UnitId, VoucherId};
use ledgerly_model::{
    BaseVoucherKind, InventoryLine, Masters, StockItem, StockMove, UnitKind, Voucher, VoucherStatus,
};

use crate::load::load_books;
use crate::{SqliteStore, StoreError};

/// Stock movements of the company, optionally limited to vouchers dated inside `period`,
/// sorted by `(date, voucher)`.
///
/// This replays the event log through [`load_books`] and then calls [`moves_from_vouchers`]; it
/// does not read `Books::stock_moves` (which `load_books` may leave empty), so it never recurses
/// into a loader that calls it.
///
/// # Errors
/// [`StoreError`] on a read failure; [`StoreError::Encoding`] when a voucher cannot be projected
/// to stock movements (see the module documentation), naming the voucher and the reason.
pub fn stock_moves(
    store: &SqliteStore,
    period: Option<Period>,
) -> Result<Vec<StockMove>, StoreError> {
    let books = load_books(store, period)?;
    moves_from_vouchers(&books.masters, &books.vouchers)
}

/// The pure part of [`stock_moves`]: derives the movements of `vouchers` using `masters`.
///
/// # Errors
/// [`StoreError::Encoding`] when a voucher cannot be projected (see the module documentation).
pub fn moves_from_vouchers(
    masters: &Masters,
    vouchers: &BTreeMap<VoucherId, Voucher>,
) -> Result<Vec<StockMove>, StoreError> {
    let mut out = Vec::new();
    for voucher in vouchers.values() {
        voucher_moves(masters, voucher, &mut out)
            .map_err(|message| StoreError::Encoding(format!("stock moves: {message}")))?;
    }
    // Stable sort: moves of one voucher keep line and allocation order.
    out.sort_by_key(|m| (m.date, m.voucher));
    Ok(out)
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
    /// Inventory lines do not move stock (orders).
    NoMovement,
    /// Cannot be projected without guessing; the text says why.
    Unsupported(&'static str),
}

fn effect_of(kind: BaseVoucherKind) -> Effect {
    match kind {
        BaseVoucherKind::Purchase | BaseVoucherKind::ReceiptNote | BaseVoucherKind::RejectionIn => {
            Effect::Moves(Direction::In)
        }
        BaseVoucherKind::Sales | BaseVoucherKind::DeliveryNote | BaseVoucherKind::RejectionOut => {
            Effect::Moves(Direction::Out)
        }
        BaseVoucherKind::SalesOrder | BaseVoucherKind::PurchaseOrder => Effect::NoMovement,
        BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => Effect::Unsupported(
            "direction of a credit or debit note with items is not decided (return or price adjustment)",
        ),
        BaseVoucherKind::StockJournal => Effect::Unsupported(
            "stock journal needs a source and a destination per line, which an inventory line does not carry",
        ),
        BaseVoucherKind::PhysicalStock => {
            Effect::Unsupported("physical stock is an absolute count, not a movement")
        }
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal => {
            Effect::Unsupported("this voucher kind does not move stock")
        }
    }
}

/// Appends the movements of one voucher.
fn voucher_moves(
    masters: &Masters,
    voucher: &Voucher,
    out: &mut Vec<StockMove>,
) -> Result<(), String> {
    if voucher.inventory_lines.is_empty() {
        return Ok(());
    }
    let id = voucher.id;
    let kind = masters
        .voucher_types
        .get(&voucher.type_id)
        .ok_or_else(|| {
            format!(
                "voucher {id:?} has inventory lines but its voucher type {:?} does not exist",
                voucher.type_id
            )
        })?
        .base;
    let direction = match effect_of(kind) {
        Effect::Moves(direction) => direction,
        Effect::NoMovement => return Ok(()),
        Effect::Unsupported(reason) => {
            return Err(format!(
                "voucher {id:?} ({kind:?}) is unsupported: {reason}"
            ));
        }
    };
    match voucher.status {
        VoucherStatus::Regular => {}
        VoucherStatus::Optional | VoucherStatus::PostDated | VoucherStatus::Cancelled => {
            return Err(format!(
                "voucher {id:?} with status {:?} has inventory lines; a stock movement carries no \
                 status, so it is unsupported",
                voucher.status
            ));
        }
    }
    for (index, line) in voucher.inventory_lines.iter().enumerate() {
        line_moves(masters, voucher, direction, line, out).map_err(|message| {
            format!(
                "voucher {id:?} inventory line {}: {message}",
                index.saturating_add(1)
            )
        })?;
    }
    Ok(())
}

/// Appends one movement per allocation of `line`.
fn line_moves(
    masters: &Masters,
    voucher: &Voucher,
    direction: Direction,
    line: &InventoryLine,
    out: &mut Vec<StockMove>,
) -> Result<(), String> {
    let item = masters
        .stock_items
        .get(&line.item)
        .ok_or_else(|| format!("stock item {:?} does not exist", line.item))?;
    let factor = base_factor(masters, item, line.unit)?;
    if line.godowns.is_empty() {
        return Err("no godown allocation, so the stock location is unknown".to_owned());
    }
    let mut qty_sum = Decimal::ZERO;
    let mut amount_sum = ledgerly_core::Paise::new(0);
    let mut moves = Vec::with_capacity(line.godowns.len());
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
        let base_qty = Qty(allocation
            .qty
            .0
            .checked_mul(factor)
            .ok_or_else(|| "quantity overflow converting to the base unit".to_owned())?);
        let (qty_in, qty_out) = match direction {
            Direction::In => (base_qty, Qty::ZERO),
            Direction::Out => (Qty::ZERO, base_qty),
        };
        moves.push(StockMove {
            voucher: voucher.id,
            date: voucher.date,
            item: line.item,
            godown: allocation.godown,
            batch: allocation.batch,
            qty_in,
            qty_out,
            value: allocation.amount,
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
    out.extend(moves);
    Ok(())
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
            UnitKind::Compound { factor, second, .. } if *second == item.unit => *factor,
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
