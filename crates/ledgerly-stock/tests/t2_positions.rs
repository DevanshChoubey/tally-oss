//! Card T2 tests: stock position per (item, godown, batch) as of a date; expiry recorded.
//! Spec 6.4. Every expected number is `derived` by hand arithmetic, shown next to it.

#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, CompanyId, Date, Decimal, GodownId, Qty, Rate, StateCode, StockItemId, UnitId,
    VoucherId,
};
use ledgerly_model::{
    Address, Batch, Books, Company, EntityKind, Features, Godown, Masters, NegativeStockPolicy,
    OpeningStock, StockItem, StockMove, ValuationMethod,
};
use ledgerly_stock::StockError;
use ledgerly_stock::positions::{Position, batch_expiry, positions};
use proptest::prelude::*;
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);
const OTHER: StockItemId = StockItemId(2);
const G1: GodownId = GodownId(1);
const G2: GodownId = GodownId(2);
const B1: BatchId = BatchId(1);
const B2: BatchId = BatchId(2);

fn q(n: i64) -> Qty {
    Qty(Decimal::from(n))
}

fn item(id: StockItemId, opening: Vec<OpeningStock>) -> StockItem {
    StockItem {
        id,
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: true,
        opening,
        standard_costs: vec![],
        tally: None,
    }
}

fn open(godown: GodownId, batch: Option<BatchId>, n: i64) -> OpeningStock {
    OpeningStock {
        godown,
        batch,
        qty: q(n),
        rate: Rate(Decimal::from(10)),
    }
}

fn mv(
    v: u64,
    on: Date,
    item: StockItemId,
    godown: GodownId,
    batch: Option<BatchId>,
    qty_in: i64,
    qty_out: i64,
) -> StockMove {
    StockMove {
        voucher: VoucherId(v),
        date: on,
        item,
        godown,
        batch,
        qty_in: q(qty_in),
        qty_out: q(qty_out),
        value: ledgerly_core::Paise::new(0),
    }
}

fn godown(id: GodownId, name: &str) -> Godown {
    Godown {
        id,
        name: name.to_owned(),
        parent: None,
        tally: None,
    }
}

fn batch(id: BatchId, of: StockItemId, expiry: Option<Date>) -> Batch {
    Batch {
        id,
        item: of,
        name: format!("LOT-{}", id.0),
        mfg: Some(date!(2026 - 01 - 15)),
        expiry,
    }
}

fn books(items: Vec<StockItem>, moves: Vec<StockMove>) -> Books {
    let mut masters = Masters::default();
    for i in items {
        masters.stock_items.insert(i.id, i);
    }
    for g in [godown(G1, "Main"), godown(G2, "Annexe")] {
        masters.godowns.insert(g.id, g);
    }
    for b in [
        batch(B1, ITEM, Some(date!(2026 - 12 - 31))),
        batch(B2, ITEM, None),
        batch(BatchId(3), OTHER, Some(date!(2027 - 03 - 31))),
    ] {
        masters.batches.insert(b.id, b);
    }
    let mut stock_moves = moves;
    stock_moves.sort_by_key(|m| (m.date, m.voucher));
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Allow,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves,
    }
}

fn pos(item: StockItemId, g: GodownId, b: Option<BatchId>, n: i64) -> Position {
    Position {
        item,
        godown: g,
        batch: b,
        qty: q(n),
    }
}

/// Dataset: item 1 opens with 100 in Main (no batch) and 20 in Main batch 1.
fn dataset() -> Books {
    books(
        vec![
            item(ITEM, vec![open(G1, None, 100), open(G1, Some(B1), 20)]),
            item(OTHER, vec![open(G2, None, 7)]),
        ],
        vec![
            // purchase 10-Apr: +50 Main
            mv(1, date!(2026 - 04 - 10), ITEM, G1, None, 50, 0),
            // transfer 12-Apr: -30 Main, +30 Annexe
            mv(2, date!(2026 - 04 - 12), ITEM, G1, None, 0, 30),
            mv(2, date!(2026 - 04 - 12), ITEM, G2, None, 30, 0),
            // sale 15-Apr: -5 from Main batch 1
            mv(3, date!(2026 - 04 - 15), ITEM, G1, Some(B1), 0, 5),
            // purchase 20-May: +40 Annexe batch 2
            mv(4, date!(2026 - 05 - 20), ITEM, G2, Some(B2), 40, 0),
        ],
    )
}

