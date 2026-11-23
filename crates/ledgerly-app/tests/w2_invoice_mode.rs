//! Wave 2 S-B: Ctrl+H changes the invoice's mode between item invoice and accounting invoice.
//!
//! TallyPrime: "Press Ctrl+H (Change Mode) > select" the mode; the modes named are Item
//! Invoice, Accounting Invoice and As Voucher (help.tallysolutions.com/sales-of-goods-services/,
//! read 2026-11-23). The mock's invoice has an "Item invoice Ctrl+H" button, and its Ctrl+H
//! "switches between item invoice and accounting invoice" (design/mock-v2/index.html, the
//! `sales-invoice` screen). As Voucher is not built, so Ctrl+H switches between the two.
//!
//! Product choices, not documented by the pages read (they are silent on what a mode change
//! does to details already entered): the header (date, party, place, reference, narration) is
//! kept, and each mode's lines are kept while the other mode shows, so switching back brings them
//! back. A company that does not maintain inventory has no stock items to pick, so it refuses
//! the item mode and says why.
//!
//! Every expected value is derived by hand and the derivation is written next to it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, InvoiceMode, ItemInvoiceScreen};
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

fn group(which: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(which))
        .unwrap()
        .id
}

fn company(inventory: bool) -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Mode Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            inventory,
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

/// The company with a buyer "Sharma Traders" and an income ledger "Service Charges".
fn setup(name: &str, inventory: bool) -> Books {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(
        &path,
        &company(inventory),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    for l in [
        ledger("Sharma Traders", ReservedGroup::SundryDebtors),
        ledger("Service Charges", ReservedGroup::SalesAccounts),
    ] {
        let made = api.execute(Command::CreateLedger(l));
        assert!(matches!(made, Ok(Outcome::Value(_))), "{made:?}");
    }
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    books
}

fn open(books: &Books, mode: InvoiceMode) -> ItemInvoiceScreen {
    let sales = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    match ItemInvoiceScreen::open_in(books, sales, fy, date!(2026 - 11 - 21), mode).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

#[test]
fn switching_to_accounting_keeps_the_header_and_goes_to_the_first_particular() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (Ctrl+H, then the mode) and
    // the product choices in the module doc. Derived: an item invoice with the party picked and
    // the keyboard on the sales ledger field (a field accounting mode does not have) becomes an
    // accounting invoice with the same party; the keyboard moves to the first particular.
    let books = setup("w2_mode_to_acc.ledgerly", true);
    let m = &books.masters;
    let mut s = open(&books, InvoiceMode::Item);
    s.set_party_text("sharma");
    s.submit(m);
    let party = s.draft().party;
    assert!(party.is_some());
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    assert_eq!(s.focus(), Field::Account, "{:?}", s.error_text());
    s.change_mode();
    assert_eq!(s.mode(), InvoiceMode::Accounting);
    assert_eq!(s.draft().party, party);
    assert_eq!(s.focus(), Field::Ledger(0));
    let subtitle = s.subtitle().unwrap();
    assert!(subtitle.starts_with("Accounting invoice"), "{subtitle}");
}

#[test]
fn switching_back_brings_back_the_particulars_typed_before() {
    // provenance: product choice in the module doc (each mode's lines are kept). Derived: a
    // particular "Service Charges" for 500 survives a switch to item mode and back.
    let books = setup("w2_mode_back.ledgerly", true);
    let mut s = open(&books, InvoiceMode::Accounting);
    s.set_particular_text(0, "service");
    "500".clone_into(&mut s.draft_mut().particulars.get_mut(0).unwrap().amount_text);
    s.change_mode();
    assert_eq!(s.mode(), InvoiceMode::Item);
    s.change_mode();
    assert_eq!(s.mode(), InvoiceMode::Accounting);
    let row = s.draft().particulars.first().unwrap();
    assert_eq!(row.ledger_text, "service");
    assert_eq!(row.amount_text, "500");
}

#[test]
fn a_company_without_inventory_refuses_the_item_mode_and_says_why() {
    // provenance: product choice in the module doc. Derived: the mode stays accounting and the
    // screen shows an error naming inventory.
    let books = setup("w2_mode_no_inv.ledgerly", false);
    let mut s = open(&books, InvoiceMode::Accounting);
    s.change_mode();
    assert_eq!(s.mode(), InvoiceMode::Accounting);
    assert!(
        s.error_text().unwrap().contains("inventory"),
        "{:?}",
        s.error_text()
    );
}

fn ctrl_h() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
        ..RawInput::default()
    }
}

#[test]
fn ctrl_h_on_the_screen_changes_the_mode_and_the_key_bar_names_it() {
    // provenance: the mock's keymap (index.html KEYMAP, "Inside a voucher": Ctrl+H Change mode).
    // Derived: one Ctrl+H press on an item invoice makes it an accounting invoice; the key bar
    // lists Ctrl+H.
    let books = setup("w2_mode_key.ledgerly", true);
    let mut s = open(&books, InvoiceMode::Item);
    assert!(s.keys().iter().any(|k| k.keys == "Ctrl+H"));
    let mut app = AppCtx {
        books: Some(books.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for input in [RawInput::default(), ctrl_h()] {
        let out = ctx.run_ui(input, |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    }
    assert_eq!(s.mode(), InvoiceMode::Accounting);
}
