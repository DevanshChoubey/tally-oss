//! Units and compound conversion. **Card T1** implements this module.
//!
//! Frozen interface (plan card T1):
//!
//! ```text
//! pub fn to_base_units(qty: Qty, unit: UnitId, item: &StockItem, masters: &Masters) -> Result<Qty, StockError>;
//! ```
//!
//! Conversion rules (spec 6.4, research R3 on compound and alternate units):
//!
//! - A compound unit `Compound { first, factor, second }` means `1 first = factor x second`. A
//!   quantity stated in the compound unit's own id counts `first`s, so "box of 12 pcs" with
//!   quantity 2 is 24 `second`s. Compound units may nest (the `second` may itself be compound).
//! - An item's alternate unit means `base_per_alt` base units make one alternate unit.
//! - A mixed quantity such as "2 box 5 pcs" is the sum of its parts, see [`sum_to_base_units`].
//! - Arithmetic is exact `Decimal`; nothing is rounded here, rounding to the unit's decimal places
//!   is for display only.

use ledgerly_core::{Decimal, Qty, UnitId};
use ledgerly_model::{Masters, StockItem, UnitKind};

use crate::StockError;

/// Longest chain of nested compound units followed before the chain is treated as a cycle.
const MAX_DEPTH: usize = 32;

fn unit_err(reason: impl Into<String>) -> StockError {
    StockError::Unit {
        reason: reason.into(),
    }
}

/// Multiplier that turns one `unit` into `target`, following compound units. `None` when `unit`
/// does not reach `target`.
fn chain_factor(
    unit: UnitId,
    target: UnitId,
    masters: &Masters,
    depth: usize,
) -> Result<Option<Decimal>, StockError> {
    if unit == target {
        return Ok(Some(Decimal::ONE));
    }
    if depth >= MAX_DEPTH {
        return Err(unit_err(format!(
            "compound units nest more than {MAX_DEPTH} levels deep or form a cycle at {unit:?}"
        )));
    }
    let Some(def) = masters.units.get(&unit) else {
        return Err(unit_err(format!("unit {unit:?} does not exist")));
    };
    match &def.kind {
        UnitKind::Simple => Ok(None),
        UnitKind::Compound {
            first,
            factor,
            second,
        } => {
            if *factor <= Decimal::ZERO {
                return Err(unit_err(format!(
                    "compound unit {unit:?} has a factor of {factor}; it must be positive"
                )));
            }
            if *first == target {
                return Ok(Some(Decimal::ONE));
            }
            match chain_factor(*second, target, masters, depth.saturating_add(1))? {
                Some(inner) => inner
                    .checked_mul(*factor)
                    .map(Some)
                    .ok_or_else(|| unit_err(format!("overflow converting unit {unit:?}"))),
                None => Ok(None),
            }
        }
    }
}

/// Convert `qty` expressed in `unit` into the item's base unit.
///
/// `unit` may be the item's base unit (returned unchanged), the item's alternate unit
/// (multiplied by `base_per_alt`), or a compound unit that reaches either of them.
///
/// # Errors
///
/// [`StockError::Unit`] when the unit is unknown, is unrelated to the item, has a non-positive
/// factor, nests too deeply, or the arithmetic overflows. Never returns a guessed number.
pub fn to_base_units(
    qty: Qty,
    unit: UnitId,
    item: &StockItem,
    masters: &Masters,
) -> Result<Qty, StockError> {
    let factor = if let Some(f) = chain_factor(unit, item.unit, masters, 0)? {
        f
    } else if let Some(alt) = item.alt_unit {
        if alt.base_per_alt <= Decimal::ZERO {
            return Err(unit_err(format!(
                "alternate unit {:?} of item {:?} has {} base units per alternate unit; it must be positive",
                alt.unit, item.id, alt.base_per_alt
            )));
        }
        match chain_factor(unit, alt.unit, masters, 0)? {
            Some(f) => f
                .checked_mul(alt.base_per_alt)
                .ok_or_else(|| unit_err(format!("overflow converting unit {unit:?}")))?,
            None => return Err(unrelated(unit, item)),
        }
    } else {
        return Err(unrelated(unit, item));
    };
    qty.0
        .checked_mul(factor)
        .map(Qty)
        .ok_or_else(|| unit_err(format!("overflow converting {} of unit {unit:?}", qty.0)))
}

fn unrelated(unit: UnitId, item: &StockItem) -> StockError {
    unit_err(format!(
        "unit {unit:?} is neither the base unit {:?}, the alternate unit, nor a compound of them for item {:?}",
        item.unit, item.id
    ))
}

/// Convert a mixed quantity such as "2 box 5 pcs" (parts `[(2, box), (5, pcs)]`) into base units
/// by converting every part and adding them.
///
/// # Errors
///
/// Any error from [`to_base_units`], or [`StockError::Unit`] on overflow of the sum.
pub fn sum_to_base_units(
    parts: &[(Qty, UnitId)],
    item: &StockItem,
    masters: &Masters,
) -> Result<Qty, StockError> {
    let mut total = Decimal::ZERO;
    for &(qty, unit) in parts {
        let base = to_base_units(qty, unit, item, masters)?;
        total = total
            .checked_add(base.0)
            .ok_or_else(|| unit_err("overflow adding the parts of a mixed quantity"))?;
    }
    Ok(Qty(total))
}