#[test]
fn position_per_item_godown_batch_as_of_30_apr() {
    // derived (opening + in - out, by hand):
    //   (1, Main, none)    = 100 + 50 - 30 = 120
    //   (1, Main, batch 1) = 20 - 5        = 15
    //   (1, Annexe, none)  = 30
    //   (1, Annexe, b2)    = purchase dated 20-May, after the date: not counted
    //   (2, Annexe, none)  = 7 (opening only)
    let got = positions(&dataset(), date!(2026 - 04 - 30)).unwrap();
    assert_eq!(
        got,
        vec![
            pos(ITEM, G1, None, 120),
            pos(ITEM, G1, Some(B1), 15),
            pos(ITEM, G2, None, 30),
            pos(OTHER, G2, None, 7),
        ]
    );
}

#[test]
fn later_date_adds_later_movements() {
    // derived: as of 31-May the 20-May purchase of 40 into (1, Annexe, batch 2) is counted;
    // every other position is unchanged from 30-Apr.
    let got = positions(&dataset(), date!(2026 - 05 - 31)).unwrap();
    assert_eq!(
        got,
        vec![
            pos(ITEM, G1, None, 120),
            pos(ITEM, G1, Some(B1), 15),
            pos(ITEM, G2, None, 30),
            pos(ITEM, G2, Some(B2), 40),
            pos(OTHER, G2, None, 7),
        ]
    );
}

#[test]
fn movement_on_the_date_is_included_and_the_day_before_is_not() {
    // derived: the 15-Apr sale of 5 from (1, Main, batch 1) counts on 15-Apr (20 - 5 = 15) and
    // not on 14-Apr (opening 20 only).
    let on = positions(&dataset(), date!(2026 - 04 - 15)).unwrap();
    assert!(on.contains(&pos(ITEM, G1, Some(B1), 15)));
    let before = positions(&dataset(), date!(2026 - 04 - 14)).unwrap();
    assert!(before.contains(&pos(ITEM, G1, Some(B1), 20)));
}

#[test]
fn opening_only_on_books_begin() {
    // derived: no movement is dated 1-Apr, so each position is its opening quantity.
    let got = positions(&dataset(), date!(2026 - 04 - 01)).unwrap();
    assert_eq!(
        got,
        vec![
            pos(ITEM, G1, None, 100),
            pos(ITEM, G1, Some(B1), 20),
            pos(OTHER, G2, None, 7),
        ]
    );
}

#[test]
fn fully_issued_position_is_omitted_and_negative_is_kept() {
    // derived: Annexe opening 7 of item 2, issue 7 on 5-Apr -> 0, omitted;
    // item 1 Main batch 2 has no opening, issue 4 -> -4, kept (never clamped to zero).
    let b = books(
        vec![item(ITEM, vec![]), item(OTHER, vec![open(G2, None, 7)])],
        vec![
            mv(1, date!(2026 - 04 - 05), OTHER, G2, None, 0, 7),
            mv(2, date!(2026 - 04 - 06), ITEM, G1, Some(B2), 0, 4),
        ],
    );
    let got = positions(&b, date!(2026 - 04 - 30)).unwrap();
    assert_eq!(got, vec![pos(ITEM, G1, Some(B2), -4)]);
}

