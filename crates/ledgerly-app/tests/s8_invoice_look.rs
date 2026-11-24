//! Voucher sheet look (round S8): the invoice screen's layout against the mock
//! (`design/mock-v2/index.html`, `sales-invoice`, lines 1275-1345, and the `.vsheet`, `.vhead`,
//! `table.lines`, `.vfoot`, `.lockbar` and `.ta` rules, lines 233-271).
//!
//! Provenance: the mock's CSS. The head is a grid of four equal columns with a 16 point gap
//! (`.vhead{grid-template-columns:repeat(4,minmax(0,1fr));gap:12px 16px}`, line 234), the party
//! spans two (`.fld.span2`, line 240), the line table runs # Item HSN Qty Unit Rate GST Amount
//! (line 1283), the lock bar sits inside the card above the head (line 1285, `.lockbar` line 260)
//! and the type-ahead is `position:fixed` under its input (`.ta`, line 263), so it never moves
//! the fields below it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::item_invoice::ItemInvoiceScreen;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Date, FinancialYear, GroupId, Gstin, LedgerId, Outcome, Paise, SignedAmount, StateCode,
    StockItemId, UnitId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, StockItem, Unit, UnitKind, ValuationMethod, VoucherType,
};
use time::macros::date;

const SHARMA: LedgerId = LedgerId(1);
const SALES: LedgerId = LedgerId(2);
const CGST: LedgerId = LedgerId(3);
const SGST: LedgerId = LedgerId(4);
const WIDGET: StockItemId = StockItemId(1);
const NOS: UnitId = UnitId(1);
const T_SALES: VoucherTypeId = VoucherTypeId(1);
/// The voucher date is the screen's default; the lock date below is set to the same day.
const TODAY: Date = date!(2026 - 11 - 21);

fn fy() -> FinancialYear {
    FinancialYear::april(2026).unwrap()
}

