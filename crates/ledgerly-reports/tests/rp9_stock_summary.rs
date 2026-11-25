//! Card RP9: stock summary.
//!
//! All data is fictional. Dataset R3 (docs/research/03-inventory.md, "Valuation algorithms", and
//! the plan's R3 line): opening 100 @ 10 = 1,000; 10-Apr purchase 50 @ 13 = 650; 15-Apr sale 120;
//! 20-May purchase 40 @ 15 = 600; 31-May sale 30; closing 40. Every expected number carries its
//! provenance next to it. Closing valuations are inputs (what the valuation methods hand the
//! report), written by hand from the same arithmetic.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GodownId, Outcome, Paise, Period, Qty, Rate, SignedAmount, StateCode,
    StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Godown, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, OpeningStock, RestartPeriod, Scenario, StandardCost,
    StockItem, StockMove, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
    VoucherType,
};
use ledgerly_reports::stock_summary::stock_summary;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};
use ledgerly_stock::other_methods::value_item;
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};
use time::macros::date;

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn item(id: u64, name: &str, method: ValuationMethod, opening: &[(u64, i64, i64)]) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: StockItemId(id),
        name: name.to_owned(),
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
            .map(|&(godown, qty, rate)| OpeningStock {
                godown: GodownId(godown),
                batch: None,
                qty: Qty(d(qty)),
                rate: Rate(d(rate)),
            })
            .collect(),
        standard_costs: vec![],
        tally: None,
    }
}

fn mv(
    voucher: u64,
    on: Date,
    item: u64,
    godown: u64,
    qty_in: i64,
    qty_out: i64,
    value: Paise,
) -> StockMove {
    StockMove {
        voucher: VoucherId(voucher),
        date: on,
        item: StockItemId(item),
        godown: GodownId(godown),
        batch: None,
        qty_in: Qty(d(qty_in)),
        qty_out: Qty(d(qty_out)),
        value,
        status: ledgerly_model::VoucherStatus::Regular,
    }
}

fn voucher(id: u64, on: Date) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::ItemInvoice,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".into(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: vec![],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Proprietorship,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn godown(id: u64, name: &str) -> Godown {
    Godown {
        id: GodownId(id),
        name: name.to_owned(),
        parent: None,
        tally: None,
    }
}

/// Books with godowns 1 ("Main godown") and 2 ("Annexe"), one regular voucher per movement.
fn books(items: Vec<StockItem>, moves: Vec<StockMove>) -> Books {
    let mut masters = ledgerly_model::Masters::default();
    for i in items {
        masters.stock_items.insert(i.id, i);
    }
    for g in [godown(1, "Main godown"), godown(2, "Annexe")] {
        masters.godowns.insert(g.id, g);
    }
    let mut vouchers = BTreeMap::new();
    for m in &moves {
        vouchers.insert(m.voucher, voucher(m.voucher.0, m.date));
    }
    Books {
        company: company(),
        masters,
        vouchers,
        postings: vec![],
        stock_moves: moves,
    }
}

/// Gives vouchers 1 and 2 a type (Sales, Purchase), a number and the unit "nos" the item names.
fn name_the_vouchers(b: &mut Books) {
    for (id, name, base) in [
        (1, "Sales", BaseVoucherKind::Sales),
        (2, "Purchase", BaseVoucherKind::Purchase),
    ] {
        b.masters.voucher_types.insert(
            VoucherTypeId(id),
            VoucherType {
                id: VoucherTypeId(id),
                name: name.to_owned(),
                base,
                abbreviation: name.to_owned(),
                numbering: NumberingConfig {
                    method: NumberingMethod::Automatic,
                    prevent_renumbering: false,
                    prefix: String::new(),
                    suffix: String::new(),
                    width: 1,
                    restart: RestartPeriod::Yearly,
                    series: vec![],
                },
                active: true,
                tally: None,
            },
        );
    }
    for (v, ty, number) in [(1, 1, "SI/0001"), (2, 2, "PB/0001")] {
        if let Some(x) = b.vouchers.get_mut(&VoucherId(v)) {
            x.type_id = VoucherTypeId(ty);
            x.number = Some(number.to_owned());
        }
    }
    b.masters.units.insert(
        UnitId(1),
        Unit {
            id: UnitId(1),
            symbol: "nos".to_owned(),
            formal_name: "Numbers".to_owned(),
            decimals: 0,
            kind: UnitKind::Simple,
            uqc: None,
            tally: None,
        },
    );
}

