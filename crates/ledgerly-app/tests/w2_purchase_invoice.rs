//! Wave 2 S-B: purchase, credit note and debit note open the invoice screen, as sales does.
//!
//! TallyPrime's keys: F9 Purchase, Alt+F6 Credit Note, Alt+F5 Debit Note
//! (help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/, read
//! 2026-11-23). A purchase of services is recorded "in accounting invoice mode", with the service
//! ledger under Particulars (help.tallysolutions.com/purchase-goods-and-services-tally/, read
//! 2026-11-23).
//!
//! Product choice, as for sales (`tests/w2_accounting_invoice.rs`): a company with Maintain
//! Inventory off opens these invoices in accounting invoice mode.
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

use egui::{Key, RawInput};
use ledgerly_api::{Api, Command};
use ledgerly_app::keymap::{Action, Chord, classic, standard};
use ledgerly_app::palette::PaletteTarget;
use ledgerly_app::screens::item_invoice::{Field, GstStatus, InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::shell::navigate::invoice_screen;
use ledgerly_app::shell::sidebar::{CommandRoute, command_route, screen_for};
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
        name: "Buyer Co".to_owned(),
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

/// The company with a Maharashtra supplier "Patil Services", "Maintenance Charges" (a purchase
/// ledger taxable at 18%), and Input CGST and SGST ledgers.
fn setup(name: &str) -> (Api, Books) {
    let path = path(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let supplier = Ledger {
        party: Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(gstin("27FGHIJ5678K1Z")),
            pan: None,
            state: Some(state(27)),
            address: Address::default(),
        }),
        ..ledger("Patil Services", ReservedGroup::SundryCreditors)
    };
    let charges = Ledger {
        gst: Some(GstClassification {
            hsn: None,
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }),
        ..ledger("Maintenance Charges", ReservedGroup::PurchaseAccounts)
    };
    let tax = |name: &str, kind| Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: None,
        }),
        ..ledger(name, ReservedGroup::DutiesAndTaxes)
    };
    for l in [
        supplier,
        charges,
        tax("Input CGST", TaxLedgerKind::Cgst),
        tax("Input SGST", TaxLedgerKind::Sgst),
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

fn app(books: &Books, api: Option<Api>) -> AppCtx {
    AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: api.map(|a| ApiWorker::spawn(a).unwrap()),
        lock_date: None,
        ai_tier: None,
    }
}

fn alt(key: Key) -> Chord {
    Chord {
        alt: true,
        ..Chord::key(key)
    }
}

#[test]
fn f9_alt_f6_and_alt_f5_are_purchase_credit_note_and_debit_note_in_both_presets() {
    // provenance: help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-
    // tally-prime/ (read 2026-11-23): F9 Purchase, Alt+F6 Credit Note, Alt+F5 Debit Note. The
    // standard preset keeps Tally's function keys as aliases (DESIGN.md section 4), and the mock's
    // keymap (index.html KEYMAP, Vouchers) names no other key for the notes.
    for km in [classic(), standard()] {
        assert_eq!(
            km.action_for(Chord::key(Key::F9)),
            Some(Action::NewPurchase)
        );
        assert_eq!(km.action_for(alt(Key::F6)), Some(Action::NewCreditNote));
        assert_eq!(km.action_for(alt(Key::F5)), Some(Action::NewDebitNote));
    }
}

#[test]
fn purchase_and_the_notes_are_screen_commands() {
    // provenance: derived. Each now opens a screen, so the shell hands it to the screen route
    // (and the palette lists it) instead of hiding it.
    for action in [
        Action::NewPurchase,
        Action::NewCreditNote,
        Action::NewDebitNote,
    ] {
        assert_eq!(command_route(action), CommandRoute::Screen, "{action:?}");
    }
}

#[test]
fn each_command_opens_its_invoice_in_accounting_mode_without_inventory() {
    // provenance: the seeded voucher type names (Purchase, Credit Note, Debit Note:
    // ledgerly-model seed_voucher_types.rs) and the screen title: "<type> invoice" for a purchase,
    // the bare type name for a note (CGST Act s.34 calls it a credit or debit note, not an
    // invoice; changed in s11). Derived: on a company with inventory off each opens in
    // accounting invoice mode; a note's head says "<Credit|Debit> note · accounting mode" (it is
    // not an invoice; changed in s11).
    let (_api, books) = setup("w2_pur_inv_routes.ledgerly");
    let ctx = app(&books, None);
    for (action, title, head) in [
        (Action::NewPurchase, "Purchase", "Accounting invoice"),
        (
            Action::NewCreditNote,
            "Credit Note",
            "Credit note \u{b7} accounting mode",
        ),
        (
            Action::NewDebitNote,
            "Debit Note",
            "Debit note \u{b7} accounting mode",
        ),
    ] {
        let screen = screen_for(&PaletteTarget::Command(action), &ctx).unwrap();
        assert_eq!(screen.title(&ledgerly_app::i18n::Tr::default()), title);
        let subtitle = screen.subtitle(&ledgerly_app::i18n::Tr::default()).unwrap();
        assert!(subtitle.starts_with(head), "{subtitle}");
    }
}

