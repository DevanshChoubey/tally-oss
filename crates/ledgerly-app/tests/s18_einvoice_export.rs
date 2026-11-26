//! S18: Ctrl+E on a saved sales invoice's voucher view writes its e-invoice JSON (INV-01) to a file.
//!
//! Provenance: derived. The file holds the NIC schema's JSON (field names from
//! <https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26) for Ten Widgets
//! at Rs 1,000.00 to a Karnataka buyer from a Karnataka company: CGST 900.00, SGST 900.00, invoice
//! value 11,800.00 (the c2 derivation). The offline route is export, then upload by the user on the
//! IRP bulk page (<https://help.tallysolutions.com/generate-e-invoice-offline-tallyhelp/>, read
//! 2026-11-26); the note after the export says nothing was sent. The note's text is the mock's
//! `balance-sheet.voucher-einvoice-exported`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, GstStatus, ItemInvoiceScreen};
use ledgerly_app::screens::report_view::{Notice, VoucherViewScreen};
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

fn books_of(worker: &ApiWorker) -> Books {
    let mut ticket = worker.run(|api: &mut Api| match api.books() {
        Ok(Outcome::Value(b)) => Some(b.clone()),
        Ok(Outcome::Unsupported(_)) | Err(_) => None,
    });
    for _ in 0..2000 {
        if let ledgerly_app::worker::Poll::Ready(books) = ticket.poll() {
            return books.unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the books did not load");
}

#[test]
fn ctrl_e_on_a_saved_sales_invoice_writes_its_json_and_says_where() {
    let (worker, books) = setup("s18_export", true);
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
    until(&mut screen, &mut app, "tax block", |s, m| {
        matches!(s.gst_status(m), GstStatus::Computed(_))
    });
    let masters = app.books.as_ref().unwrap().masters.clone();
    screen.save(&masters);
    until(&mut screen, &mut app, "saved", |s, _| {
        s.info_text() == Some("Saved.")
    });
    app.books = Some(books_of(app.api.as_ref().unwrap()));
    let id = *app.books.as_ref().unwrap().vouchers.keys().next().unwrap();

    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s18_export.json");
    let _gone = std::fs::remove_file(&out);
    let mut view = VoucherViewScreen::new(id);
    let target = out.clone();
    view.set_export_picker(Box::new(move |_suggested| Some(target.clone())));
    let ctx = egui::Context::default();
    let run = |view: &mut VoucherViewScreen, app: &mut AppCtx, input: egui::RawInput| {
        let o = ctx.run_ui(input, |ui| {
            let _stay = view.ui(ui, app);
        });
        o.drop_without_applying_deltas();
    };
    run(&mut view, &mut app, egui::RawInput::default());
    let ctrl_e = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::E,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..egui::RawInput::default()
    };
    run(&mut view, &mut app, ctrl_e);
    for _ in 0..2000 {
        run(&mut view, &mut app, egui::RawInput::default());
        if view.export_note().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let Some(Notice::Info(said)) = view.export_note() else {
        panic!("the export said: {:?}", view.export_note());
    };
    assert_eq!(
        said,
        &format!(
            "Exported the e-invoice JSON (INV-01) to {}. Upload it on the government portal's bulk page; nothing is sent from here.",
            out.display()
        )
    );
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"Typ\":\"INV\""), "{json}");
    assert!(json.contains("\"TotInvVal\":11800.00"), "{json}");
    assert!(json.contains("\"CgstAmt\":900.00"), "{json}");
}