fn params(from: Date, to: Date) -> ReportParams {
    ReportParams {
        kind: ReportKind::StockSummary,
        period: Period::new(from, to).unwrap(),
        as_of: to,
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn valuation(item: u64, as_of: Date, qty: i64, value: Paise) -> StockValuation {
    StockValuation {
        item: StockItemId(item),
        as_of,
        qty: Qty(d(qty)),
        value,
        rate: None,
        status: ValuationStatus::Final,
    }
}

/// `v` with the valuation's own rate set (hand-derived by the test that uses it).
fn rated(mut v: StockValuation, rate: Decimal) -> StockValuation {
    v.rate = Some(Rate(rate));
    v
}

fn run(b: &Books, p: &ReportParams, vals: Vec<StockValuation>) -> Report {
    stock_summary(b, p, &Outcome::Value(vals))
        .unwrap()
        .value()
        .unwrap()
}

fn amount(p: Paise) -> Cell {
    Cell::Amount(SignedAmount::dr(p))
}

fn qty(n: i64) -> Cell {
    Cell::Qty(Qty(d(n)))
}

fn row_of<'a>(r: &'a Report, label: &str) -> &'a ReportRow {
    r.rows.iter().find(|row| row.label == label).unwrap()
}

fn is_unsupported(c: &Cell) -> bool {
    matches!(c, Cell::Unsupported(_))
}

/// The R3 movements (vouchers 1 to 4), all in godown 1, of item 1.
fn r3_moves() -> Vec<StockMove> {
    vec![
        mv(1, date!(2026 - 04 - 10), 1, 1, 50, 0, rs(650)),
        // The sale values are selling prices; the report must never use them.
        mv(2, date!(2026 - 04 - 15), 1, 1, 0, 120, rs(2400)),
        mv(3, date!(2026 - 05 - 20), 1, 1, 40, 0, rs(600)),
        mv(4, date!(2026 - 05 - 31), 1, 1, 0, 30, rs(600)),
    ]
}

fn r3_period() -> ReportParams {
    params(date!(2026 - 04 - 01), date!(2026 - 05 - 31))
}

fn r3_books(method: ValuationMethod) -> Books {
    books(vec![item(1, "Widget", method, &[(1, 100, 10)])], r3_moves())
}

#[test]
fn r3_average_cost_item_row() {
    // provenance: derived, R3 dataset with our documented periodic average (spec open question 1,
    // not observed in TallyPrime). Pool = 1,000 + 650 + 600 = 2,250 over 100 + 90 = 190 units;
    // closing 40 x 2,250 / 190 = 473.68 (research 03, valuation item 2).
    // Quantities: 100 opening; 50 + 40 = 90 in; 120 + 30 = 150 out; 100 + 90 - 150 = 40.
    // Inwards value 650 + 600 = 1,250.00. Outwards (consumption) = 1,000 + 1,250 - 473.68 =
    // 1,776.32 (the plan's T3 figure).
    let b = r3_books(ValuationMethod::AverageCost);
    // Closing rate: 2,250 / 190 = 11.8421..., shown as 11.84.
    let closing = rated(
        valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368)),
        d(2250) / d(190),
    );
    let r = run(&b, &r3_period(), vec![closing]);
    assert_eq!(r.rows.len(), 1);
    let row = &r.rows[0];
    assert_eq!(row.label, "Widget");
    assert_eq!(row.key, Some(DrillKey::Item(StockItemId(1))));
    assert_eq!(row.kind, RowKind::Item);
    assert_eq!(row.depth, 0);
    assert_eq!(
        row.cells,
        vec![
            qty(100),
            amount(rs(1000)),
            qty(90),
            amount(rs(1250)),
            qty(150),
            amount(Paise::new(177_632)),
            qty(40),
            amount(Paise::new(47_368)),
            amount(Paise::new(1_184)),
        ]
    );
    assert_eq!(r.columns.len(), 9);
    assert_eq!(r.title, "Stock Summary");
}

#[test]
fn r3_fifo_item_row() {
    // provenance: derived, research 03 valuation item 4 (FIFO annual): the 15-Apr sale takes
    // 100 @ 10 and 20 @ 13, the 31-May sale takes the remaining 30 @ 13, leaving 40 @ 15 = 600.
    // Outwards = 1,000 + 1,250 - 600 = 1,650.00 (= 1,000 + 260 + 390).
    let b = r3_books(ValuationMethod::Fifo);
    // The 40 left are all the 20-May purchase at 15.00, so the closing rate is 15.00.
    let closing = rated(valuation(1, date!(2026 - 05 - 31), 40, rs(600)), d(15));
    let r = run(&b, &r3_period(), vec![closing]);
    assert_eq!(
        r.rows[0].cells,
        vec![
            qty(100),
            amount(rs(1000)),
            qty(90),
            amount(rs(1250)),
            qty(150),
            amount(rs(1650)),
            qty(40),
            amount(rs(600)),
            amount(rs(15)),
        ]
    );
}

