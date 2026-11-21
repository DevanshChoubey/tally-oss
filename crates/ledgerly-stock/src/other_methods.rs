//! Last purchase cost and standard cost valuation; dispatch by method. **Card T5** implements this module.
//!
//! Frozen interface (plan card T5):
//!
//! ```text
//! pub fn last_purchase_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! pub fn standard_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! pub fn value_item(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
//!
//! # Closing quantity
//!
//! Both methods value the closing quantity at `period.to()`: the item's opening stock plus every
//! receipt minus every issue dated on or before `period.to()`, across godowns and batches.
//! Moves of other items and moves after the period end are ignored. `period.from()` plays no part
//! (the closing quantity is a running balance since books-begin). The moves need not be sorted.
//! Refused with [`StockError::Invalid`] (no number): a negative quantity or value, a move that
//! both receives and issues, a negative opening quantity or rate, and a negative closing quantity
//! (its valuation has no verified rule). A running quantity below zero earlier on, with a
//! non-negative closing quantity, gives [`ValuationStatus::Provisional`] with the first date it
//! went negative (the same rule as the average and FIFO methods).
//!
//! # Last purchase cost
//!
//! Closing quantity times the unit rate of the latest receipt dated on or before `period.to()`:
//! `value = closing quantity x receipt value / receipt quantity`, rounded once, half away from
//! zero, to the paisa; the reported rate is `receipt value / receipt quantity` at full precision
//! (`docs/research/03-inventory.md`, valuation worked examples, item 8: 40 x 15 = 600). The latest
//! receipt is the one with the greatest `(date, voucher)`.
//!
//! A [`StockMove`] carries no voucher kind, so every receipt counts as a purchase, including a
//! sales return or a godown-transfer receipt. Opening stock never supplies a rate: an item with
//! stock left but no receipt on or before the date is refused with [`StockError::Invalid`], and a
//! zero closing quantity is worth nothing without needing a rate.
//!
//! # Standard cost
//!
//! Closing quantity times the item's standard cost with the latest `from` on or before
//! `period.to()`, rounded half away from zero to the paisa. **No fallback**: when the item has no
//! standard cost dated on or before that date the result is [`StockError::NoStandardCost`], even
//! when the closing quantity is zero, and never the last purchase rate. Two standard costs
//! with the same `from` and different rates are ambiguous and refused with
//! [`StockError::Invalid`]; a negative standard rate is refused too.
//!
//! # Dispatch
//!
//! [`value_item`] picks the method from `item.valuation`, matching every variant.

use ledgerly_core::round_rupees_to_paise;
use ledgerly_core::{Date, Decimal, MoneyError, Paise, Period, Qty, Rate, RoundingRule};
use ledgerly_model::{StockItem, StockMove, ValuationMethod};

use crate::negative::{first_negative_with_opening, opening_qty};
use crate::{ProvisionalReason, StockError, StockValuation, ValuationStatus, avg, fifo};

fn invalid(field: &str, reason: impl Into<String>) -> StockError {
    StockError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn overflow() -> StockError {
    StockError::Money(MoneyError::Overflow)
}

/// Closing quantity at `as_of` and whether the history was ever negative up to then.
fn closing_position(
    item: &StockItem,
    moves: &[StockMove],
    as_of: Date,
) -> Result<(Decimal, ValuationStatus), StockError> {
    let mut qty = opening_qty(item)?;
    for o in &item.opening {
        if o.rate.0 < Decimal::ZERO {
            return Err(invalid("opening", "negative opening rate"));
        }
    }
    for m in moves
        .iter()
        .filter(|m| m.item == item.id && m.date <= as_of)
    {
        let (q_in, q_out) = (m.qty_in.0, m.qty_out.0);
        if q_in < Decimal::ZERO || q_out < Decimal::ZERO {
            return Err(invalid("moves", "negative movement quantity"));
        }
        if !q_in.is_zero() && !q_out.is_zero() {
            return Err(invalid("moves", "a movement cannot be both in and out"));
        }
        if !q_in.is_zero() && m.value.is_negative() {
            return Err(invalid("moves", "negative inward value"));
        }
        qty = qty
            .checked_add(q_in)
            .and_then(|r| r.checked_sub(q_out))
            .ok_or_else(overflow)?;
    }
    if qty < Decimal::ZERO {
        return Err(invalid(
            "qty",
            "closing quantity is negative; its valuation has no verified rule yet",
        ));
    }
    let status = match first_negative_with_opening(item, moves, as_of)? {
        Some(date) => ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: item.id,
            date,
        }),
        None => ValuationStatus::Final,
    };
    Ok((qty, status))
}

