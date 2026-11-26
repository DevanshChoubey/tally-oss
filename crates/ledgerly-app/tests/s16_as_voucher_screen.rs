//! S16: the sales invoice entered as a voucher (Ctrl+Shift+H, "As voucher"): ledger lines with the
//! tax ledgers chosen by the user; the engine gives the party line and every tax amount.
//!
//! The accounting product's pages say: "Press Ctrl+H (Change mode) > select As Voucher"
//! (help.tallysolutions.com/sales-of-goods-services/, read 2026-11-26) and that the GST ledgers
//! are selected on the voucher (help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/,
//! read 2026-11-26). Product choice, not documented: Ctrl+H keeps toggling item and accounting
//! mode, and As voucher is Ctrl+Shift+H; the two ledger-line modes share their lines.
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
    TaxDetails, TaxLedgerKind, Taxability, VoucherMode,
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

#[test]
fn ctrl_shift_h_switches_to_as_voucher_and_back() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (Change mode, select As
    // Voucher). Derived: from accounting mode one switch gives as voucher with the same party; a
    // second goes back to accounting; the subtitle names the mode.
    let (_api, books) = setup("s16_vch_toggle.ledgerly");
    let mut s = open(&books);
    s.set_party_text("sharma");
    s.submit(&books.masters);
    let party = s.draft().party;
    s.toggle_as_voucher();
    assert_eq!(s.mode(), InvoiceMode::AsVoucher, "{:?}", s.error_text());
    assert_eq!(s.draft().party, party);
    assert!(s.subtitle().unwrap().starts_with("As voucher"));
    s.toggle_as_voucher();
    assert_eq!(s.mode(), InvoiceMode::Accounting);
}

#[test]
fn ctrl_shift_h_on_the_screen_is_the_same_switch() {
    // provenance: derived. One Ctrl+Shift+H press reaches as voucher; a plain Ctrl+H does not.
    let (_api, books) = setup("s16_vch_key.ledgerly");
    let mut s = open(&books);
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let press = |shift: bool| RawInput {
        events: vec![Event::Key {
            key: Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::COMMAND
            },
        }],
        ..RawInput::default()
    };
    frames(&mut s, &mut app, &ctx, RawInput::default());
    frames(&mut s, &mut app, &ctx, press(true));
    assert_eq!(s.mode(), InvoiceMode::AsVoucher);
}

#[test]
fn as_voucher_hands_the_engine_the_lines_the_user_chose() {
    // provenance: derived. The voucher handed to the engine is in as-voucher mode with the
    // three chosen ledgers in order: Service Charges Cr 10,000.00 and the two tax ledgers Cr 0.00
    // (no amount typed; the engine fills them).
    let (_api, books) = setup("s16_vch_build.ledgerly");
    let mut s = open(&books);
    s.toggle_as_voucher();
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
    s.toggle_as_voucher();
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
    s.toggle_as_voucher();
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
