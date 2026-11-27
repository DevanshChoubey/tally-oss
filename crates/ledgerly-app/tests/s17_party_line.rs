//! S17: in As voucher mode the party is line 1 of the table, typed there, not in the header.
//!
//! The accounting product's sales page (help.tallysolutions.com/sales-of-goods-services/, read
//! 2026-11-26): after Ctrl+H > As Voucher, "In the Dr field, select the party ledger" (the Cr or
//! Dr line comes first), then the Sales ledger on the Cr line; the GST ledgers follow
//! (help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/, read 2026-11-26).
//! So the party input is the first row of the table; the head keeps no Party box in this mode.
//!
//! Every expected value is derived from that order and written beside the assertion.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use egui::RawInput;
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, FinancialYear, GroupId, Gstin, LedgerId, Outcome, Paise, Rate,
    SignedAmount, StateCode, gstin_check_character,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstClassification,
    GstRegistration, Ledger, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup,
    TaxDetails, TaxLedgerKind, Taxability,
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
        group: Some(group(which)),
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
/// Ctrl+H, then the row for `mode` with Down and Enter: what the user does with the keys.
fn choose(s: &mut ItemInvoiceScreen, mode: InvoiceMode) {
    s.open_mode_list();
    let row = s.mode_choices().iter().position(|m| *m == mode).unwrap();
    while s.mode_list() != Some(row) {
        s.step_mode_list(s.mode_list().unwrap() < row);
    }
    s.pick_listed_mode();
}

/// The service line items typed after the party: Service Charges 10,000.00, Output CGST and
/// Output SGST with no amount (the engine fills them). The party is typed by the caller.
fn fill_ledgers(s: &mut ItemInvoiceScreen, books: &Books) {
    let m = &books.masters;
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

fn app_for(books: &Books) -> AppCtx {
    AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    }
}

fn ledger_heading_bottom(s: &ItemInvoiceScreen) -> f32 {
    s.heading_rects()
        .iter()
        .find(|(n, _)| *n == "Ledger" || *n == "Particulars")
        .map(|(_, r)| r.bottom())
        .expect("the table has a ledger heading")
}

#[test]
fn as_voucher_has_the_party_input_as_line_one_of_the_table_not_in_the_head() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (read 2026-11-26): in As Voucher
    // mode the party ledger is the first line (Dr) the user fills. Derived: the one "Party" input
    // sits below the table's Ledger heading and above the second row (the Sales ledger line);
    // accounting mode keeps the party in the head, above the heading.
    let (_api, books) = setup("s17_line1.ledgerly");
    let mut s = open(&books);
    let mut app = app_for(&books);
    let ctx = egui::Context::default();
    frames(&mut s, &mut app, &ctx, RawInput::default());
    let head_party = s.field_rect("Party").unwrap();
    assert!(
        head_party.bottom() <= ledger_heading_bottom(&s),
        "accounting mode: the party is in the head"
    );
    choose(&mut s, InvoiceMode::AsVoucher);
    frames(&mut s, &mut app, &ctx, RawInput::default());
    frames(&mut s, &mut app, &ctx, RawInput::default());
    let party = s.field_rect("Party").unwrap();
    assert!(
        party.top() >= ledger_heading_bottom(&s),
        "as voucher: the party input is a row of the table, below the headings ({party:?})"
    );
}

#[test]
fn the_list_for_line_one_offers_parties_under_the_line() {
    // provenance: derived. On line 1 the type-ahead list is the party list ("Parties"), hung
    // under the line-1 input, which is in the table.
    let (_api, books) = setup("s17_list.ledgerly");
    let mut s = open(&books);
    let mut app = app_for(&books);
    let ctx = egui::Context::default();
    choose(&mut s, InvoiceMode::AsVoucher);
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("sharma");
    for _ in 0..3 {
        frames(&mut s, &mut app, &ctx, RawInput::default());
    }
    let party = s.field_rect("Party").unwrap();
    let list = s.picker_rect().expect("the party list is open");
    assert!(party.top() >= ledger_heading_bottom(&s));
    assert!(
        list.top() >= party.bottom() - 1.0,
        "the list hangs under line 1: {list:?} vs {party:?}"
    );
    assert!(
        s.pick_labels(&books.masters)
            .iter()
            .any(|l| l.contains("Sharma Traders")),
        "{:?}",
        s.pick_labels(&books.masters)
    );
}

