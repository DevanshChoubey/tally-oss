//! Wave 2 S-B: Alt+C on the item invoice's party and sales (or purchase) ledger fields creates a
//! ledger on the fly and comes back with it in the field.
//!
//! TallyPrime does this: "In the Voucher Creation screen, when your cursor is on the ledger
//! selection field, press Alt+C" (help.tallysolutions.com/ledgers-in-tallyprime/), and Alt+C is
//! "To create another master on the fly while creating masters and vouchers"
//! (help.tallysolutions.com/keyboard-shortcuts-tally-prime/). The mock's sales invoice key bar
//! lists "Alt+C Create master" (`design/mock-v2/index.html:1344`).
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
use ledgerly_app::screens::item_invoice::{Field, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, NegativeStockPolicy,
    ReservedGroup,
};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Invoice Create Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn reserved(which: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(which))
        .unwrap()
        .id
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn ledger(name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(0),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
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

/// A new company with one customer, "Sharma Traders", and one sales ledger, "Sales - Local".
fn setup(name: &str) -> (Api, Books) {
    let path = path(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for (name, group) in [
        ("Sharma Traders", ReservedGroup::SundryDebtors),
        ("Sales - Local", ReservedGroup::SalesAccounts),
    ] {
        let made = api.execute(Command::CreateLedger(ledger(name, reserved(group))));
        assert!(matches!(made, Ok(Outcome::Value(_))), "{made:?}");
    }
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    (api, books)
}

/// A sales invoice on the seeded Sales voucher type.
fn open(books: &Books) -> ItemInvoiceScreen {
    let sales = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .expect("a new company has a Sales voucher type")
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    match ItemInvoiceScreen::open(books, sales, fy, date!(2026 - 11 - 21)).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn app(books: Books, api: Option<Api>) -> AppCtx {
    AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books),
        api: api.map(|a| ApiWorker::spawn(a).unwrap()),
        lock_date: None,
        ai_tier: None,
    }
}

fn frame(ctx: &egui::Context, s: &mut ItemInvoiceScreen, app: &mut AppCtx, input: RawInput) {
    let out = ctx.run_ui(input, |ui| {
        let _stay = s.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

fn alt_c() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::ALT,
        }],
        ..RawInput::default()
    }
}

/// The screen with "Ramesh Kirana" typed in the party field and Alt+C pressed there.
fn party_form(app: &mut AppCtx) -> (egui::Context, ItemInvoiceScreen) {
    let ctx = egui::Context::default();
    let mut s = open(app.books.as_ref().unwrap());
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("Ramesh Kirana");
    frame(&ctx, &mut s, app, RawInput::default());
    frame(&ctx, &mut s, app, alt_c());
    (ctx, s)
}

#[test]
fn alt_c_on_the_party_field_opens_a_ledger_form_with_the_typed_name() {
    // provenance: help.tallysolutions.com/ledgers-in-tallyprime/: Alt+C on a voucher's ledger
    // selection field creates a ledger. Derived: the form opens with the text typed in the field,
    // "Ramesh Kirana", as its name (voucher entry does the same).
    let (_api, books) = setup("w2_inv_create_opens.ledgerly");
    let mut app = app(books, None);
    let (_ctx, s) = party_form(&mut app);
    assert!(s.is_creating());
    assert_eq!(s.creating_name(), Some("Ramesh Kirana"));
}

#[test]
fn alt_c_on_an_item_field_opens_nothing() {
    // provenance: the TallyPrime pages read name Alt+C on a voucher's ledger selection field and a
    // ledger's Under field only (design/PARITY-GAPS.md); an item field is not among them, so this
    // screen keeps refusing to create items there.
    let (_api, books) = setup("w2_inv_create_item.ledgerly");
    let mut app = app(books, None);
    let ctx = egui::Context::default();
    let mut s = open(app.books.as_ref().unwrap());
    let m = app.books.as_ref().unwrap().masters.clone();
    s.draft_mut().party = None;
    s.set_item_text(0, "Widget");
    // Walk the keyboard to the first item field with the public focus field.
    while s.focus() != Field::Item(0) {
        let before = s.focus();
        match before {
            Field::Party => {
                s.set_party_text("Sharma");
                s.submit(&m);
            }
            Field::Place => {
                "27".clone_into(&mut s.draft_mut().place_text);
                s.submit(&m);
            }
            Field::Account => {
                s.set_account_text("Sales - Local");
                s.submit(&m);
            }
            Field::Date
            | Field::Number
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => s.submit(&m),
        }
        assert_ne!(
            s.focus(),
            before,
            "stuck on {before:?}: {:?}",
            s.error_text()
        );
    }
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, alt_c());
    assert!(!s.is_creating());
}

#[test]
fn confirming_puts_the_name_in_the_party_field_and_asks_for_the_ledger() {
    // provenance: derived. Confirming with Sundry Debtors records one request for the host with
    // that name and group; the field shows the name and has no ledger until the engine names it,
    // so the invoice cannot be saved on a ledger that does not exist yet.
    let (_api, books) = setup("w2_inv_create_confirm.ledgerly");
    let masters = books.masters.clone();
    let mut app = app(books, None);
    let (_ctx, mut s) = party_form(&mut app);
    s.confirm_create(
        &masters,
        "Ramesh Kirana",
        Some(reserved(ReservedGroup::SundryDebtors)),
    );
    assert!(!s.is_creating());
    let requests = s.take_master_requests();
    assert_eq!(requests.len(), 1);
    let request = requests.first().unwrap();
    assert_eq!(request.name, "Ramesh Kirana");
    assert_eq!(request.group, reserved(ReservedGroup::SundryDebtors));
    assert_eq!(s.draft().party_text, "Ramesh Kirana");
    assert_eq!(s.draft().party, None);
}

