//! Card S7 (accessibility): Tab and Shift+Tab move focus through the sales invoice header in
//! reading order, and the focused field is the one a screen reader is told about.
//!
//! derived: the header is laid out in a three-column grid, row by row, left to right
//! (`item_invoice/draw.rs` `draw_header`): Date, then the party field (captioned "Customer" for a
//! sales invoice), then "Place of supply", then the reference field on the next row. Egui moves
//! focus in widget creation order, so Tab must visit the fields in that order.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::accesskit::{Node, NodeId, Role, TreeUpdate};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::screens::item_invoice::ItemInvoiceScreen;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen};
use std::collections::BTreeMap;

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

/// One frame of `body` with the given key pressed, with AccessKit on. Returns the update.
fn frame(
    ctx: &Context,
    key: Option<(Key, Modifiers)>,
    body: &mut impl FnMut(&mut Ui),
) -> TreeUpdate {
    let events = match key {
        Some((key, modifiers)) => vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        None => Vec::new(),
    };
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(input, &mut *body);
    let update = out.platform_output.accesskit_update.take();
    out.drop_without_applying_deltas();
    update.expect("AccessKit is enabled")
}

/// The role and accessible name of the node that has focus in `update`.
fn focused(update: &TreeUpdate) -> Option<(Role, String)> {
    let focus: NodeId = update.focus;
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == focus)
        .and_then(|(_, node): &(NodeId, Node)| Some((node.role(), node.label()?.to_owned())))
}

/// One key press (followed by a settling frame, as egui moves focus on the frame after the key),
/// returning the role and name of what then has focus.
fn press(
    ctx: &Context,
    key: Key,
    shift: bool,
    body: &mut impl FnMut(&mut Ui),
) -> Option<(Role, String)> {
    let modifiers = if shift {
        Modifiers::SHIFT
    } else {
        Modifiers::NONE
    };
    frame(ctx, Some((key, modifiers)), body);
    focused(&frame(ctx, None, body))
}

/// Presses Tab (or Shift+Tab) `presses` times and returns the text fields that took focus, in
/// order. Suggestion buttons that an open field shows are skipped: they are not fields.
fn field_stops(
    ctx: &Context,
    shift: bool,
    presses: usize,
    body: &mut impl FnMut(&mut Ui),
) -> Vec<String> {
    (0..presses)
        .filter_map(|_| press(ctx, Key::Tab, shift, body))
        .filter(|(role, _)| *role == Role::TextInput)
        .map(|(_, name)| name)
        .collect()
}

fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

// ---- fixture (copied from s7_a11y_screens.rs, the sales invoice test book) ----

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

fn sales_app(b: &Books) -> AppCtx {
    AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: Some(TODAY),
        ai_tier: None,
    }
}

#[test]
fn tab_visits_the_header_and_line_fields_in_reading_order() {
    // derived: from the party field (where the screen starts), Tab moves through the header in
    // reading order: "Place of supply" (first row), "Sales ledger" (account), "Order or reference"
    // (reference, the second row of the grid), then the first line's "Item", "Qty" and "Rate"
    // (draw_header, draw_lines in item_invoice/draw.rs). The suggestion buttons of an open field
    // are skipped here; they are a known gap, see the Accessibility section of PARITY-GAPS.
    let ctx = context();
    let b = books();
    let mut screen = open(&b);
    let mut app = sales_app(&b);
    let mut body = |ui: &mut Ui| {
        let _ = screen.ui(ui, &mut app);
    };
    for _ in 0..3 {
        frame(&ctx, None, &mut body);
    }
    let stops = field_stops(&ctx, false, 14, &mut body);
    let expected = [
        "Place of supply",
        "Sales ledger",
        "Order or reference",
        "Item",
        "Qty",
        "Rate",
    ];
    assert_eq!(
        &stops[..expected.len()],
        expected,
        "field order was {stops:?}"
    );
}

#[test]
fn shift_tab_from_the_party_field_goes_back_to_the_date() {
    // derived: the screen starts with focus on the party field ("Customer", draw_header). The
    // field before it in reading order is "Date", so Shift+Tab must reach "Date" first, and a
    // second Shift+Tab must not go past it to a field below.
    let ctx = context();
    let b = books();
    let mut screen = open(&b);
    let mut app = sales_app(&b);
    let mut body = |ui: &mut Ui| {
        let _ = screen.ui(ui, &mut app);
    };
    for _ in 0..3 {
        frame(&ctx, None, &mut body);
    }
    let back = field_stops(&ctx, true, 1, &mut body);
    assert_eq!(
        back,
        vec!["Date".to_owned()],
        "back from the party field was {back:?}"
    );
}
