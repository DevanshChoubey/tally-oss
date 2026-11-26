//! S18: Ctrl+P on the item invoice screen prints the invoice it just saved.
//!
//! Provenance: as `s18_entry_print.rs` (help.tallysolutions.com/print-documents/ and
//! /print-invoices-reports/, read 2026-11-26): record the invoice, then Ctrl+P (Current) shows the
//! preview. The preview is headed like the mock's `sales-invoice.print-preview`: head "Sales invoice
//! <number>", subtitle "Saved · <party> · <date>". Derived: the invoice is Ka Buyer's, Ten Widgets
//! at Rs 1,000.00 on 5 Oct 2026 (the c2 derivation: Rs 11,800.00 with CGST and SGST).
//! A company with no address cannot print a tax invoice (the print crate refuses with a reason),
//! so the test company has one.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, GstStatus, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, FinancialYear, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, gstin_check_character,
};
use ledgerly_model::seed_godowns::default_godowns;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstHistory, GstRegistration,
    Ledger, Masters, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup, StockItem,
    TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod,
};
use time::OffsetDateTime;
use time::macros::date;

const BUYER: LedgerId = LedgerId(10);
const SALES: LedgerId = LedgerId(20);
const CGST: LedgerId = LedgerId(30);
const SGST: LedgerId = LedgerId(31);
const IGST: LedgerId = LedgerId(32);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn plain(id: LedgerId, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
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

fn tax(id: LedgerId, name: &str, kind: TaxLedgerKind) -> Ledger {
    Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: None,
        }),
        ..plain(id, name, ReservedGroup::DutiesAndTaxes)
    }
}

fn gstin_of(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn ledgers() -> Vec<Ledger> {
    vec![
        Ledger {
            party: Some(PartyDetails {
                registration: RegistrationType::Regular,
                gstin: Some(gstin_of("29FGHIJ5678K1Z")),
                pan: None,
                state: Some(state(29)),
                address: Address {
                    lines: vec!["12 Test Road".to_owned(), "Bengaluru".to_owned()],
                    ..Address::default()
                },
            }),
            ..plain(BUYER, "Ka Buyer", ReservedGroup::SundryDebtors)
        },
        plain(SALES, "Sales", ReservedGroup::SalesAccounts),
        tax(CGST, "CGST", TaxLedgerKind::Cgst),
        tax(SGST, "SGST", TaxLedgerKind::Sgst),
        tax(IGST, "IGST", TaxLedgerKind::Igst),
    ]
}

fn unit() -> Unit {
    Unit {
        id: UnitId(1),
        symbol: "pcs".to_owned(),
        formal_name: "Pieces".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: Some("PCS".to_owned()),
        tally: None,
    }
}

fn widget() -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: StockItemId(1),
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: Some(Hsn::parse("8471").unwrap()),
        gst_history: vec![GstHistory {
            effective_from: date!(2025 - 09 - 22),
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Test Road".to_owned(), "Bengaluru".to_owned()],
            ..Address::default()
        },
        state: state(29),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin_of("29ABCDE1234F1Z"),
            state: state(29),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// The worker's company file and the same masters as a `Books` value for the screen.
fn setup(name: &str) -> (ApiWorker, Books) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("c2_item_invoice_gst");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let mut masters = Masters::default();
    for l in ledgers() {
        let out = api.execute(Command::CreateLedger(l.clone())).unwrap();
        assert!(matches!(out, Outcome::Value(_)));
        masters.ledgers.insert(l.id, l);
    }
    assert!(matches!(
        api.execute(Command::CreateUnit(unit())).unwrap(),
        Outcome::Value(_)
    ));
    assert!(matches!(
        api.execute(Command::CreateStockItem(widget())).unwrap(),
        Outcome::Value(_)
    ));
    masters.units.insert(UnitId(1), unit());
    masters.stock_items.insert(StockItemId(1), widget());
    for t in default_voucher_types() {
        masters.voucher_types.insert(t.id, t);
    }
    // The company file holds Main Location from creation (seed_godowns), so the screen's copy
    // of the masters does too.
    for g in default_godowns() {
        masters.godowns.insert(g.id, g);
    }
    let books = Books {
        company: company(),
        masters,
        vouchers: std::collections::BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    };
    (ApiWorker::spawn(api).unwrap(), books)
}

fn frame(screen: &mut ItemInvoiceScreen, app: &mut AppCtx) {
    let ctx = egui::Context::default();
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _stay = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

fn fill(s: &mut ItemInvoiceScreen, m: &Masters) {
    s.set_party_text("ka buyer");
    s.submit(m);
    assert_eq!(s.focus(), Field::Place);
    "29".clone_into(&mut s.draft_mut().place_text);
    s.submit(m);
    s.set_account_text("sales");
    s.submit(m);
    s.submit(m);
    assert_eq!(s.focus(), Field::Item(0));
    s.set_item_text(0, "widget");
    s.submit(m);
    "10".clone_into(&mut s.draft_mut().rows[0].qty_text);
    s.submit(m);
    "1000".clone_into(&mut s.draft_mut().rows[0].rate_text);
    s.submit(m);
    s.submit(m);
}

/// Runs frames until `done` holds, or fails after ten seconds.
fn until(
    screen: &mut ItemInvoiceScreen,
    app: &mut AppCtx,
    what: &str,
    done: impl Fn(&ItemInvoiceScreen, &Masters) -> bool,
) {
    let start = Instant::now();
    loop {
        frame(screen, app);
        let masters = &app.books.as_ref().unwrap().masters;
        if done(screen, masters) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn press(screen: &mut ItemInvoiceScreen, app: &mut AppCtx, key: Key) {
    let ctx = egui::Context::default();
    let input = RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if key == Key::P {
                Modifiers::COMMAND
            } else {
                Modifiers::NONE
            },
        }],
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _stay = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn ctrl_p_after_a_save_previews_the_invoice_and_esc_closes_it() {
    let (worker, books) = setup("s18_print");
    let fy = FinancialYear::april(2026).unwrap();
    let sales = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    let Outcome::Value(mut screen) =
        ItemInvoiceScreen::open(&books, sales, fy, date!(2026 - 10 - 05)).unwrap()
    else {
        panic!("the sales type is entered on this screen");
    };
    fill(&mut screen, &books.masters);
    let mut app = AppCtx {
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
    };
    // Lines typed and not saved: asks to save first.
    press(&mut screen, &mut app, Key::P);
    assert_eq!(
        screen.info_text(),
        Some("Save the voucher first with Ctrl+A. Ctrl+P prints a saved voucher.")
    );
    until(&mut screen, &mut app, "tax block", |s, m| {
        matches!(s.gst_status(m), GstStatus::Computed(_))
    });
    let masters = app.books.as_ref().unwrap().masters.clone();
    screen.save(&masters);
    until(&mut screen, &mut app, "saved", |s, _| {
        s.info_text() == Some("Saved.")
    });
    press(&mut screen, &mut app, Key::P);
    until(&mut screen, &mut app, "preview", |s, _| {
        s.print_preview().is_some() || s.error_text().is_some()
    });
    assert_eq!(screen.error_text(), None);
    assert_eq!(
        screen.subtitle().as_deref(),
        Some("Saved \u{b7} Ka Buyer \u{b7} 5 Oct 2026")
    );
    press(&mut screen, &mut app, Key::Escape);
    assert!(screen.print_preview().is_none());
}
