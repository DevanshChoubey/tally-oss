//! FIFO (annual) valuation. **Card T4** implements this module.
//!
//! Frozen interface (plan card T4):
//!
//! ```text
//! pub fn fifo(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
//!
//! # Method
//!
//! Cost layers are built from the item's opening stock, then from every receipt (`qty_in`) dated
//! on or before the end of the period, oldest first. Every issue (`qty_out`) consumes the oldest
//! layer first (AS 2: cost of items sold is taken from the earliest layers). Closing stock is what
//! is left. A year's closing layers are exactly the next year's opening layers, because the
//! valuation is always rebuilt from the whole movement history up to the period end.
//!
//! * Moves are taken in slice order after a stable sort by date. The caller supplies them sorted by
//!   `(date, voucher)` (see `Books`), which fixes the order of same-day moves.
//! * Godowns and batches are not distinguished: FIFO runs per item.
//! * Rounding: a partly consumed layer gives up `value x qty_used / qty_left` paise, rounded half
//!   away from zero to the paisa; the layer keeps the exact remainder, so no paisa is created or
//!   lost across a layer's life.
//! * Negative stock: an issue larger than the layers leaves a shortfall. Later receipts first
//!   cover the shortfall (those units were issued at an unknown cost). The result is then
//!   `Provisional { NegativeStock }` with the first date the quantity went negative, and a
//!   negative closing quantity carries no value and no rate.
//!
//! Not supported, returned as [`StockError::Invalid`] instead of a number: an item whose valuation
//! method is not FIFO, a move of another item, negative quantities or values, a move that both
//! receives and issues, a movement with a value but no quantity, and a voucher that both receives
//! and issues the item (godown transfer, stock journal), whose cost basis FIFO cannot know here.

use std::collections::{BTreeSet, VecDeque};

use ledgerly_core::{
    Decimal, MoneyError, Paise, Period, Qty, Rate, RoundingRule, VoucherId, round_rupees_to_paise,
};
use ledgerly_model::{OpeningStock, StockItem, StockMove, ValuationMethod};

use crate::{ProvisionalReason, StockError, StockValuation, ValuationStatus};

/// An unconsumed cost layer.
#[derive(Debug, Clone, Copy)]
struct Layer {
    qty: Decimal,
    value: Paise,
}

fn invalid(field: &str, reason: impl Into<String>) -> StockError {
    StockError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn dec_add(a: Decimal, b: Decimal) -> Result<Decimal, StockError> {
    a.checked_add(b)
        .ok_or(StockError::Money(MoneyError::Overflow))
}

fn dec_sub(a: Decimal, b: Decimal) -> Result<Decimal, StockError> {
    a.checked_sub(b)
        .ok_or(StockError::Money(MoneyError::Overflow))
}

/// Paise given up when `used` of `left` units of a layer worth `value` are consumed.
fn partial_cost(value: Paise, used: Decimal, left: Decimal) -> Result<Paise, StockError> {
    let paise = Decimal::from(value.get())
        .checked_mul(used)
        .and_then(|n| n.checked_div(left))
        .ok_or(StockError::Money(MoneyError::Overflow))?;
    let rupees = paise
        .checked_div(Decimal::ONE_HUNDRED)
        .ok_or(StockError::Money(MoneyError::Overflow))?;
    Ok(round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)?)
}

/// Consumes `qty` from the oldest layers and returns the quantity no layer could supply.
fn consume(layers: &mut VecDeque<Layer>, qty: Decimal) -> Result<Decimal, StockError> {
    let mut remaining = qty;
    while remaining > Decimal::ZERO {
        let Some(front) = layers.front_mut() else {
            break;
        };
        if front.qty <= remaining {
            remaining = dec_sub(remaining, front.qty)?;
            layers.pop_front();
        } else {
            let cost = partial_cost(front.value, remaining, front.qty)?;
            front.value = front.value.checked_sub(cost)?;
            front.qty = dec_sub(front.qty, remaining)?;
            remaining = Decimal::ZERO;
        }
    }
    Ok(remaining)
}

fn opening_layer(o: &OpeningStock) -> Result<Option<Layer>, StockError> {
    if o.qty.0.is_sign_negative() || o.rate.0.is_sign_negative() {
        return Err(invalid(
            "opening",
            "opening quantity and rate must not be negative",
        ));
    }
    if o.qty.0.is_zero() {
        return Ok(None);
    }
    let rupees = o
        .qty
        .0
        .checked_mul(o.rate.0)
        .ok_or(StockError::Money(MoneyError::Overflow))?;
    let value = round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)?;
    Ok(Some(Layer {
        qty: o.qty.0,
        value,
    }))
}