#[test]
fn r3_with_the_real_valuation_method_as_input() {
    // provenance: derived, the R3 figures of r3_average_cost_item_row: closing 40 x 2,250 / 190 =
    // 473.68 = 47,368 paise; consumption 1,000 + 1,250 - 473.68 = 1,776.32 = 177,632 paise. The
    // closing valuation is produced by ledgerly-stock itself and only used as input; the expected
    // cells are the hand-derived numbers, not read from that output.
    let b = r3_books(ValuationMethod::AverageCost);
    let p = r3_period();
    let item = b.masters.stock_items.get(&StockItemId(1)).unwrap();
    let v = value_item(item, &b.stock_moves, p.period).unwrap();
    let r = run(&b, &p, vec![v]);
    assert_eq!(r.rows[0].cells.get(5), Some(&amount(Paise::new(177_632))));
    assert_eq!(r.rows[0].cells.get(7), Some(&amount(Paise::new(47_368))));
}

#[test]
fn detailed_report_adds_a_godown_row_that_repeats_a_single_godown_item() {
    // provenance: derived, the item lives in one godown only, so the godown row has the same
    // quantities (100, 90, 150, 40) and values as the item row (R3 arithmetic above).
    let b = r3_books(ValuationMethod::AverageCost);
    let mut p = r3_period();
    p.detail = Detail::Detailed;
    let closing = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    let r = run(&b, &p, vec![closing]);
    assert_eq!(r.rows.len(), 2);
    let godown_row = &r.rows[1];
    assert_eq!(godown_row.label, "Main godown");
    assert_eq!(godown_row.depth, 1);
    assert_eq!(godown_row.cells, r.rows[0].cells);
}

#[test]
fn condensed_report_has_item_rows_only() {
    let b = r3_books(ValuationMethod::AverageCost);
    let closing = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    let r = run(&b, &r3_period(), vec![closing]);
    assert!(r.rows.iter().all(|row| row.depth == 0));
}

#[test]
fn two_godowns_show_own_quantities_and_no_invented_values() {
    // provenance: derived. Item opens with 60 in Main and 40 in Annexe at 10 = 1,000.00.
    // 15-Apr purchase of 20 @ 13 = 260.00 into Main; 20-Apr sale of 10 out of Annexe.
    // Item: 100 open, 20 in, 10 out, 110 closing. Periodic average over April:
    // pool (1,000 + 260) / 120 = 10.50; closing 110 x 10.50 = 1,155.00; consumption =
    // 1,000 + 260 - 1,155 = 105.00 (= 10 x 10.50).
    // Main: 60, 20, 0, 80. Annexe: 40, 0, 10, 30. Stock is valued per item (spec 6.4), so a
    // godown's opening, outwards and closing value are Unsupported; its inwards value is the
    // voucher value (260.00 in Main, nothing in Annexe).
    let b = books(
        vec![item(
            1,
            "Widget",
            ValuationMethod::AverageCost,
            &[(1, 60, 10), (2, 40, 10)],
        )],
        vec![
            mv(1, date!(2026 - 04 - 15), 1, 1, 20, 0, rs(260)),
            mv(2, date!(2026 - 04 - 20), 1, 2, 0, 10, rs(150)),
        ],
    );
    let mut p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    p.detail = Detail::Detailed;
    let r = run(
        &b,
        &p,
        // 1,155.00 over 110 units is 10.50 each.
        vec![rated(
            valuation(1, date!(2026 - 04 - 30), 110, rs(1155)),
            Decimal::new(1050, 2),
        )],
    );
    assert_eq!(r.rows.len(), 3);
    assert_eq!(
        r.rows[0].cells,
        vec![
            qty(100),
            amount(rs(1000)),
            qty(20),
            amount(rs(260)),
            qty(10),
            amount(rs(105)),
            qty(110),
            amount(rs(1155)),
            amount(Paise::new(1_050)),
        ]
    );
    let main = row_of(&r, "Main godown");
    assert_eq!(main.cells.first(), Some(&qty(60)));
    assert_eq!(main.cells.get(2), Some(&qty(20)));
    assert_eq!(main.cells.get(3), Some(&amount(rs(260))));
    assert_eq!(main.cells.get(4), Some(&qty(0)));
    assert_eq!(main.cells.get(6), Some(&qty(80)));
    let annexe = row_of(&r, "Annexe");
    assert_eq!(annexe.cells.first(), Some(&qty(40)));
    assert_eq!(annexe.cells.get(2), Some(&qty(0)));
    assert_eq!(annexe.cells.get(3), Some(&amount(Paise::ZERO)));
    assert_eq!(annexe.cells.get(4), Some(&qty(10)));
    assert_eq!(annexe.cells.get(6), Some(&qty(30)));
    for row in [main, annexe] {
        assert!(is_unsupported(&row.cells[1]));
        assert!(is_unsupported(&row.cells[5]));
        assert!(is_unsupported(&row.cells[7]));
    }
}

