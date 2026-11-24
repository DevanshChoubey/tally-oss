//! Card S7-A11Y: what a screen reader is told on the shell, the home screen, a report and the
//! sales invoice. Each test reads the AccessKit tree of a real frame and checks roles and names.
//! Expected names are written from the labels the screen draws and from the amounts the fixture
//! holds (derived: `5,00,000.00` is `50_000_000` paise in Indian grouping), never from this crate's
//! output.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::accesskit::{Node, Role, Toggled, TreeUpdate};
use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::screens::home::{HomeData, HomeScreen, HomeTarget, Kpi};
use ledgerly_app::screens::item_invoice::ItemInvoiceScreen;
use ledgerly_app::screens::report_view::{
    ReportSource, ReportToken, ReportViewScreen, ViewCell, ViewDetail, ViewError, ViewKey,
    ViewParams, ViewReport, ViewRow, ViewRowKind,
};
use ledgerly_app::shell::Shell;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Outcome, Paise, Period, SignedAmount};
use std::collections::BTreeMap;
use time::macros::date;

use ledgerly_core::{
    Date, FinancialYear, GroupId, Gstin, LedgerId, StateCode, StockItemId, UnitId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, StockItem, Unit, UnitKind, ValuationMethod, VoucherType,
};

// ---- harness ---------------------------------------------------------------------------------

/// Runs `frames` frames of `body` with AccessKit on, and returns the last frame's tree.
fn tree(ctx: &Context, frames: usize, mut body: impl FnMut(&mut Ui)) -> TreeUpdate {
    let mut last = None;
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(input, &mut body);
        last = out.platform_output.accesskit_update.take();
        out.drop_without_applying_deltas();
    }
    last.expect("AccessKit is enabled")
}

fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

fn nodes(update: &TreeUpdate) -> impl Iterator<Item = &Node> {
    update.nodes.iter().map(|(_, node)| node)
}

fn named<'a>(update: &'a TreeUpdate, role: Role, label: &str) -> Option<&'a Node> {
    nodes(update).find(|node| node.role() == role && node.label() == Some(label))
}

/// The node of `role` whose name starts with `prefix`, for names that carry a figure.
fn named_starting<'a>(update: &'a TreeUpdate, role: Role, prefix: &str) -> Option<&'a Node> {
    nodes(update).find(|node| {
        node.role() == role && node.label().is_some_and(|label| label.starts_with(prefix))
    })
}

fn kpi(label: &str, value: Outcome<Paise>, footnote: &str, target: HomeTarget) -> Kpi {
    Kpi {
        label: label.to_owned(),
        value,
        footnote: footnote.to_owned(),
        target,
    }
}

/// Two tiles of the home screen with figures: receivables 45,000.00 and payables 8,000.00.
fn home_data() -> HomeData {
    HomeData {
        company_name: "Fictional Traders".to_owned(),
        subtitle: "Figures as of 30 Apr 2026".to_owned(),
        kpis: vec![
            kpi(
                "Receivables",
                Outcome::Value(Paise::new(4_500_000)),
                "Closing balance as of 30 Apr 2026",
                HomeTarget::BalanceSheet {
                    node: "Sundry debtors".to_owned(),
                },
            ),
            kpi(
                "Payables",
                Outcome::Value(Paise::new(800_000)),
                "Closing balance as of 30 Apr 2026",
                HomeTarget::BalanceSheet {
                    node: "Sundry creditors".to_owned(),
                },
            ),
        ],
        attention: Vec::new(),
    }
}

// ---- the shell -------------------------------------------------------------------------------

#[test]
fn the_shell_names_its_sidebar_entries_and_marks_the_open_one() {
    // provenance: the sidebar rows are the labels of the mock's `nav.side` (index.html, the
    // Books group: "Day book"). The row of the screen on top is pressed.
    let ctx = context();
    let mut shell = Shell::new(Box::new(HomeScreen::new(home_data())));
    let update = tree(&ctx, 2, |ui| shell.show(ui));
    assert!(named(&update, Role::Button, "Day book").is_some());
    let home = named(&update, Role::Button, "Home").expect("a sidebar button named Home");
    assert_eq!(home.toggled(), Some(Toggled::True));
}

