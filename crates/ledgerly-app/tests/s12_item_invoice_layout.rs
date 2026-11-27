//! Purchase, credit note and debit note follow their own mock states (round s12): the head, the
//! columns, the buttons and the foot of `SCREENS.purchase` and `noteScreen`
//! (`design/mock-v2/index.html`, lines 1966-2041), while the sales invoice keeps its own
//! (`sales-invoice`, lines 1275-1345).
//!
//! Provenance: the mock's source. Purchase head, row one: Supplier across two columns, Supplier
//! bill no., Bill date; row two: Purchase ledger, Place of supply (index.html:1992-1998). Its
//! table is # Item HSN Qty Rate GST Amount, with no Unit (index.html:1999). Its buttons are
//! "Bill photo Ctrl+Alt+O" and Save (index.html:2182). Its foot has a bare "Narration" caption and the
//! tax box (index.html:2003-2011). The notes' head is Party across two columns and Date, then
//! the "Against sales invoice" / "Against purchase bill" reference (index.html:2029-2033), with
//! Save as the only button (index.html:2027) and the total labelled "Note total".
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Tr;
use std::collections::BTreeMap;

use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::item_invoice::{Field, ItemInvoiceScreen};
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
const T_PURCHASE: VoucherTypeId = VoucherTypeId(2);
const T_CREDIT: VoucherTypeId = VoucherTypeId(3);
const T_DEBIT: VoucherTypeId = VoucherTypeId(4);
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
    for (id, name, base) in [
        (T_SALES, "Sales", BaseVoucherKind::Sales),
        (T_PURCHASE, "Purchase", BaseVoucherKind::Purchase),
        (T_CREDIT, "Credit Note", BaseVoucherKind::CreditNote),
        (T_DEBIT, "Debit Note", BaseVoucherKind::DebitNote),
    ] {
        masters.voucher_types.insert(
            id,
            VoucherType {
                id,
                name: name.to_owned(),
                base,
                abbreviation: name.chars().take(3).collect(),
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
            },
        );
    }
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