#[test]
fn invoice_screen_refuses_a_kind_that_is_not_an_invoice() {
    // provenance: derived. A journal is not entered on the invoice screen; asking for one says
    // why instead of opening a wrong screen.
    let (_api, books) = setup("w2_pur_inv_journal.ledgerly");
    assert!(invoice_screen(&books, BaseVoucherKind::Journal, date!(2026 - 11 - 21)).is_err());
}

fn open_purchase(books: &Books) -> ItemInvoiceScreen {
    let id = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Purchase)
        .unwrap()
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    match ItemInvoiceScreen::open_in(
        books,
        id,
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

/// Patil Services, place 27, Maintenance Charges for 10,000.00; leaves the keyboard on the
/// blank second particular.
fn fill(s: &mut ItemInvoiceScreen, m: &ledgerly_model::Masters) {
    // changed in s12: a purchase asks the supplier's invoice number and date first, then the
    // party A/c name (help.tallysolutions.com/purchase-goods-services/, read 2026-11-25: "Supplier
    // Invoice No. and Date", then "Party A/c name"), then the place of supply (the Party Details
    // that follow the party); both bill fields are optional.
    assert_eq!(s.focus(), Field::Reference);
    s.submit(m);
    assert_eq!(s.focus(), Field::ReferenceDate);
    s.submit(m);
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("patil");
    s.submit(m);
    assert_eq!(s.focus(), Field::Place);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(0));
    s.set_particular_text(0, "maint");
    s.submit(m);
    "10000".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1), "{:?}", s.error_text());
}

#[test]
fn a_purchase_of_services_debits_the_ledger_credits_the_supplier_and_saves() {
    // provenance: help.tallysolutions.com/purchase-goods-and-services-tally/ (the service ledger
    // under Particulars in accounting invoice mode). Derived: a purchase debits its expense
    // ledger, so Maintenance Charges is Dr 10,000.00. Intra-state (both in 27) at 18%: input
    // CGST 9% = 900.00 and SGST 9% = 900.00, total 11,800.00, no round-off. The supplier is
    // credited with the total: Patil Services Cr 11,800.00.
    let (api, books) = setup("w2_pur_inv_save.ledgerly");
    let m = &books.masters;
    let mut s = open_purchase(&books);
    fill(&mut s, m);
    let built = s.take_compute_request(m).unwrap();
    assert_eq!(built.mode, VoucherMode::AccountingInvoice);
    let lines: Vec<(LedgerId, SignedAmount)> = built
        .ledger_lines
        .iter()
        .map(|l| (l.ledger, l.amount))
        .collect();
    assert_eq!(
        lines,
        vec![(
            id_of(&books, "Maintenance Charges"),
            SignedAmount {
                amount: Paise::new(1_000_000),
                side: DrCr::Dr
            }
        )]
    );
    let mut s = open_purchase(&books);
    fill(&mut s, m);
    let mut ctx = app(&books, Some(api));
    let egui_ctx = egui::Context::default();
    let frame = |s: &mut ItemInvoiceScreen, app: &mut AppCtx| {
        let out = egui_ctx.run_ui(RawInput::default(), |ui| {
            let _stay = s.ui(ui, app);
        });
        out.drop_without_applying_deltas();
    };
    for _ in 0..2000 {
        frame(&mut s, &mut ctx);
        if !matches!(s.gst_status(m), GstStatus::NotComputedYet) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let GstStatus::Computed(block) = s.gst_status(m) else {
        panic!("not computed: {:?}", s.gst_status(m));
    };
    assert_eq!(block.invoice_total, Paise::new(1_180_000));
    s.save(m);
    for _ in 0..2000 {
        frame(&mut s, &mut ctx);
        if s.info_text() == Some("Saved.") || s.error_text().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.info_text(), Some("Saved."), "{:?}", s.error_text());
    drop(ctx);
    let reopened = Api::open(&path("w2_pur_inv_save.ledgerly")).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let posted = books.vouchers.values().next().expect("one voucher posted");
    let side = |name: &str| {
        posted
            .ledger_lines
            .iter()
            .find(|l| l.ledger == id_of(&books, name))
            .unwrap()
            .amount
    };
    assert_eq!(
        side("Patil Services"),
        SignedAmount::cr(Paise::new(1_180_000))
    );
    assert_eq!(side("Input CGST"), SignedAmount::dr(Paise::new(90_000)));
    assert_eq!(side("Input SGST"), SignedAmount::dr(Paise::new(90_000)));
}