#[test]
fn the_shell_chips_read_their_words() {
    // provenance: index.html:900 search chip "Search, jump, ask or type an entry" and
    // index.html:902 the theme switch, named by its hover text.
    let ctx = context();
    let mut shell = Shell::new(Box::new(HomeScreen::new(home_data())));
    let update = tree(&ctx, 2, |ui| shell.show(ui));
    assert!(named(&update, Role::Button, "Search, jump, ask or type an entry").is_some());
    assert!(named(&update, Role::Button, "Theme: Light (switch)").is_some());
    assert!(named(&update, Role::Button, "Keyboard map (?)").is_some());
}

// ---- the home screen -------------------------------------------------------------------------

#[test]
fn a_home_tile_reads_its_label_its_figure_and_its_footnote() {
    // provenance: index.html home tiles: label, figure, footnote. The figure 45,000.00 is
    // derived: 4_500_000 paise is 45,000.00 rupees.
    let ctx = context();
    let mut screen = HomeScreen::new(home_data());
    let mut app = AppCtx::default();
    let update = tree(&ctx, 2, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let tile = named_starting(&update, Role::Button, "Receivables, ")
        .expect("a tile named after Receivables");
    let name = tile.label().unwrap();
    assert!(name.contains("45,000.00"), "{name}");
    assert!(name.contains("Closing balance as of 30 Apr 2026"), "{name}");
}

// ---- a report --------------------------------------------------------------------------------

fn cr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

fn dr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn balance_sheet() -> ViewReport {
    ViewReport {
        token: ReportToken("bs".to_owned()),
        title: "Balance Sheet".to_owned(),
        params: ViewParams {
            period: Period::new(date!(2025 - 04 - 01), date!(2026 - 03 - 31)).unwrap(),
            detail: ViewDetail::Condensed,
        },
        columns: vec!["Liabilities".to_owned(), "Assets".to_owned()],
        rows: vec![
            ViewRow {
                key: None,
                label: "Capital Account".to_owned(),
                depth: 0,
                kind: ViewRowKind::Group,
                // 50_000_000 paise is 5,00,000.00 rupees, on the liabilities side (credit).
                cells: vec![cr(50_000_000), ViewCell::Empty],
            },
            ViewRow {
                key: None,
                label: "Current Assets".to_owned(),
                depth: 0,
                kind: ViewRowKind::Group,
                cells: vec![ViewCell::Empty, dr(50_000_000)],
            },
        ],
        totals: vec![cr(50_000_000), dr(50_000_000)],
        notes: Vec::new(),
    }
}

fn host_source() -> ReportSource {
    ReportSource::new(
        |_: &ReportToken, _: ViewParams| -> Result<Outcome<ViewReport>, ViewError> {
            Err(ViewError::Source {
                message: "not used in this test".to_owned(),
            })
        },
        |_: &ReportToken, _: &ViewKey, _: ViewParams| {
            Err(ViewError::Source {
                message: "not used in this test".to_owned(),
            })
        },
    )
}

#[test]
fn a_report_row_reads_its_label_and_its_amount_as_drawn() {
    // provenance: the generic report keeps the Dr or Cr word in every cell (`cell_text`, the same
    // text the row draws). Derived: 50_000_000 paise is 5,00,000.00 rupees, on the credit side.
    let ctx = context();
    let mut screen = ReportViewScreen::new(balance_sheet(), host_source()).unwrap();
    let mut app = AppCtx::default();
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    assert!(named(&update, Role::Row, "Capital Account, 5,00,000.00 Cr").is_some());
}
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

#[test]
fn the_sales_invoice_fields_are_named_by_their_captions_and_columns() {
    // provenance: the captions the invoice draws (item_invoice/draw.rs header and line table):
    // "Date", "Customer" (InvoiceKind::Sales party label, mod.rs), "Item", "Qty", "Rate".
    let ctx = context();
    let b = books();
    let mut screen = open(&b);
    let mut app = AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: Some(TODAY),
        ai_tier: None,
    };
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    for name in ["Date", "Customer", "Item", "Qty", "Rate", "Narration"] {
        assert!(
            named(&update, Role::TextInput, name).is_some(),
            "a field named {name}"
        );
    }
    assert!(named(&update, Role::Button, "Save").is_some());
}
