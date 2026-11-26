//! S16: the purchase draft shows the input tax credit the engine finds eligible, before the 2B
//! match. The screen only shows the engine's answer (rule C4).
//!
//! CGST Act s.16(1) allows the credit on goods or services used for business; the accounting
//! product marks a purchase ineligible with "Is ineligible for input credit?"
//! (<https://help.tallysolutions.com/docs/te9rel66/Tax_India/gst/gst_accounting_for_ineligible_itc.htm>,
//! read 2026-11-26). The s.16(2) conditions (bill in 2B, receipt, payment) cannot be known on a
//! draft, so the line says "before the 2B match".
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use egui::RawInput;
use ledgerly_api::{Api, Command};
use ledgerly_app::screens::item_invoice::{Field, GstStatus, InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Decimal, FinancialYear, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Rate,
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
fn setup(name: &str, ineligible: Option<bool>, hsn: Option<&str>) -> (Api, Books) {
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
        ineligible_itc: ineligible,
        gst: Some(GstClassification {
            hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
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

fn app(books: &Books, api: Option<Api>) -> AppCtx {
    AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: api.map(|a| ApiWorker::spawn(a).unwrap()),
        lock_date: None,
        ai_tier: None,
    }
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

fn run(ineligible: Option<bool>, hsn: Option<&str>, name: &str) -> (ItemInvoiceScreen, Books) {
    let (api, books) = setup(name, ineligible, hsn);
    let mut s = open_purchase(&books);
    fill(&mut s, &books.masters);
    let mut app = app(&books, Some(api));
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        let out = ctx.run_ui(RawInput::default(), |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
        if !matches!(s.gst_status(&books.masters), GstStatus::NotComputedYet) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    (s, books)
}

#[test]
fn an_eligible_purchase_shows_all_the_tax_as_credit() {
    // provenance: derived. Intra-state (27 and 27), Maintenance Charges 10,000.00 at 18%: CGST
    // 900.00 + SGST 900.00 = 1,800.00; the ledger is not marked ineligible, so all of it is
    // eligible before the 2B match.
    let (s, books) = run(None, Some("998719"), "s16_itc_line_eligible.ledgerly");
    assert_eq!(
        s.eligible_itc_line(&books.masters),
        Some((
            "Input credit eligible before the 2B match".to_owned(),
            "1,800.00".to_owned()
        ))
    );
}

#[test]
fn an_ineligible_ledger_shows_nil_credit_and_the_blocked_tax() {
    // provenance: derived. The purchase ledger is marked "Is ineligible for input credit": the
    // 1,800.00 of tax is not claimable and the eligible credit is 0.00.
    let (s, books) = run(Some(true), None, "s16_itc_line_blocked.ledgerly");
    assert_eq!(
        s.eligible_itc_line(&books.masters),
        Some((
            "Input credit eligible before the 2B match".to_owned(),
            "0.00 (1,800.00 not claimable)".to_owned()
        ))
    );
}

#[test]
fn a_purchase_ledger_without_an_hsn_shows_not_computed_and_the_reason() {
    // provenance: derived. Whether a credit is blocked under s.17(5) depends on the HSN/SAC, and
    // the ledger has none: the engine does not guess, the line says it is not computed and why.
    let (s, books) = run(None, None, "s16_itc_line_no_hsn.ledgerly");
    let (label, value) = s.eligible_itc_line(&books.masters).unwrap();
    assert_eq!(label, "Input credit eligible before the 2B match");
    assert!(value.starts_with("Not computed: "), "{value}");
    assert!(value.contains("HSN/SAC"), "{value}");
}
