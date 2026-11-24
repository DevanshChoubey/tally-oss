//! Card C2: the item invoice screen asks the API for the tax through the worker and shows its
//! answer, and Accept then posts (rule C4: the screen computes nothing).
//!
//! Provenance: derived. Ten Widgets at Rs 1,000.00 is Rs 10,000.00 taxable. The company is in
//! Karnataka (29) and the buyer's place of supply is 29, so the supply is intra-state: the 18 per
//! cent slab (Notification No. 9/2025-Central Tax (Rate) dated 17 Sep 2025, read in the bundled
//! pack) splits into CGST 9% = 900.00 and SGST 9% = 900.00; the invoice value is 11,800.00, a
//! whole rupee, so no round-off. A sale debits the party and credits the sales and tax ledgers.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, GstStatus, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, FinancialYear, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise,
    Rate, SignedAmount, StateCode, StockItemId, UnitId, gstin_check_character,
};
use ledgerly_model::seed_godowns::default_godowns;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstHistory, GstRegistration,
    Ledger, LedgerLine, Masters, NegativeStockPolicy, PartyDetails, RegistrationType,
    ReservedGroup, StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind,
    ValuationMethod,
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

#[test]
fn the_screen_gets_its_tax_block_from_the_worker_and_accept_posts() {
    let (worker, books) = setup("accept");
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
    fill(&mut screen, &books.masters);
    let mut app = AppCtx {
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
    };

    until(&mut screen, &mut app, "tax block", |s, m| {
        matches!(s.gst_status(m), GstStatus::Computed(_))
    });
    let masters = &app.books.as_ref().unwrap().masters;
    let GstStatus::Computed(block) = screen.gst_status(masters) else {
        panic!("the block was computed");
    };
    let rupees = |r: i64| Paise::new(r * 100);
    assert_eq!(block.invoice_total, rupees(11_800));
    assert_eq!(block.round_off, Paise::ZERO);
    let heads: Vec<(&str, Paise)> = block
        .heads
        .iter()
        .map(|h| (h.head.as_str(), h.amount))
        .collect();
    assert_eq!(heads, vec![("CGST", rupees(900)), ("SGST", rupees(900))]);
    let line = |ledger, side, amount| LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: SignedAmount { amount, side },
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    assert_eq!(
        block.ledger_lines,
        vec![
            line(BUYER, DrCr::Dr, rupees(11_800)),
            line(SALES, DrCr::Cr, rupees(10_000)),
            line(CGST, DrCr::Cr, rupees(900)),
            line(SGST, DrCr::Cr, rupees(900)),
        ]
    );

    let masters = app.books.as_ref().unwrap().masters.clone();
    screen.save(&masters);
    until(&mut screen, &mut app, "saved", |s, _| {
        s.info_text() == Some("Saved.")
    });
}

#[test]
fn an_invoice_with_a_round_off_shows_unsupported_and_does_not_save() {
    // Derivation: 1 Widget at Rs 100.01: CGST = SGST = 9.0009 -> 9.00 each, value 118.01 rounds to
    // 118.00, round-off -0.01. No invoice-rounding ledger exists in the model, so the API answers
    // Unsupported and Accept refuses.
    let (worker, books) = setup("round_off");
    let fy = FinancialYear::april(2026).unwrap();
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    let Outcome::Value(mut screen) =
        ItemInvoiceScreen::open(&books, type_id, fy, date!(2026 - 10 - 05)).unwrap()
    else {
        panic!("the sales type is entered on this screen");
    };
    fill(&mut screen, &books.masters);
    screen.draft_mut().rows[0].qty_text = "1".to_owned();
    screen.draft_mut().rows[0].rate_text = "100.01".to_owned();
    let mut app = AppCtx {
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
    };
    until(&mut screen, &mut app, "answer", |s, m| {
        matches!(s.gst_status(m), GstStatus::Unsupported(_))
    });
    let masters = app.books.as_ref().unwrap().masters.clone();
    screen.save(&masters);
    assert!(screen.error_text().unwrap().contains("round-off"));
}

/// Every piece of text a frame painted, in drawing order.
fn painted_text(out: &egui::FullOutput) -> String {
    fn walk(shape: &egui::Shape, into: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                into.push_str(t.galley.text());
                into.push('\n');
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, into)),
            _ => {}
        }
    }
    let mut text = String::new();
    for clipped in &out.shapes {
        walk(&clipped.shape, &mut text);
    }
    text
}

#[test]
fn the_form_stays_on_screen_while_the_post_is_pending() {
    // provenance: the sales-invoice mock keeps the sheet on screen and tells the save in a toast
    // (index.html:1328 mount, `toast(...)`); it never replaces the sheet with a bare label. A
    // post that queues behind a slow job must therefore leave the form drawn, with the notice
    // "Saving..." in it.
    let (worker, books) = setup("pending_post");
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
    fill(&mut screen, &books.masters);
    let masters = books.masters.clone();
    let mut app = AppCtx {
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
    };
    until(&mut screen, &mut app, "tax block", |s, m| {
        matches!(s.gst_status(m), GstStatus::Computed(_))
    });
    // A job the worker is busy with for a moment: the post queues behind it.
    let busy = app
        .api
        .as_ref()
        .unwrap()
        .run(|_api| std::thread::sleep(Duration::from_millis(400)));
    screen.save(&masters);
    let ctx = egui::Context::default();
    // The first frame sends the post; the next ones find it pending.
    for _ in 0..3 {
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _stay = screen.ui(ui, &mut app);
        });
        let text = painted_text(&out);
        out.drop_without_applying_deltas();
        assert!(
            text.contains("Narration"),
            "the form was not drawn:\n{text}"
        );
    }
    assert_eq!(screen.info_text(), Some("Saving..."));
    drop(busy);
    until(&mut screen, &mut app, "saved", |s, _| {
        s.info_text() == Some("Saved.")
    });
}