/// Checks one move; returns `false` for an all-zero move that changes nothing.
fn validate(item: &StockItem, m: &StockMove) -> Result<bool, StockError> {
    if m.item != item.id {
        return Err(invalid(
            "moves",
            format!(
                "movement of item {:?} passed for item {:?}",
                m.item, item.id
            ),
        ));
    }
    if m.qty_in.0.is_sign_negative() || m.qty_out.0.is_sign_negative() {
        return Err(invalid("moves", "movement quantities must not be negative"));
    }
    let has_in = !m.qty_in.0.is_zero();
    let has_out = !m.qty_out.0.is_zero();
    if has_in && has_out {
        return Err(invalid("moves", "a movement cannot both receive and issue"));
    }
    if !has_in && !has_out {
        if m.value.is_zero() {
            return Ok(false);
        }
        return Err(invalid(
            "moves",
            "a movement with a value but no quantity is not supported by FIFO",
        ));
    }
    if has_in && m.value.is_negative() {
        return Err(invalid("moves", "a receipt must not have a negative value"));
    }
    Ok(true)
}

/// Rejects a voucher that both receives and issues the item.
fn reject_in_out_vouchers(moves: &[&StockMove]) -> Result<(), StockError> {
    let receiving: BTreeSet<VoucherId> = moves
        .iter()
        .filter(|m| !m.qty_in.0.is_zero())
        .map(|m| m.voucher)
        .collect();
    let clash = moves
        .iter()
        .any(|m| !m.qty_out.0.is_zero() && receiving.contains(&m.voucher));
    if clash {
        return Err(invalid(
            "moves",
            "a voucher that both receives and issues the item (transfer or stock journal) is not supported by FIFO",
        ));
    }
    Ok(())
}

/// Values the closing stock of `item` at the end of `period` by FIFO.
///
/// Uses every movement dated on or before `period.to()`, so a year's closing layers are the next
/// year's opening layers. See the module documentation for the rounding and negative-stock rules.
///
/// # Errors
/// [`StockError::Invalid`] for input FIFO cannot value (see the module documentation) and
/// [`StockError::Money`] on arithmetic overflow.
pub fn fifo(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    match item.valuation {
        ValuationMethod::Fifo => {}
        ValuationMethod::AverageCost
        | ValuationMethod::LastPurchaseCost
        | ValuationMethod::StandardCost => {
            return Err(invalid(
                "valuation",
                "item is not set to FIFO; refusing to value it by another method",
            ));
        }
    }

    let mut in_period: Vec<&StockMove> = moves.iter().filter(|m| m.date <= period.to()).collect();
    in_period.sort_by_key(|m| m.date);
    let mut live: Vec<&StockMove> = Vec::with_capacity(in_period.len());
    for m in in_period {
        if validate(item, m)? {
            live.push(m);
        }
    }
    reject_in_out_vouchers(&live)?;

    let mut layers: VecDeque<Layer> = VecDeque::new();
    for o in &item.opening {
        if let Some(layer) = opening_layer(o)? {
            layers.push_back(layer);
        }
    }

    let mut shortfall = Decimal::ZERO;
    let mut first_negative = None;
    for m in live {
        if m.qty_out.0.is_zero() {
            layers.push_back(Layer {
                qty: m.qty_in.0,
                value: m.value,
            });
            if shortfall > Decimal::ZERO {
                let cover = shortfall.min(m.qty_in.0);
                consume(&mut layers, cover)?;
                shortfall = dec_sub(shortfall, cover)?;
            }
        } else {
            let unmet = consume(&mut layers, m.qty_out.0)?;
            if unmet > Decimal::ZERO {
                shortfall = dec_add(shortfall, unmet)?;
                first_negative.get_or_insert(m.date);
            }
        }
    }

    let mut qty = Decimal::ZERO;
    let mut value = Paise::ZERO;
    for layer in &layers {
        qty = dec_add(qty, layer.qty)?;
        value = value.checked_add(layer.value)?;
    }
    let qty = dec_sub(qty, shortfall)?;

    let rate = if qty > Decimal::ZERO {
        Some(Rate(
            value
                .to_rupees()
                .checked_div(qty)
                .ok_or(StockError::Money(MoneyError::Overflow))?,
        ))
    } else {
        None
    };
    let status = match first_negative {
        Some(date) => ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: item.id,
            date,
        }),
        None => ValuationStatus::Final,
    };
    Ok(StockValuation {
        item: item.id,
        as_of: period.to(),
        qty: Qty(qty),
        value,
        rate,
        status,
    })
}
