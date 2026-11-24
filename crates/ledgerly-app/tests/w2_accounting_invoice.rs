//! Wave 2 S-B: the sales invoice of a company that keeps no inventory, entered as an accounting
//! invoice: the party, then ledgers under Particulars with their amounts. The engine adds the
//! party line and the tax.
//!
//! TallyPrime records a sale without stock items "in accounting invoice mode": select the ledger
//! "under Particulars, and enter the Amount" (help.tallysolutions.com/sales-of-goods-services/,
//! read 2026-11-23). In that mode "The Sales ledger field will not appear" below the party
//! (help.tallysolutions.com/tally-prime/sales-process/faq-sales/, read 2026-11-23).
//!
//! Product choice, not documented by those pages: a company with Maintain Inventory off opens
//! its sales vouchers in accounting invoice mode (the pages are silent on what that setting does
//! to the mode; an item invoice has no items to pick there).
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

use egui::RawInput;
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, GstStatus, InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::shell::navigate::sales_screen;
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

/// Sharma Traders, place 27, Service Charges for 10,000.00; leaves the keyboard on the blank
/// second particular.
fn fill(s: &mut ItemInvoiceScreen, books: &Books) {
    let m = &books.masters;
    s.set_party_text("sharma");
    s.submit(m);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Reference, "no sales ledger field");
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(0));
    s.set_particular_text(0, "service");
    s.submit(m);
    assert_eq!(s.focus(), Field::LedgerAmount(0));
    "10000".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1), "{:?}", s.error_text());
}

#[test]
fn the_accounting_invoice_has_no_sales_ledger_field_and_builds_ledger_lines() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (ledger under Particulars,
    // then Amount) and the sales FAQ (no Sales ledger field in accounting invoice mode).
    // Derived: Enter goes Party, Place, Reference, then the first particular's ledger and amount;
    // the voucher handed to the engine is an accounting invoice with one line, Service Charges
    // Cr 10,000.00 (a sale credits its income ledger), and no inventory lines.
    let (_api, books) = setup("w2_acc_inv_build.ledgerly");
    let mut s = open(&books);
    fill(&mut s, &books);
    let v = s.take_compute_request(&books.masters).unwrap();
    assert_eq!(v.mode, VoucherMode::AccountingInvoice);
    assert_eq!(v.inventory_lines, Vec::new());
    let lines: Vec<(LedgerId, SignedAmount)> = v
        .ledger_lines
        .iter()
        .map(|l| (l.ledger, l.amount))
        .collect();
    assert_eq!(
        lines,
        vec![(
            id_of(&books, "Service Charges"),
            SignedAmount {
                amount: Paise::new(1_000_000),
                side: DrCr::Cr
            }
        )]
    );
}

#[test]
fn a_particular_amount_that_is_not_a_number_is_refused_and_keeps_the_keyboard() {
    // provenance: derived. "ten" is not an amount: Enter says so and stays on the field.
    let (_api, books) = setup("w2_acc_inv_amount.ledgerly");
    let m = &books.masters;
    let mut s = open(&books);
    s.set_party_text("sharma");
    s.submit(m);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    s.submit(m);
    s.set_particular_text(0, "service");
    s.submit(m);
    "ten".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::LedgerAmount(0));
    assert!(s.error_text().is_some());
}

#[test]
fn with_the_worker_the_engine_taxes_it_and_it_saves() {
    // provenance: derived. Intra-state (company and buyer in 27), Service Charges 10,000.00 at
    // 18%: CGST 9% = 900.00, SGST 9% = 900.00, invoice total 11,800.00 with no round-off. Saving
    // posts it: the stored voucher's party line is Sharma Traders Dr 11,800.00.
    let (api, books) = setup("w2_acc_inv_save.ledgerly");
    let mut s = open(&books);
    fill(&mut s, &books);
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let frame = |s: &mut ItemInvoiceScreen, app: &mut AppCtx| {
        let out = ctx.run_ui(RawInput::default(), |ui| {
            let _stay = s.ui(ui, app);
        });
        out.drop_without_applying_deltas();
    };
    for _ in 0..2000 {
        frame(&mut s, &mut app);
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
        frame(&mut s, &mut app);
        if s.info_text() == Some("Saved.") || s.error_text().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.info_text(), Some("Saved."), "{:?}", s.error_text());
    drop(app);
    let reopened = Api::open(&path("w2_acc_inv_save.ledgerly")).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let posted = books.vouchers.values().next().expect("one voucher posted");
    assert_eq!(posted.mode, VoucherMode::AccountingInvoice);
    let party = posted
        .ledger_lines
        .iter()
        .find(|l| l.ledger == id_of(&books, "Sharma Traders"))
        .unwrap();
    assert_eq!(party.amount, SignedAmount::dr(Paise::new(1_180_000)));
}

