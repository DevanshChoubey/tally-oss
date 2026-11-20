//! Card T4: FIFO (annual) valuation. Every expected value is `derived` by hand, shown in place.
//! Spec 6.4: FIFO (annual); a negative running quantity makes the result Provisional.

#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    reason = "integration tests may unwrap (engineering rules B2)"
)]

use ledgerly_core::{Decimal, GodownId, Paise, Period, Qty, Rate, StockItemId, UnitId, VoucherId};
use ledgerly_model::{OpeningStock, StockItem, StockMove, ValuationMethod};
use ledgerly_stock::fifo::fifo;
use ledgerly_stock::{ProvisionalReason, StockError, ValuationStatus};
use proptest::prelude::*;
use time::Date;
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);

fn item(method: ValuationMethod, opening: Vec<OpeningStock>) -> StockItem {
    StockItem {
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
        opening,
        standard_costs: vec![],
        tally: None,
    }
}

fn q(n: i64) -> Qty {
    Qty(Decimal::from(n))
}

fn buy(v: u64, d: Date, qty: i64, value_paise: i64) -> StockMove {
    StockMove {
        voucher: VoucherId(v),
        date: d,
        item: ITEM,
        godown: GodownId(1),
        batch: None,
        qty_in: q(qty),
        qty_out: Qty::ZERO,
        value: Paise::new(value_paise),
    }
}

fn sell(v: u64, d: Date, qty: i64, value_paise: i64) -> StockMove {
    StockMove {
        voucher: VoucherId(v),
        date: d,
        item: ITEM,
        godown: GodownId(1),
        batch: None,
        qty_in: Qty::ZERO,
        qty_out: q(qty),
        value: Paise::new(value_paise),
    }
}

fn fy1() -> Period {
    Period::new(date!(2025 - 04 - 01), date!(2026 - 03 - 31)).unwrap()
}

fn fy2() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn opening_10_at_40() -> Vec<OpeningStock> {
    vec![OpeningStock {
        godown: GodownId(1),
        batch: None,
        qty: q(10),
        rate: Rate(Decimal::from(40)),
    }]
}

/// Year 1 history shared by the tests.
fn year1_moves() -> Vec<StockMove> {
    vec![
        buy(1, date!(2025 - 05 - 10), 20, 100_000), // 20 @ 50.00 = 1000.00
        sell(2, date!(2025 - 07 - 15), 18, 1_800_000), // sale value is not a cost
    ]
}

