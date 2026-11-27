//! Card C4: the GST screen runs GSTR-1 and GSTR-3B through the API worker.
//!
//! Provenance: every expected figure is `derived` by hand from the vouchers posted below.
//! 18 per cent is the GST 2.0 slab; an intra-state supply splits it into CGST 9 and SGST 9, an
//! inter-state supply is IGST 18 (CGST Act s.9, IGST Act s.5). Outward supplies go to GSTR-1
//! (CGST Act s.37). The GSTR-3B "several registrations" refusal is the documented behaviour in the
//! GSTR-3B builder's module header (`crates/ledgerly-gst/src/gstr3b.rs`, "Not decided").
//! Tally and GST law pages consulted 2026-11-21: the GST portal's return page,
//! <https://tutorial.gst.gov.in/userguide/returns/index.htm>, names Table 4 (B2B), Table 12 (HSN
//! summary) in GSTR-1 and Tables 3.1(a) and 4(A)(5) in GSTR-3B.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_app::screens::gst::{Bucket, FigureRow, GstScreen, GstTab, amount_cell};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Rate,
    SignedAmount, StateCode, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GstClassification, GstRegistration,
    Ledger, LedgerLine, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup,
    TaxDetails, TaxLedgerKind, Taxability, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const COMPANY: &str = "29ABCDE1234F1Z";
const SECOND: &str = "27ABCDE1234F1Z";
const ALPHA_GSTIN: &str = "29FGHIJ5678K1Z";
const GAMMA_GSTIN: &str = "27RSTUV3456W1Z";

const ALPHA: LedgerId = LedgerId(10);
const GAMMA: LedgerId = LedgerId(11);
const SALES: LedgerId = LedgerId(20);
const OUT_CGST: LedgerId = LedgerId(30);
const OUT_SGST: LedgerId = LedgerId(31);
const OUT_IGST: LedgerId = LedgerId(32);

fn gstin_of(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn oct() -> Period {
    Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
}

fn group_id(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn sales_type() -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group_id(group)),
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

fn buyer(id: LedgerId, name: &str, gstin: &str, place: u8) -> Ledger {
    let mut l = ledger(id, name, ReservedGroup::SundryDebtors);
    l.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(gstin_of(gstin)),
        pan: None,
        state: Some(state(place)),
        address: Address::default(),
    });
    l
}

fn tax_ledger(id: LedgerId, name: &str, kind: TaxLedgerKind) -> Ledger {
    let mut l = ledger(id, name, ReservedGroup::DutiesAndTaxes);
    l.tax = Some(TaxDetails {
        kind,
        rate_percent: None,
    });
    l
}

fn registration(gstin: &str, code: u8) -> GstRegistration {
    GstRegistration {
        gstin: gstin_of(gstin),
        state: state(code),
        effective_from: date!(2026 - 04 - 01),
        composition: false,
    }
}

fn company(registrations: Vec<GstRegistration>) -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(29),
        pan: None,
        registrations,
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

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn dr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, SignedAmount::dr(Paise::new(paise)))
}

fn cr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, SignedAmount::cr(Paise::new(paise)))
}

