//! S16: the sales invoice entered as a voucher (Ctrl+H, then "As voucher" in the list): ledger
//! lines with the tax ledgers chosen by the user; the engine gives the party line and every tax
//! amount.
//!
//! The accounting product's pages say: "Press Ctrl+H (Change mode) > select As Voucher"
//! (help.tallysolutions.com/sales-of-goods-services/, read 2026-11-26) and that the GST ledgers
//! are selected on the voucher (help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/,
//! read 2026-11-26). The list Ctrl+H opens names Item Invoice, Accounting Invoice and As Voucher in
//! that order (the sales page); a note is offered the first two only (product choice, as is the two
//! ledger-line modes sharing their lines).
//!
//! Every expected value is derived by hand and the derivation is written next to it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{GstStatus, InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, FinancialYear, GroupId, Gstin, LedgerId, Outcome, Paise, Rate,
    SignedAmount, StateCode, gstin_check_character,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstClassification,
    GstRegistration, Ledger, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup,
    ReservedLedger, TaxDetails, TaxLedgerKind, Taxability, VoucherMode,
};
use time::OffsetDateTime;
use time::macros::date;

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn gstin(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn group(which: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(which))
        .unwrap()
        .id
}

/// A Maharashtra company, GST on, inventory off.
fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Services Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(27),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin("27ABCDE1234F1Z"),
            state: state(27),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            inventory: false,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn ledger(name: &str, which: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(0),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(which),
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

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// The company with a Maharashtra buyer, "Service Charges" (taxable at 18%), and Output CGST
/// and SGST ledgers.
fn setup(name: &str) -> (Api, Books) {
    let path = path(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let buyer = Ledger {
        party: Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(gstin("27FGHIJ5678K1Z")),
            pan: None,
            state: Some(state(27)),
            address: Address::default(),
        }),
        ..ledger("Sharma Traders", ReservedGroup::SundryDebtors)
    };
    let services = Ledger {
        gst: Some(GstClassification {
            hsn: None,
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }),
        ..ledger("Service Charges", ReservedGroup::SalesAccounts)
    };
    let tax = |name: &str, kind| Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: None,
        }),
        ..ledger(name, ReservedGroup::DutiesAndTaxes)
    };
    for l in [
        buyer,
        services,
        tax("Output CGST", TaxLedgerKind::Cgst),
        tax("Output SGST", TaxLedgerKind::Sgst),
    ] {
        let made = api.execute(Command::CreateLedger(l));
        assert!(matches!(made, Ok(Outcome::Value(_))), "{made:?}");
    }
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    (api, books)
}

fn id_of(books: &Books, name: &str) -> LedgerId {
    books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == name)
        .unwrap()
        .id
}