#[test]
fn enter_order_is_date_party_place_then_the_ledger_lines() {
    // provenance: help.tallysolutions.com/sales-of-goods-services/ (read 2026-11-26): the party
    // ledger comes first, then the Sales ledger and the tax ledgers. Derived: the keyboard starts
    // on the party (line 1), Place of supply follows (the Party Details step of the product,
    // gst-sales-tally page), then Reference, then the first typed ledger.
    let (_api, books) = setup("s17_order.ledgerly");
    let m = &books.masters;
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    let mut seen = vec![s.focus()];
    s.set_party_text("sharma");
    "27".clone_into(&mut s.draft_mut().place_text);
    for _ in 0..3 {
        s.submit(m);
        seen.push(s.focus());
    }
    s.set_particular_text(0, "service");
    s.submit(m);
    seen.push(s.focus());
    assert_eq!(
        seen,
        vec![
            Field::Party,
            Field::Place,
            Field::Reference,
            Field::Ledger(0),
            Field::LedgerAmount(0),
        ]
    );
}

#[test]
fn switching_modes_keeps_the_party_in_both_directions() {
    // provenance: derived. The party chosen on line 1 of As voucher is the header's party in
    // accounting mode and back again.
    let (_api, books) = setup("s17_switch.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    s.set_party_text("sharma");
    s.submit(&books.masters);
    let party = s.draft().party;
    assert_eq!(party, Some(id_of(&books, "Sharma Traders")));
    choose(&mut s, InvoiceMode::Accounting);
    assert_eq!(s.draft().party, party);
    assert_eq!(s.draft().party_text, "Sharma Traders");
    choose(&mut s, InvoiceMode::AsVoucher);
    assert_eq!(s.draft().party, party);
    let rows = s.voucher_rows(&books.masters);
    assert_eq!(
        rows.first().map(|r| r.ledger.as_str()),
        Some("Sharma Traders")
    );
}

fn walk_to(s: &mut ItemInvoiceScreen, field: Field) {
    for _ in 0..40 {
        if s.focus() == field {
            return;
        }
        s.step_back();
    }
    panic!("never reached {field:?}, on {:?}", s.focus());
}

#[test]
fn ctrl_d_on_a_ledger_row_removes_it_and_keeps_the_party_line() {
    // provenance: derived. Rows: 1 party, 2 Service Charges, 3 Output CGST, 4 Output SGST. Ctrl+D
    // on the Output CGST row (particular index 1) removes that row; the party line is untouched.
    let (_api, books) = setup("s17_del_row.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    s.set_party_text("sharma");
    s.submit(&books.masters);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(&books.masters);
    s.submit(&books.masters);
    fill_ledgers(&mut s, &books);
    walk_to(&mut s, Field::Ledger(1));
    s.delete_line();
    let names: Vec<String> = s
        .voucher_rows(&books.masters)
        .into_iter()
        .map(|r| r.ledger)
        .collect();
    assert_eq!(
        names,
        vec!["Sharma Traders", "Service Charges", "Output SGST"]
    );
    assert_eq!(s.draft().party, Some(id_of(&books, "Sharma Traders")));
}

#[test]
fn ctrl_d_on_line_one_empties_the_party_and_leaves_the_ledger_rows() {
    // provenance: derived from the rule that a lone line is only emptied (the engine needs a
    // party, so line 1 cannot be removed): Ctrl+D on the party line clears the party and keeps
    // the keyboard on it; the typed ledger rows stay.
    let (_api, books) = setup("s17_del_party.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    s.set_party_text("sharma");
    s.submit(&books.masters);
    "27".clone_into(&mut s.draft_mut().place_text);
    s.submit(&books.masters);
    s.submit(&books.masters);
    fill_ledgers(&mut s, &books);
    walk_to(&mut s, Field::Party);
    s.delete_line();
    assert_eq!(s.draft().party, None);
    assert_eq!(s.draft().party_text, "");
    assert_eq!(s.focus(), Field::Party);
    assert_eq!(
        s.draft()
            .particulars
            .iter()
            .filter(|r| !r.ledger_text.is_empty())
            .count(),
        3
    );
}

#[test]
fn alt_c_on_line_one_creates_the_party_ledger() {
    // provenance: help.tallysolutions.com/ledgers-in-tallyprime/ (read in an earlier round, as
    // create.rs cites): Alt+C on a ledger selection field creates a ledger. Derived: on line 1
    // with "Patel Stores" typed, Alt+C opens the form with that name, and confirming it makes the
    // party text "Patel Stores", queues the master request and moves on to Place of supply.
    let (_api, books) = setup("s17_altc.ledgerly");
    let mut s = open(&books);
    choose(&mut s, InvoiceMode::AsVoucher);
    s.set_party_text("Patel Stores");
    let sundry = group(ReservedGroup::SundryDebtors);
    s.begin_create();
    assert!(s.is_creating());
    assert_eq!(s.creating_name(), Some("Patel Stores"));
    s.confirm_create(&books.masters, "Patel Stores", Some(sundry));
    assert_eq!(s.draft().party_text, "Patel Stores");
    assert_eq!(s.take_master_requests().len(), 1);
    let rows = s.voucher_rows(&books.masters);
    assert_eq!(
        rows.first().map(|r| r.ledger.as_str()),
        Some("Patel Stores")
    );
}
