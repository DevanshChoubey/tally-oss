//! S18: the item invoice's e-Invoice cell says whether the invoice maps to e-invoice JSON.
//!
//! Provenance: derived from the NIC e-Invoice schema page
//! (<https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26): `Pin` is
//! required on `SellerDtls` and `BuyerDtls`, so the company with no PIN code is "not ready" and the
//! words are the API's: "the company has no PIN code (Pin is required)". With a PIN on the company
//! and the buyer, the invoice (Ten Widgets at Rs 1,000.00, Rs 11,800.00 with CGST and SGST, the c2
//! derivation) maps and the cell says "JSON ready (INV-01)", the mock's text (index.html, the
//! `sales-invoice.default` cell). The mock's not-ready state is `sales-invoice.einvoice-not-ready`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, ItemInvoiceScreen};
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

fn ledgers(pin: bool) -> Vec<Ledger> {
    vec![
        Ledger {
            party: Some(PartyDetails {
                registration: RegistrationType::Regular,
                gstin: Some(gstin_of("29FGHIJ5678K1Z")),
                pan: None,
                state: Some(state(29)),
                address: Address {
                    lines: vec!["12 Test Road".to_owned(), "Bengaluru".to_owned()],
                    pincode: pin.then(|| "560001".to_owned()),
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

fn company(pin: bool) -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Test Road".to_owned(), "Bengaluru".to_owned()],
            pincode: pin.then(|| "560002".to_owned()),
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
fn setup(name: &str, pin: bool) -> (ApiWorker, Books) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("c2_item_invoice_gst");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let mut api = Api::create(
        &path,
        &company(pin),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let mut masters = Masters::default();
    for l in ledgers(pin) {
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
        company: company(pin),
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

fn note_for(name: &str, pin: bool) -> Option<String> {
    let (worker, books) = setup(name, pin);
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
        tr: ledgerly_app::i18n::Tr::default(),
    };
    until(&mut screen, &mut app, "e-invoice answer", |s, masters| {
        s.einvoice_note(masters).is_some()
    });
    screen
        .einvoice_note(&app.books.as_ref().unwrap().masters)
        .map(str::to_owned)
}

#[test]
fn a_company_and_buyer_with_pins_say_json_ready() {
    assert_eq!(
        note_for("s18_cell_ready", true).as_deref(),
        Some("JSON ready (INV-01)")
    );
}

#[test]
fn a_company_without_a_pin_says_what_is_missing() {
    assert_eq!(
        note_for("s18_cell_pin", false).as_deref(),
        Some("the company has no PIN code (Pin is required)")
    );
}

/// Derived: a ready answer is about the invoice that was checked. With the quantity of the only
/// line cleared the invoice is no longer complete, the tax box shows dashes, and the cell must not
/// go on saying "JSON ready".
#[test]
fn an_incomplete_invoice_does_not_say_json_ready() {
    let (worker, books) = setup("s18_cell_stale", true);
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
        tr: ledgerly_app::i18n::Tr::default(),
    };
    until(&mut screen, &mut app, "e-invoice answer", |s, masters| {
        s.einvoice_note(masters) == Some("JSON ready (INV-01)")
    });
    screen.draft_mut().rows[0].qty_text.clear();
    frame(&mut screen, &mut app);
    let masters = &app.books.as_ref().unwrap().masters;
    assert_eq!(screen.einvoice_note(masters), None);
}
