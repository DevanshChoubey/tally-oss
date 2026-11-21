//! Card T3 tests: periodic average cost.
//!
//! Dataset (R3, docs/research/03-inventory.md, "Valuation worked examples"): opening 100 @ 10 =
//! 1,000; 10-Apr purchase 50 @ 13 = 650; 15-Apr sale 120; 20-May purchase 40 @ 15 = 600; 31-May
//! sale 30; closing 40. Provenance for every expected number below is `derived`: hand arithmetic
//! from the vendor's published formula "total inward cost / total inward quantity applied to
//! closing quantity" with opening stock included in the pool. Whether TallyPrime itself behaves
//! this way is NOT observed (spec open question 1); these tests pin our documented definition.
#![allow(clippy::unwrap_used, reason = "tests may unwrap")]

use ledgerly_core::{
    Date, Decimal, GodownId, Paise, Period, Qty, Rate, StockItemId, UnitId, VoucherId,
};
use ledgerly_model::{OpeningStock, StockItem, StockMove, ValuationMethod};
use ledgerly_stock::avg::{average_consumption, average_cost};
use ledgerly_stock::{ProvisionalReason, StockError, ValuationStatus};
use proptest::prelude::*;
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn item(opening: &[(i64, i64)]) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: opening
            .iter()
            .map(|&(q, r)| OpeningStock {
                godown: GodownId(1),
                batch: None,
                qty: Qty(d(q)),
                rate: Rate(d(r)),
            })
            .collect(),
        standard_costs: vec![],
        tally: None,
    }
}

fn mv(v: u64, on: Date, item: StockItemId, qty_in: i64, qty_out: i64, paise: i64) -> StockMove {
    StockMove {
        voucher: VoucherId(v),
        date: on,
        item,
        godown: GodownId(1),
        batch: None,
        qty_in: Qty(d(qty_in)),
        qty_out: Qty(d(qty_out)),
        value: Paise::new(paise),
        status: ledgerly_model::VoucherStatus::Regular,
    }
}

fn period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 05 - 31)).unwrap()
}

fn r3_moves() -> Vec<StockMove> {
    vec![
        mv(1, date!(2026 - 04 - 10), ITEM, 50, 0, 65_000),
        // sale value is at the sale rate and must not affect cost
        mv(2, date!(2026 - 04 - 15), ITEM, 0, 120, 240_000),
        mv(3, date!(2026 - 05 - 20), ITEM, 40, 0, 60_000),
        mv(4, date!(2026 - 05 - 31), ITEM, 0, 30, 45_000),
    ]
}

#[test]
fn closing_is_473_68() {
    // derived: pool qty = 100 + 50 + 40 = 190; pool value = 1000 + 650 + 600 = 2,250.00;
    // average rate = 2250 / 190 = 11.842105...; closing qty = 190 - 120 - 30 = 40;
    // closing value = 40 x 2250 / 190 = 90000 / 190 = 473.684... -> 473.68 (half-up to paise).
    let v = average_cost(&item(&[(100, 10)]), &r3_moves(), period()).unwrap();
    assert_eq!(v.item, ITEM);
    assert_eq!(v.as_of, date!(2026 - 05 - 31));
    assert_eq!(v.qty, Qty(d(40)));
    assert_eq!(v.value, Paise::new(47_368));
    assert_eq!(v.status, ValuationStatus::Final);
    // derived: rate in rupees at full precision = 2250 / 190.
    assert_eq!(v.rate, Some(Rate(d(2250) / d(190))));
}

#[test]
fn consumption_is_1776_32() {
    // derived (R3: consumption = opening + inwards - closing): 2,250.00 - 473.68 = 1,776.32.
    let c = average_consumption(&item(&[(100, 10)]), &r3_moves(), period()).unwrap();
    assert_eq!(c, Paise::new(177_632));
}

#[test]
fn other_items_and_later_moves_are_ignored() {
    // derived: the same dataset with noise that must not change 473.68: another item's purchase,
    // and a June purchase after the period end.
    let mut moves = r3_moves();
    moves.push(mv(
        5,
        date!(2026 - 05 - 25),
        StockItemId(2),
        1000,
        0,
        9_999_900,
    ));
    moves.push(mv(6, date!(2026 - 06 - 05), ITEM, 500, 0, 5_000_000));
    moves.sort_by_key(|m| (m.date, m.voucher));
    let v = average_cost(&item(&[(100, 10)]), &moves, period()).unwrap();
    assert_eq!(v.value, Paise::new(47_368));
    assert_eq!(v.qty, Qty(d(40)));
}