#[test]
fn closing_is_600_00() {
    // provenance: derived (FIFO per AS 2 para 23: cost of items sold is taken from the oldest
    // layers first). Opening 10 @ 40.00 = 400.00. Purchase 20 @ 50.00 = 1000.00.
    // Sale of 18 consumes the 10 opening units (400.00) and 8 of the 20 purchased (8 x 50 = 400.00).
    // Remaining: 12 units @ 50.00 = 600.00.
    let it = item(ValuationMethod::Fifo, opening_10_at_40());
    let v = fifo(&it, &year1_moves(), fy1()).unwrap();
    assert_eq!(v.item, ITEM);
    assert_eq!(v.as_of, date!(2026 - 03 - 31));
    assert_eq!(v.qty, q(12));
    assert_eq!(v.value, Paise::new(60_000));
    assert_eq!(v.rate, Some(Rate(Decimal::from(50))));
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn next_year_opening_rate_is_closing_value_over_qty() {
    // provenance: derived. Year 1 closing is 600.00 over 12 units (see closing_is_600_00), so the
    // opening rate of year 2 is 600.00 / 12 = 50.00 per unit.
    let it = item(ValuationMethod::Fifo, opening_10_at_40());
    let y1 = fifo(&it, &year1_moves(), fy1()).unwrap();
    let rate = y1.value.to_rupees() / y1.qty.0;
    assert_eq!(rate, Decimal::from(50));
    assert_eq!(y1.rate, Some(Rate(rate)));

    // Year 2 with no movements: still 12 units, 600.00, opening rate 50.00 carried forward.
    let y2 = fifo(&it, &year1_moves(), fy2()).unwrap();
    assert_eq!(y2.qty, q(12));
    assert_eq!(y2.value, Paise::new(60_000));
    assert_eq!(y2.rate, Some(Rate(Decimal::from(50))));
}

#[test]
fn year_two_consumes_carried_layer_before_new_purchase() {
    // provenance: derived. Layers entering year 2: 12 @ 50.00. Purchase 10 @ 60.00 = 600.00 on
    // 2026-05-01. Sale of 15 on 2026-06-01 takes 12 @ 50 = 600.00 then 3 @ 60 = 180.00.
    // Remaining 7 @ 60.00 = 420.00.
    let it = item(ValuationMethod::Fifo, opening_10_at_40());
    let mut moves = year1_moves();
    moves.push(buy(3, date!(2026 - 05 - 01), 10, 60_000));
    moves.push(sell(4, date!(2026 - 06 - 01), 15, 2_000_000));
    let v = fifo(&it, &moves, fy2()).unwrap();
    assert_eq!(v.qty, q(7));
    assert_eq!(v.value, Paise::new(42_000));
    assert_eq!(v.rate, Some(Rate(Decimal::from(60))));
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn closing_is_600_00_on_the_research_dataset() {
    // provenance: derived (AS 2 para 23, oldest lots first, lot by lot). The card's 600.00 case.
    // Lots: 01-Apr 100 @ 10.00 = 1000.00; 10-Apr 50 @ 13.00 = 650.00; 20-May 40 @ 15.00 = 600.00.
    // Sale 15-Apr of 120: all 100 of the first lot (1000.00), then 20 of the 50 @ 13.00
    // (650.00 x 20/50 = 260.00); 30 @ 13.00 = 390.00 stay.
    // Sale 31-May of 30: exactly the 30 @ 13.00 left; the 40 @ 15.00 lot is untouched.
    // Closing: 40 @ 15.00 = 600.00. Check: 1000 + 650 + 600 - 1000 - 260 - 390 = 600.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 04 - 01), 100, 100_000),
        buy(2, date!(2025 - 04 - 10), 50, 65_000),
        sell(3, date!(2025 - 04 - 15), 120, 0),
        buy(4, date!(2025 - 05 - 20), 40, 60_000),
        sell(5, date!(2025 - 05 - 31), 30, 0),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(40));
    assert_eq!(v.value, Paise::new(60_000));
    assert_eq!(v.rate, Some(Rate(Decimal::from(15))));
    assert_eq!(v.status, ValuationStatus::Final);

    // Closing on 30-Apr (before the May moves): the 30 @ 13.00 left after the 15-Apr sale = 390.00.
    let april = Period::new(date!(2025 - 04 - 01), date!(2025 - 04 - 30)).unwrap();
    let a = fifo(&it, &moves, april).unwrap();
    assert_eq!(a.qty, q(30));
    assert_eq!(a.value, Paise::new(39_000));
    assert_eq!(a.rate, Some(Rate(Decimal::from(13))));
}

#[test]
fn year_end_collapses_lots_into_one_at_closing_value_over_qty() {
    // provenance: derived (card T4: next-year opening rate = closing value / qty; research note
    // 03-inventory section 4: lot identity is collapsed at the year boundary).
    // Year 1 (FY 2025-26): buy 10 @ 50.00 = 500.00 and 10 @ 60.00 = 600.00, no sales.
    // Closing 20 units, 1100.00; year-2 opening rate = 1100.00 / 20 = 55.00, one lot of 20 @ 55.
    // Year 2: sell 10: cost = 10 x 55.00 = 550.00 (a lot-preserving FIFO would take 10 @ 50 = 500).
    // Closing 10 @ 55.00 = 550.00 (lot-preserving would give 600.00).
    let it = item(ValuationMethod::Fifo, vec![]);
    let mut moves = vec![
        buy(1, date!(2025 - 05 - 01), 10, 50_000),
        buy(2, date!(2025 - 06 - 01), 10, 60_000),
    ];
    let y1 = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(y1.qty, q(20));
    assert_eq!(y1.value, Paise::new(110_000));
    assert_eq!(y1.rate, Some(Rate(Decimal::from(55))));

    moves.push(sell(3, date!(2026 - 05 - 01), 10, 0));
    let y2 = fifo(&it, &moves, fy2()).unwrap();
    assert_eq!(y2.qty, q(10));
    assert_eq!(y2.value, Paise::new(55_000));
    assert_eq!(y2.rate, Some(Rate(Decimal::from(55))));
    assert_eq!(y2.status, ValuationStatus::Final);
}

#[test]
fn collapse_applies_across_a_year_with_no_movements() {
    // provenance: derived. Same lots as the collapse test above (closing FY 2025-26: 20 @ 55.00,
    // 1100.00). FY 2026-27 has no movements; the sale of 10 is in FY 2027-28 (2027-05-01).
    // Collapsing is idempotent, so the cost is 10 x 55.00 = 550.00 and 10 @ 55.00 = 550.00 stay.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 10, 50_000),
        buy(2, date!(2025 - 06 - 01), 10, 60_000),
        sell(3, date!(2027 - 05 - 01), 10, 0),
    ];
    let fy3 = Period::new(date!(2027 - 04 - 01), date!(2028 - 03 - 31)).unwrap();
    let v = fifo(&it, &moves, fy3).unwrap();
    assert_eq!(v.qty, q(10));
    assert_eq!(v.value, Paise::new(55_000));
}