#[test]
fn provisional_valuation_is_flagged_never_a_plain_number() {
    // provenance: derived. Opening 10 @ 10 = 100.00. 15-Apr sale of 15 takes the running quantity
    // to 10 - 15 = -5 (below zero on 2026-04-15); 20-Apr purchase of 20 @ 12 = 240.00 gives
    // closing 10 - 15 + 20 = 15. The valuation supplied is provisional (negative stock) with
    // value 180.00. Outwards = 100 + 240 - 180 = 160.00, provisional because its input is.
    let mut b = books(
        vec![item(1, "Widget", ValuationMethod::Fifo, &[(1, 10, 10)])],
        vec![
            mv(1, date!(2026 - 04 - 15), 1, 1, 0, 15, rs(300)),
            mv(2, date!(2026 - 04 - 20), 1, 1, 20, 0, rs(240)),
        ],
    );
    name_the_vouchers(&mut b);
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let mut closing = valuation(1, date!(2026 - 04 - 30), 15, rs(180));
    closing.status = ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
        item: StockItemId(1),
        date: date!(2026 - 04 - 15),
    });
    let r = run(&b, &p, vec![closing]);
    // The reason names the item and the vouchers, with dates as the app writes them: the sale
    // SI/0001 (voucher 1) leaves 10 - 15 = -5 nos, and the purchase on 20 Apr leaves
    // -5 + 20 = 15, so "brought it back to 15".
    let reason = "Widget first went below zero on 15 Apr 2026 (\u{2212}5 nos after SI/0001); a purchase on 20 Apr brought it back to 15".to_owned();
    assert_eq!(
        r.rows[0].cells.get(7),
        Some(&Cell::Provisional {
            amount: SignedAmount::dr(rs(180)),
            reason: reason.clone(),
        })
    );
    assert_eq!(
        r.rows[0].cells.get(5),
        Some(&Cell::Provisional {
            amount: SignedAmount::dr(rs(160)),
            reason: reason.clone(),
        })
    );
    assert_eq!(
        r.totals.get(7),
        Some(&Cell::Provisional {
            amount: SignedAmount::dr(rs(180)),
            reason: reason.clone(),
        })
    );
    // The opening value (100.00) is final and stays a plain amount.
    assert_eq!(r.rows[0].cells.get(1), Some(&amount(rs(100))));
    assert!(
        r.notes
            .iter()
            .any(|n| n.contains("Widget") && n.contains(&reason))
    );
}

#[test]
fn exceptions_only_keeps_negative_or_provisional_items() {
    // provenance: derived. Item 1 goes negative (see the provisional test: closing 15, but
    // provisional). Item 2 is plain: opening 10 @ 5 = 50.00, no movement, closing 10 = 50.00.
    let b = books(
        vec![
            item(1, "Widget", ValuationMethod::Fifo, &[(1, 10, 10)]),
            item(2, "Gadget", ValuationMethod::Fifo, &[(1, 10, 5)]),
        ],
        vec![
            mv(1, date!(2026 - 04 - 15), 1, 1, 0, 15, rs(300)),
            mv(2, date!(2026 - 04 - 20), 1, 1, 20, 0, rs(240)),
        ],
    );
    let mut p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let mut widget = valuation(1, date!(2026 - 04 - 30), 15, rs(180));
    widget.status = ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
        item: StockItemId(1),
        date: date!(2026 - 04 - 15),
    });
    let gadget = valuation(2, date!(2026 - 04 - 30), 10, rs(50));
    let all = run(&b, &p, vec![widget.clone(), gadget.clone()]);
    assert_eq!(all.rows.len(), 2);
    p.exceptions_only = true;
    let only = run(&b, &p, vec![widget, gadget]);
    assert_eq!(only.rows.len(), 1);
    assert_eq!(only.rows[0].label, "Widget");
}

#[test]
fn a_period_after_the_first_uses_the_valuation_of_the_day_before() {
    // provenance: derived, R3 FIFO: after the 15-Apr sale 30 @ 13 = 390.00 remain (100 + 50 - 120
    // = 30 units). May: purchase 40 @ 15 = 600.00, sale of 30 (takes the 30 @ 13), closing 40 @ 15
    // = 600.00. Outwards = 390 + 600 - 600 = 390.00.
    let b = r3_books(ValuationMethod::Fifo);
    let p = params(date!(2026 - 05 - 01), date!(2026 - 05 - 31));
    let r = run(
        &b,
        &p,
        vec![
            valuation(1, date!(2026 - 04 - 30), 30, rs(390)),
            rated(valuation(1, date!(2026 - 05 - 31), 40, rs(600)), d(15)),
        ],
    );
    assert_eq!(
        r.rows[0].cells,
        vec![
            qty(30),
            amount(rs(390)),
            qty(40),
            amount(rs(600)),
            qty(30),
            amount(rs(390)),
            qty(40),
            amount(rs(600)),
            amount(rs(15)),
        ]
    );
}