fn open(b: &Books, t: VoucherTypeId) -> ItemInvoiceScreen {
    match ItemInvoiceScreen::open(b, t, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn texts_of(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        use egui::epaint::Shape;
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Draws the screen a few times in a 1280 by 800 window and returns every text of the last frame.
fn draw(s: &mut ItemInvoiceScreen, b: &Books) -> Vec<String> {
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let mut last = Vec::new();
    for _ in 0..3 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        last = texts_of(&out.shapes);
        out.drop_without_applying_deltas();
    }
    last
}

fn rect(s: &ItemInvoiceScreen, name: &str) -> Rect {
    s.field_rect(name)
        .unwrap_or_else(|| panic!("the field {name:?} was not drawn"))
}

fn heads(s: &ItemInvoiceScreen) -> Vec<&'static str> {
    s.heading_rects().iter().map(|(n, _)| *n).collect()
}

#[test]
fn a_purchase_head_is_supplier_bill_no_bill_date_then_ledger_and_place() {
    // provenance: index.html:1992-1998. Row one: Supplier (two columns), Supplier bill no., Bill
    // date. Row two: Purchase ledger, Place of supply. There is no "Party GSTIN" box: the GSTIN
    // is written after the supplier's name.
    let b = books();
    let mut s = open(&b, T_PURCHASE);
    draw(&mut s, &b);
    let (supplier, bill, date) = (
        rect(&s, "Supplier"),
        rect(&s, "Supplier bill no."),
        rect(&s, "Bill date"),
    );
    let (ledger, place) = (rect(&s, "Purchase ledger"), rect(&s, "Place of supply"));
    let unit = bill.width();
    assert!(
        (supplier.width() - (2.0 * unit + 16.0)).abs() < 1.0,
        "{supplier:?}"
    );
    assert!((bill.min.x - (supplier.max.x + 16.0)).abs() < 1.0);
    assert!((date.min.x - (bill.max.x + 16.0)).abs() < 1.0);
    assert!((supplier.min.y - bill.min.y).abs() < 0.5 && (bill.min.y - date.min.y).abs() < 0.5);
    assert!(
        ledger.min.y > supplier.max.y,
        "the second row is under the first"
    );
    assert!((ledger.min.x - supplier.min.x).abs() < 0.5);
    assert!((place.min.x - (ledger.max.x + 16.0)).abs() < 1.0);
    assert!(s.field_rect("Party GSTIN").is_none());
}

#[test]
fn a_purchase_table_has_no_unit_column_and_a_sale_keeps_it() {
    // provenance: index.html:1999 (purchase: # Item HSN Qty Rate GST Amount) and :1283 (sales
    // invoice: # Item HSN Qty Unit Rate GST Amount).
    let b = books();
    let mut p = open(&b, T_PURCHASE);
    draw(&mut p, &b);
    assert_eq!(
        heads(&p),
        ["#", "Item", "HSN", "Qty", "Rate", "GST", "Amount"]
    );
    let mut c = open(&b, T_CREDIT);
    draw(&mut c, &b);
    assert_eq!(
        heads(&c),
        ["#", "Item", "HSN", "Qty", "Rate", "GST", "Amount"]
    );
    let mut d = open(&b, T_DEBIT);
    draw(&mut d, &b);
    assert_eq!(
        heads(&d),
        ["#", "Item", "HSN", "Qty", "Rate", "GST", "Amount"]
    );
    let mut sale = open(&b, T_SALES);
    draw(&mut sale, &b);
    assert_eq!(
        heads(&sale),
        ["#", "Item", "HSN", "Qty", "Unit", "Rate", "GST", "Amount"]
    );
}

#[test]
fn a_purchase_offers_bill_photo_and_save_not_the_sales_buttons() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    // provenance: index.html:2182 (`Bill photo Ctrl+Alt+O`, Save) against :1308 (sales invoice: the
    // mode button, More details, Save).
    let b = books();
    let mut p = open(&b, T_PURCHASE);
    let shown = draw(&mut p, &b);
    for want in ["Bill photo", "Ctrl+Alt+O", "Save"] {
        assert!(shown.iter().any(|t| t == want), "{want}: {shown:?}");
    }
    for not in ["Item invoice", "More details", "Item mode"] {
        assert!(!shown.iter().any(|t| t == not), "{not}: {shown:?}");
    }
    let mut sale = open(&b, T_SALES);
    let shown = draw(&mut sale, &b);
    for want in ["Item invoice", "More details", "Save"] {
        assert!(shown.iter().any(|t| t == want), "{want}: {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == "Bill photo"), "{shown:?}");
}

#[test]
fn a_note_has_save_as_its_only_button() {
    // provenance: index.html:2027, `noteScreen`: the page head holds Save and nothing else.
    let b = books();
    for t in [T_CREDIT, T_DEBIT] {
        let mut s = open(&b, t);
        let shown = draw(&mut s, &b);
        assert!(shown.iter().any(|x| x == "Save"), "{shown:?}");
        for not in ["Item mode", "More details", "Bill photo"] {
            assert!(!shown.iter().any(|x| x == not), "{not}: {shown:?}");
        }
    }
}

#[test]
fn a_note_head_reads_party_and_date_then_the_invoice_it_is_against() {
    // provenance: index.html:2029-2033: "Party" across two columns then "Date"; "Against sales
    // invoice" (credit note) or "Against purchase bill" (debit note) under the party.
    let b = books();
    for (t, label) in [
        (T_CREDIT, "Against sales invoice"),
        (T_DEBIT, "Against purchase bill"),
    ] {
        let mut s = open(&b, t);
        draw(&mut s, &b);
        let (party, date, against) = (rect(&s, "Party"), rect(&s, "Date"), rect(&s, label));
        assert!(
            (party.width() - (2.0 * date.width() + 16.0)).abs() < 1.0,
            "{party:?}"
        );
        assert!((date.min.x - (party.max.x + 16.0)).abs() < 1.0);
        assert!(against.min.y > party.max.y);
        assert!((against.min.x - party.min.x).abs() < 0.5);
        assert!(s.field_rect("Original invoice no.").is_none());
    }
}

#[test]
fn a_purchase_and_a_note_foot_has_no_balance_line_place_note_or_words() {
    // provenance: index.html:2003-2011 and :2036-2040: under the narration there is only the
    // meta line, and the tax box ends at the total; the sales invoice's place note, balance line
    // and "Rupees ... Only" are not there. The narration's caption is the bare word.
    let b = books();
    for t in [T_PURCHASE, T_CREDIT, T_DEBIT] {
        let mut s = open(&b, t);
        let shown = draw(&mut s, &b);
        assert!(shown.iter().any(|x| x == "Narration"), "{shown:?}");
        for not in ["Alt+R", "recalls the last one for this party"] {
            assert!(!shown.iter().any(|x| x.contains(not)), "{not}: {shown:?}");
        }
    }
}

#[test]
fn the_total_of_a_note_is_labelled_note_total() {
    // provenance: index.html:2038, `Note total`; a purchase and a sale say "Invoice total".
    let b = books();
    let mut c = open(&b, T_CREDIT);
    let shown = draw(&mut c, &b);
    assert!(
        shown.iter().any(|x| x.starts_with("Note total")),
        "{shown:?}"
    );
    assert!(
        !shown.iter().any(|x| x.starts_with("Invoice total")),
        "{shown:?}"
    );
    let mut p = open(&b, T_PURCHASE);
    let shown = draw(&mut p, &b);
    assert!(
        shown.iter().any(|x| x.starts_with("Invoice total")),
        "{shown:?}"
    );
}

#[test]
fn a_purchase_and_a_note_are_titled_as_the_mock_titles_them() {
    // provenance: index.html:1976 (`Purchase PB/0146`; the crumb is "Purchase") and :2024
    // (`<title> CN/0007`); the sales invoice keeps "Sales invoice" (the voucher type's name
    // here is the data; the mock writes "Credit note" in sentence case, ours keeps the master's).
    let b = books();
    assert_eq!(open(&b, T_PURCHASE).title(&Tr::default()), "Purchase");
    assert_eq!(open(&b, T_CREDIT).title(&Tr::default()), "Credit Note");
    assert_eq!(open(&b, T_SALES).title(&Tr::default()), "Sales invoice");
    let purchase = open(&b, T_PURCHASE).subtitle(&Tr::default()).unwrap();
    assert_eq!(
        purchase,
        "Typed by hand \u{b7} no bill reader \u{b7} tax is recomputed by the GST engine on every keystroke"
    );
    assert_eq!(
        open(&b, T_CREDIT).subtitle(&Tr::default()).unwrap(),
        "Credit note, CGST Act s.34(1): reduces the tax on an invoice already issued"
    );
    assert_eq!(
        open(&b, T_DEBIT).subtitle(&Tr::default()).unwrap(),
        "Debit note, purchase return: reduces what you owe the supplier"
    );
}

#[test]
fn enter_walks_a_purchase_in_the_documented_order() {
    // provenance: help.tallysolutions.com/purchase-goods-services/ (read 2026-11-25): Supplier
    // Invoice No. and Date, then Party A/c name, then the Purchase ledger, then the items. The
    // place of supply follows the party details. No Date step: the page shows no date among the
    // fields.
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_PURCHASE);
    assert_eq!(s.focus(), Field::Reference);
    s.submit(m);
    assert_eq!(s.focus(), Field::ReferenceDate);
    s.submit(m);
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("sharma");
    s.submit(m);
    assert_eq!(s.focus(), Field::Account);
}

#[test]
fn enter_walks_a_note_party_then_original_invoice_then_ledger() {
    // provenance: help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/ (read
    // 2026-11-25): Party A/c Name, the original invoice number and date (Receipt Details), the
    // Sales ledger, then the items; help.tallysolutions.com/tally-prime/purchase-process/
    // purchase-returns-tally/ (same date): Party A/c name, Original Invoice Details, the items.
    let b = books();
    let m = &b.masters;
    for t in [T_CREDIT, T_DEBIT] {
        let mut s = open(&b, t);
        assert_eq!(s.focus(), Field::Party);
        s.set_party_text("sharma");
        s.submit(m);
        assert_eq!(s.focus(), Field::Reference);
        s.submit(m);
        assert_eq!(s.focus(), Field::ReferenceDate);
        s.submit(m);
        assert_eq!(s.focus(), Field::Account);
    }
}

#[test]
fn a_sale_still_starts_on_the_party_and_goes_party_place_ledger() {
    // provenance: the sales invoice's own mock (index.html:1287-1297); unchanged by s12.
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("sharma");
    s.submit(m);
    assert_eq!(s.focus(), Field::Place);
}

#[test]
fn ctrl_alt_o_on_a_purchase_opens_the_bill_screen_and_alt_o_is_left_to_import() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    // provenance: index.html:2182 and :2204, "Bill photo Ctrl+Alt+O" / "Read a bill", on the
    // purchase only; index.html:1139, Alt+O is Import ("Alt+O (Import)",
    // help.tallysolutions.com/import-data-from-xml-or-json/), so the purchase form leaves it to the
    // shell. Ctrl+Alt+O is not on help.tallysolutions.com/keyboard-shortcuts-tally-prime/ (read
    // 2026-11-27).
    use egui::{Event, Key, Modifiers};
    ledgerly_app::release::set_ai_enabled(true);
    let b = books();
    let press = |modifiers: Modifiers| RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events: vec![Event::Key {
            key: Key::O,
            physical_key: Some(Key::O),
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    };
    let ctrl_alt = Modifiers {
        alt: true,
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    for (t, held, pushes) in [
        (T_PURCHASE, ctrl_alt, true),
        (T_SALES, ctrl_alt, false),
        (T_PURCHASE, Modifiers::ALT, false),
    ] {
        let mut s = open(&b, t);
        let ctx = egui::Context::default();
        let mut pushed = false;
        let out = ctx.run_ui(press(held), |ui| {
            pushed = matches!(s.ui(ui, &mut app), ledgerly_app::ScreenAction::Push(_));
        });
        out.drop_without_applying_deltas();
        assert_eq!(pushed, pushes, "{t:?} {held:?}");
    }
}