#[test]
fn lots_within_one_year_are_not_collapsed() {
    // provenance: derived. Same lots, but the sale of 10 is in the same year (2025-07-01):
    // it takes the whole first lot, 10 @ 50.00 = 500.00; closing 10 @ 60.00 = 600.00.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 10, 50_000),
        buy(2, date!(2025 - 06 - 01), 10, 60_000),
        sell(3, date!(2025 - 07 - 01), 10, 0),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(10));
    assert_eq!(v.value, Paise::new(60_000));
}

#[test]
fn negative_stock_in_an_earlier_year_keeps_later_years_provisional() {
    // provenance: derived. FY 2025-26: buy 5 @ 10.00, sell 8 on 2025-06-10 (goes to -3).
    // FY 2026-27: buy 10 @ 20.00 = 200.00 on 2026-05-01; its first 3 units cover the shortfall
    // (cost 200.00 x 3/10 = 60.00), leaving 7 @ 20.00 = 140.00. Quantity is 5 - 8 + 10 = 7.
    // The first negative date is still 2025-06-10, so year 2 is Provisional too.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 5, 5_000),
        sell(2, date!(2025 - 06 - 10), 8, 0),
        buy(3, date!(2026 - 05 - 01), 10, 20_000),
    ];
    let v = fifo(&it, &moves, fy2()).unwrap();
    assert_eq!(v.qty, q(7));
    assert_eq!(v.value, Paise::new(14_000));
    assert_eq!(
        v.status,
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: ITEM,
            date: date!(2025 - 06 - 10)
        })
    );
}

#[test]
fn moves_after_period_end_are_ignored() {
    // provenance: derived. The sale on 2026-04-02 is outside FY 2025-26, so closing at
    // 2026-03-31 is the purchase alone: 20 @ 50.00 = 1000.00.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 10), 20, 100_000),
        sell(2, date!(2026 - 04 - 02), 20, 1),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(20));
    assert_eq!(v.value, Paise::new(100_000));
}

#[test]
fn partial_layers_conserve_every_paisa() {
    // provenance: derived. One layer of 3 units costing 100.00 (10000 paise).
    // Sell 1: cost = 10000 x 1/3 = 3333.33 paise, rounds half-up to 3333. Layer left: 2 @ 6667.
    // Sell 1: cost = 6667 x 1/2 = 3333.5 paise, half rounds away from zero to 3334. Left: 1 @ 3333.
    // 3333 + 3334 + 3333 = 10000, so no paisa is lost. Closing: 1 unit, 33.33.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 3, 10_000),
        sell(2, date!(2025 - 06 - 01), 1, 0),
        sell(3, date!(2025 - 07 - 01), 1, 0),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(1));
    assert_eq!(v.value, Paise::new(3_333));
}