#[test]
fn without_an_opening_valuation_after_earlier_movement_opening_is_unsupported() {
    // provenance: derived quantities as above (30 open); no valuation of 30-Apr is supplied, so
    // the opening value, and the consumption that needs it, are Unsupported; closing is plain.
    let b = r3_books(ValuationMethod::Fifo);
    let p = params(date!(2026 - 05 - 01), date!(2026 - 05 - 31));
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 05 - 31), 40, rs(600))],
    );
    let cells = &r.rows[0].cells;
    assert_eq!(cells.first(), Some(&qty(30)));
    assert!(is_unsupported(&cells[1]));
    assert!(is_unsupported(&cells[5]));
    assert_eq!(cells.get(7), Some(&amount(rs(600))));
}

#[test]
fn missing_closing_valuation_leaves_quantities_and_unsupported_values() {
    // provenance: derived, R3 quantities (100 + 90 - 150 = 40 closing) and opening value
    // 100 x 10 = 1,000.00; no closing valuation is supplied, so closing value and consumption
    // (which needs it) are Unsupported.
    let b = r3_books(ValuationMethod::AverageCost);
    let r = run(&b, &r3_period(), vec![]);
    let cells = &r.rows[0].cells;
    assert_eq!(cells.get(6), Some(&qty(40)));
    assert!(is_unsupported(&cells[7]));
    assert!(is_unsupported(&cells[5]));
    assert_eq!(cells.get(1), Some(&amount(rs(1000))));
    assert!(is_unsupported(&r.totals[7]));
}

#[test]
fn a_valuation_of_other_vouchers_is_unsupported_not_a_number() {
    // provenance: derived. The movements give closing 40; a valuation covering 41 units was run
    // over different vouchers, so its value is not the value of these 40.
    let b = r3_books(ValuationMethod::AverageCost);
    let r = run(
        &b,
        &r3_period(),
        vec![valuation(1, date!(2026 - 05 - 31), 41, rs(486))],
    );
    assert!(is_unsupported(&r.rows[0].cells[7]));
    assert!(is_unsupported(&r.rows[0].cells[5]));
    assert_eq!(r.rows[0].cells.get(6), Some(&qty(40)));
}

#[test]
fn standard_cost_has_no_verified_consumption_rule() {
    // provenance: derived. Opening 10 @ 5, no movement; standard cost 5 gives closing 50.00.
    // Opening stock at its entered rate is not the standard-cost value, so without a valuation of
    // the day before the opening value is Unsupported, and so is consumption.
    let mut std_item = item(1, "Widget", ValuationMethod::StandardCost, &[(1, 10, 5)]);
    std_item.standard_costs = vec![StandardCost {
        from: date!(2026 - 04 - 01),
        rate: Rate(d(5)),
    }];
    let b = books(vec![std_item], vec![]);
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 10, rs(50))],
    );
    let cells = &r.rows[0].cells;
    assert!(is_unsupported(&cells[1]));
    assert!(is_unsupported(&cells[5]));
    assert_eq!(cells.get(7), Some(&amount(rs(50))));
    // With the opening valuation supplied the opening value is shown, consumption stays so.
    let r2 = run(
        &b,
        &p,
        vec![
            valuation(1, date!(2026 - 03 - 31), 10, rs(50)),
            valuation(1, date!(2026 - 04 - 30), 10, rs(50)),
        ],
    );
    assert_eq!(r2.rows[0].cells.get(1), Some(&amount(rs(50))));
    assert!(is_unsupported(&r2.rows[0].cells[5]));
}

#[test]
fn totals_add_values_and_leave_mixed_unit_quantities_empty() {
    // provenance: derived. Item 1 (average, April only): pool 1,000 + 650 = 1,650 over 150 units
    // = 11.00; closing 30 x 11 = 330.00 (research 03, monthly average, April); consumption
    // 1,000 + 650 - 330 = 1,320.00. Item 2 (FIFO): opening 10 @ 5 = 50.00, untouched, closing
    // 50.00, consumption 0. Totals: opening 1,050; inwards 650; outwards 1,320; closing 380.
    let b = books(
        vec![
            item(1, "Widget", ValuationMethod::AverageCost, &[(1, 100, 10)]),
            item(2, "Gadget", ValuationMethod::Fifo, &[(1, 10, 5)]),
        ],
        r3_moves(),
    );
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let vals = vec![
        valuation(1, date!(2026 - 04 - 30), 30, rs(330)),
        valuation(2, date!(2026 - 04 - 30), 10, rs(50)),
    ];
    let r = run(&b, &p, vals.clone());
    assert_eq!(
        r.totals,
        vec![
            Cell::Empty,
            amount(rs(1050)),
            Cell::Empty,
            amount(rs(650)),
            Cell::Empty,
            amount(rs(1320)),
            Cell::Empty,
            amount(rs(380)),
            Cell::Empty,
        ]
    );
    // One unsupported row makes that column's total unsupported, not partial.
    let r2 = run(&b, &p, vals.into_iter().take(1).collect());
    assert!(is_unsupported(&r2.totals[7]));
    assert!(is_unsupported(&r2.totals[5]));
    assert_eq!(r2.totals.get(1), Some(&amount(rs(1050))));
    assert_eq!(r2.totals.get(3), Some(&amount(rs(650))));
}

