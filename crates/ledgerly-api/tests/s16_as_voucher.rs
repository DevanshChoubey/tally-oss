//! Sprint 16, card "As Voucher": the engine computes GST for a sales or purchase entered as
//! ledger lines (TallyPrime's As Voucher mode, Ctrl+H).
//!
//! Sources, read 2026-11-26:
//! * <https://help.tallysolutions.com/sales-of-goods-services/> and
//!   <https://help.tallysolutions.com/purchase-goods-services/>: "Press Ctrl+H (Change mode) >
//!   select As Voucher mode. In As Voucher mode, the voucher displays By and To instead of Dr and
//!   Cr." The party (or cash or bank) ledger is the debit line, the sales ledger the credit line.
//!   These pages do not mention tax lines in this mode.
//! * <https://help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/>: "Select the
//!   applicable GST ledgers: CGST and SGST/UTGST for local sales, IGST for interstate sales (and
//!   Cess, if required)"; the sales ledger is "marked as GST applicable".
//! * <https://help.tallysolutions.com/tally-prime/other-scenarios/gst-not-calculated-tally/>: "If
//!   Place of Supply and GST ledger do not match, GST will not be calculated correctly", for
//!   example IGST for a local transaction or CGST/SGST for an interstate one.
//!
//! Product choice (the pages do not say): a tax ledger line typed with no amount takes the
//! engine's figure; a mismatch of tax ledgers with the place of supply is `Unsupported` (we do
//! not post a figure the page itself calls incorrect), as is a voucher with no tax ledger
//! selected and a voucher with stock item allocation.
//!
//! Provenance of every figure: `derived` by hand from the 18 per cent slab (pack entry
//! `slab.eighteen`) and the rounding policy of the engine docs (head totals to the paisa, invoice
//! to the rupee), as in `c2_voucher_gst.rs`.

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
    Clock, CompanyId, Decimal, DrCr, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise, Period,
    Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
    gstin_check_character,
};
use ledgerly_gst::pack::load_pack;
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation, GstClassification,
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
        group: Some(group(kind)),
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

// ---------------------------------------------------------------------------------------------
// As Voucher
// ---------------------------------------------------------------------------------------------

/// A sales ledger for services, classified taxable at 18 per cent (the taxable line of an entry).
const SERVICES: LedgerId = LedgerId(22);

fn services(api: &mut Api) {
    ok(api.execute(Command::CreateLedger(Ledger {
        gst: Some(GstClassification {
            hsn: None,
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }),
        ..plain(SERVICES, "Service Charges", ReservedGroup::SalesAccounts)
    })));
}

/// An as-voucher entry: the lines as the user types them. The tax lines carry no amount.
fn as_voucher(
    base: BaseVoucherKind,
    party: LedgerId,
    pos: Option<u8>,
    lines: Vec<LedgerLine>,
) -> Voucher {
    invoice(base, VoucherMode::AsVoucher, party, pos, lines)
}

fn unsupported_reason(out: Result<Outcome<ledgerly_api::InvoiceGst>, ApiError>) -> String {
    match out {
        Ok(Outcome::Unsupported(u)) => format!("{} / {}", u.what, u.reason),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

/// Derivation: Service Charges Cr 10,000.00 at 18 per cent, company and place of supply both
/// Karnataka (29): intra-state (IGST Act s.8), so CGST 9% = 900.00 and SGST 9% = 900.00 (the
/// tax ledger lines are chosen as the page says: CGST and SGST for a local sale). Value
/// 10,000.00 + 900.00 + 900.00 = 11,800.00, a whole rupee, so no round-off. The party takes the
/// debit side: Dr Ka Buyer 11,800.00. The lines balance (Dr 11,800.00 = Cr 10,000 + 900 + 900).
#[test]
fn an_as_voucher_sale_with_local_tax_ledgers_gets_its_amounts_from_the_engine() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 0), cr(SGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(built.invoice_total, Paise::new(1_180_000));
    assert_eq!(
        built.ledger_lines,
        vec![
            dr(KA_BUYER, 1_180_000),
            cr(SERVICES, 1_000_000),
            cr(CGST, 90_000),
            cr(SGST, 90_000),
        ]
    );
}

/// Derivation: as above with a Maharashtra buyer (27): inter-state (IGST Act s.7), so IGST 18% =
/// 1,800.00 on 10,000.00; value 11,800.00. The page: IGST for an interstate sale.
#[test]
fn an_as_voucher_sale_with_igst_gets_its_amount_from_the_engine() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        MH_BUYER,
        Some(27),
        vec![cr(SERVICES, 1_000_000), cr(IGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(
        built.ledger_lines,
        vec![
            dr(MH_BUYER, 1_180_000),
            cr(SERVICES, 1_000_000),
            cr(IGST, 180_000)
        ]
    );
}

/// Derivation: a purchase from a Karnataka supplier into Karnataka: CGST 9% and SGST 9% of
/// 10,000.00 = 900.00 each, debited; the supplier is credited 11,800.00 (a purchase debits its
/// expense and tax, credits the supplier).
#[test]
fn an_as_voucher_purchase_debits_its_tax_and_credits_the_supplier() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Purchase,
        SUPPLIER,
        Some(29),
        vec![dr(SERVICES, 1_000_000), dr(CGST, 0), dr(SGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(
        built.ledger_lines,
        vec![
            cr(SUPPLIER, 1_180_000),
            dr(SERVICES, 1_000_000),
            dr(CGST, 90_000),
            dr(SGST, 90_000),
        ]
    );
}

/// Derivation: the GST-not-calculated page: IGST on a local transaction does not calculate
/// correctly, so no amount is shown: Unsupported, naming the heads the place of supply needs.
#[test]
fn igst_selected_for_a_local_sale_is_unsupported() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(IGST, 0)],
    );
    let why = unsupported_reason(api.invoice_gst(&draft));
    assert!(why.contains("CGST and SGST"), "{why}");
}