#[test]
fn a_movement_that_receives_and_issues_counts_its_net() {
    // derived: in 10, out 4 on one line is a net +6 on top of an opening of 1 -> 7.
    let b = books(
        vec![item(ITEM, vec![open(G1, None, 1)])],
        vec![mv(1, date!(2026 - 04 - 05), ITEM, G1, None, 10, 4)],
    );
    let got = positions(&b, date!(2026 - 04 - 30)).unwrap();
    assert_eq!(got, vec![pos(ITEM, G1, None, 7)]);
}

#[test]
fn fractional_quantities_stay_exact() {
    // derived: 0.1 + 0.2 - 0.3 = 0 exactly in decimal arithmetic, so the position is omitted;
    // 2.5 - 0.75 = 1.75.
    let mut a = mv(1, date!(2026 - 04 - 05), ITEM, G1, None, 0, 0);
    a.qty_in = Qty(Decimal::new(1, 1));
    let mut b2 = mv(2, date!(2026 - 04 - 06), ITEM, G1, None, 0, 0);
    b2.qty_in = Qty(Decimal::new(2, 1));
    let mut c = mv(3, date!(2026 - 04 - 07), ITEM, G1, None, 0, 0);
    c.qty_out = Qty(Decimal::new(3, 1));
    let mut d = mv(4, date!(2026 - 04 - 08), ITEM, G2, None, 0, 0);
    d.qty_in = Qty(Decimal::new(25, 1));
    let mut e = mv(5, date!(2026 - 04 - 09), ITEM, G2, None, 0, 0);
    e.qty_out = Qty(Decimal::new(75, 2));
    let held = books(vec![item(ITEM, vec![])], vec![a, b2, c, d, e]);
    let got = positions(&held, date!(2026 - 04 - 30)).unwrap();
    assert_eq!(
        got,
        vec![Position {
            item: ITEM,
            godown: G2,
            batch: None,
            qty: Qty(Decimal::new(175, 2)),
        }]
    );
}

#[test]
fn empty_books_have_no_positions() {
    let b = books(vec![], vec![]);
    assert_eq!(
        positions(&b, date!(2026 - 04 - 30)).unwrap(),
        Vec::<Position>::new()
    );
}

#[test]
fn date_before_books_begin_is_an_error() {
    // derived: opening stock is taken at books-begin (1-Apr-2026); 31-Mar-2026 has no defined
    // position, so an error, not an empty or opening-only list.
    let err = positions(&dataset(), date!(2026 - 03 - 31)).unwrap_err();
    assert!(matches!(err, StockError::Invalid { ref field, .. } if field == "as_of"));
}

#[test]
fn unknown_godown_in_a_move_is_an_error() {
    let b = books(
        vec![item(ITEM, vec![])],
        vec![mv(1, date!(2026 - 04 - 05), ITEM, GodownId(99), None, 5, 0)],
    );
    let err = positions(&b, date!(2026 - 04 - 30)).unwrap_err();
    assert!(matches!(err, StockError::Invalid { ref field, .. } if field == "moves"));
}

#[test]
fn unknown_godown_in_an_opening_line_is_an_error() {
    let b = books(vec![item(ITEM, vec![open(GodownId(99), None, 5)])], vec![]);
    let err = positions(&b, date!(2026 - 04 - 30)).unwrap_err();
    assert!(matches!(err, StockError::Invalid { ref field, .. } if field == "opening"));
}

#[test]
fn unknown_batch_and_other_items_batch_are_errors() {
    let unknown = books(
        vec![item(ITEM, vec![])],
        vec![mv(
            1,
            date!(2026 - 04 - 05),
            ITEM,
            G1,
            Some(BatchId(99)),
            5,
            0,
        )],
    );
    assert!(matches!(
        positions(&unknown, date!(2026 - 04 - 30)).unwrap_err(),
        StockError::Invalid { .. }
    ));
    // batch 3 belongs to item 2, not item 1
    let foreign = books(
        vec![item(ITEM, vec![]), item(OTHER, vec![])],
        vec![mv(
            1,
            date!(2026 - 04 - 05),
            ITEM,
            G1,
            Some(BatchId(3)),
            5,
            0,
        )],
    );
    assert!(matches!(
        positions(&foreign, date!(2026 - 04 - 30)).unwrap_err(),
        StockError::Invalid { .. }
    ));
}

