//! Round s16: an invoice with an incomplete line (the mock's `*.validation-error` states).
//!
//! The accounting product's help pages (help.tallysolutions.com/sales-of-goods-services/ and
//! help.tallysolutions.com/tally-prime/sales-process/faq-sales/, read 2026-11-26) describe
//! saving with Ctrl+A and say nothing about a line left incomplete, so the rule here is the
//! mock's and the journal's: Save is disabled while any line is invalid, every invalid line is
//! named, and the lines that are valid still show the engine's amount and rate.
//!
//! Every expected figure is derived by hand: 10 Widgets at 1,000.00 is 10,000.00 taxable at 18%
//! (the bundled slab for Widget's HSN 8471 on 22 Sep 2025 onward).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::ItemInvoiceScreen;
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
                address: Address::default(),
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
        address: Address::default(),
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
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s16_validation_state");
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

/// The screen with line 1 complete (10 Widgets at 1,000.00) and line 2 an item with no quantity.
fn screen_with_a_bad_second_line(name: &str) -> (ItemInvoiceScreen, AppCtx) {
    let (worker, books) = setup(name);
    let fy = FinancialYear::april(2026).unwrap();
    let Outcome::Value(mut screen) = ItemInvoiceScreen::open(
        &books,
        default_voucher_types()
            .into_iter()
            .find(|t| t.base == BaseVoucherKind::Sales)
            .unwrap()
            .id,
        fy,
        date!(2026 - 10 - 05),
    )
    .unwrap() else {
        panic!("the sales type is entered on this screen");
    };
    let m = &books.masters;
    screen.set_party_text("ka buyer");
    screen.submit(m);
    "29".clone_into(&mut screen.draft_mut().place_text);
    screen.submit(m);
    screen.set_account_text("sales");
    screen.submit(m);
    screen.submit(m);
    screen.set_item_text(0, "widget");
    screen.submit(m);
    "10".clone_into(&mut screen.draft_mut().rows[0].qty_text);
    screen.submit(m);
    "1000".clone_into(&mut screen.draft_mut().rows[0].rate_text);
    screen.submit(m);
    screen.set_item_text(1, "widget");
    screen.submit(m);
    let app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
    };
    (screen, app)
}

#[test]
fn a_valid_line_shows_the_engine_s_amount_and_rate_while_another_line_is_invalid() {
    // provenance: derived. Line 1 is 10 x 1,000.00 = 10,000.00 taxable at 18%; line 2 has an item
    // and no quantity, so it is invalid and shows nothing.
    let (mut screen, mut app) = screen_with_a_bad_second_line("figures");
    until(&mut screen, &mut app, "line 1 figures", |s, m| {
        s.lines_shown(m)[0].amount == "10,000.00"
    });
    let masters = &app.books.as_ref().unwrap().masters;
    let shown = screen.lines_shown(masters);
    assert_eq!(shown[0].gst, "18%");
    assert_eq!(shown[1].amount, "");
    assert_eq!(shown[1].gst, "");
}

#[test]
fn save_is_disabled_while_a_line_is_invalid_and_enabled_once_it_is_not() {
    // provenance: derived from the mock (`sales-invoice.validation-error` draws Save grey while
    // an error stands) and the journal, which disables Save while its lines are out of balance.
    let (mut screen, app) = screen_with_a_bad_second_line("save_disabled");
    let masters = &app.books.as_ref().unwrap().masters;
    assert!(!screen.save_enabled(masters), "line 2 has no quantity");
    "2".clone_into(&mut screen.draft_mut().rows[1].qty_text);
    "500".clone_into(&mut screen.draft_mut().rows[1].rate_text);
    assert!(screen.save_enabled(masters), "line 2 is now complete");
}

#[test]
fn every_invalid_line_is_named_in_the_error() {
    // provenance: derived from the mock's red box ("N things to fix before saving", one line
    // each; a single problem is the line alone). Line 2 has an item and no quantity; line 3 has
    // text that names no item.
    let (mut screen, app) = screen_with_a_bad_second_line("named");
    let masters = &app.books.as_ref().unwrap().masters;
    screen.save(masters);
    assert_eq!(screen.error_text(), Some("Line 2: quantity is missing."));
    screen
        .draft_mut()
        .rows
        .push(ledgerly_app::screens::item_invoice::InvoiceRow {
            item_text: "Drip kit".to_owned(),
            ..Default::default()
        });
    screen.save(masters);
    assert_eq!(
        screen.error_text(),
        Some(
            "2 things to fix before saving\nLine 2: quantity is missing.\nLine 3: no item named \"Drip kit\". Pick one from the list or create it in Masters."
        )
    );
}

#[test]
fn a_line_whose_text_names_no_item_says_so_in_the_mocks_words() {
    // provenance: derived from the mock's `purchase.validation-error` (index.html: `Line 2: no
    // item named "Brass elbow 25 mm". Pick one from the list or create it with Alt+C.`). The
    // screen cannot create an item with Alt+C (the pages read document Alt+C on ledger fields
    // only; see item_invoice/create.rs), so the last sentence names Masters, where an item is
    // made. A line with a quantity typed and text that names no item: the item is the problem.
    let (mut screen, app) = screen_with_a_bad_second_line("unknown_item");
    let masters = &app.books.as_ref().unwrap().masters;
    "50".clone_into(&mut screen.draft_mut().rows[1].qty_text);
    "Brass elbow 25 mm".clone_into(&mut screen.draft_mut().rows[1].item_text);
    screen.draft_mut().rows[1].item = None;
    screen.save(masters);
    assert_eq!(
        screen.error_text(),
        Some(
            "Line 2: no item named \"Brass elbow 25 mm\". Pick one from the list or create it in Masters."
        )
    );
}
