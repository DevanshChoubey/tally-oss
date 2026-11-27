//! Card S7 (accessibility): Tab and Shift+Tab move focus through the sales invoice header in
//! reading order, and the focused field is the one a screen reader is told about.
//!
//! derived: the header is laid out in a three-column grid, row by row, left to right
//! (`item_invoice/draw.rs` `draw_header`): Date, then the party field (captioned "Party" for a
//! sales invoice), then "Place of supply", then the reference field on the next row. Egui moves
//! focus in widget creation order, so Tab must visit the fields in that order.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::accesskit::{Node, NodeId, Role, Toggled, TreeUpdate};
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
        group: Some(GroupId(1)),
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
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: Some(TODAY),
        ai_tier: None,
    }
}

#[test]
fn tab_visits_the_header_and_line_fields_in_reading_order() {
    // derived: from the party field (where the screen starts), Tab moves through the header in
    // reading order: "Place of supply" (first row), "Sales ledger" (account), "Buyer's order no."
    // (reference, the second row of the grid), then the first line's "Item", "Qty" and "Rate"
    // (draw_header, draw_lines in item_invoice/draw.rs). This test keeps to the fields; the
    // suggestion entries are not Tab stops at all, which the tests below check.
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
        "Buyer's order no.",
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
    // derived: the screen starts with focus on the party field ("Party", the mock's label, design/mock-v2/index.html line 1288; draw_head). The
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

/// The labels of the entries highlighted (selected) in `update`.
fn highlighted(update: &TreeUpdate) -> Vec<String> {
    update
        .nodes
        .iter()
        .filter(|(_, node)| node.toggled() == Some(Toggled::True))
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect()
}

/// One key press, then a settling frame, returning the entries highlighted afterwards.
fn highlight_after(ctx: &Context, key: Key, body: &mut impl FnMut(&mut Ui)) -> Vec<String> {
    frame(ctx, Some((key, Modifiers::NONE)), body);
    highlighted(&frame(ctx, None, body))
}

#[test]
fn tab_from_an_open_suggestion_list_goes_straight_to_the_next_field() {
    // Tally: "To move to the next field" is Tab (help.tallysolutions.com/keyboard-shortcuts-tally-prime/,
    // read 2026-11-24, Vouchers and Masters table). The suggestion list under the party field is
    // open when the screen starts, so a single Tab must land on "Place of supply", not on one of
    // the list's entries. A suggestion entry is a button, not a field, so its role is not TextInput.
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
    let landed = press(&ctx, Key::Tab, false, &mut body);
    let role = landed.as_ref().map(|(role, _)| *role);
    assert_eq!(
        role,
        Some(Role::TextInput),
        "one Tab from the party field landed on {landed:?}"
    );
}

#[test]
fn arrow_keys_move_the_highlighted_suggestion() {
    // Tally: "To move one line down in a list" is Down arrow and "To move one line up in a list" is
    // Up arrow (help.tallysolutions.com/keyboard-shortcuts-tally-prime/, read 2026-11-24). The mock
    // (design/mock-v2/index.html, the transaction option list) moves its highlight the same way.
    // derived: while the list is open exactly one entry is highlighted; Down moves the highlight
    // off the first entry and Up moves it back.
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
    let start = highlighted(&frame(&ctx, None, &mut body));
    assert_eq!(
        start.len(),
        1,
        "one entry should be highlighted, got {start:?}"
    );
    let down = highlight_after(&ctx, Key::ArrowDown, &mut body);
    assert_eq!(
        down.len(),
        1,
        "one entry should be highlighted after Down, got {down:?}"
    );
    assert_ne!(down, start, "Down did not move the highlight");
    let up = highlight_after(&ctx, Key::ArrowUp, &mut body);
    assert_eq!(up, start, "Up did not move the highlight back");
}

#[test]
fn enter_picks_the_highlighted_suggestion() {
    // The mock's option list picks the highlighted entry with Enter (design/mock-v2/index.html,
    // taPick, near line 1257). Tally's keyboard page (help.tallysolutions.com/keyboard-shortcuts-
    // tally-prime/, read 2026-11-24) documents Up and Down for moving in a list, not Enter for
    // picking one. derived: the
    // party list opens on the first ledger, "Output CGST" (ledgers sort by name, "Output CGST" is
    // before "Sales - Local" and "Sharma Traders"); one Down highlights the second, and Enter
    // must put that ledger's name into the party field.
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
    let before = highlighted(&frame(&ctx, None, &mut body));
    let down = highlight_after(&ctx, Key::ArrowDown, &mut body);
    assert_ne!(down, before, "Down did not move the highlight");
    let picked = highlighted(&frame(&ctx, Some((Key::Enter, Modifiers::NONE)), &mut body));
    let party = party_value(&frame(&ctx, None, &mut body));
    assert_eq!(
        party,
        down.first().cloned(),
        "Enter put {party:?} in the party field, but {down:?} was highlighted (highlighted after Enter: {picked:?})"
    );
}

/// The text of the field named "Party" in `update`, read from its AccessKit value.
fn party_value(update: &TreeUpdate) -> Option<String> {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TextInput && node.label() == Some("Party"))
        .and_then(|(_, node)| node.value().map(str::to_owned))
}