#[test]
fn no_opening_no_moves_is_zero_with_no_rate() {
    // derived: nothing in, nothing out: quantity 0, value 0, no rate to report.
    let v = average_cost(&item(&[]), &[], period()).unwrap();
    assert_eq!(v.qty, Qty::ZERO);
    assert_eq!(v.value, Paise::new(0));
    assert_eq!(v.rate, None);
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn half_paisa_rounds_away_from_zero() {
    // derived: purchase 2 units for Rs 0.05 (5 paise), sell 1. Pool rate = 0.05 / 2 = 0.025;
    // closing 1 x 0.025 = 0.025 Rs -> exact half paisa -> 0.03 (HalfUpToPaise, spec 6.4: round
    // only at posting; midpoint away from zero) = 3 paise.
    let moves = vec![
        mv(1, date!(2026 - 04 - 02), ITEM, 2, 0, 5),
        mv(2, date!(2026 - 04 - 03), ITEM, 0, 1, 0),
    ];
    let v = average_cost(&item(&[]), &moves, period()).unwrap();
    assert_eq!(v.value, Paise::new(3));
}

#[test]
fn rounds_down_below_half() {
    // derived: purchase 3 units for Rs 1.00, sell 1; closing 2 x 1.00 / 3 = 0.6666... Rs -> 0.67.
    let moves = vec![
        mv(1, date!(2026 - 04 - 02), ITEM, 3, 0, 100),
        mv(2, date!(2026 - 04 - 03), ITEM, 0, 1, 0),
    ];
    let v = average_cost(&item(&[]), &moves, period()).unwrap();
    assert_eq!(v.value, Paise::new(67));
}

#[test]
fn dip_below_zero_then_recovering_is_provisional() {
    // derived: opening 0; 5-Apr sell 10 (running qty -10, first negative date 5-Apr);
    // 10-Apr buy 30 for Rs 300.00. Closing qty = 30 - 10 = 20; pool = 300.00 / 30 = 10.00/unit;
    // closing value = 20 x 10 = 200.00. Spec 6.4: negative running quantity => Provisional.
    let moves = vec![
        mv(1, date!(2026 - 04 - 05), ITEM, 0, 10, 0),
        mv(2, date!(2026 - 04 - 10), ITEM, 30, 0, 30_000),
    ];
    let v = average_cost(&item(&[]), &moves, period()).unwrap();
    assert_eq!(v.qty, Qty(d(20)));
    assert_eq!(v.value, Paise::new(20_000));
    assert_eq!(
        v.status,
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: ITEM,
            date: date!(2026 - 04 - 05),
        })
    );
}

#[test]
fn closing_negative_is_an_error_not_a_number() {
    // pending: how a negative closing quantity is valued has no provenance (R3 open question 3),
    // so the function must refuse rather than invent a number.
    let moves = vec![mv(1, date!(2026 - 04 - 05), ITEM, 0, 10, 0)];
    let e = average_cost(&item(&[]), &moves, period()).unwrap_err();
    assert!(matches!(e, StockError::Invalid { .. }), "{e:?}");
}

#[test]
fn moves_before_period_start_are_refused() {
    // pending: carrying a prior period's closing in as opening is not derived; refuse.
    let moves = vec![mv(1, date!(2026 - 03 - 30), ITEM, 5, 0, 500)];
    let e = average_cost(&item(&[]), &moves, period()).unwrap_err();
    assert!(matches!(e, StockError::Invalid { .. }), "{e:?}");
}

#[test]
fn malformed_moves_are_refused() {
    // a move that is both in and out, a negative quantity, a negative inward value, and an
    // unsorted slice are input errors, never silently valued.
    let both = vec![mv(1, date!(2026 - 04 - 05), ITEM, 1, 1, 100)];
    let neg = vec![mv(1, date!(2026 - 04 - 05), ITEM, -1, 0, 100)];
    let negval = vec![mv(1, date!(2026 - 04 - 05), ITEM, 1, 0, -100)];
    let unsorted = vec![
        mv(2, date!(2026 - 04 - 06), ITEM, 1, 0, 100),
        mv(1, date!(2026 - 04 - 05), ITEM, 1, 0, 100),
    ];
    for bad in [both, neg, negval, unsorted] {
        let e = average_cost(&item(&[]), &bad, period()).unwrap_err();
        assert!(matches!(e, StockError::Invalid { .. }), "{e:?}");
    }
}

#[test]
fn overflow_is_an_error() {
    // derived: two inward values of i64::MAX paise cannot be summed.
    let moves = vec![
        mv(1, date!(2026 - 04 - 05), ITEM, 1, 0, i64::MAX),
        mv(2, date!(2026 - 04 - 06), ITEM, 1, 0, i64::MAX),
    ];
    assert!(matches!(
        average_cost(&item(&[]), &moves, period()).unwrap_err(),
        StockError::Money(_)
    ));
}

proptest! {
    // invariant (derived algebraically from the definition): consumption + closing value =
    // opening value + inward value, and closing qty = opening qty + in - out.
    #[test]
    fn consumption_plus_closing_equals_pool(
        open_q in 0i64..1000, open_r in 0i64..500,
        buys in proptest::collection::vec((1i64..100, 0i64..1_000_000), 0..6),
        sell_pct in 0i64..=100,
    ) {
        let it = item(&[(open_q, open_r)]);
        let mut moves = Vec::new();
        let mut total_q = open_q;
        let mut in_val: i64 = 0;
        for (i, (q, p)) in buys.iter().enumerate() {
            moves.push(mv(i as u64 + 1, date!(2026 - 04 - 10), ITEM, *q, 0, *p));
            total_q += q;
            in_val += p;
        }
        let sell = total_q * sell_pct / 100;
        moves.push(mv(100, date!(2026 - 04 - 20), ITEM, 0, sell, 0));
        let v = average_cost(&it, &moves, period()).unwrap();
        let c = average_consumption(&it, &moves, period()).unwrap();
        let pool = open_q * open_r * 100 + in_val;
        prop_assert_eq!(v.qty, Qty(d(total_q - sell)));
        prop_assert_eq!(v.value.get() + c.get(), pool);
        prop_assert_eq!(v.status, ValuationStatus::Final);
    }
}