fn open(books: &Books) -> ItemInvoiceScreen {
    let sales = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    match ItemInvoiceScreen::open_in(
        books,
        sales,
        fy,
        date!(2026 - 11 - 21),
        InvoiceMode::Accounting,
    )
    .unwrap()
    {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn frames(s: &mut ItemInvoiceScreen, app: &mut AppCtx, ctx: &egui::Context, input: RawInput) {
    let out = ctx.run_ui(input, |ui| {
        let _stay = s.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

/// Sharma Traders, place 27, then the particulars Service Charges 10,000.00, Output CGST and
/// Output SGST with no amount typed.
fn fill_voucher(s: &mut ItemInvoiceScreen, books: &Books) {
    let m = &books.masters;
    s.set_party_text("sharma");
    s.submit(m);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    s.submit(m);
    for (i, (name, amount)) in [
        ("service", "10000"),
        ("output cgst", ""),
        ("output sgst", ""),
    ]
    .into_iter()
    .enumerate()
    {
        s.set_particular_text(i, name);
        s.submit(m);
        amount.clone_into(&mut s.draft_mut().particulars.get_mut(i).unwrap().amount_text);
        s.submit(m);
    }
}

/// Ctrl+H, then the row for `mode` with Down and Enter: what the user does with the keys.
fn choose(s: &mut ItemInvoiceScreen, mode: InvoiceMode) {
    s.open_mode_list();
    let row = s.mode_choices().iter().position(|m| *m == mode).unwrap();
    while s.mode_list() != Some(row) {
        s.step_mode_list(s.mode_list().unwrap() < row);
    }
    s.pick_listed_mode();
}

#[test]
fn the_mode_list_names_the_three_modes_in_the_order_the_sales_page_gives() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (read 2026-11-26) names the
    // modes Item Invoice, Accounting Invoice, As Voucher, in that order. A sales and a purchase
    // invoice offer all three; the list opens on the current mode.
    let (_api, books) = setup("s16_vch_list.ledgerly");
    let mut s = open(&books);
    assert_eq!(
        s.mode_choices(),
        vec![
            InvoiceMode::Item,
            InvoiceMode::Accounting,
            InvoiceMode::AsVoucher
        ]
    );
    s.open_mode_list();
    assert_eq!(
        s.mode_list(),
        Some(1),
        "the screen opens in accounting mode"
    );
}

#[test]
fn picking_as_voucher_in_the_mode_list_switches_and_picking_accounting_goes_back() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (Change mode, select As
    // Voucher). Derived: from accounting mode picking As Voucher gives as voucher with the same
    // party; picking Accounting Invoice goes back; the subtitle names the mode.
    let (_api, books) = setup("s16_vch_toggle.ledgerly");
    let mut s = open(&books);
    s.set_party_text("sharma");
    s.submit(&books.masters);
    let party = s.draft().party;
    choose(&mut s, InvoiceMode::AsVoucher);
    assert_eq!(s.mode(), InvoiceMode::AsVoucher, "{:?}", s.error_text());
    assert_eq!(s.draft().party, party);
    assert!(s.subtitle().unwrap().starts_with("As voucher"));
    choose(&mut s, InvoiceMode::Accounting);
    assert_eq!(s.mode(), InvoiceMode::Accounting);
}

#[test]
fn ctrl_h_down_enter_on_the_screen_reaches_as_voucher() {
    // provenance: derived. Ctrl+H opens the list on the current row (Accounting invoice, row 1);
    // Down moves to row 2 (As voucher); Enter picks it. Ctrl+H alone changes no mode.
    let (_api, books) = setup("s16_vch_key.ledgerly");
    let mut s = open(&books);
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let press = |key: Key, modifiers: Modifiers| RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    };
    frames(&mut s, &mut app, &ctx, RawInput::default());
    frames(&mut s, &mut app, &ctx, press(Key::H, Modifiers::COMMAND));
    assert_eq!(s.mode(), InvoiceMode::Accounting);
    assert_eq!(s.mode_list(), Some(1));
    frames(
        &mut s,
        &mut app,
        &ctx,
        press(Key::ArrowDown, Modifiers::NONE),
    );
    frames(&mut s, &mut app, &ctx, press(Key::Enter, Modifiers::NONE));
    assert_eq!(s.mode(), InvoiceMode::AsVoucher);
}

#[test]
fn as_voucher_hands_the_engine_the_lines_the_user_chose() {
    // provenance: derived. The voucher handed to the engine is in as-voucher mode with the
    // three chosen ledgers in order: Service Charges Cr 10,000.00 and the two tax ledgers Cr 0.00
    // (no amount typed; the engine fills them).
    let (_api, books) = setup("s16_vch_build.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    let v = s.take_compute_request(&books.masters).unwrap();
    assert_eq!(v.mode, VoucherMode::AsVoucher);
    let lines: Vec<(LedgerId, SignedAmount)> = v
        .ledger_lines
        .iter()
        .map(|l| (l.ledger, l.amount))
        .collect();
    let cr = |paise| SignedAmount {
        amount: Paise::new(paise),
        side: DrCr::Cr,
    };
    assert_eq!(
        lines,
        vec![
            (id_of(&books, "Service Charges"), cr(1_000_000)),
            (id_of(&books, "Output CGST"), cr(0)),
            (id_of(&books, "Output SGST"), cr(0)),
        ]
    );
}

#[test]
fn as_voucher_is_taxed_by_the_engine_and_saved_as_an_accounting_invoice() {
    // provenance: derived. Intra-state (27 and 27), 10,000.00 at 18%: CGST 900.00, SGST 900.00,
    // invoice total 11,800.00, no round-off. The stored voucher is an accounting invoice (the
    // GST returns read that mode): party Dr 11,800.00, Output CGST Cr 900.00, Output SGST Cr 900.00.
    let (api, books) = setup("s16_vch_save.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        frames(&mut s, &mut app, &ctx, RawInput::default());
        if !matches!(s.gst_status(&books.masters), GstStatus::NotComputedYet) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let GstStatus::Computed(block) = s.gst_status(&books.masters) else {
        panic!("not computed: {:?}", s.gst_status(&books.masters));
    };
    assert_eq!(block.invoice_total, Paise::new(1_180_000));
    s.save(&books.masters);
    for _ in 0..2000 {
        frames(&mut s, &mut app, &ctx, RawInput::default());
        if s.info_text() == Some("Saved.") || s.error_text().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.info_text(), Some("Saved."), "{:?}", s.error_text());
    drop(app);
    let reopened = Api::open(&path("s16_vch_save.ledgerly")).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let posted = books.vouchers.values().next().expect("one voucher posted");
    assert_eq!(posted.mode, VoucherMode::AccountingInvoice);
    let amount_of = |name: &str| {
        posted
            .ledger_lines
            .iter()
            .find(|l| l.ledger == id_of(&books, name))
            .unwrap()
            .amount
    };
    assert_eq!(
        amount_of("Sharma Traders"),
        SignedAmount::dr(Paise::new(1_180_000))
    );
    assert_eq!(
        amount_of("Output CGST"),
        SignedAmount::cr(Paise::new(90_000))
    );
    assert_eq!(
        amount_of("Output SGST"),
        SignedAmount::cr(Paise::new(90_000))
    );
}

#[test]
fn a_missing_tax_ledger_shows_the_engine_s_reason_and_does_not_save() {
    // provenance: derived. Only Output CGST chosen for a local supply: the engine says CGST and
    // SGST are needed (GST is not calculated correctly otherwise), nothing is saved.
    let (api, books) = setup("s16_vch_mismatch.ledgerly");
    let m = &books.masters;
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    s.set_particular_text(2, "");
    "".clone_into(&mut s.draft_mut().particulars.get_mut(2).unwrap().amount_text);
    let _ = m;
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        frames(&mut s, &mut app, &ctx, RawInput::default());
        if !matches!(s.gst_status(&books.masters), GstStatus::NotComputedYet) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let GstStatus::Unsupported(u) = s.gst_status(&books.masters) else {
        panic!("not unsupported: {:?}", s.gst_status(&books.masters));
    };
    assert!(u.reason.contains("CGST and SGST"), "{}", u.reason);
    s.save(&books.masters);
    assert!(s.error_text().is_some());
}

/// Runs frames until the engine has answered for the screen's invoice.
fn wait_for_engine(s: &mut ItemInvoiceScreen, api: Api, books: &Books) {
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        frames(s, &mut app, &ctx, RawInput::default());
        if !matches!(s.gst_status(&books.masters), GstStatus::NotComputedYet) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the engine did not answer");
}

fn rows(s: &ItemInvoiceScreen, books: &Books) -> Vec<(String, String, String)> {
    s.voucher_rows(&books.masters)
        .into_iter()
        .map(|r| (r.ledger, r.side.to_owned(), r.amount))
        .collect()
}

fn row(ledger: &str, side: &str, amount: &str) -> (String, String, String) {
    (ledger.to_owned(), side.to_owned(), amount.to_owned())
}

#[test]
fn as_voucher_lists_the_party_first_as_a_dr_line_with_the_engine_s_figures() {
    // provenance: derived. help.tallysolutions.com/sales-of-goods-services/ (read 2026-11-26):
    // in As Voucher mode the party ledger is the first (Dr) line, the Sales ledger is a Cr line,
    // and the GST ledgers follow. 10,000.00 at 18%, intra-state: CGST 900.00, SGST 900.00; the
    // party is debited 11,800.00 (11,800.00 is whole rupees, so no round-off line). Amounts show
    // with Indian grouping (the engine's figures, formatted).
    let (api, books) = setup("s16_vch_rows.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    wait_for_engine(&mut s, api, &books);
    assert_eq!(
        rows(&s, &books),
        vec![
            row("Sharma Traders", "Dr", "11,800.00"),
            row("Service Charges", "Cr", "10,000.00"),
            row("Output CGST", "Cr", "900.00"),
            row("Output SGST", "Cr", "900.00"),
        ]
    );
}

#[test]
fn as_voucher_shows_no_figure_for_the_party_and_tax_lines_until_the_engine_answers() {
    // provenance: derived from the rule "never a wrong number": before the engine has answered,
    // the party and the tax ledgers have no amount (not 0.00); the typed Sales amount is the
    // user's own and shows grouped.
    let (_api, books) = setup("s16_vch_rows_wait.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    assert_eq!(
        rows(&s, &books),
        vec![
            row("Sharma Traders", "Dr", ""),
            row("Service Charges", "Cr", "10,000.00"),
            row("Output CGST", "Cr", ""),
            row("Output SGST", "Cr", ""),
        ]
    );
}

#[test]
fn as_voucher_lists_the_engine_s_round_off_as_its_own_row() {
    // provenance: derived. 10,000.50 at 18% intra-state: CGST = SGST = 10,000.50 x 9% = 900.045,
    // rounded half up to 900.05 each; gross = 10,000.50 + 900.05 + 900.05 = 11,800.60, rounded to
    // the rupee 11,801.00; the invoice rounding line is Cr 0.40 (11,801.00 Dr = 11,800.60 + 0.40).
    let (mut api, _books) = setup("s16_vch_rows_round.ledgerly");
    let round = Ledger {
        reserved: Some(ReservedLedger::InvoiceRounding),
        ..ledger("Round Off", ReservedGroup::IndirectExpenses)
    };
    let made = api.execute(Command::CreateLedger(round));
    assert!(matches!(made, Ok(Outcome::Value(_))), "{made:?}");
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    fill_voucher(&mut s, &books);
    "10000.50".clone_into(&mut s.draft_mut().particulars.first_mut().unwrap().amount_text);
    wait_for_engine(&mut s, api, &books);
    assert_eq!(
        rows(&s, &books),
        vec![
            row("Sharma Traders", "Dr", "11,801.00"),
            row("Service Charges", "Cr", "10,000.50"),
            row("Output CGST", "Cr", "900.05"),
            row("Output SGST", "Cr", "900.05"),
            row("Round Off", "Cr", "0.40"),
        ]
    );
}