#[test]
fn f8_on_a_company_without_inventory_opens_the_accounting_invoice() {
    // provenance: product choice written in the module doc (the pages read are silent on
    // Maintain Inventory and the mode). Derived: F8's screen opens without an error, is the sales
    // invoice, and says it is an accounting invoice. Catches F8 opening a screen that refuses
    // Sales, as the accounting voucher screen does.
    let (_api, books) = setup("w2_acc_inv_f8.ledgerly");
    let screen = sales_screen(&books, date!(2026 - 11 - 21)).unwrap();
    assert_eq!(
        screen.title(&ledgerly_app::i18n::Tr::default()),
        "Sales invoice"
    );
    let subtitle = screen.subtitle(&ledgerly_app::i18n::Tr::default()).unwrap();
    assert!(subtitle.starts_with("Accounting invoice"), "{subtitle}");
}

/// The invoice up to the first particular: Sharma Traders, place 27, no reference.
fn to_particulars(s: &mut ItemInvoiceScreen, books: &Books) {
    let m = &books.masters;
    s.set_party_text("sharma");
    s.submit(m);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(0));
}

#[test]
fn enter_on_a_blank_second_particular_ends_them_and_a_zero_amount_is_refused() {
    // provenance: derived, as on the item lines: a blank row after the first ends the list and
    // goes to the narration (the blank row is dropped); a particular of 0.00 is refused (a line
    // with no amount carries no sale); typing over a picked ledger un-picks it.
    let (_api, books) = setup("w2_acc_inv_rows.ledgerly");
    let m = &books.masters;
    let mut s = open(&books);
    to_particulars(&mut s, &books);
    s.set_particular_text(0, "service");
    s.submit(m);
    "0".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::LedgerAmount(0));
    assert!(s.error_text().is_some());
    "10000".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1));
    s.submit(m);
    assert_eq!(s.focus(), Field::Narration);
    assert_eq!(s.draft().particulars.len(), 1);
    s.set_particular_text(0, "serv");
    assert_eq!(s.draft().particulars.first().unwrap().ledger, None);
}

#[test]
fn alt_c_on_a_particular_creates_its_ledger_without_a_worker() {
    // provenance: help.tallysolutions.com/ledgers-in-tallyprime/: "In the Voucher Creation
    // screen, when your cursor is on the ledger selection field, press Alt+C". Derived: Enter on
    // a name that matches nothing says how to create it; Alt+C opens the form with the typed
    // name; confirming moves on to the amount with no ledger behind the name yet; Enter on the
    // name says it is still being created; the host's answer (id 77) fills the particular.
    let (_api, books) = setup("w2_acc_inv_create.ledgerly");
    let m = &books.masters;
    let mut s = open(&books);
    to_particulars(&mut s, &books);
    s.set_particular_text(0, "Consulting Fees");
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(0));
    assert!(s.error_text().unwrap().contains("Alt+C"));
    s.begin_create();
    assert_eq!(s.creating_name(), Some("Consulting Fees"));
    s.confirm_create(
        m,
        "Consulting Fees",
        Some(group(ReservedGroup::SalesAccounts)),
    );
    assert!(!s.is_creating());
    assert_eq!(s.focus(), Field::LedgerAmount(0));
    let requests = s.take_master_requests();
    assert_eq!(requests.len(), 1);
    s.step_back();
    assert_eq!(s.focus(), Field::Ledger(0));
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(0));
    assert!(
        s.error_text().unwrap().contains("still being created"),
        "{:?}",
        s.error_text()
    );
    s.resolve_created_ledger("Consulting Fees", LedgerId(77));
    assert_eq!(
        s.draft().particulars.first().unwrap().ledger,
        Some(LedgerId(77))
    );
}