#[test]
fn fully_sold_gives_zero_and_no_rate() {
    // provenance: derived. Buy 5, sell 5: nothing remains, value 0, no unit rate.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 5, 5_000),
        sell(2, date!(2025 - 06 - 01), 5, 0),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, Qty::ZERO);
    assert_eq!(v.value, Paise::ZERO);
    assert_eq!(v.rate, None);
    assert_eq!(v.status, ValuationStatus::Final);
}

#[test]
fn negative_stock_is_provisional_with_first_date() {
    // provenance: derived (spec 6.4: valuation over a period with negative stock is
    // Provisional { NegativeStock }). Buy 5 @ 10.00; sell 8 on 2025-06-10 leaves -3 from that day.
    // Closing qty = 5 - 8 = -3. The 3 missing units have no cost layer, so no value is claimed.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 5, 5_000),
        sell(2, date!(2025 - 06 - 10), 8, 0),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(-3));
    assert_eq!(v.value, Paise::ZERO);
    assert_eq!(v.rate, None);
    assert_eq!(
        v.status,
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: ITEM,
            date: date!(2025 - 06 - 10)
        })
    );
}

#[test]
fn negative_then_replenished_stays_provisional() {
    // provenance: derived. As above (-3 on 2025-06-10), then buy 10 @ 20.00 on 2025-07-01.
    // Closing qty = 5 - 8 + 10 = 7. The first 3 received units cover the earlier shortfall
    // (their cost was never known), leaving 7 units of the new layer = 7 x 20.00 = 140.00.
    // The result is still Provisional because the quantity went negative in the period.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 5, 5_000),
        sell(2, date!(2025 - 06 - 10), 8, 0),
        buy(3, date!(2025 - 07 - 01), 10, 20_000),
    ];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(7));
    assert_eq!(v.value, Paise::new(14_000));
    assert_eq!(
        v.status,
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: ITEM,
            date: date!(2025 - 06 - 10)
        })
    );
}

#[test]
fn opening_stock_rate_times_qty_rounds_half_up_to_paise() {
    // provenance: derived. Opening 3 units @ 33.335 rupees = 100.005 rupees, half rounds away
    // from zero to 100.01 rupees = 10001 paise.
    let it = item(
        ValuationMethod::Fifo,
        vec![OpeningStock {
            godown: GodownId(1),
            batch: None,
            qty: q(3),
            rate: Rate(Decimal::new(33_335, 3)),
        }],
    );
    let v = fifo(&it, &[], fy1()).unwrap();
    assert_eq!(v.qty, q(3));
    assert_eq!(v.value, Paise::new(10_001));
}

#[test]
fn other_valuation_method_is_an_error_not_a_fifo_number() {
    // provenance: derived (rule A1: no silent substitution of one method for another).
    let it = item(ValuationMethod::AverageCost, vec![]);
    let err = fifo(&it, &[], fy1()).unwrap_err();
    assert!(matches!(err, StockError::Invalid { .. }), "{err:?}");
}

#[test]
fn malformed_moves_are_errors() {
    // provenance: derived (rule A1: no best effort). Each case cannot be valued by FIFO.
    let it = item(ValuationMethod::Fifo, vec![]);
    let d = date!(2025 - 05 - 01);

    // another item's move
    let mut other = buy(1, d, 1, 100);
    other.item = StockItemId(2);
    assert!(matches!(
        fifo(&it, &[other], fy1()),
        Err(StockError::Invalid { .. })
    ));

    // negative quantity
    assert!(matches!(
        fifo(&it, &[buy(1, d, -1, 100)], fy1()),
        Err(StockError::Invalid { .. })
    ));

    // value-only movement (no quantity)
    assert!(matches!(
        fifo(&it, &[buy(1, d, 0, 100)], fy1()),
        Err(StockError::Invalid { .. })
    ));

    // negative value on a receipt
    assert!(matches!(
        fifo(&it, &[buy(1, d, 1, -100)], fy1()),
        Err(StockError::Invalid { .. })
    ));

    // a move that is both in and out
    let mut both = buy(1, d, 1, 100);
    both.qty_out = q(1);
    assert!(matches!(
        fifo(&it, &[both], fy1()),
        Err(StockError::Invalid { .. })
    ));

    // a voucher that both receives and issues the item (godown transfer, stock journal)
    let moves = [buy(1, d, 5, 500), buy(2, d, 5, 500), sell(2, d, 3, 0)];
    assert!(matches!(
        fifo(&it, &moves, fy1()),
        Err(StockError::Invalid { .. })
    ));
}

