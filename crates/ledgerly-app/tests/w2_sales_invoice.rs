//! Wave 2, card S-B: the sales invoice screen. The expected values are derived by hand and the
//! derivation sits next to each one; nothing is copied from this crate's output. The books are a
//! small fictional fixture. The GST block is a test double standing in for the engine's answer:
//! it checks that the screen shows and gates on exactly what it is given, and says nothing about
//! what the real engine returns.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::item_invoice::{
    Field, GstBlock, GstBlockLine, GstResult, GstTotal, ItemInvoiceScreen, tax_rows,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Date, Decimal, FinancialYear, GroupId, Gstin, LedgerId, Outcome, Paise, SignedAmount,
    StateCode, StockItemId, UnitId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, StockItem, StoredGst, Unit, UnitKind, ValuationMethod, Voucher, VoucherType,
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

/// Keyboard flow for 100 Nos of "Widget Large" at 100.00 to Sharma Traders, place 27. Leaves the
/// keyboard on the blank second row, as Enter after the first discount does.
fn fill(s: &mut ItemInvoiceScreen, m: &Masters) {
    s.set_party_text("sharma");
    s.submit(m);
    s.draft_mut().place_text = "27".to_owned();
    s.submit(m);
    s.set_account_text("sales");
    s.submit(m);
    s.submit(m);
    s.set_item_text(0, "widget");
    s.submit(m);
    s.draft_mut().rows[0].qty_text = "100".to_owned();
    s.submit(m);
    s.draft_mut().rows[0].rate_text = "100".to_owned();
    s.submit(m);
    s.submit(m);
    assert_eq!(s.focus(), Field::Item(1));
}

fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

/// Engine stand-in for the filled invoice.
///
/// provenance: derived. The 18% slab is CGST 9% + SGST 9%. Taxable 100 x 100.00 = 10,000.00;
/// CGST = 10,000 x 9% = 900.00; SGST = 900.00; no round-off; invoice total = 11,800.00.
fn block_for_10000() -> GstBlock {
    let ledger_line = |ledger, amount| ledgerly_model::LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    GstBlock {
        pack_version: "test-pack".to_owned(),
        lines: vec![GstBlockLine {
            taxable: rupees(10_000),
            rate_percent: Decimal::from(18),
        }],
        taxable_total: rupees(10_000),
        heads: vec![
            GstTotal {
                head: "CGST".to_owned(),
                amount: rupees(900),
            },
            GstTotal {
                head: "SGST".to_owned(),
                amount: rupees(900),
            },
        ],
        round_off: Paise::ZERO,
        invoice_total: rupees(11_800),
        ledger_lines: vec![
            ledger_line(SHARMA, SignedAmount::dr(rupees(11_800))),
            ledger_line(SALES, SignedAmount::cr(rupees(10_000))),
            ledger_line(CGST, SignedAmount::cr(rupees(900))),
            ledger_line(SGST, SignedAmount::cr(rupees(900))),
        ],
        stored: StoredGst {
            pack_version: "test-pack".to_owned(),
            lines: Vec::new(),
            round_off: Paise::ZERO,
        },
    }
}

fn answered(b: &Books) -> ItemInvoiceScreen {
    let m = &b.masters;
    let mut s = open(b);
    fill(&mut s, m);
    let v: Voucher = s.take_compute_request(m).unwrap();
    s.set_gst(v, GstResult::Computed(block_for_10000()));
    s
}

fn frame(
    ctx: &egui::Context,
    screen: &mut ItemInvoiceScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
    action
}

fn key(key: Key) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

// ---- tax box ---------------------------------------------------------------------------------

#[test]
fn tax_box_shows_the_engine_heads_in_order_without_adding_anything() {
    let rows = tax_rows(&block_for_10000());
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Taxable value", "CGST", "SGST", "Round off"]);
    // provenance: derived. Indian grouping of 10,000.00, 900.00 and 900.00 (below one lakh).
    assert_eq!(rows[0].1, "10,000.00");
    assert_eq!(rows[1].1, "900.00");
    assert_eq!(rows[2].1, "900.00");
}

// ---- lines -----------------------------------------------------------------------------------

#[test]
fn lines_show_no_figures_before_the_engine_answers() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b);
    fill(&mut s, m);
    let lines = s.lines_shown(m);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].number, 1);
    assert_eq!(lines[0].unit, "Nos");
    assert!(lines[0].gst.is_empty());
    assert!(lines[0].amount.is_empty());
    assert!(lines[1].amount.is_empty());
}

#[test]
fn lines_take_gst_and_amount_from_the_engine_only() {
    let b = books();
    let m = &b.masters;
    let s = answered(&b);
    let lines = s.lines_shown(m);
    assert_eq!(lines.len(), 2);
    // provenance: derived. 100 x 100.00 = 10,000.00 taxable, from the engine block.
    assert_eq!(lines[0].amount, "10,000.00");
    assert!(lines[0].problem.is_none());
    // The blank row shows nothing.
    assert!(lines[1].amount.is_empty());
    assert!(lines[1].problem.is_none());
}

// ---- lock ------------------------------------------------------------------------------------

#[test]
fn a_locked_period_refuses_the_save_with_the_lock_message() {
    let b = books();
    let m = &b.masters;
    let mut s = answered(&b);
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: Some(TODAY),
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    // provenance: derived from the lock rule in the screen: a voucher dated on or before the lock
    // date is refused. The invoice is dated TODAY and the lock is TODAY.
    s.save(m);
    assert!(s.take_save_request().is_none());
    assert!(s.error_text().unwrap().contains("period is locked"));
}

// ---- save confirmation -----------------------------------------------------------------------

#[test]
fn enter_on_narration_asks_first_and_a_second_save_posts_the_voucher() {
    let b = books();
    let m = &b.masters;
    let mut s = answered(&b);
    s.submit(m);
    assert_eq!(s.focus(), Field::Narration);
    s.submit(m);
    assert!(s.take_save_request().is_none());
    assert!(s.error_text().is_none());
    s.save(m);
    assert!(s.take_save_request().is_some());
}

#[test]
fn escape_while_asking_keeps_the_draft_and_does_not_leave() {
    let b = books();
    let m = &b.masters;
    let mut s = answered(&b);
    s.submit(m);
    s.submit(m);
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(!s.draft().is_blank());
    assert!(s.take_save_request().is_none());
}

// ---- screen identity -------------------------------------------------------------------------

#[test]
fn the_screen_names_its_type_and_shows_the_key_bar() {
    let b = books();
    let s = open(&b);
    assert_eq!(s.title(), "Sales invoice");
    assert!(s.subtitle().is_some());
    let keys = s.keys();
    assert!(keys.iter().any(|k| k.keys == "Ctrl+A"));
    assert!(keys.iter().any(|k| k.keys == "Enter"));
}

#[test]
fn the_screen_draws_with_a_filled_invoice_and_a_lock() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = answered(&b);
    let mut app = AppCtx {
        books: Some(b.clone()),
        api: None,
        lock_date: Some(TODAY),
        ai_tier: None,
    };
    let action = frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(matches!(action, ScreenAction::Stay));
}