#[test]
fn an_item_with_no_stock_activity_and_no_valuation_has_no_row() {
    let b = books(
        vec![
            item(1, "Widget", ValuationMethod::AverageCost, &[(1, 100, 10)]),
            item(2, "Unused", ValuationMethod::AverageCost, &[]),
        ],
        vec![],
    );
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 100, rs(1000))],
    );
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0].label, "Widget");
}

#[test]
fn item_filter_restricts_and_unknown_item_is_not_found() {
    let b = books(
        vec![
            item(1, "Widget", ValuationMethod::Fifo, &[(1, 10, 10)]),
            item(2, "Gadget", ValuationMethod::Fifo, &[(1, 10, 5)]),
        ],
        vec![],
    );
    let mut p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let vals = vec![
        valuation(1, date!(2026 - 04 - 30), 10, rs(100)),
        valuation(2, date!(2026 - 04 - 30), 10, rs(50)),
    ];
    p.filter.items = vec![StockItemId(2)];
    let r = run(&b, &p, vals.clone());
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0].label, "Gadget");
    p.filter.items = vec![StockItemId(9)];
    let err = stock_summary(&b, &p, &Outcome::Value(vals)).unwrap_err();
    assert!(matches!(err, ReportError::NotFound { .. }));
}

#[test]
fn optional_and_cancelled_vouchers_count_only_when_the_rules_say_so() {
    // provenance: derived. Voucher 5 is an optional purchase of 10 on 25-May, voucher 4 (the
    // 31-May sale of 30) is cancelled. Actual scenario: neither counts, so the quantities are the
    // R3 ones without the sale: in 90, out 120 (15-Apr only), closing 100 + 90 - 120 = 70.
    // Including Optional adds 10 in: in 100, closing 80.
    let mut b = r3_books(ValuationMethod::AverageCost);
    b.stock_moves
        .push(mv(5, date!(2026 - 05 - 25), 1, 1, 10, 0, rs(150)));
    b.stock_moves.sort_by_key(|m| (m.date, m.voucher));
    b.vouchers
        .insert(VoucherId(5), voucher(5, date!(2026 - 05 - 25)));
    b.vouchers.get_mut(&VoucherId(5)).unwrap().status = VoucherStatus::Optional;
    b.vouchers.get_mut(&VoucherId(4)).unwrap().status = VoucherStatus::Cancelled;
    let p = r3_period();
    let r = run(&b, &p, vec![]);
    assert_eq!(r.rows[0].cells.get(2), Some(&qty(90)));
    assert_eq!(r.rows[0].cells.get(4), Some(&qty(120)));
    assert_eq!(r.rows[0].cells.get(6), Some(&qty(70)));
    let mut with_optional = p;
    with_optional.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let r2 = run(&b, &with_optional, vec![]);
    assert_eq!(r2.rows[0].cells.get(2), Some(&qty(100)));
    assert_eq!(r2.rows[0].cells.get(6), Some(&qty(80)));
}

#[test]
fn unsupported_inputs_are_reported_not_guessed() {
    let b = r3_books(ValuationMethod::AverageCost);
    let closing = Outcome::Value(vec![valuation(
        1,
        date!(2026 - 05 - 31),
        40,
        Paise::new(47_368),
    )]);
    let mut comparative = r3_period();
    comparative.columns = ColumnSpec::Monthly;
    assert!(
        stock_summary(&b, &comparative, &closing)
            .unwrap()
            .is_unsupported()
    );
    let mut by_ledger = r3_period();
    by_ledger.filter.ledgers = vec![ledgerly_core::LedgerId(1)];
    assert!(
        stock_summary(&b, &by_ledger, &closing)
            .unwrap()
            .is_unsupported()
    );
    let mut cancelled = r3_period();
    cancelled.scenario = Scenario::Including(vec![VoucherStatus::Cancelled]);
    assert!(
        stock_summary(&b, &cancelled, &closing)
            .unwrap()
            .is_unsupported()
    );
    let no_valuation: Outcome<Vec<StockValuation>> =
        Outcome::unsupported("average cost", "negative closing quantity");
    assert!(
        stock_summary(&b, &r3_period(), &no_valuation)
            .unwrap()
            .is_unsupported()
    );
}

