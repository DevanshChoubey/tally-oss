//! Sprint 16: the figures of a credit or debit note's hint, from the engine: the date and the
//! quantity of the original invoice (CGST Act s.34 and Rule 53(1A): a credit note names the
//! tax invoice it corrects; <https://taxguru.in/goods-and-service-tax/credit-note-debit-note-section-34-rule-53-cgst-act-2017.html>,
//! read 2026-11-25 in the mock notes). The accounting product's sales-return page
//! (<https://help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/>) asks the
//! original invoice number and date in the note. Showing the quantity invoiced is our own aid:
//! the product choice is that it is the quantity on the original invoice, read from the books.
//!
//! Provenance: `derived`. The figures are those typed into the posted invoice in each test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    dead_code,
    reason = "test code: rule B2 lets tests unwrap and panic; the shared fixture has helpers some files do not use"
)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise,
    Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
    gstin_check_character,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation,
    GstHistory, GstRegistration, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy,
    PartyDetails, RegistrationType, ReservedGroup, StockItem, TaxDetails, TaxLedgerKind,
    Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const COMPANY: &str = "29ABCDE1234F1Z";
const BUYER_GSTIN: &str = "27RSTUV3456W1Z";
const SUPPLIER_GSTIN: &str = "29LMNOP9012Q1Z";

const KA_BUYER: LedgerId = LedgerId(10);
const MH_BUYER: LedgerId = LedgerId(11);
const SUPPLIER: LedgerId = LedgerId(12);
const SALES: LedgerId = LedgerId(20);
const PURCHASE: LedgerId = LedgerId(21);
const CGST: LedgerId = LedgerId(30);
const SGST: LedgerId = LedgerId(31);
const IGST: LedgerId = LedgerId(32);
const ITEM: StockItemId = StockItemId(1);
const UNIT: UnitId = UnitId(1);
const MAIN: GodownId = GodownId(1);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn gstin_of(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn vtype(base: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == base)
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

fn party(id: LedgerId, name: &str, kind: ReservedGroup, gstin: &str, code: u8) -> Ledger {
    Ledger {
        party: Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(gstin_of(gstin)),
            pan: None,
            state: Some(state(code)),
            address: Address::default(),
        }),
        ..plain(id, name, kind)
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
            gstin: gstin_of(COMPANY),
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

/// A company file with the masters: a Karnataka buyer (state 29, intra-state for the company), a
/// Maharashtra buyer (27, inter-state), a Karnataka supplier, one ledger per tax head, and a
/// Widget item taxed at 18 per cent.
fn company_file() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    let ledgers = vec![
        party(
            KA_BUYER,
            "Ka Buyer",
            ReservedGroup::SundryDebtors,
            "29FGHIJ5678K1Z",
            29,
        ),
        party(
            MH_BUYER,
            "Mh Buyer",
            ReservedGroup::SundryDebtors,
            BUYER_GSTIN,
            27,
        ),
        party(
            SUPPLIER,
            "Supplier",
            ReservedGroup::SundryCreditors,
            SUPPLIER_GSTIN,
            29,
        ),
        plain(SALES, "Sales", ReservedGroup::SalesAccounts),
        plain(PURCHASE, "Purchases", ReservedGroup::PurchaseAccounts),
        tax(CGST, "CGST", TaxLedgerKind::Cgst),
        tax(SGST, "SGST", TaxLedgerKind::Sgst),
        tax(IGST, "IGST", TaxLedgerKind::Igst),
    ];
    for l in ledgers {
        ok(api.execute(Command::CreateLedger(l)));
    }
    ok(api.execute(Command::CreateUnit(Unit {
        id: UNIT,
        symbol: "pcs".to_owned(),
        formal_name: "Pieces".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: Some("PCS".to_owned()),
        tally: None,
    })));
    ok(api.execute(Command::CreateStockItem(widget())));
    // Main Location is Tally's default godown and every company file holds it from creation as
    // godown 1 (seed_godowns; TallyPrime godowns FAQ, accessed 2026-11-21), so this test no
    // longer creates it by hand (updated 2026-11-21 on the owner's "follow Tally" instruction).
    assert_eq!(MAIN, MAIN_LOCATION);
    (dir, api)
}

fn widget() -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UNIT,
        alt_unit: None,
        hsn: None,
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

fn ok<T: Debug>(out: Result<Outcome<T>, ApiError>) -> T {
    match out {
        Ok(Outcome::Value(v)) => v,
        other => panic!("expected a value, got {other:?}"),
    }
}

fn line(ledger: LedgerId, side: DrCr, paise: i64) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: SignedAmount {
            amount: Paise::new(paise),
            side,
        },
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn dr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, DrCr::Dr, paise)
}

fn cr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, DrCr::Cr, paise)
}

