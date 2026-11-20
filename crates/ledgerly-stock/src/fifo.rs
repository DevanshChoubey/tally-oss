//! FIFO (annual) valuation. **Card T4** implements this module.
//!
//! Frozen interface (plan card T4):
//!
//! ```text
//! pub fn fifo(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
//!
//! # Method (annual FIFO)
//!
//! Cost layers are built from the item's opening stock, then from every receipt (`qty_in`) dated
//! on or before the end of the period, oldest first. Every issue (`qty_out`) consumes the oldest
//! layer first (AS 2: cost of items sold is taken from the earliest layers). Closing stock is what
//! is left, each lot at its own cost.
//!
//! **Year-end collapse (the "annual" in annual FIFO).** At each financial-year boundary (1 April)
//! the remaining layers are merged into one layer worth the total closing value for the total
//! closing quantity, so the next year's opening rate is `closing value / closing quantity`
//! (`docs/research/03-inventory.md` section 4; the vendor behaviour is `unconfirmed` against a real
//! build, the rule here is the card's). Lot identity therefore never crosses a year boundary. This
//! is deliberately not FIFO-perpetual: with year-1 closing lots 10 @ 50 and 10 @ 60, year 2 opens
//! as 20 @ 55 and a year-2 sale of 10 costs 550, not 500.
//!
//! * The collapse keeps the exact total value in paise and the exact total quantity; the unit
//!   rate `value / qty` is never rounded to a paisa. Rounding happens only when part of a layer is
//!   issued (below), so the collapse itself creates or loses no paisa.
//! * Financial years are April to March (`FinancialYear::containing`). The books-begin date is
//!   not known here, so the opening stock is treated as the opening of the financial year of the
//!   first movement dated on or before the period end; it is first collapsed when a movement in a
//!   later financial year is reached.
//! * Moves are taken in slice order after a stable sort by date. The caller supplies them sorted by
//!   `(date, voucher)` (see `Books`), which fixes the order of same-day moves.
//! * Godowns and batches are not distinguished: FIFO runs per item.
//! * Rounding: a partly consumed layer gives up `value x qty_used / qty_left` paise, rounded half
//!   away from zero to the paisa; the layer keeps the exact remainder, so no paisa is created or
//!   lost across a layer's life.
//! * Negative stock: an issue larger than the layers leaves a shortfall. Later receipts first
//!   cover the shortfall (those units were issued at an unknown cost). The result is then
//!   `Provisional { NegativeStock }` with the first date the quantity went negative, and a
//!   negative closing quantity carries no value and no rate. The history is always replayed from
//!   the start, so that first date may fall in an earlier year than the period, and every later
//!   period stays Provisional (never silently final).
//! * Moves dated after the period end are dropped before validation, so a malformed later move is
//!   not reported by this period's valuation.
//!
//! Not supported, returned as [`StockError::Invalid`] instead of a number: an item whose valuation
//! method is not FIFO, a move of another item, negative quantities or values, a move that both
//! receives and issues, a movement with a value but no quantity, and a voucher that both receives
//! and issues the item (godown transfer, stock journal), whose cost basis FIFO cannot know here.
//! A dedicated `StockError::Unsupported` variant is requested in `docs/interface-requests.md`
//! (T4); until the wave reviewer adds it these cases use `Invalid`.

use std::collections::{BTreeSet, VecDeque};

use ledgerly_core::{
    Date, Decimal, FinancialYear, MoneyError, Paise, Period, Qty, Rate, RoundingRule, VoucherId,
    round_rupees_to_paise,
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

/// Running state of one item's FIFO replay.
struct Book {
    layers: VecDeque<Layer>,
    shortfall: Decimal,
    first_negative: Option<Date>,
    year: Option<FinancialYear>,
}

impl Book {
    /// Total quantity and value of the layers.
    fn totals(&self) -> Result<(Decimal, Paise), StockError> {
        let mut qty = Decimal::ZERO;
        let mut value = Paise::ZERO;
        for layer in &self.layers {
            qty = dec_add(qty, layer.qty)?;
            value = value.checked_add(layer.value)?;
        }
        Ok((qty, value))
    }

    /// Merges all layers into one at `value / qty` (year-end collapse of annual FIFO).
    fn collapse(&mut self) -> Result<(), StockError> {
        if self.layers.len() < 2 {
            return Ok(());
        }
        let (qty, value) = self.totals()?;
        self.layers.clear();
        self.layers.push_back(Layer { qty, value });
        Ok(())
    }

    /// Collapses the layers when `date` is in a later financial year than the previous move.
    fn enter_year(&mut self, date: Date) -> Result<(), StockError> {
        let fy = FinancialYear::containing(date)
            .map_err(|e| invalid("moves", format!("movement date {date}: {e}")))?;
        if self.year.is_some_and(|prev| fy > prev) {
            self.collapse()?;
        }
        self.year = Some(fy);
        Ok(())
    }

    /// Applies one validated movement.
    fn apply(&mut self, m: &StockMove) -> Result<(), StockError> {
        self.enter_year(m.date)?;
        if m.qty_out.0.is_zero() {
            self.layers.push_back(Layer {
                qty: m.qty_in.0,
                value: m.value,
            });
            if self.shortfall > Decimal::ZERO {
                let cover = self.shortfall.min(m.qty_in.0);
                consume(&mut self.layers, cover)?;
                self.shortfall = dec_sub(self.shortfall, cover)?;
            }
        } else {
            let unmet = consume(&mut self.layers, m.qty_out.0)?;
            if unmet > Decimal::ZERO {
                self.shortfall = dec_add(self.shortfall, unmet)?;
                self.first_negative.get_or_insert(m.date);
            }
        }
        Ok(())
    }
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
/// Uses every movement dated on or before `period.to()`, collapsing the layers to one at
/// `value / qty` at each financial-year boundary. See the module documentation for the collapse,
/// rounding and negative-stock rules.
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

    let mut book = Book {
        layers: VecDeque::new(),
        shortfall: Decimal::ZERO,
        first_negative: None,
        year: None,
    };
    for o in &item.opening {
        if let Some(layer) = opening_layer(o)? {
            book.layers.push_back(layer);
        }
    }
    for m in live {
        book.apply(m)?;
    }

    let (layer_qty, value) = book.totals()?;
    let qty = dec_sub(layer_qty, book.shortfall)?;

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
    let status = match book.first_negative {
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