#[test]
fn bad_parameters_and_valuations_are_errors() {
    let b = r3_books(ValuationMethod::AverageCost);
    let ok = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    let invalid = |r: Result<Outcome<Report>, ReportError>| {
        assert!(matches!(r.unwrap_err(), ReportError::Invalid { .. }));
    };
    let mut wrong_kind = r3_period();
    wrong_kind.kind = ReportKind::TrialBalance;
    invalid(stock_summary(
        &b,
        &wrong_kind,
        &Outcome::Value(vec![ok.clone()]),
    ));
    let before_books = params(date!(2026 - 03 - 01), date!(2026 - 05 - 31));
    invalid(stock_summary(
        &b,
        &before_books,
        &Outcome::Value(vec![ok.clone()]),
    ));
    let misdated = valuation(1, date!(2026 - 05 - 15), 40, Paise::new(47_368));
    invalid(stock_summary(
        &b,
        &r3_period(),
        &Outcome::Value(vec![misdated]),
    ));
    invalid(stock_summary(
        &b,
        &r3_period(),
        &Outcome::Value(vec![ok.clone(), ok.clone()]),
    ));
    let stranger = valuation(9, date!(2026 - 05 - 31), 1, rs(1));
    let err = stock_summary(&b, &r3_period(), &Outcome::Value(vec![stranger])).unwrap_err();
    assert!(matches!(err, ReportError::NotFound { .. }));
}

#[test]
fn malformed_movements_are_errors() {
    let ok = valuation(1, date!(2026 - 04 - 30), 100, rs(1000));
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let case = |bad: StockMove| {
        let b = books(
            vec![item(
                1,
                "Widget",
                ValuationMethod::AverageCost,
                &[(1, 100, 10)],
            )],
            vec![bad],
        );
        stock_summary(&b, &p, &Outcome::Value(vec![ok.clone()])).unwrap_err()
    };
    // Before the books begin (1-Apr-2026).
    let e = case(mv(1, date!(2026 - 03 - 31), 1, 1, 1, 0, rs(1)));
    assert!(matches!(e, ReportError::Invalid { .. }));
    // Both receives and issues.
    let e = case(mv(1, date!(2026 - 04 - 10), 1, 1, 1, 1, rs(1)));
    assert!(matches!(e, ReportError::Invalid { .. }));
    // Value without quantity.
    let e = case(mv(1, date!(2026 - 04 - 10), 1, 1, 0, 0, rs(1)));
    assert!(matches!(e, ReportError::Invalid { .. }));
    // Negative quantity.
    let e = case(mv(1, date!(2026 - 04 - 10), 1, 1, -1, 0, rs(1)));
    assert!(matches!(e, ReportError::Invalid { .. }));
    // Unknown godown and unknown item.
    let e = case(mv(1, date!(2026 - 04 - 10), 1, 7, 1, 0, rs(1)));
    assert!(matches!(e, ReportError::NotFound { .. }));
    let e = case(mv(1, date!(2026 - 04 - 10), 8, 1, 1, 0, rs(1)));
    assert!(matches!(e, ReportError::NotFound { .. }));
}

#[test]
fn a_movement_of_an_unknown_voucher_is_not_found() {
    let mut b = r3_books(ValuationMethod::AverageCost);
    b.vouchers.remove(&VoucherId(1));
    let closing = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    let err = stock_summary(&b, &r3_period(), &Outcome::Value(vec![closing])).unwrap_err();
    assert!(matches!(err, ReportError::NotFound { .. }));
}

#[test]
fn movements_after_the_period_end_are_ignored() {
    // provenance: derived. Period ends 30-Apr: the May purchase and sale do not count. Quantities:
    // 100 open, 50 in, 120 out, 30 closing.
    let b = r3_books(ValuationMethod::AverageCost);
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 30, rs(330))],
    );
    assert_eq!(r.rows[0].cells.get(2), Some(&qty(50)));
    assert_eq!(r.rows[0].cells.get(4), Some(&qty(120)));
    assert_eq!(r.rows[0].cells.get(6), Some(&qty(30)));
}

