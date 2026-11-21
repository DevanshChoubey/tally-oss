//! Card T1 tests: units and compound conversion (spec 6.4).
//!
//! Every expected value is `derived` by hand arithmetic from the unit definitions built in the
//! test (`1 first = factor x second`), never from engine output.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Decimal, Qty, StockItemId, UnitId};
use ledgerly_model::{AltUnit, Masters, StockItem, Unit, UnitKind, ValuationMethod};
use ledgerly_stock::StockError;
use ledgerly_stock::units::{sum_to_base_units, to_base_units};
use proptest::prelude::*;

const PCS: UnitId = UnitId(1);
/// "box of 12 pcs": 1 box = 12 pcs.
const BOX12: UnitId = UnitId(2);
const BOX: UnitId = UnitId(3);
/// "carton of 4 box12": 1 carton = 4 x BOX12.
const CARTON4: UnitId = UnitId(4);
const CARTON: UnitId = UnitId(5);
const DOZEN_ALT: UnitId = UnitId(6);

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn unit(id: UnitId, kind: UnitKind) -> Unit {
    Unit {
        id,
        symbol: format!("u{}", id.0),
        formal_name: format!("Unit {}", id.0),
        decimals: 0,
        kind,
        uqc: None,
        tally: None,
    }
}

fn masters() -> Masters {
    let mut m = Masters::default();
    for u in [
        unit(PCS, UnitKind::Simple),
        unit(BOX, UnitKind::Simple),
        unit(CARTON, UnitKind::Simple),
        unit(DOZEN_ALT, UnitKind::Simple),
        unit(
            BOX12,
            UnitKind::Compound {
                first: BOX,
                factor: d(12),
                second: PCS,
            },
        ),
        unit(
            CARTON4,
            UnitKind::Compound {
                first: CARTON,
                factor: d(4),
                second: BOX12,
            },
        ),
    ] {
        m.units.insert(u.id, u);
    }
    m
}

fn item(alt: Option<AltUnit>) -> StockItem {
    StockItem {
        id: StockItemId(1),
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: PCS,
        alt_unit: alt,
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        tally: None,
    }
}

fn is_unit_err(r: &Result<Qty, StockError>) -> bool {
    matches!(r, Err(StockError::Unit { .. }))
}

#[test]
fn base_unit_is_unchanged() {
    // provenance: derived — identity: a quantity already in the base unit is the base quantity.
    let r = to_base_units(Qty(d(7)), PCS, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(7)));
}

#[test]
fn one_box_is_twelve_pcs() {
    // provenance: derived — definition 1 box = 12 pcs; 1 x 12 = 12.
    let r = to_base_units(Qty(d(1)), BOX12, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(12)));
}

#[test]
fn two_box_five_pcs_is_twenty_nine_pcs() {
    // provenance: derived — 1 box = 12 pcs; 2 x 12 + 5 = 24 + 5 = 29.
    let parts = [(Qty(d(2)), BOX12), (Qty(d(5)), PCS)];
    let r = sum_to_base_units(&parts, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(29)));
}

#[test]
fn nested_compound_multiplies_through() {
    // provenance: derived — 1 carton = 4 box, 1 box = 12 pcs; 3 x 4 x 12 = 144.
    let r = to_base_units(Qty(d(3)), CARTON4, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(144)));
}

#[test]
fn fractional_compound_quantity_is_exact() {
    // provenance: derived — 2.5 box x 12 pcs/box = 30 pcs exactly.
    let r = to_base_units(Qty(Decimal::new(25, 1)), BOX12, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(30)));
}

#[test]
fn alternate_unit_uses_decimal_without_rounding() {
    // provenance: derived — 1 alt = 0.333333 base; 3 x 0.333333 = 0.999999 (no rounding to 1).
    let alt = AltUnit {
        unit: DOZEN_ALT,
        base_per_alt: Decimal::new(333_333, 6),
    };
    let r = to_base_units(Qty(d(3)), DOZEN_ALT, &item(Some(alt)), &masters()).unwrap();
    assert_eq!(r, Qty(Decimal::new(999_999, 6)));
}