#[test]
fn overflow_is_an_error() {
    // provenance: derived. Two receipts of i64::MAX paise each cannot be summed.
    let it = item(ValuationMethod::Fifo, vec![]);
    let moves = vec![
        buy(1, date!(2025 - 05 - 01), 1, i64::MAX),
        buy(2, date!(2025 - 05 - 02), 1, i64::MAX),
    ];
    assert!(matches!(
        fifo(&it, &moves, fy1()),
        Err(StockError::Money(_))
    ));
}

#[test]
fn same_date_moves_keep_slice_order() {
    // provenance: derived. Both on one day; the slice is sorted by (date, voucher) upstream.
    // Buy 4 @ 10.00 (voucher 1), sell 4 (voucher 2), buy 4 @ 30.00 (voucher 3): the sale takes
    // the 10.00 layer, leaving 4 @ 30.00 = 120.00.
    let it = item(ValuationMethod::Fifo, vec![]);
    let d = date!(2025 - 05 - 01);
    let moves = vec![buy(1, d, 4, 4_000), sell(2, d, 4, 0), buy(3, d, 4, 12_000)];
    let v = fifo(&it, &moves, fy1()).unwrap();
    assert_eq!(v.qty, q(4));
    assert_eq!(v.value, Paise::new(12_000));
}

proptest! {
    /// provenance: derived invariants. With integer quantities and no negative stock:
    /// closing qty = sum(in) - sum(out); with no sales, closing value = sum of receipt values.
    #[test]
    fn no_sales_closing_value_is_sum_of_receipts(
        lots in proptest::collection::vec((1i64..1_000, 0i64..10_000_000), 1..20)
    ) {
        let it = item(ValuationMethod::Fifo, vec![]);
        let mut moves = Vec::new();
        let (mut tq, mut tv) = (0i64, 0i64);
        for (i, (qty, val)) in lots.iter().enumerate() {
            moves.push(buy(i as u64 + 1, date!(2025 - 05 - 01), *qty, *val));
            tq += qty;
            tv += val;
        }
        let v = fifo(&it, &moves, fy1()).unwrap();
        prop_assert_eq!(v.qty, q(tq));
        prop_assert_eq!(v.value, Paise::new(tv));
        prop_assert_eq!(v.status, ValuationStatus::Final);
    }

    /// provenance: derived. Selling all but k units never yields a value above the total cost,
    /// and selling everything yields zero.
    #[test]
    fn closing_never_exceeds_cost_and_qty_balances(
        lots in proptest::collection::vec((1i64..100, 0i64..1_000_000), 1..10),
        sold_pct in 0i64..=100,
    ) {
        let it = item(ValuationMethod::Fifo, vec![]);
        let mut moves = Vec::new();
        let (mut tq, mut tv) = (0i64, 0i64);
        for (i, (qty, val)) in lots.iter().enumerate() {
            moves.push(buy(i as u64 + 1, date!(2025 - 05 - 01), *qty, *val));
            tq += qty;
            tv += val;
        }
        let sold = tq * sold_pct / 100;
        if sold > 0 {
            moves.push(sell(1000, date!(2025 - 06 - 01), sold, 0));
        }
        let v = fifo(&it, &moves, fy1()).unwrap();
        prop_assert_eq!(v.qty, q(tq - sold));
        prop_assert!(v.value.get() <= tv);
        prop_assert!(v.value.get() >= 0);
        if sold == tq {
            prop_assert_eq!(v.value, Paise::ZERO);
        }
        prop_assert_eq!(v.status, ValuationStatus::Final);
    }
}
