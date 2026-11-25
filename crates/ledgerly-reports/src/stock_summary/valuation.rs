//! Value figures of a stock item and the cost of stock consumed (part of the stock summary,
//! card RP9). The periodic average cost and FIFO rules live here; the module docs of
//! [`super`] say which figure comes from where.

use ledgerly_core::{
    Decimal, Paise, Qty, RoundingRule, SignedAmount, Unsupported, round_rupees_to_paise,
};
use ledgerly_model::{Books, StockItem, ValuationMethod};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};

use super::{ItemFlow, Table, overflow};
use crate::stock_reason::negative_stock_reason;
use crate::{Cell, ReportError};

/// A value figure with its standing.
#[derive(Clone)]
pub(super) enum Val {
    Final(Paise),
    Provisional(Paise, String),
    Unsupported(Unsupported),
}

impl Val {
    pub(super) fn unsupported(what: String, reason: impl Into<String>) -> Val {
        Val::Unsupported(Unsupported {
            what,
            reason: reason.into(),
        })
    }

    fn from_valuation(v: &StockValuation, books: &Books) -> Val {
        match &v.status {
            ValuationStatus::Final => Val::Final(v.value),
            ValuationStatus::Provisional(ProvisionalReason::NegativeStock { item, date }) => {
                Val::Provisional(v.value, negative_stock_reason(books, *item, *date, v.as_of))
            }
        }
    }

    pub(super) fn cell(&self) -> Result<Cell, ReportError> {
        Ok(match self {
            Val::Final(p) => Cell::Amount(SignedAmount::from_signed(*p)?),
            Val::Provisional(p, reason) => Cell::Provisional {
                amount: SignedAmount::from_signed(*p)?,
                reason: reason.clone(),
            },
            Val::Unsupported(u) => Cell::Unsupported(u.clone()),
        })
    }
}

fn mismatch(item: &StockItem, which: &str, valued: Qty, computed: Decimal) -> Val {
    Val::unsupported(
        format!("stock summary: {which} value of {}", item.name),
        format!(
            "the valuation covers {} units but the movements give {computed}; value the same vouchers the report counts",
            valued.0
        ),
    )
}

fn missing(item: &StockItem, which: &str, why: &str) -> Val {
    Val::unsupported(
        format!("stock summary: {which} value of {}", item.name),
        why.to_owned(),
    )
}

pub(super) fn closing_value(item: &StockItem, qty_close: Decimal, table: &Table<'_>) -> Val {
    match table.closing.get(&item.id) {
        None => missing(
            item,
            "closing",
            "no closing valuation was supplied for this item",
        ),
        Some(v) if v.qty.0 != qty_close => mismatch(item, "closing", v.qty, qty_close),
        Some(v) => Val::from_valuation(v, table.books),
    }
}

/// The closing rate cell: the closing valuation's own rate, rounded half up to the paisa.
///
/// Nothing on hand has no rate (`Empty`). Stock on hand whose valuation gives no rate is
/// `Unsupported`, as is a rate when the closing value is. The rate is not itself marked
/// provisional; the closing value is.
pub(super) fn closing_rate(
    item: &StockItem,
    closing: &Val,
    qty_close: Decimal,
    table: &Table<'_>,
) -> Result<Cell, ReportError> {
    if let Val::Unsupported(u) = closing {
        return Ok(Cell::Unsupported(u.clone()));
    }
    if qty_close <= Decimal::ZERO {
        return Ok(Cell::Empty);
    }
    let rate = table.closing.get(&item.id).and_then(|v| v.rate);
    Ok(match rate {
        Some(r) => Cell::Amount(SignedAmount::from_signed(round_rupees_to_paise(
            r.0,
            RoundingRule::HalfUpToPaise,
        )?)?),
        None => Cell::Unsupported(Unsupported {
            what: format!("stock summary: closing rate of {}", item.name),
            reason: "the valuation gives no rate for the stock on hand".to_owned(),
        }),
    })
}

