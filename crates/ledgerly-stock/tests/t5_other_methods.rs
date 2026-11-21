//! Card T5 tests: last purchase cost, standard cost, dispatch by method.
//!
//! Dataset (R3, docs/research/03-inventory.md, "Valuation worked examples"): opening 100 @ 10;
//! 10-Apr purchase 50 @ 13 = 650; 15-Apr sale 120; 20-May purchase 40 @ 15 = 600; 31-May sale 30;
//! closing quantity 100 + 50 - 120 + 40 - 30 = 40. Provenance for every expected number below is
//! `derived`: R3 item 8 (last purchase: closing qty x rate of the latest purchase on or before the
//! as-of date, 40 x 15 = 600) and item 9 (standard cost: closing qty x the standard rate in force,
//! e.g. 40 x 12 = 480), plus the hand arithmetic written in each test. Whether TallyPrime behaves
//! this way is not observed.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{
    Date, Decimal, GodownId, Paise, Period, Qty, Rate, StockItemId, UnitId, VoucherId,
};
use ledgerly_model::{OpeningStock, StandardCost, StockItem, StockMove, ValuationMethod};
use ledgerly_stock::other_methods::{last_purchase_cost, standard_cost, value_item};
use ledgerly_stock::{ProvisionalReason, StockError, ValuationStatus};
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn item(method: ValuationMethod, opening: &[(i64, i64)], stds: &[(Date, i64)]) -> StockItem {
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
        valuation: method,
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
        standard_costs: stds
            .iter()
            .map(|&(from, r)| StandardCost {
                from,
                rate: Rate(d(r)),
            })
            .collect(),
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
    }
}

fn period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 05 - 31)).unwrap()
}

fn r3_moves() -> Vec<StockMove> {
    vec![
        mv(1, date!(2026 - 04 - 10), ITEM, 50, 0, 65_000),
        // sale values are at the sale rate and must not affect cost
        mv(2, date!(2026 - 04 - 15), ITEM, 0, 120, 240_000),
        mv(3, date!(2026 - 05 - 20), ITEM, 40, 0, 60_000),
        mv(4, date!(2026 - 05 - 31), ITEM, 0, 30, 45_000),
    ]
}

