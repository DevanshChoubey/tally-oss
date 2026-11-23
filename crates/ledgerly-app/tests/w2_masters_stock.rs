//! The Stock and value columns of the masters item list, and its "Closing stock" line, come from
//! the engine's stock summary, run on the API worker (`design/mock-v2/index.html:1798-1800`:
//! `Stock`, `Value at average cost ${prov('eng','stock engine')}`, each row
//! `${sp.qty[i.id]} ${i.unit}` and `inr(..)`, then `<tr class="total">Closing stock`).
//!
//! Provenance: every expected figure is `derived` by hand from the opening stock created below,
//! with the arithmetic next to it. The figures are the stock summary's closing quantity and
//! closing value (the summary's own provenance is in `ledgerly-reports/tests/rp9_stock_summary.rs`)
//! over the period the stock summary opens with: the financial year to its last voucher.
//!
//! The screen joins the engine's rows to items by id only. An item the stock summary has no row
//! for (a stock summary filtered to some items, say) shows nothing; a cell the engine cannot compute shows a dash; the total line is the
//! report's own total, never a sum made by the screen (rule C4).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{
    Api, ApiError, Cell, ColumnHeader, ColumnSpec, Command, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_app::screens::masters::MastersScreen;
use ledgerly_app::screens::masters::stock::{
    ItemStock, ItemStockFigures, QTY_HEADING, StockFigures, StockState, StockValue, VALUE_HEADING,
    from_report,
};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, Outcome, Paise, Period, Qty, Rate, SignedAmount, StateCode,
    StockItemId, UnitId, Unsupported,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::{
    Address, Company, EntityKind, Features, NegativeStockPolicy, OpeningStock, Scenario, StockItem,
    Unit, UnitKind, ValuationMethod,
};
use time::OffsetDateTime;
use time::macros::date;

const NOS: UnitId = UnitId(1);
const BOLT: StockItemId = StockItemId(1);
const NUT: StockItemId = StockItemId(2);
const WASHER: StockItemId = StockItemId(3);

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn unit() -> Unit {
    Unit {
        id: NOS,
        symbol: "Nos".to_owned(),
        formal_name: "Numbers".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    }
}