#[test]
fn alt_c_on_a_particular_with_the_worker_creates_the_ledger_in_the_store() {
    // provenance: help.tallysolutions.com/keyboard-shortcuts-tally-prime/ (Alt+C creates
    // "another master on the fly while creating ... vouchers"). Derived: the engine creates
    // "Consulting Fees" under Sales Accounts and the particular holds the id the store gave it
    // (found by name after reopening the file).
    let (api, books) = setup("w2_acc_inv_create_worker.ledgerly");
    let masters = books.masters.clone();
    let mut s = open(&books);
    to_particulars(&mut s, &books);
    s.set_particular_text(0, "Consulting Fees");
    s.begin_create();
    s.confirm_create(
        &masters,
        "Consulting Fees",
        Some(group(ReservedGroup::SalesAccounts)),
    );
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        let out = ctx.run_ui(RawInput::default(), |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
        if s.draft().particulars.first().unwrap().ledger.is_some() || s.error_text().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.error_text(), None);
    let picked = s.draft().particulars.first().unwrap().ledger;
    drop(app);
    let reopened = Api::open(&path("w2_acc_inv_create_worker.ledgerly")).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let made = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Consulting Fees")
        .expect("the ledger was created");
    assert_eq!(picked, Some(made.id));
    assert_eq!(made.group, group(ReservedGroup::SalesAccounts));
}

#[test]
fn the_open_ledger_list_keeps_its_height_and_the_invoice_stays_in_the_window() {
    // provenance: derived from the layout. Window 1280 by 800 points; the keyboard is on the
    // empty party field, so the list shows every ledger (16 or more: the 4 of the fixture and 12
    // more). Each row is at least 18 points, so the rows are taller than the list's 220-point cap
    // and the list is drawn at the cap (at least 220 points with its frame). The header, the
    // list, the lines and the narration then fit in 800 points, so the narration's bottom edge is
    // inside the window. Before the fix the header grid laid its columns out at their minimum
    // width, every ledger name wrapped a letter or two per line, and the narration was pushed far
    // below the window; the list was also cut to about three rows.
    let (mut api, _) = setup("w2_acc_inv_layout.ledgerly");
    for n in 1..=12 {
        let made = api.execute(Command::CreateLedger(ledger(
            &format!("Customer {n:02}"),
            ReservedGroup::SundryDebtors,
        )));
        assert!(matches!(made, Ok(Outcome::Value(_))), "{made:?}");
    }
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1280.0, 800.0));
    let sales = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    for mode in [InvoiceMode::Accounting, InvoiceMode::Item] {
        let fy = FinancialYear::april(2026).unwrap();
        let Ok(Outcome::Value(mut s)) =
            ItemInvoiceScreen::open_in(&books, sales, fy, date!(2026 - 11 - 21), mode)
        else {
            panic!("the screen did not open");
        };
        let mut app = AppCtx {
            tr: ledgerly_app::i18n::Tr::default(),
            books: Some(books.clone()),
            api: None,
            lock_date: None,
            ai_tier: None,
        };
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let input = RawInput {
                screen_rect: Some(window),
                ..RawInput::default()
            };
            let out = ctx.run_ui(input, |ui| {
                let _stay = s.ui(ui, &mut app);
            });
            out.drop_without_applying_deltas();
        }
        let list = s.picker_rect().expect("the party list is open");
        assert!(list.height() >= 220.0, "{mode:?}: list {list:?}");
        let narration = s.narration_rect().expect("the narration is drawn");
        assert!(
            narration.max.y <= window.max.y,
            "{mode:?}: narration {narration:?} must lie inside the window {window:?}"
        );
    }
}