fn round(rupees: Decimal) -> Result<Paise, StockError> {
    Ok(round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)?)
}

/// Closing stock of `item` at `period.to()` at the rate of its last receipt (module docs).
///
/// # Errors
/// [`StockError::Invalid`] for the refused inputs in the module docs; [`StockError::Money`] on
/// overflow.
pub fn last_purchase_cost(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    let as_of = period.to();
    let (qty, status) = closing_position(item, moves, as_of)?;
    if qty.is_zero() {
        return Ok(StockValuation {
            item: item.id,
            as_of,
            qty: Qty(qty),
            value: Paise::new(0),
            rate: None,
            status,
        });
    }
    let last = moves
        .iter()
        .filter(|m| m.item == item.id && m.date <= as_of && m.qty_in.0 > Decimal::ZERO)
        .max_by_key(|m| (m.date, m.voucher))
        .ok_or_else(|| {
            invalid(
                "rate",
                format!(
                    "item {:?} has stock on {as_of} but no purchase on or before it; opening stock gives no last purchase rate",
                    item.id
                ),
            )
        })?;
    let receipt_rupees = last.value.to_rupees();
    let rate = receipt_rupees
        .checked_div(last.qty_in.0)
        .ok_or_else(overflow)?;
    let closing_rupees = receipt_rupees
        .checked_mul(qty)
        .and_then(|n| n.checked_div(last.qty_in.0))
        .ok_or_else(overflow)?;
    Ok(StockValuation {
        item: item.id,
        as_of,
        qty: Qty(qty),
        value: round(closing_rupees)?,
        rate: Some(Rate(rate)),
        status,
    })
}

/// The standard rate in force on `as_of`: the entry with the latest `from` on or before it.
fn standard_rate(item: &StockItem, as_of: Date) -> Result<Decimal, StockError> {
    let latest = item
        .standard_costs
        .iter()
        .filter(|s| s.from <= as_of)
        .map(|s| s.from)
        .max()
        .ok_or(StockError::NoStandardCost {
            item: item.id,
            date: as_of,
        })?;
    let mut rate: Option<Decimal> = None;
    for s in item.standard_costs.iter().filter(|s| s.from == latest) {
        if s.rate.0 < Decimal::ZERO {
            return Err(invalid("standard_costs", "negative standard cost"));
        }
        match rate {
            Some(r) if r != s.rate.0 => {
                return Err(invalid(
                    "standard_costs",
                    format!("two different standard costs are dated {latest}"),
                ));
            }
            Some(_) | None => rate = Some(s.rate.0),
        }
    }
    rate.ok_or(StockError::NoStandardCost {
        item: item.id,
        date: as_of,
    })
}

/// Closing stock of `item` at `period.to()` at its dated standard cost (module docs). There is no
/// fallback to another rate.
///
/// # Errors
/// [`StockError::NoStandardCost`] when no standard cost is dated on or before `period.to()`;
/// [`StockError::Invalid`] for the refused inputs in the module docs; [`StockError::Money`] on
/// overflow.
pub fn standard_cost(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    let as_of = period.to();
    let rate = standard_rate(item, as_of)?;
    let (qty, status) = closing_position(item, moves, as_of)?;
    let rupees = qty.checked_mul(rate).ok_or_else(overflow)?;
    Ok(StockValuation {
        item: item.id,
        as_of,
        qty: Qty(qty),
        value: round(rupees)?,
        rate: Some(Rate(rate)),
        status,
    })
}

/// Values `item` with the method set on the item.
///
/// # Errors
/// Whatever the chosen method returns.
pub fn value_item(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    match item.valuation {
        ValuationMethod::AverageCost => avg::average_cost(item, moves, period),
        ValuationMethod::Fifo => fifo::fifo(item, moves, period),
        ValuationMethod::LastPurchaseCost => last_purchase_cost(item, moves, period),
        ValuationMethod::StandardCost => standard_cost(item, moves, period),
    }
}