#[test]
fn unknown_item_and_negative_quantities_are_errors() {
    let no_item = books(
        vec![item(ITEM, vec![])],
        vec![mv(
            1,
            date!(2026 - 04 - 05),
            StockItemId(77),
            G1,
            None,
            5,
            0,
        )],
    );
    assert!(matches!(
        positions(&no_item, date!(2026 - 04 - 30)).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let negative_move = books(
        vec![item(ITEM, vec![])],
        vec![mv(1, date!(2026 - 04 - 05), ITEM, G1, None, -5, 0)],
    );
    assert!(matches!(
        positions(&negative_move, date!(2026 - 04 - 30)).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let negative_opening = books(vec![item(ITEM, vec![open(G1, None, -3)])], vec![]);
    assert!(matches!(
        positions(&negative_opening, date!(2026 - 04 - 30)).unwrap_err(),
        StockError::Invalid { .. }
    ));
}

#[test]
fn malformed_later_move_does_not_affect_an_earlier_date() {
    // derived: a bad move dated 20-May is not looked at when the date is 30-Apr.
    let b = books(
        vec![item(ITEM, vec![open(G1, None, 3)])],
        vec![mv(1, date!(2026 - 05 - 20), ITEM, GodownId(99), None, 5, 0)],
    );
    assert_eq!(
        positions(&b, date!(2026 - 04 - 30)).unwrap(),
        vec![pos(ITEM, G1, None, 3)]
    );
    assert!(positions(&b, date!(2026 - 05 - 31)).is_err());
}

#[test]
fn expiry_is_recorded_on_the_batch() {
    // derived: the masters in `books()` give batch 1 expiry 31-Dec-2026, batch 2 none and
    // batch 3 31-Mar-2027; the function must return exactly what the master holds.
    let b = dataset();
    assert_eq!(batch_expiry(&b, B1).unwrap(), Some(date!(2026 - 12 - 31)));
    assert_eq!(batch_expiry(&b, B2).unwrap(), None);
    assert_eq!(
        batch_expiry(&b, BatchId(3)).unwrap(),
        Some(date!(2027 - 03 - 31))
    );
}

#[test]
fn expiry_of_a_missing_batch_is_an_error() {
    let err = batch_expiry(&dataset(), BatchId(99)).unwrap_err();
    assert!(matches!(err, StockError::Invalid { ref field, .. } if field == "batch"));
}

proptest! {
    #[test]
    fn positions_conserve_total_quantity(
        moves in proptest::collection::vec((1u64..=2, 0i64..50, 0i64..50, 1u32..=60), 0..20),
        opening in 0i64..100,
    ) {
        // derived invariant: the sum of an item's positions equals opening + sum(in) - sum(out)
        // over moves dated on or before the date (zero positions drop out without changing a sum).
        let base = date!(2026 - 04 - 01);
        let stock_moves: Vec<StockMove> = moves
            .iter()
            .enumerate()
            .map(|(i, &(g, qin, qout, day))| {
                mv(
                    i as u64,
                    base.saturating_add(time::Duration::days(i64::from(day))),
                    ITEM,
                    GodownId(g),
                    None,
                    qin,
                    qout,
                )
            })
            .collect();
        let as_of = base.saturating_add(time::Duration::days(30));
        let expected: i64 = opening
            + moves
                .iter()
                .filter(|&&(_, _, _, day)| day <= 30)
                .map(|&(_, qin, qout, _)| qin - qout)
                .sum::<i64>();
        let b = books(vec![item(ITEM, vec![open(G1, None, opening)])], stock_moves);
        let got = positions(&b, as_of).unwrap();
        let total: Decimal = got.iter().map(|p| p.qty.0).sum();
        prop_assert_eq!(total, Decimal::from(expected));
    }
}