/// Derivation: the same page for CGST and SGST on an interstate sale.
#[test]
fn cgst_and_sgst_selected_for_an_interstate_sale_are_unsupported() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        MH_BUYER,
        Some(27),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 0), cr(SGST, 0)],
    );
    let why = unsupported_reason(api.invoice_gst(&draft));
    assert!(why.contains("IGST"), "{why}");
}

/// Derivation: tax is calculated from the tax ledgers selected (the sales page); with none
/// selected the entry has no tax lines to fill, and the engine will not post an untaxed
/// taxable sale as if it were right.
#[test]
fn an_as_voucher_sale_with_no_tax_ledger_is_unsupported() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000)],
    );
    let why = unsupported_reason(api.invoice_gst(&draft));
    assert!(why.contains("tax ledger"), "{why}");
}

/// Derivation: a typed tax amount is the engine's to give (invariant 9: no hand-typed tax may
/// differ). A CGST line typed as 1.00 is replaced by the engine's 900.00, never kept.
#[test]
fn a_typed_tax_amount_is_replaced_by_the_engines() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 100), cr(SGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(built.ledger_lines[2], cr(CGST, 90_000));
}

/// Derivation: stock item allocation in this mode would take its rate from the item, which the
/// engine does not combine with ledger lines here: Unsupported, never the ledger's rate.
#[test]
fn an_as_voucher_with_stock_allocation_is_unsupported() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let mut draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 0), cr(SGST, 0)],
    );
    draft.inventory_lines = items(10, 1_000_000, SALES);
    let why = unsupported_reason(api.invoice_gst(&draft));
    assert!(why.contains("stock"), "{why}");
}

/// Derivation: the draft tax of `voucher_gst` for an as-voucher entry is the same engine answer:
/// CGST 900.00 and SGST 900.00 on 10,000.00.
#[test]
fn voucher_gst_answers_for_an_as_voucher_entry() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 0), cr(SGST, 0)],
    );
    let b = ok(api.voucher_gst(&draft));
    assert_eq!(b.totals.cgst, Paise::new(90_000));
    assert_eq!(b.totals.sgst, Paise::new(90_000));
}

/// Derivation (guards the wrong-number risk): an entry posted in As Voucher mode is not an
/// invoice the returns read (their line attribution is for item and accounting invoices), so
/// GSTR-3B must answer Unsupported, never leave the sale out of 3.1(a) silently. The screen
/// therefore saves an as-voucher entry as an accounting invoice; this pins the refusal for any
/// other path.
#[test]
fn gstr3b_does_not_silently_drop_a_posted_as_voucher_sale() {
    let (_dir, mut api) = company_file();
    services(&mut api);
    let draft = as_voucher(
        BaseVoucherKind::Sales,
        KA_BUYER,
        Some(29),
        vec![cr(SERVICES, 1_000_000), cr(CGST, 0), cr(SGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    let mut v = draft;
    v.ledger_lines = built.ledger_lines;
    ok(api.execute(Command::PostVoucher(v)));
    let slabs = load_pack(include_str!("../../ledgerly-gst/packs/rates-slabs.toml")).unwrap();
    let period = Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap();
    match api.gstr3b_with_pack(&gstin_of(COMPANY), period, &slabs) {
        Ok(Outcome::Unsupported(_)) | Err(_) => {}
        other => panic!("expected Unsupported or an error, got {other:?}"),
    }
}