pub(super) fn opening_value(
    item: &StockItem,
    flow: &ItemFlow,
    qty_open: Decimal,
    table: &Table<'_>,
) -> Result<Val, ReportError> {
    if let Some(v) = table.opening.get(&item.id) {
        return Ok(if v.qty.0 == qty_open {
            Val::from_valuation(v, table.books)
        } else {
            mismatch(item, "opening", v.qty, qty_open)
        });
    }
    if flow.moves_before {
        return Ok(missing(
            item,
            "opening",
            "stock moved before the period; supply the valuation dated the day before it starts",
        ));
    }
    if qty_open.is_zero() {
        return Ok(Val::Final(Paise::ZERO));
    }
    match item.valuation {
        ValuationMethod::AverageCost | ValuationMethod::Fifo => {
            Ok(Val::Final(opening_stock_value(item)?))
        }
        ValuationMethod::LastPurchaseCost | ValuationMethod::StandardCost => Ok(missing(
            item,
            "opening",
            "this valuation method does not value opening stock at its entered rate; supply the valuation dated the day before the period starts",
        )),
    }
}

/// Opening stock at its entered rates: each entry rounded half away from zero to the paisa.
fn opening_stock_value(item: &StockItem) -> Result<Paise, ReportError> {
    let mut total = Paise::ZERO;
    for o in &item.opening {
        let rupees = o
            .qty
            .0
            .checked_mul(o.rate.0)
            .ok_or_else(|| overflow("opening value"))?;
        let value = round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)?;
        total = total.checked_add(value)?;
    }
    Ok(total)
}

/// An addend of a sum: the amount and, when provisional, why.
pub(super) fn addend(v: &Val) -> Result<(Paise, Option<&str>), &Unsupported> {
    match v {
        Val::Final(p) => Ok((*p, None)),
        Val::Provisional(p, reason) => Ok((*p, Some(reason.as_str()))),
        Val::Unsupported(u) => Err(u),
    }
}

/// Cost of stock consumed: opening + inwards - closing, for the methods where that is the
/// definition (module docs).
pub(super) fn outwards_value(
    item: &StockItem,
    opening: &Val,
    in_value: Paise,
    closing: &Val,
    qty_close: Decimal,
    transfer: bool,
) -> Result<Val, ReportError> {
    let what = format!("stock summary: outwards value of {}", item.name);
    match item.valuation {
        ValuationMethod::AverageCost | ValuationMethod::Fifo => {}
        ValuationMethod::LastPurchaseCost | ValuationMethod::StandardCost => {
            return Ok(Val::unsupported(
                what,
                "no verified rule values consumption under last purchase or standard cost",
            ));
        }
    }
    if qty_close < Decimal::ZERO {
        return Ok(Val::unsupported(
            what,
            "closing quantity is negative; consumption cannot be valued",
        ));
    }
    if transfer {
        return Ok(Val::unsupported(
            what,
            "a voucher in the period moves this item between godowns, so its inwards and outwards include the transfer and consumption cannot be told apart",
        ));
    }
    let (open, open_reason) = match addend(opening) {
        Ok(parts) => parts,
        Err(u) => {
            return Ok(Val::unsupported(
                what,
                format!("needs the opening value: {}", u.reason),
            ));
        }
    };
    let (close, close_reason) = match addend(closing) {
        Ok(parts) => parts,
        Err(u) => {
            return Ok(Val::unsupported(
                what,
                format!("needs the closing value: {}", u.reason),
            ));
        }
    };
    let value = open.checked_add(in_value)?.checked_sub(close)?;
    if value.is_negative() {
        return Ok(Val::unsupported(
            what,
            "opening value + inwards value is less than the closing value; the valuations are inconsistent with the movements",
        ));
    }
    let reasons: Vec<&str> = [open_reason, close_reason].into_iter().flatten().collect();
    Ok(if reasons.is_empty() {
        Val::Final(value)
    } else {
        Val::Provisional(value, reasons.join("; "))
    })
}