#[test]
fn alternate_unit_with_fractional_ratio() {
    // provenance: derived — 1 alt = 2.5 base; 3 x 2.5 = 7.5.
    let alt = AltUnit {
        unit: DOZEN_ALT,
        base_per_alt: Decimal::new(25, 1),
    };
    let r = to_base_units(Qty(d(3)), DOZEN_ALT, &item(Some(alt)), &masters()).unwrap();
    assert_eq!(r, Qty(Decimal::new(75, 1)));
}

#[test]
fn base_route_wins_over_alternate_override() {
    // provenance: derived — CARTON4 reaches the base unit PCS through BOX12 (1 box = 12 pcs), so
    // the base route wins over the alternate-unit override: 2 x 4 x 12 = 96.
    let alt = AltUnit {
        unit: BOX12,
        base_per_alt: d(10),
    };
    let r = to_base_units(Qty(d(2)), CARTON4, &item(Some(alt)), &masters()).unwrap();
    assert_eq!(r, Qty(d(96)));
}

#[test]
fn negative_quantity_converts_sign_preserved() {
    // provenance: derived — -2 x 12 = -24.
    let r = to_base_units(Qty(d(-2)), BOX12, &item(None), &masters()).unwrap();
    assert_eq!(r, Qty(d(-24)));
}

#[test]
fn unrelated_unit_is_an_error_not_a_guess() {
    // provenance: derived — BOX is a simple unit that is neither base nor alternate.
    let r = to_base_units(Qty(d(1)), BOX, &item(None), &masters());
    assert!(is_unit_err(&r));
}

#[test]
fn unknown_unit_is_an_error() {
    // provenance: derived — UnitId(99) is not in the masters.
    let r = to_base_units(Qty(d(1)), UnitId(99), &item(None), &masters());
    assert!(is_unit_err(&r));
}

#[test]
fn zero_factor_is_an_error() {
    // provenance: derived — a compound factor of 0 is not a valid definition.
    let mut m = masters();
    m.units.insert(
        BOX12,
        unit(
            BOX12,
            UnitKind::Compound {
                first: BOX,
                factor: d(0),
                second: PCS,
            },
        ),
    );
    let r = to_base_units(Qty(d(1)), BOX12, &item(None), &m);
    assert!(is_unit_err(&r));
}

#[test]
fn non_positive_alternate_ratio_is_an_error() {
    // provenance: derived — 0 base units per alternate unit is not a valid definition.
    let alt = AltUnit {
        unit: DOZEN_ALT,
        base_per_alt: d(0),
    };
    let r = to_base_units(Qty(d(1)), DOZEN_ALT, &item(Some(alt)), &masters());
    assert!(is_unit_err(&r));
}

#[test]
fn compound_cycle_is_an_error_not_a_hang() {
    // provenance: derived — A = 2 x B and B = 2 x A never reaches PCS; the depth cap stops it.
    let a = UnitId(10);
    let b = UnitId(11);
    let mut m = masters();
    m.units.insert(
        a,
        unit(
            a,
            UnitKind::Compound {
                first: UnitId(20),
                factor: d(2),
                second: b,
            },
        ),
    );
    m.units.insert(
        b,
        unit(
            b,
            UnitKind::Compound {
                first: UnitId(21),
                factor: d(2),
                second: a,
            },
        ),
    );
    let r = to_base_units(Qty(d(1)), a, &item(None), &m);
    assert!(is_unit_err(&r));
}

#[test]
fn overflow_is_an_error() {
    // provenance: derived — Decimal::MAX x 12 exceeds the Decimal range.
    let r = to_base_units(Qty(Decimal::MAX), BOX12, &item(None), &masters());
    assert!(is_unit_err(&r));
}

proptest! {
    #[test]
    fn mixed_quantity_matches_integer_arithmetic(boxes in 0i64..100_000, pcs in 0i64..100_000) {
        // provenance: derived — boxes x 12 + pcs, computed on i64 in the test.
        let parts = [(Qty(d(boxes)), BOX12), (Qty(d(pcs)), PCS)];
        let r = sum_to_base_units(&parts, &item(None), &masters()).unwrap();
        prop_assert_eq!(r, Qty(d(boxes * 12 + pcs)));
    }
}