/// An accounting-invoice voucher dated 5 Oct 2026 (the sale and purchase ledgers carry no GST
/// classification here, so the taxable base of an accounting invoice is the classified ledger; the
/// helper classifies them through [`classify`] before use).
fn invoice(
    base: BaseVoucherKind,
    mode: VoucherMode,
    party: LedgerId,
    pos: Option<u8>,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: vtype(base),
        number: None,
        series: None,
        date: date!(2026 - 10 - 05),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party: Some(party),
        place_of_supply: pos.map(state),
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

/// Item invoice lines: `qty` Widgets, `amount` in paise, posted to `ledger`.
fn items(qty: i64, amount: i64, ledger: LedgerId) -> Vec<InventoryLine> {
    vec![InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::new(qty, 0)),
        unit: UNIT,
        rate: Rate(Decimal::new(amount, 2) / Decimal::new(qty, 0)),
        discount_percent: None,
        amount: Paise::new(amount),
        ledger: Some(ledger),
        godowns: vec![GodownAllocation {
            godown: MAIN,
            batch: None,
            qty: Qty(Decimal::new(qty, 0)),
            amount: Paise::new(amount),
        }],
    }]
}

/// An item invoice sold to `party` for `qty` Widgets worth `amount` paise, no ledger lines.
fn item_sale(party: LedgerId, pos: Option<u8>, qty: i64, amount: i64) -> Voucher {
    Voucher {
        inventory_lines: items(qty, amount, SALES),
        ..invoice(
            BaseVoucherKind::Sales,
            VoucherMode::ItemInvoice,
            party,
            pos,
            Vec::new(),
        )
    }
}


use ledgerly_model::BaseVoucherKind as Kind;

fn post_sale(api: &mut Api, qty: i64) -> Voucher {
    let draft = item_sale(KA_BUYER, Some(29), qty, qty * 100_000);
    let built = ok(api.invoice_gst(&draft));
    let mut v = draft;
    v.date = date!(2026 - 11 - 14);
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    ok(api.execute(Command::PostVoucher(v))).posted.unwrap()
}

/// Derivation: 67 Widgets sold on 14 Nov 2026 to Ka Buyer: the note against that invoice is told
/// the date 14 Nov 2026 and 67 pcs of Widget.
#[test]
fn a_credit_note_is_told_the_date_and_quantity_of_its_invoice() {
    let (_dir, mut api) = company_file();
    let sale = post_sale(&mut api, 67);
    let number = sale.number.clone().unwrap();
    let got = ok(api.invoiced_against(Kind::CreditNote, KA_BUYER, &number));
    assert_eq!(got.date, date!(2026 - 11 - 14));
    assert_eq!(got.number, number);
    assert_eq!(got.lines.len(), 1);
    assert_eq!(got.lines[0].item, ITEM);
    assert_eq!(got.lines[0].qty.0, Decimal::from(67));
    assert_eq!(got.lines[0].unit_symbol, "pcs");
}

/// Derivation: an invoice of another party is not the one the note corrects.
#[test]
fn another_partys_invoice_is_not_found() {
    let (_dir, mut api) = company_file();
    let sale = post_sale(&mut api, 67);
    let number = sale.number.unwrap();
    match api.invoiced_against(Kind::CreditNote, MH_BUYER, &number).unwrap() {
        Outcome::Unsupported(u) => assert!(u.reason.contains(&number), "{u:?}"),
        other @ Outcome::Value(_) => panic!("expected Unsupported, got {other:?}"),
    }
}

/// Derivation: no invoice has this number.
#[test]
fn an_unknown_number_is_not_found() {
    let (_dir, mut api) = company_file();
    post_sale(&mut api, 5);
    assert!(matches!(
        api.invoiced_against(Kind::CreditNote, KA_BUYER, "NOPE/1").unwrap(),
        Outcome::Unsupported(_)
    ));
}

/// Derivation: a debit note goes against the supplier's bill, found by the supplier's bill number
/// (the purchase's reference), with the bill's date.
#[test]
fn a_debit_note_is_told_its_bills_quantity() {
    let (_dir, mut api) = company_file();
    let mut v = Voucher {
        inventory_lines: items(40, 4_000_000, PURCHASE),
        reference: Some("NST/1150".to_owned()),
        ..invoice(
            BaseVoucherKind::Purchase,
            VoucherMode::ItemInvoice,
            SUPPLIER,
            Some(29),
            Vec::new(),
        )
    };
    v.date = date!(2026 - 10 - 15);
    v.reference_date = Some(date!(2026 - 10 - 15));
    v.ledger_lines = vec![
        dr(PURCHASE, 4_000_000),
        dr(CGST, 360_000),
        dr(SGST, 360_000),
        cr(SUPPLIER, 4_720_000),
    ];
    ok(api.execute(Command::PostVoucher(v)));
    let got = ok(api.invoiced_against(Kind::DebitNote, SUPPLIER, "NST/1150"));
    assert_eq!(got.date, date!(2026 - 10 - 15));
    assert_eq!(got.lines[0].qty.0, Decimal::from(40));
}

/// Derivation: only a credit note or a debit note has an original invoice.
#[test]
fn other_kinds_have_no_original_invoice() {
    let (_dir, api) = company_file();
    assert!(matches!(
        api.invoiced_against(Kind::Sales, KA_BUYER, "X").unwrap(),
        Outcome::Unsupported(_)
    ));
}