fn item(id: StockItemId, name: &str, opening: Option<(i64, Decimal)>) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: NOS,
        alt_unit: None,
        hsn: None,
        gst_history: Vec::new(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: opening
            .map(|(qty, rate)| OpeningStock {
                godown: MAIN_LOCATION,
                batch: None,
                qty: Qty(Decimal::from(qty)),
                rate: Rate(rate),
            })
            .into_iter()
            .collect(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn committed(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn setup(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    committed(api.execute(Command::CreateUnit(unit())));
    for it in [
        item(BOLT, "Bolt", Some((10, Decimal::from(50)))),
        item(NUT, "Nut", Some((4, Decimal::new(1250, 2)))),
        item(WASHER, "Washer", None),
    ] {
        committed(api.execute(Command::CreateStockItem(it)));
    }
    api
}

fn year() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn loaded(worker: &ApiWorker, period: Period) -> ItemStockFigures {
    let mut figures = ItemStockFigures::new();
    figures.request(period);
    for _ in 0..2000 {
        figures.tick(Some(worker));
        if !figures.is_loading() {
            return figures;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the stock figures did not arrive within the bound");
}

#[test]
fn each_item_shows_its_closing_stock_and_value_from_the_engine() {
    // provenance: derived. Opening stock at Main Location, valued at average cost, and no
    // vouchers, so the closing stock is the opening stock and the average cost is the opening
    // rate:
    // Bolt: 10 Nos at 50.00 = 500.00. Nut: 4 Nos at 12.50 = 50.00.
    // Washer: no opening and no movement, so it holds 0 Nos worth 0.00. It still has a row: the
    // API supplies a valuation of every item (ledgerly-api report.rs `valuations_at`), and the
    // stock summary leaves out only an item with no opening, no movement and no valuation.
    // Closing stock: 500.00 + 50.00 = 550.00, the stock summary's own total.
    let worker = ApiWorker::spawn(setup("w2_masters_stock.ledgerly")).unwrap();
    let f = loaded(&worker, year());
    assert_eq!(f.qty_text(BOLT, "Nos"), "10 Nos");
    assert_eq!(f.value_text(BOLT), "500.00");
    assert_eq!(f.qty_text(NUT, "Nos"), "4 Nos");
    assert_eq!(f.value_text(NUT), "50.00");
    assert_eq!(f.qty_text(WASHER, "Nos"), "0 Nos");
    assert_eq!(f.value_text(WASHER), "0.00");
    assert_eq!(f.total_text().as_deref(), Some("550.00"));
    assert_eq!(f.heading_hint(), None);
}

fn report(rows: Vec<ReportRow>, total: Cell) -> Report {
    let headings = [
        "Opening quantity",
        "Opening value",
        "Inwards quantity",
        "Inwards value (at voucher rate)",
        "Outwards quantity",
        "Outwards value (cost of consumption)",
        QTY_HEADING,
        VALUE_HEADING,
    ];
    let mut totals = vec![Cell::Empty; 7];
    totals.push(total);
    Report {
        title: "Stock summary".to_owned(),
        params: ReportParams {
            kind: ReportKind::StockSummary,
            period: year(),
            as_of: year().to(),
            scenario: Scenario::Actual,
            detail: Detail::Condensed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        },
        columns: headings
            .iter()
            .map(|label| ColumnHeader {
                label: (*label).to_owned(),
                period: None,
            })
            .collect(),
        rows,
        totals,
        notes: Vec::new(),
    }
}

fn row(key: Option<DrillKey>, kind: RowKind, qty: Cell, value: Cell) -> ReportRow {
    let mut cells = vec![Cell::Empty; 6];
    cells.push(qty);
    cells.push(value);
    ReportRow {
        key,
        label: String::new(),
        depth: 0,
        kind,
        cells,
    }
}

fn qty(n: i64) -> Cell {
    Cell::Qty(Qty(Decimal::from(n)))
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn nope(what: &str) -> Unsupported {
    Unsupported {
        what: what.to_owned(),
        reason: "for the test".to_owned(),
    }
}

#[test]
fn rows_are_joined_to_items_by_id_and_other_rows_are_skipped() {
    // provenance: derived. A group row and a row with no key belong to no item and are skipped;
    // the item rows give their closing quantity and value columns, read by their headings.
    let r = report(
        vec![
            row(
                None,
                RowKind::Group,
                qty(99),
                Cell::Amount(SignedAmount::dr(rs(9))),
            ),
            row(
                Some(DrillKey::Item(BOLT)),
                RowKind::Item,
                qty(3),
                Cell::Amount(SignedAmount::dr(rs(30))),
            ),
            row(None, RowKind::Item, qty(7), Cell::Empty),
        ],
        Cell::Amount(SignedAmount::dr(rs(30))),
    );
    let mut items = BTreeMap::new();
    items.insert(
        BOLT,
        ItemStock {
            qty: Ok(Qty(Decimal::from(3))),
            value: StockValue::Final(SignedAmount::dr(rs(30))),
        },
    );
    assert_eq!(
        from_report(&r),
        StockState::Loaded(StockFigures {
            items,
            total: StockValue::Final(SignedAmount::dr(rs(30))),
        })
    );
}

#[test]
fn a_negative_or_provisional_value_is_never_shown_as_plain_stock_in_hand() {
    // provenance: derived (rule A1). Stock in hand is a debit value and shows as the bare figure,
    // as the mock's `inr(..)` does. A credit value (stock below zero) keeps its side, and a value
    // the engine marks provisional (negative stock) says so; neither is shown as a plain figure.
    // A quantity below zero shows with its sign, as the engine gives it.
    let r = report(
        vec![
            row(
                Some(DrillKey::Item(BOLT)),
                RowKind::Item,
                qty(-2),
                Cell::Amount(SignedAmount::cr(rs(100))),
            ),
            row(
                Some(DrillKey::Item(NUT)),
                RowKind::Item,
                qty(1),
                Cell::Provisional {
                    amount: SignedAmount::dr(rs(125_000)),
                    reason: "negative stock".to_owned(),
                },
            ),
        ],
        Cell::Provisional {
            amount: SignedAmount::dr(rs(124_900)),
            reason: "negative stock".to_owned(),
        },
    );
    let mut f = ItemStockFigures::new();
    f.set(from_report(&r));
    assert_eq!(f.qty_text(BOLT, "Nos"), "-2 Nos");
    assert_eq!(f.value_text(BOLT), "100.00 Cr");
    assert_eq!(f.value_text(NUT), "1,25,000.00 (provisional)");
    assert_eq!(f.total_text().as_deref(), Some("1,24,900.00 (provisional)"));
}

#[test]
fn a_cell_the_engine_cannot_compute_shows_a_dash() {
    // provenance: derived (rule A1). An unsupported cell, or a cell of the wrong kind, has no
    // figure, so the cell shows a dash rather than a number.
    let r = report(
        vec![
            row(
                Some(DrillKey::Item(BOLT)),
                RowKind::Item,
                Cell::Unsupported(nope("closing quantity")),
                Cell::Unsupported(nope("closing value")),
            ),
            row(
                Some(DrillKey::Item(NUT)),
                RowKind::Item,
                Cell::Text("4".to_owned()),
                Cell::Empty,
            ),
        ],
        Cell::Unsupported(nope("total")),
    );
    let mut f = ItemStockFigures::new();
    f.set(from_report(&r));
    for id in [BOLT, NUT] {
        assert_eq!(f.qty_text(id, "Nos"), "\u{2014}");
        assert_eq!(f.value_text(id), "\u{2014}");
    }
    assert_eq!(f.total_text().as_deref(), Some("\u{2014}"));
}

#[test]
fn a_report_without_the_closing_columns_gives_no_figures() {
    // provenance: derived. The figures are read by the stock summary's headings; a report whose
    // headings are not those is not read by position.
    let mut r = report(Vec::new(), Cell::Empty);
    r.columns.clear();
    assert!(matches!(from_report(&r), StockState::Unsupported(_)));
}

#[test]
fn what_the_columns_show_in_each_state() {
    // provenance: derived. Before the answer, and when the engine cannot answer, every cell shows
    // a dash, the headings say why on hover, and there is no total line.
    let mut f = ItemStockFigures::new();
    assert_eq!(f.qty_text(BOLT, "Nos"), "\u{2014}");
    assert_eq!(f.value_text(BOLT), "\u{2014}");
    assert_eq!(f.total_text(), None);
    assert!(f.heading_hint().is_some());
    assert_eq!(f.notice(), None);
    f.set(StockState::Unsupported("the books are locked".to_owned()));
    assert_eq!(f.value_text(BOLT), "\u{2014}");
    assert_eq!(f.total_text(), None);
    assert!(f.heading_hint().is_some());
    assert_eq!(f.notice(), Some("the books are locked"));
}

#[test]
fn without_a_worker_nothing_is_asked_and_nothing_is_shown() {
    let mut f = ItemStockFigures::new();
    f.request(year());
    f.tick(None);
    assert!(f.is_loading());
    assert_eq!(f.state(), &StockState::NotAsked);
}

fn frame(ctx: &egui::Context, screen: &mut MastersScreen, app: &mut AppCtx) {
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn the_screen_asks_on_open_and_shows_the_engines_figures() {
    // provenance: derived, as in the first test: Bolt 10 Nos and 500.00, total 550.00. The
    // screen asks over the stock summary's opening period; with no vouchers that is the whole
    // financial year.
    let api = setup("w2_masters_stock_screen.ledgerly");
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    let mut app = AppCtx {
        books: Some(books),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let mut screen = MastersScreen::new();
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        frame(&ctx, &mut screen, &mut app);
        if matches!(screen.stock().state(), StockState::Loaded(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(screen.stock().qty_text(BOLT, "Nos"), "10 Nos");
    assert_eq!(screen.stock().value_text(BOLT), "500.00");
    assert_eq!(screen.stock().total_text().as_deref(), Some("550.00"));
    assert_eq!(screen.stock_note(), None);
}

#[test]
fn when_the_engine_cannot_give_the_figures_the_list_says_why() {
    let mut screen = MastersScreen::new();
    screen.stock_mut().set(StockState::Unsupported(
        "the stock summary failed".to_owned(),
    ));
    let note = screen.stock_note().unwrap();
    assert!(note.contains("the stock summary failed"), "{note}");
    assert!(note.contains("Nothing is estimated"), "{note}");
}