#[test]
fn last_purchase_is_600_00() {
    // derived (R3 item 8): closing qty = 100 + 50 - 120 + 40 - 30 = 40; the latest purchase on or
    // before 31-May is 20-May, 40 units for 600.00, rate 600.00 / 40 = 15.00;
    // closing value = 40 x 15 = 600.00 = 60,000 paise.
    let it = item(ValuationMethod::LastPurchaseCost, &[(100, 10)], &[]);
    let v = last_purchase_cost(&it, &r3_moves(), period()).unwrap();
    assert_eq!(v.item, ITEM);
    assert_eq!(v.as_of, date!(2026 - 05 - 31));
    assert_eq!(v.qty, Qty(d(40)));
    assert_eq!(v.value, Paise::new(60_000));
    assert_eq!(v.rate, Some(Rate(d(15))));
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn last_purchase_as_of_earlier_date_uses_the_earlier_purchase() {
    // derived: as of 30-Apr the latest purchase is 10-Apr, 50 units for 650.00, rate 13.00.
    // closing qty = 100 + 50 - 120 = 30; value = 30 x 13 = 390.00 = 39,000 paise.
    let it = item(ValuationMethod::LastPurchaseCost, &[(100, 10)], &[]);
    let p = Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap();
    let v = last_purchase_cost(&it, &r3_moves(), p).unwrap();
    assert_eq!(v.qty, Qty(d(30)));
    assert_eq!(v.value, Paise::new(39_000));
    assert_eq!(v.rate, Some(Rate(d(13))));
}

#[test]
fn last_purchase_ignores_later_purchases_and_other_items() {
    // derived: a June purchase is after the period end and another item's purchase is not this
    // item's; neither changes 40 x 15 = 600.00.
    let it = item(ValuationMethod::LastPurchaseCost, &[(100, 10)], &[]);
    let mut moves = r3_moves();
    moves.push(mv(5, date!(2026 - 05 - 25), StockItemId(2), 10, 0, 999_900));
    moves.push(mv(6, date!(2026 - 06 - 05), ITEM, 500, 0, 5_000_000));
    let v = last_purchase_cost(&it, &moves, period()).unwrap();
    assert_eq!(v.qty, Qty(d(40)));
    assert_eq!(v.value, Paise::new(60_000));
}

#[test]
fn last_purchase_fractional_rate_rounds_once_at_the_end() {
    // derived: no opening; purchase 3 units for 100.00 (rate 33.333...); sale of 1. Closing qty
    // = 2; value = 2 x 100.00 / 3 = 66.666... -> 66.67 (half away from zero) = 6,667 paise.
    // Rounding the rate to 33.33 first would give 66.66, which is wrong: round once, at the end.
    let it = item(ValuationMethod::LastPurchaseCost, &[], &[]);
    let moves = vec![
        mv(1, date!(2026 - 04 - 02), ITEM, 3, 0, 10_000),
        mv(2, date!(2026 - 04 - 03), ITEM, 0, 1, 5_000),
    ];
    let v = last_purchase_cost(&it, &moves, period()).unwrap();
    assert_eq!(v.qty, Qty(d(2)));
    assert_eq!(v.value, Paise::new(6_667));
    assert_eq!(v.rate, Some(Rate(d(100) / d(3))));
}

#[test]
fn last_purchase_unsorted_moves_pick_the_latest_by_date() {
    // derived: the 20-May purchase (15.00) is the latest even when listed first.
    let it = item(ValuationMethod::LastPurchaseCost, &[(100, 10)], &[]);
    let mut moves = r3_moves();
    moves.reverse();
    let v = last_purchase_cost(&it, &moves, period()).unwrap();
    assert_eq!(v.value, Paise::new(60_000));
}

#[test]
fn last_purchase_with_only_opening_stock_is_an_error_not_the_opening_rate() {
    // derived: 10 units in stock, no purchase at all. R3 item 8 defines the rate as that of the
    // latest purchase; there is none, so there is no number (opening rate is not substituted).
    let it = item(ValuationMethod::LastPurchaseCost, &[(10, 10)], &[]);
    let err = last_purchase_cost(&it, &[], period()).unwrap_err();
    assert!(matches!(err, StockError::Invalid { .. }), "{err:?}");
}

#[test]
fn last_purchase_with_nothing_in_stock_is_zero_without_a_rate() {
    // derived: 5 bought and 5 sold, closing qty 0; 0 x any rate = 0.00, nothing to report as rate.
    let it = item(ValuationMethod::LastPurchaseCost, &[], &[]);
    let moves = vec![
        mv(1, date!(2026 - 04 - 02), ITEM, 5, 0, 5_000),
        mv(2, date!(2026 - 04 - 03), ITEM, 0, 5, 9_000),
    ];
    let v = last_purchase_cost(&it, &moves, period()).unwrap();
    assert_eq!(v.qty, Qty(d(0)));
    assert_eq!(v.value, Paise::new(0));
    assert_eq!(v.rate, None);
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn standard_cost_12_gives_480_00() {
    // derived (R3 item 9): closing qty 40 x standard 12.00 = 480.00 = 48,000 paise.
    let it = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 04 - 01), 12)],
    );
    let v = standard_cost(&it, &r3_moves(), period()).unwrap();
    assert_eq!(v.qty, Qty(d(40)));
    assert_eq!(v.value, Paise::new(48_000));
    assert_eq!(v.rate, Some(Rate(d(12))));
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn standard_cost_uses_the_entry_in_force_at_the_period_end() {
    // derived: standards 12.00 from 1-Apr and 14.00 from 1-Jun. As of 31-May the 12.00 one is in
    // force (480.00); as of 30-Jun the 14.00 one is: 40 x 14 = 560.00 = 56,000 paise.
    let it = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 06 - 01), 14), (date!(2026 - 04 - 01), 12)],
    );
    let may = standard_cost(&it, &r3_moves(), period()).unwrap();
    assert_eq!(may.value, Paise::new(48_000));
    let june = Period::new(date!(2026 - 04 - 01), date!(2026 - 06 - 30)).unwrap();
    let jun = standard_cost(&it, &r3_moves(), june).unwrap();
    assert_eq!(jun.value, Paise::new(56_000));
    assert_eq!(jun.rate, Some(Rate(d(14))));
}

#[test]
fn standard_cost_dated_on_the_period_end_applies() {
    // derived: "from" is the first date the cost applies, inclusive: a standard dated 31-May
    // is in force on 31-May: 40 x 12 = 480.00.
    let it = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 05 - 31), 12)],
    );
    let v = standard_cost(&it, &r3_moves(), period()).unwrap();
    assert_eq!(v.value, Paise::new(48_000));
}

#[test]
fn standard_cost_with_no_dated_standard_is_an_error_never_a_fallback() {
    // derived (spec 6.4 "no silent fallback"; R3 item 9 says the vendor falls back to the last
    // purchase rate, 15.00, which would give 600.00 - that number must not appear).
    let it = item(ValuationMethod::StandardCost, &[(100, 10)], &[]);
    let err = standard_cost(&it, &r3_moves(), period()).unwrap_err();
    assert_eq!(
        err,
        StockError::NoStandardCost {
            item: ITEM,
            date: date!(2026 - 05 - 31)
        }
    );
}

#[test]
fn standard_cost_dated_after_the_period_end_is_not_in_force() {
    // derived: the only standard starts 1-Jun, after the 31-May as-of date: none in force.
    let it = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 06 - 01), 12)],
    );
    let err = standard_cost(&it, &r3_moves(), period()).unwrap_err();
    assert!(matches!(err, StockError::NoStandardCost { .. }), "{err:?}");
}