fn ledger(id: LedgerId, name: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one fixture: the company, its ledgers and its stock item in one place"
)]
fn books() -> Books {
    let mut masters = Masters::default();
    let mut sharma = ledger(SHARMA, "Sharma Traders");
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        // A valid GSTIN from ledgerly-core's own tested vectors (gstin.rs `VALID`).
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    });
    for l in [
        sharma,
        ledger(SALES, "Sales - Local"),
        ledger(CGST, "Output CGST"),
        ledger(SGST, "Output SGST"),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    let unit = Unit {
        id: NOS,
        symbol: "Nos".to_owned(),
        formal_name: "Nos".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    };
    masters.units.insert(unit.id, unit);
    let item = StockItem {
        ineligible_itc: None,
        id: WIDGET,
        name: "Widget Large".to_owned(),
        aliases: vec!["WL".to_owned()],
        group: None,
        unit: NOS,
        alt_unit: None,
        hsn: None,
        gst_history: Vec::new(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    };
    masters.stock_items.insert(item.id, item);
    let sales = VoucherType {
        id: T_SALES,
        name: "Sales".to_owned(),
        base: BaseVoucherKind::Sales,
        abbreviation: "Sal".to_owned(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    };
    masters.voucher_types.insert(sales.id, sales);
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
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
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn open(b: &Books) -> ItemInvoiceScreen {
    match ItemInvoiceScreen::open(b, T_SALES, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

/// Draws the screen `frames` times in a 1280 by 800 window.
fn draw(s: &mut ItemInvoiceScreen, b: &Books, lock: Option<Date>, frames: usize) {
    let mut app = AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: lock,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    }
}

fn rect(s: &ItemInvoiceScreen, name: &str) -> Rect {
    s.field_rect(name)
        .unwrap_or_else(|| panic!("the field {name:?} was not drawn"))
}

#[test]
fn the_party_list_floats_under_its_field_and_moves_nothing() {
    // provenance: index.html:263, `.ta{position:fixed}`: the list is out of the flow, so the
    // fields under the party sit where they sit with the list closed.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, None, 3);
    let party = rect(&s, "Customer");
    let ledger_open = rect(&s, "Sales ledger");
    let list = s
        .picker_rect()
        .expect("the party list is open on an empty party");
    assert!(
        list.min.y >= party.max.y,
        "the list {list:?} starts under the party field {party:?}"
    );
    assert!(
        list.max.y > ledger_open.min.y,
        "the list {list:?} floats over the field under it {ledger_open:?}"
    );
    s.set_party_text("sharma");
    s.submit(&b.masters);
    draw(&mut s, &b, None, 3);
    assert!(
        s.picker_rect().is_none(),
        "no list once the party is chosen"
    );
    let ledger_closed = rect(&s, "Sales ledger");
    assert!(
        (ledger_open.min.y - ledger_closed.min.y).abs() < 0.5,
        "Sales ledger moved from {ledger_open:?} to {ledger_closed:?} when the list closed"
    );
}

#[test]
fn the_head_is_four_columns_with_the_party_across_two() {
    // provenance: index.html:234 and 240. Four equal columns, 16 points apart; the party spans
    // two. Row one: Date, Party, Place of supply. Row two: GSTIN, Sales ledger, Buyer's order no.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, None, 3);
    let (date, party, place) = (
        rect(&s, "Date"),
        rect(&s, "Customer"),
        rect(&s, "Place of supply"),
    );
    let (gstin, ledger, order) = (
        rect(&s, "Party GSTIN"),
        rect(&s, "Sales ledger"),
        rect(&s, "Order or reference"),
    );
    let unit = date.width();
    assert!(
        (party.width() - (2.0 * unit + 16.0)).abs() < 1.0,
        "party {party:?} spans two of {unit}"
    );
    assert!(
        (party.min.x - (date.max.x + 16.0)).abs() < 1.0,
        "party follows date"
    );
    assert!(
        (place.min.x - (party.max.x + 16.0)).abs() < 1.0,
        "place follows party"
    );
    assert!((place.width() - unit).abs() < 1.0, "place is one column");
    assert!((date.min.y - party.min.y).abs() < 0.5 && (date.min.y - place.min.y).abs() < 0.5);
    assert!((gstin.min.x - date.min.x).abs() < 0.5, "GSTIN under Date");
    assert!(
        (ledger.min.x - party.min.x).abs() < 0.5,
        "Sales ledger under the party's first column"
    );
    assert!(
        (order.min.x - (ledger.max.x + 16.0)).abs() < 1.0,
        "order no. beside Sales ledger"
    );
    assert!(gstin.min.y > date.max.y && (gstin.min.y - ledger.min.y).abs() < 0.5);
}

#[test]
fn the_line_table_runs_as_the_mock_has_it() {
    // provenance: index.html:1283. The mock's headings in its order; ours adds Disc % and Typed
    // amount (live inputs the mock has no column for) between Rate and GST.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, None, 3);
    let heads = s.heading_rects();
    let names: Vec<&str> = heads.iter().map(|(n, _)| *n).collect();
    let mock = ["#", "Item", "HSN", "Qty", "Unit", "Rate", "GST", "Amount"];
    let ours: Vec<&str> = names.iter().copied().filter(|n| mock.contains(n)).collect();
    assert_eq!(ours, mock, "headings were {names:?}");
    assert_eq!(names.last(), Some(&"Amount"), "Amount closes the line");
    for pair in heads.windows(2) {
        assert!(pair[0].1.min.x < pair[1].1.min.x, "{pair:?} out of order");
        assert!(
            (pair[0].1.min.y - pair[1].1.min.y).abs() < 0.5,
            "{pair:?} on one line"
        );
    }
    let sheet = s.sheet_rect().expect("the sheet is drawn");
    let (first, last) = (heads[0].1, heads[heads.len() - 1].1);
    assert!(
        (first.min.x - sheet.min.x).abs() < 2.0,
        "the table starts at the card's edge"
    );
    assert!(
        (last.max.x - sheet.max.x).abs() < 2.0,
        "the table ends at the card's edge"
    );
}

#[test]
fn a_locked_date_puts_a_bar_across_the_top_of_the_sheet() {
    // provenance: index.html:1285. `.lockbar` is the first child of the card, full width, above
    // the head's Date.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, Some(TODAY), 3);
    let bar = s.lock_bar_rect().expect("the lock bar is drawn");
    let sheet = s.sheet_rect().expect("the sheet is drawn");
    let date = rect(&s, "Date");
    assert!((bar.min.x - sheet.min.x).abs() < 2.0 && (bar.max.x - sheet.max.x).abs() < 2.0);
    assert!(
        (bar.min.y - sheet.min.y).abs() < 2.0,
        "bar {bar:?} opens the sheet {sheet:?}"
    );
    assert!(
        bar.max.y <= date.min.y,
        "bar {bar:?} is above the date {date:?}"
    );
    let mut open_books = open(&b);
    draw(&mut open_books, &b, None, 3);
    assert!(
        open_books.lock_bar_rect().is_none(),
        "no bar when the date is open"
    );
}