#[test]
fn a_godown_transfer_makes_item_consumption_unsupported() {
    // provenance: derived. Opening 100 @ 10 in Main. On 10-Apr one voucher issues 30 from Main and
    // receives 30 into Annexe (a transfer, each side valued at 300.00). Item level: 30 in, 30 out,
    // closing 100 + 30 - 30 = 100. Main: 100 open, 30 out, 70 closing. Annexe: 0 open, 30 in, 30
    // closing. The transfer is not consumption, so the item's outwards value is Unsupported
    // (opening + inwards - closing would give 300.00, which is wrong).
    let b = books(
        vec![item(
            1,
            "Widget",
            ValuationMethod::AverageCost,
            &[(1, 100, 10)],
        )],
        vec![
            mv(1, date!(2026 - 04 - 10), 1, 1, 0, 30, rs(300)),
            mv(1, date!(2026 - 04 - 10), 1, 2, 30, 0, rs(300)),
        ],
    );
    let mut p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    p.detail = Detail::Detailed;
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 100, rs(1300))],
    );
    let cells = &r.rows[0].cells;
    assert_eq!(cells.get(2), Some(&qty(30)));
    assert_eq!(cells.get(4), Some(&qty(30)));
    assert_eq!(cells.get(6), Some(&qty(100)));
    assert!(is_unsupported(&cells[5]));
    let main = row_of(&r, "Main godown");
    assert_eq!(main.cells.first(), Some(&qty(100)));
    assert_eq!(main.cells.get(4), Some(&qty(30)));
    assert_eq!(main.cells.get(6), Some(&qty(70)));
    let annexe = row_of(&r, "Annexe");
    assert_eq!(annexe.cells.first(), Some(&qty(0)));
    assert_eq!(annexe.cells.get(2), Some(&qty(30)));
    assert_eq!(annexe.cells.get(6), Some(&qty(30)));
}

#[test]
fn a_godown_transfer_makes_inwards_values_unsupported() {
    // provenance: derived. The movements of the transfer test above: on 10-Apr 30 leave Main and
    // 30 arrive in Annexe, each side valued at 300.00. The 300.00 on the receiving side is a
    // transfer value, not a purchase value, so the inwards value is Unsupported at item level and
    // in both godown rows. The quantities are unchanged: item inwards 30.
    let b = books(
        vec![item(
            1,
            "Widget",
            ValuationMethod::AverageCost,
            &[(1, 100, 10)],
        )],
        vec![
            mv(1, date!(2026 - 04 - 10), 1, 1, 0, 30, rs(300)),
            mv(1, date!(2026 - 04 - 10), 1, 2, 30, 0, rs(300)),
        ],
    );
    let mut p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    p.detail = Detail::Detailed;
    let r = run(
        &b,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 100, rs(1300))],
    );
    let cells = &r.rows[0].cells;
    assert_eq!(cells.get(2), Some(&qty(30)));
    assert!(is_unsupported(&cells[3]));
    assert!(is_unsupported(&row_of(&r, "Main godown").cells[3]));
    assert!(is_unsupported(&row_of(&r, "Annexe").cells[3]));
}

#[test]
fn the_closing_rate_is_the_valuations_rate_rounded_to_the_paisa() {
    // provenance: derived, R3 average cost. Pool 2,250.00 over 190 units gives the rate
    // 2,250 / 190 = 11.842105..., shown to the paisa as 11.84; closing 40 x that rate = 473.68
    // (research 03). The rate is the engine's (the valuation's own `rate`); the report only
    // rounds it half up to the paisa. It is the ninth cell of the row and has no total.
    let b = r3_books(ValuationMethod::AverageCost);
    let mut closing = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    closing.rate = Some(Rate(Decimal::from(2250) / Decimal::from(190)));
    let r = run(&b, &r3_period(), vec![closing]);
    assert_eq!(
        r.columns.last().map(|c| c.label.as_str()),
        Some("Closing rate")
    );
    assert_eq!(r.rows[0].cells.get(8), Some(&amount(Paise::new(1_184))));
    assert_eq!(r.totals.get(8), Some(&Cell::Empty));
}

#[test]
fn stock_on_hand_with_no_rate_is_unsupported_and_no_stock_has_no_rate() {
    // provenance: derived. Stock on hand whose valuation gives no rate cannot be stated, so the
    // cell is Unsupported (never a guess). Opening 10, all 10 sold on 15 Apr: closing quantity
    // 10 - 10 = 0, nothing on hand has no rate to show, so the cell is Empty.
    let b = r3_books(ValuationMethod::AverageCost);
    let on_hand = valuation(1, date!(2026 - 05 - 31), 40, Paise::new(47_368));
    let r = run(&b, &r3_period(), vec![on_hand]);
    assert!(is_unsupported(r.rows[0].cells.get(8).unwrap()));
    let sold = books(
        vec![item(
            1,
            "Widget",
            ValuationMethod::AverageCost,
            &[(1, 10, 10)],
        )],
        vec![mv(1, date!(2026 - 04 - 15), 1, 1, 0, 10, rs(150))],
    );
    let p = params(date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = run(
        &sold,
        &p,
        vec![valuation(1, date!(2026 - 04 - 30), 0, rs(0))],
    );
    assert_eq!(r.rows[0].cells.get(8), Some(&Cell::Empty));
}