#[test]
fn standard_cost_two_rates_on_one_date_are_ambiguous() {
    // derived: two different rates both effective 1-Apr: there is no single rate in force.
    let it = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 04 - 01), 12), (date!(2026 - 04 - 01), 13)],
    );
    let err = standard_cost(&it, &r3_moves(), period()).unwrap_err();
    assert!(matches!(err, StockError::Invalid { .. }), "{err:?}");
}

#[test]
fn negative_running_quantity_makes_both_methods_provisional() {
    // derived: opening 10; purchase 5 on 5-Apr (running 15); sale 20 on 10-Apr (running -5, the
    // first negative date); purchase 10 @ 12.00 = 120.00 on 20-Apr (running 5). Closing qty 5.
    // Last purchase: 5 x 12 = 60.00 = 6,000 paise. Standard 11.00: 5 x 11 = 55.00 = 5,500 paise.
    let moves = vec![
        mv(1, date!(2026 - 04 - 05), ITEM, 5, 0, 5_000),
        mv(2, date!(2026 - 04 - 10), ITEM, 0, 20, 40_000),
        mv(3, date!(2026 - 04 - 20), ITEM, 10, 0, 12_000),
    ];
    let status = ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
        item: ITEM,
        date: date!(2026 - 04 - 10),
    });
    let lp = item(ValuationMethod::LastPurchaseCost, &[(10, 10)], &[]);
    let v = last_purchase_cost(&lp, &moves, period()).unwrap();
    assert_eq!(v.qty, Qty(d(5)));
    assert_eq!(v.value, Paise::new(6_000));
    assert_eq!(v.status, status);
    let sc = item(
        ValuationMethod::StandardCost,
        &[(10, 10)],
        &[(date!(2026 - 04 - 01), 11)],
    );
    let v = standard_cost(&sc, &moves, period()).unwrap();
    assert_eq!(v.value, Paise::new(5_500));
    assert_eq!(v.status, status);
}

#[test]
fn negative_closing_quantity_is_refused() {
    // derived: opening 10, sale 15: closing -5. No rule for valuing negative closing stock has
    // been verified, so there is no number.
    let moves = vec![mv(1, date!(2026 - 04 - 10), ITEM, 0, 15, 15_000)];
    let lp = item(ValuationMethod::LastPurchaseCost, &[(10, 10)], &[]);
    assert!(matches!(
        last_purchase_cost(&lp, &moves, period()).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let sc = item(
        ValuationMethod::StandardCost,
        &[(10, 10)],
        &[(date!(2026 - 04 - 01), 11)],
    );
    assert!(matches!(
        standard_cost(&sc, &moves, period()).unwrap_err(),
        StockError::Invalid { .. }
    ));
}

#[test]
fn malformed_moves_are_errors() {
    // derived: a move that both receives and issues, and a negative quantity, cannot be valued.
    let it = item(ValuationMethod::LastPurchaseCost, &[(10, 10)], &[]);
    let both = vec![mv(1, date!(2026 - 04 - 10), ITEM, 5, 5, 5_000)];
    assert!(matches!(
        last_purchase_cost(&it, &both, period()).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let neg = vec![mv(1, date!(2026 - 04 - 10), ITEM, -5, 0, 5_000)];
    assert!(matches!(
        last_purchase_cost(&it, &neg, period()).unwrap_err(),
        StockError::Invalid { .. }
    ));
}

#[test]
fn value_item_dispatches_on_the_item_method() {
    // derived: the same R3 data under each method gives that method's own figure:
    // average 473.68 (R3 item 1: 40 x 2250 / 190), FIFO 600.00 (R3 item 4: 40 @ 15),
    // last purchase 600.00 (R3 item 8), standard 12.00 -> 480.00 (R3 item 9).
    let avg = item(ValuationMethod::AverageCost, &[(100, 10)], &[]);
    assert_eq!(
        value_item(&avg, &r3_moves(), period()).unwrap().value,
        Paise::new(47_368)
    );
    let fifo = item(ValuationMethod::Fifo, &[(100, 10)], &[]);
    assert_eq!(
        value_item(&fifo, &r3_moves(), period()).unwrap().value,
        Paise::new(60_000)
    );
    let lp = item(ValuationMethod::LastPurchaseCost, &[(100, 10)], &[]);
    assert_eq!(
        value_item(&lp, &r3_moves(), period()).unwrap().value,
        Paise::new(60_000)
    );
    let sc = item(
        ValuationMethod::StandardCost,
        &[(100, 10)],
        &[(date!(2026 - 04 - 01), 12)],
    );
    assert_eq!(
        value_item(&sc, &r3_moves(), period()).unwrap().value,
        Paise::new(48_000)
    );
}

#[test]
fn value_item_standard_without_standard_cost_is_still_an_error() {
    // derived: dispatch adds no fallback of its own.
    let sc = item(ValuationMethod::StandardCost, &[(100, 10)], &[]);
    assert!(matches!(
        value_item(&sc, &r3_moves(), period()).unwrap_err(),
        StockError::NoStandardCost { .. }
    ));
}