fn sale(id: u64, on: Date, party: LedgerId, pos: u8, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: sales_type(),
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(party),
        place_of_supply: Some(state(pos)),
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with two buyers, a taxable sales ledger and the output tax ledgers, and two
/// sales in October 2026:
/// * V1, 5 Oct, Alpha (state 29, intra-state): taxable Rs 1,00,000, CGST 9,000, SGST 9,000,
///   invoice value Rs 1,18,000.
/// * V3, 7 Oct, Gamma (state 27, inter-state): taxable Rs 50,000, IGST 9,000, invoice value
///   Rs 59,000.
fn setup(name: &str, registrations: Vec<GstRegistration>) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(
        &path,
        &company(registrations),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let mut sales = ledger(SALES, "Sales", ReservedGroup::SalesAccounts);
    sales.gst = Some(GstClassification {
        hsn: Some(Hsn::parse("998313").unwrap()),
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::from(18))),
    });
    for l in [
        buyer(ALPHA, "Alpha", ALPHA_GSTIN, 29),
        buyer(GAMMA, "Gamma", GAMMA_GSTIN, 27),
        sales,
        tax_ledger(OUT_CGST, "Output CGST", TaxLedgerKind::Cgst),
        tax_ledger(OUT_SGST, "Output SGST", TaxLedgerKind::Sgst),
        tax_ledger(OUT_IGST, "Output IGST", TaxLedgerKind::Igst),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    for v in [
        sale(
            1,
            date!(2026 - 10 - 05),
            ALPHA,
            29,
            vec![
                dr(ALPHA, 11_800_000),
                cr(SALES, 10_000_000),
                cr(OUT_CGST, 900_000),
                cr(OUT_SGST, 900_000),
            ],
        ),
        sale(
            3,
            date!(2026 - 10 - 07),
            GAMMA,
            27,
            vec![
                dr(GAMMA, 5_900_000),
                cr(SALES, 5_000_000),
                cr(OUT_IGST, 900_000),
            ],
        ),
    ] {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

/// Loads the screen from the worker and waits (bounded) for the answer.
fn loaded(worker: &ApiWorker) -> GstScreen {
    let mut screen = GstScreen::new("October 2026", Some(gstin_of(COMPANY)));
    screen.request_load(gstin_of(COMPANY), oct());
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return screen;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the returns did not arrive within the bound");
}

/// The amount of the one row whose label starts with `start` and ends with `end`.
fn amount_of(rows: &[FigureRow], start: &str, end: &str) -> Paise {
    let found: Vec<&FigureRow> = rows
        .iter()
        .filter(|r| r.label.starts_with(start) && r.label.ends_with(end))
        .collect();
    assert_eq!(found.len(), 1, "rows for {start} .. {end}: {found:?}");
    found.first().unwrap().amount
}

#[test]
fn gstr1_shows_hand_derived_figures_from_the_worker() {
    // provenance: derived. Vouchers V1 and V3 above.
    // Table 4 B2B: Alpha invoice value 1,18,000.00 = 1,00,000 + 9,000 + 9,000 = 11,800,000
    // paise; Gamma invoice value 59,000.00 = 50,000 + 9,000 = 5,900,000 paise.
    // Table 12 HSN 998313 at 18%: taxable 1,00,000 + 50,000 = 15,000,000 paise; IGST 9,000 (V3)
    // = 900,000; CGST 9,000 (V1) = 900,000; SGST 9,000 (V1) = 900,000; cess nil.
    // Both vouchers are outward supplies with every master present, so both are Ready.
    let worker = ApiWorker::spawn(setup(
        "c4_gst_oct.ledgerly",
        vec![registration(COMPANY, 29)],
    ))
    .unwrap();
    let screen = loaded(&worker);

    assert_eq!(screen.load_error(), None);
    assert_eq!(screen.unsupported_line(GstTab::Gstr1), None);
    assert_eq!(screen.item_count(GstTab::Gstr1), Some(2));
    assert_eq!(screen.bucket_count(GstTab::Gstr1, Bucket::Ready), Some(2));

    let rows = &screen.figures(GstTab::Gstr1).unwrap().rows;
    let b2b = |value: i64| {
        rows.iter()
            .filter(|r| r.label.starts_with("Table 4 B2B") && r.amount == Paise::new(value))
            .count()
    };
    assert_eq!(b2b(11_800_000), 1);
    assert_eq!(b2b(5_900_000), 1);
    assert_eq!(
        amount_of(rows, "Table 12 HSN 998313", ": taxable value"),
        Paise::new(15_000_000)
    );
    assert_eq!(
        amount_of(rows, "Table 12 HSN 998313", ": integrated tax"),
        Paise::new(900_000)
    );
    assert_eq!(
        amount_of(rows, "Table 12 HSN 998313", ": central tax"),
        Paise::new(900_000)
    );
    assert_eq!(
        amount_of(rows, "Table 12 HSN 998313", ": state/UT tax"),
        Paise::new(900_000)
    );
    assert_eq!(
        amount_of(rows, "Table 12 HSN 998313", ": cess"),
        Paise::ZERO
    );
    // The figure text is the Indian-grouped amount: 11,800,000 paise is 1,18,000.00.
    assert_eq!(amount_cell(Some(Paise::new(11_800_000))), "1,18,000.00");
}

#[test]
fn gstr3b_shows_hand_derived_figures_from_the_worker() {
    // provenance: derived. Same two sales, no purchases in October, so no input credit.
    // Table 3.1(a): taxable 1,00,000 + 50,000 = 15,000,000 paise; integrated 900,000 (V3);
    // central 900,000 and state/UT 900,000 (V1); cess nil. Table 4(A)(5): no purchase, so nil.
    let worker =
        ApiWorker::spawn(setup("c4_gst_3b.ledgerly", vec![registration(COMPANY, 29)])).unwrap();
    let screen = loaded(&worker);

    assert_eq!(screen.unsupported_line(GstTab::Gstr3b), None);
    let rows = &screen.figures(GstTab::Gstr3b).unwrap().rows;
    assert_eq!(
        amount_of(rows, "Table 3.1(a)", ": taxable value"),
        Paise::new(15_000_000)
    );
    assert_eq!(
        amount_of(rows, "Table 3.1(a)", ": integrated tax"),
        Paise::new(900_000)
    );
    assert_eq!(
        amount_of(rows, "Table 3.1(a)", ": central tax"),
        Paise::new(900_000)
    );
    assert_eq!(
        amount_of(rows, "Table 3.1(a)", ": state/UT tax"),
        Paise::new(900_000)
    );
    assert_eq!(amount_of(rows, "Table 3.1(a)", ": cess"), Paise::ZERO);
    assert_eq!(
        amount_of(rows, "Table 4(A)(5)", ": integrated tax"),
        Paise::ZERO
    );
    assert_eq!(
        amount_of(rows, "Table 4(A)(5)", ": central tax"),
        Paise::ZERO
    );
}

#[test]
fn an_unsupported_gstr3b_shows_its_reason_and_no_figure() {
    // provenance: derived. The company has two GST registrations (29 and 27). The GSTR-3B
    // builder documents that a company with several registrations is Unsupported (vouchers are
    // not tied to one GSTIN), so the screen must show the reason and no figure at all, never an
    // empty table that reads as nil.
    let worker = ApiWorker::spawn(setup(
        "c4_gst_unsupported.ledgerly",
        vec![registration(COMPANY, 29), registration(SECOND, 27)],
    ))
    .unwrap();
    let screen = loaded(&worker);

    let line = screen.unsupported_line(GstTab::Gstr3b).unwrap();
    assert!(line.starts_with("Not supported yet: "), "{line}");
    assert!(line.len() > "Not supported yet: ".len());
    let figures = screen.figures(GstTab::Gstr3b).unwrap();
    assert_eq!(figures.rows, Vec::<FigureRow>::new());
    assert_eq!(screen.item_count(GstTab::Gstr3b), None);
}

#[test]
fn a_load_with_no_company_open_runs_nothing_and_says_so() {
    // provenance: derived. No worker means no company file; no return can be built.
    let mut screen = GstScreen::new("October 2026", None);
    screen.request_load(gstin_of(COMPANY), oct());
    screen.tick(None);
    assert!(!screen.is_loading());
    assert!(screen.load_error().is_some());
    assert_eq!(
        screen.figures(GstTab::Gstr1).unwrap().rows,
        Vec::<FigureRow>::new()
    );
}