#[test]
fn the_created_ledger_fills_the_party_and_a_refusal_says_why() {
    // provenance: derived. When the host reports "Ramesh Kirana" created as ledger 77, the party
    // is 77. When it refuses instead, the party stays empty and the error carries the reason.
    let (_api, books) = setup("w2_inv_create_resolve.ledgerly");
    let masters = books.masters.clone();
    let mut app = app(books, None);
    let debtors = Some(reserved(ReservedGroup::SundryDebtors));

    let (_ctx, mut s) = party_form(&mut app);
    s.confirm_create(&masters, "Ramesh Kirana", debtors);
    s.resolve_created_ledger("Ramesh Kirana", LedgerId(77));
    assert_eq!(s.draft().party, Some(LedgerId(77)));

    let (_ctx, mut s) = party_form(&mut app);
    s.confirm_create(&masters, "Ramesh Kirana", debtors);
    s.reject_created_ledger("Ramesh Kirana", "the name is taken");
    assert_eq!(s.draft().party, None);
    let error = s.error_text().unwrap();
    assert!(error.contains("the name is taken"), "{error}");
}

#[test]
fn a_name_already_in_the_ledgers_is_refused() {
    // provenance: derived. The books have "Sharma Traders" (made in `setup`); creating another
    // "sharma traders" (any case) is refused and nothing is asked for.
    let (_api, books) = setup("w2_inv_create_dup.ledgerly");
    let masters = books.masters.clone();
    assert!(
        masters.ledgers.values().any(|l| l.name == "Sharma Traders"),
        "setup made Sharma Traders"
    );
    let mut app = app(books, None);
    let (_ctx, mut s) = party_form(&mut app);
    s.confirm_create(
        &masters,
        "sharma traders",
        Some(reserved(ReservedGroup::SundryDebtors)),
    );
    assert_eq!(s.take_master_requests(), Vec::new());
    assert!(s.error_text().is_some());
}

#[test]
fn alt_c_on_the_sales_ledger_field_fills_it() {
    // provenance: as above; the sales ledger field is a ledger selection field too. Derived: a
    // ledger "Sales - Retail" created for it as ledger 78 becomes the invoice's sales ledger, and
    // the party is untouched.
    let (_api, books) = setup("w2_inv_create_account.ledgerly");
    let masters = books.masters.clone();
    let mut app = app(books, None);
    let ctx = egui::Context::default();
    let mut s = open(app.books.as_ref().unwrap());
    s.set_party_text("Sharma");
    s.submit(&masters);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(&masters);
    assert_eq!(s.focus(), Field::Account);
    let party = s.draft().party;
    assert!(party.is_some());
    s.set_account_text("Sales - Retail");
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, alt_c());
    assert_eq!(s.creating_name(), Some("Sales - Retail"));
    s.confirm_create(
        &masters,
        "Sales - Retail",
        Some(reserved(ReservedGroup::SalesAccounts)),
    );
    s.resolve_created_ledger("Sales - Retail", LedgerId(78));
    assert_eq!(s.draft().account, Some(LedgerId(78)));
    assert_eq!(s.draft().party, party);
}

#[test]
fn with_the_worker_the_ledger_is_created_and_lands_in_the_party() {
    // provenance: help.tallysolutions.com/keyboard-shortcuts-tally-prime/ ("create another master
    // on the fly while creating ... vouchers"). Derived: the engine creates "Ramesh Kirana" under
    // Sundry Debtors, and the party field holds the id the store gives it (found by name after
    // reopening the file). Catches the request never sent, or the answer never read.
    let (api, books) = setup("w2_inv_create_worker.ledgerly");
    let masters = books.masters.clone();
    let mut app = app(books, Some(api));
    let (ctx, mut s) = party_form(&mut app);
    s.confirm_create(
        &masters,
        "Ramesh Kirana",
        Some(reserved(ReservedGroup::SundryDebtors)),
    );
    for _ in 0..2000 {
        frame(&ctx, &mut s, &mut app, RawInput::default());
        if s.draft().party.is_some() || s.error_text().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.error_text(), None);
    let party = s.draft().party;
    drop(app);
    let reopened = Api::open(&path("w2_inv_create_worker.ledgerly")).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let made = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Ramesh Kirana")
        .expect("the ledger was created");
    assert_eq!(party, Some(made.id));
    assert_eq!(made.group, Some(reserved(ReservedGroup::SundryDebtors)));
}

#[test]
fn the_key_bar_offers_alt_c() {
    // provenance: index.html:1344, the sales invoice keys: "Alt+C" Create master.
    let (_api, books) = setup("w2_inv_create_keys.ledgerly");
    let s = open(&books);
    let keys = s.keys(&ledgerly_app::i18n::Tr::default());
    assert!(
        keys.iter()
            .any(|k| k.keys == "Alt+C" && k.label == "Create master"),
        "{keys:?}"
    );
}
