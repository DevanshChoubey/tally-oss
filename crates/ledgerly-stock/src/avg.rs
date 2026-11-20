//! Average cost (periodic) valuation. **Card T3** implements this module.
//!
//! Frozen interface (plan card T3):
//!
//! ```text
//! pub fn average_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
//!
//! # Definition (ours, documented; spec open question 1)
//!
//! Over the period, the pool is the item's opening stock plus every inward movement dated inside
//! the period. The average rate is `pool value / pool quantity`, kept at full `Decimal`
//! precision. Closing quantity is `pool quantity - outward quantity`; closing value is
//! `closing quantity x pool value / pool quantity`, rounded once, half away from zero, to the
//! paisa. Outward movements consume at the average and their own `value` is ignored. Moves of
//! other items and moves dated after `period.to()` are ignored. This is **not** a claim about
//! TallyPrime's output until observed.
//!
//! Refused with [`StockError::Invalid`] (no provenance for a number, so no number): moves dated
//! before `period.from()`, a negative closing quantity, a move that is both in and out, negative
//! quantities or values, and an item's moves not sorted by `(date, voucher)`.
//!
//! A running quantity below zero at any point, with a non-negative closing quantity, yields
//! [`ValuationStatus::Provisional`] with the first date it went negative.

use ledgerly_core::{Date, Decimal, Paise, Period, Qty, Rate, RoundingRule, round_rupees_to_paise};
use ledgerly_model::{StockItem, StockMove};

use crate::{ProvisionalReason, StockError, StockValuation, ValuationStatus};

/// The pool and running totals for one item over one period.
struct Pool {
    open_qty: Decimal,
    open_value: Paise,
    in_qty: Decimal,
    in_value: Paise,
    out_qty: Decimal,
    first_negative: Option<Date>,
}

impl Pool {
    fn pool_qty(&self) -> Result<Decimal, StockError> {
        checked(self.open_qty.checked_add(self.in_qty), "pool quantity")
    }

    fn pool_value(&self) -> Result<Paise, StockError> {
        Ok(self.open_value.checked_add(self.in_value)?)
    }

    fn closing_qty(&self) -> Result<Decimal, StockError> {
        checked(
            self.pool_qty()?.checked_sub(self.out_qty),
            "closing quantity",
        )
    }
}

fn invalid(field: &str, reason: impl Into<String>) -> StockError {
    StockError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn checked(v: Option<Decimal>, field: &str) -> Result<Decimal, StockError> {
    v.ok_or_else(|| invalid(field, "decimal overflow"))
}

fn build_pool(item: &StockItem, moves: &[StockMove], period: Period) -> Result<Pool, StockError> {
    let mut open_qty = Decimal::ZERO;
    let mut open_value = Paise::new(0);
    for o in &item.opening {
        if o.qty.0.is_sign_negative() || o.rate.0.is_sign_negative() {
            return Err(invalid("opening", "negative opening quantity or rate"));
        }
        open_qty = checked(open_qty.checked_add(o.qty.0), "opening quantity")?;
        let rupees = checked(o.qty.0.checked_mul(o.rate.0), "opening value")?;
        let v = round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)?;
        open_value = open_value.checked_add(v)?;
    }
    let mut pool = Pool {
        open_qty,
        open_value,
        in_qty: Decimal::ZERO,
        in_value: Paise::new(0),
        out_qty: Decimal::ZERO,
        first_negative: None,
    };
    let mut running = open_qty;
    let mut last: Option<(Date, ledgerly_core::VoucherId)> = None;
    for m in moves.iter().filter(|m| m.item == item.id) {
        if last.is_some_and(|l| l > (m.date, m.voucher)) {
            return Err(invalid(
                "moves",
                "stock moves must be sorted by (date, voucher)",
            ));
        }
        last = Some((m.date, m.voucher));
        if m.date > period.to() {
            continue;
        }
        if m.date < period.from() {
            return Err(invalid(
                "moves",
                "movements before the period start need a carried-in opening, which is not supported",
            ));
        }
        apply(&mut pool, &mut running, m)?;
    }
    Ok(pool)
}

fn apply(pool: &mut Pool, running: &mut Decimal, m: &StockMove) -> Result<(), StockError> {
    let (q_in, q_out) = (m.qty_in.0, m.qty_out.0);
    if q_in.is_sign_negative() || q_out.is_sign_negative() {
        return Err(invalid("moves", "negative movement quantity"));
    }
    if !q_in.is_zero() && !q_out.is_zero() {
        return Err(invalid("moves", "a movement cannot be both in and out"));
    }
    if !q_in.is_zero() {
        if m.value.is_negative() {
            return Err(invalid("moves", "negative inward value"));
        }
        pool.in_qty = checked(pool.in_qty.checked_add(q_in), "inward quantity")?;
        pool.in_value = pool.in_value.checked_add(m.value)?;
        *running = checked(running.checked_add(q_in), "running quantity")?;
    }
    if !q_out.is_zero() {
        pool.out_qty = checked(pool.out_qty.checked_add(q_out), "outward quantity")?;
        *running = checked(running.checked_sub(q_out), "running quantity")?;
        if running.is_sign_negative() && !running.is_zero() && pool.first_negative.is_none() {
            pool.first_negative = Some(m.date);
        }
    }
    Ok(())
}

/// Closing stock of `item` at `period.to()` under periodic average cost (definition in the
/// module docs).
///
/// # Errors
/// [`StockError::Money`] on overflow; [`StockError::Invalid`] for the refused inputs listed in
/// the module docs.
pub fn average_cost(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    let pool = build_pool(item, moves, period)?;
    let closing_qty = pool.closing_qty()?;
    if closing_qty.is_sign_negative() && !closing_qty.is_zero() {
        return Err(invalid(
            "qty",
            "closing quantity is negative; its valuation has no verified rule yet",
        ));
    }
    let pool_qty = pool.pool_qty()?;
    let (value, rate) = if pool_qty.is_zero() {
        (Paise::new(0), None)
    } else {
        let pool_rupees = pool.pool_value()?.to_rupees();
        let rate = checked(pool_rupees.checked_div(pool_qty), "average rate")?;
        let closing_rupees = checked(
            pool_rupees
                .checked_mul(closing_qty)
                .and_then(|n| n.checked_div(pool_qty)),
            "closing value",
        )?;
        (
            round_rupees_to_paise(closing_rupees, RoundingRule::HalfUpToPaise)?,
            Some(Rate(rate)),
        )
    };
    let status = match pool.first_negative {
        Some(date) => ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: item.id,
            date,
        }),
        None => ValuationStatus::Final,
    };
    Ok(StockValuation {
        item: item.id,
        as_of: period.to(),
        qty: Qty(closing_qty),
        value,
        rate,
        status,
    })
}

/// Cost of goods consumed in the period: opening value + inward value - closing value (R3,
/// "consumption"). Uses the same pool and rounding as [`average_cost`].
///
/// # Errors
/// As [`average_cost`].
pub fn average_consumption(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<Paise, StockError> {
    let pool = build_pool(item, moves, period)?;
    let closing = average_cost(item, moves, period)?;
    Ok(pool.pool_value()?.checked_sub(closing.value)?)
}
