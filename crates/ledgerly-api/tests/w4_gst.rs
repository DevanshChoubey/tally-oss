//! Card W4 tests: GST returns over real company files (spec section 6.3).
//!
//! Provenance: every expected value is `derived` by hand, in each test, from the amounts posted
//! and the rates. 18 per cent is the GST 2.0 slab (bundled pack entry `slab.eighteen`): CGST 9
//! and SGST 9 for an intra-state supply, IGST 18 for an inter-state one. The statutory references
//! are CGST Act s.9 and IGST Act s.5 (levy), CGST Act s.17(5) (blocked credit), CGST Act s.37
//! (outward supplies in GSTR-1) and CGST Rules rule 46 (tax invoice contents). No expected figure
//! is copied from engine output.
//!
//! Pending: the JSON key names (`fp`, `b2b`, `b2cs`, `hsn`, `inum`, `idt`, `val`) follow the GSTN
//! offline tool as the formats crate writes them. The formats crate has not yet been checked
//! against the official GSTN sample, so the JSON test asserts those names as `pending`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ledgerly_api::{Api, ApiError, Bucket, BucketedVoucher, Command, Gstr1Return, RulePack};
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Rate,
    SignedAmount, StateCode, Unsupported, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_gst::pack::load_pack;
use ledgerly_gst_formats::gstr1::{B2bInvoiceType, B2csKind, ItemTax};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GstClassification, GstRegistration,
    Ledger, LedgerLine, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup,
    TaxDetails, TaxLedgerKind, Taxability, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const BUNDLED_RATE_PACK: &str = include_str!("../../ledgerly-gst/packs/rates-slabs.toml");

const COMPANY: &str = "29ABCDE1234F1Z";
const ALPHA_GSTIN: &str = "29FGHIJ5678K1Z";
const GAMMA_GSTIN: &str = "27RSTUV3456W1Z";
const SUPPLIER_GSTIN: &str = "29LMNOP9012Q1Z";

const ALPHA: LedgerId = LedgerId(10);
const GAMMA: LedgerId = LedgerId(11);
const DELTA: LedgerId = LedgerId(12);
const CONSUMER: LedgerId = LedgerId(13);
const SUPPLIER: LedgerId = LedgerId(14);
const SALES: LedgerId = LedgerId(20);
const PURCHASE: LedgerId = LedgerId(21);
const CAR: LedgerId = LedgerId(22);
const OUT_CGST: LedgerId = LedgerId(30);
const OUT_SGST: LedgerId = LedgerId(31);
const OUT_IGST: LedgerId = LedgerId(32);
const IN_CGST: LedgerId = LedgerId(33);
const IN_SGST: LedgerId = LedgerId(34);

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A GSTIN from its first 14 characters, with the check character worked out by the core.
fn gstin_of(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn rate(percent: i64) -> Rate {
    Rate(Decimal::from(percent))
}

fn oct() -> Period {
    Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
}

fn nov() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

/// The bundled GST 2.0 pack plus three fixture `itc.blocked.*` entries (CGST Act s.17(5)). The
/// bundled pack has none yet, so tests that reach the blocked-credit rules use this fixture. The
/// HSN codes are the fixture's own, not a verified list.
fn fixture_pack() -> RulePack {
    let mut text = BUNDLED_RATE_PACK.to_owned();
    text.push('\n');
    text.push_str(
        "[[entry]]\nid = \"itc.blocked.motor_vehicles\"\neffective_from = \"2017-07-01\"\n\
         legal_ref = \"CGST Act s.17(5)(a)\"\n[entry.params]\nsection = \"17(5)(a)\"\n\
         hsn_prefixes = \"8703\"\n\
         [[entry]]\nid = \"itc.blocked.vessels_aircraft\"\neffective_from = \"2017-07-01\"\n\
         legal_ref = \"CGST Act s.17(5)(aa)\"\n[entry.params]\nsection = \"17(5)(aa)\"\n\
         hsn_prefixes = \"8802\"\n\
         [[entry]]\nid = \"itc.blocked.food_beverages\"\neffective_from = \"2017-07-01\"\n\
         legal_ref = \"CGST Act s.17(5)(b)\"\n[entry.params]\nsection = \"17(5)(b)\"\n\
         hsn_prefixes = \"9963\"\n",
    );
    load_pack(&text).unwrap()
}

fn group_id(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
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

fn ledger(id: LedgerId, name: &str, group: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group_id(group),
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

fn party(
    id: LedgerId,
    name: &str,
    group: ReservedGroup,
    registration: RegistrationType,
    gstin: Option<&str>,
    place: u8,
) -> Ledger {
    let mut l = ledger(id, name, group);
    l.party = Some(PartyDetails {
        registration,
        gstin: gstin.map(gstin_of),
        pan: None,
        state: Some(state(place)),
        address: Address::default(),
    });
    l
}

/// A ledger that carries a GST classification: an accounting invoice's taxable base.
fn classified(mut l: Ledger, hsn: &str, percent: i64) -> Ledger {
    l.gst = Some(GstClassification {
        hsn: Some(Hsn::parse(hsn).unwrap()),
        taxability: Taxability::Taxable,
        rate_percent: Some(rate(percent)),
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

/// The masters of the test company. `car_ineligible` is the setting on the car purchase ledger:
/// `None` leaves it unmarked, so only the HSN rules decide.
fn ledgers(car_ineligible: Option<bool>) -> Vec<Ledger> {
    let mut car = classified(
        ledger(CAR, "Car", ReservedGroup::PurchaseAccounts),
        "8703",
        18,
    );
    car.ineligible_itc = car_ineligible;
    vec![
        party(
            ALPHA,
            "Alpha",
            ReservedGroup::SundryDebtors,
            RegistrationType::Regular,
            Some(ALPHA_GSTIN),
            29,
        ),
        party(
            GAMMA,
            "Gamma",
            ReservedGroup::SundryDebtors,
            RegistrationType::Regular,
            Some(GAMMA_GSTIN),
            27,
        ),
        party(
            DELTA,
            "Delta",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            27,
        ),
        party(
            CONSUMER,
            "Walk-in",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            29,
        ),
        party(
            SUPPLIER,
            "Supplier",
            ReservedGroup::SundryCreditors,
            RegistrationType::Regular,
            Some(SUPPLIER_GSTIN),
            29,
        ),
        classified(
            ledger(SALES, "Sales", ReservedGroup::SalesAccounts),
            "998313",
            18,
        ),
        classified(
            ledger(PURCHASE, "Purchases", ReservedGroup::PurchaseAccounts),
            "998313",
            18,
        ),
        car,
        tax_ledger(OUT_CGST, "Output CGST", TaxLedgerKind::Cgst),
        tax_ledger(OUT_SGST, "Output SGST", TaxLedgerKind::Sgst),
        tax_ledger(OUT_IGST, "Output IGST", TaxLedgerKind::Igst),
        tax_ledger(IN_CGST, "Input CGST", TaxLedgerKind::Cgst),
        tax_ledger(IN_SGST, "Input SGST", TaxLedgerKind::Sgst),
    ]
}

/// A new company file with the masters above, and no vouchers.
fn company_file(car_ineligible: Option<bool>) -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    for l in ledgers(car_ineligible) {
        value(api.execute(Command::CreateLedger(l)));
    }
    (dir, api)
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

/// A voucher whose place of supply is `pos`. The number is left for the engine to allocate.
/// Every GST voucher here is an accounting invoice (`VoucherMode::AccountingInvoice`): its taxable
/// bases are the ledger lines whose ledger carries a GST classification (compute.rs). A plain
/// Dr/Cr voucher has no tax computation and is refused as `Unsupported`.
fn voucher(
    id: u64,
    base: BaseVoucherKind,
    on: Date,
    party: LedgerId,
    pos: u8,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: vtype(base),
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

// The vouchers. Amounts are paise; each voucher balances by hand.

/// V1, 5 Oct 2026: Alpha (state 29), intra-state B2B. Taxable Rs 1,00,000 at 18 per cent gives
/// CGST 9,000 and SGST 9,000, so the invoice value is Rs 1,18,000.
fn v1_alpha_b2b() -> Voucher {
    voucher(
        1,
        BaseVoucherKind::Sales,
        date!(2026 - 10 - 05),
        ALPHA,
        29,
        vec![
            dr(ALPHA, 11_800_000),
            cr(SALES, 10_000_000),
            cr(OUT_CGST, 900_000),
            cr(OUT_SGST, 900_000),
        ],
    )
}

/// V2, 6 Oct 2026: unregistered consumer in state 29, intra-state, below the B2C threshold, so B2CS.
/// Taxable Rs 2,000 at 18 per cent gives CGST 180 and SGST 180, so the total is Rs 2,360.
fn v2_consumer_b2cs() -> Voucher {
    voucher(
        2,
        BaseVoucherKind::Sales,
        date!(2026 - 10 - 06),
        CONSUMER,
        29,
        vec![
            dr(CONSUMER, 236_000),
            cr(SALES, 200_000),
            cr(OUT_CGST, 18_000),
            cr(OUT_SGST, 18_000),
        ],
    )
}

/// V3, 7 Oct 2026: Gamma (state 27), inter-state B2B. Taxable Rs 50,000 at 18 per cent gives IGST
/// 9,000, so the invoice value is Rs 59,000.
fn v3_gamma_b2b() -> Voucher {
    voucher(
        3,
        BaseVoucherKind::Sales,
        date!(2026 - 10 - 07),
        GAMMA,
        27,
        vec![
            dr(GAMMA, 5_900_000),
            cr(SALES, 5_000_000),
            cr(OUT_IGST, 900_000),
        ],
    )
}

/// V4, 10 Oct 2026: purchase from a registered supplier in state 29, intra-state. Taxable Rs 40,000
/// gives CGST 3,600 and SGST 3,600, so the bill is Rs 47,200. This is ordinary input credit.
fn v4_purchase() -> Voucher {
    voucher(
        4,
        BaseVoucherKind::Purchase,
        date!(2026 - 10 - 10),
        SUPPLIER,
        29,
        vec![
            dr(PURCHASE, 4_000_000),
            dr(IN_CGST, 360_000),
            dr(IN_SGST, 360_000),
            cr(SUPPLIER, 4_720_000),
        ],
    )
}

/// V5, 5 Nov 2026: Delta (unregistered, state 27), inter-state B2C. Taxable Rs 2,00,000 at 18 per
/// cent gives IGST 36,000.
fn v5_delta_b2c_nov() -> Voucher {
    voucher(
        5,
        BaseVoucherKind::Sales,
        date!(2026 - 11 - 05),
        DELTA,
        27,
        vec![
            dr(DELTA, 23_600_000),
            cr(SALES, 20_000_000),
            cr(OUT_IGST, 3_600_000),
        ],
    )
}

/// V6, 12 Oct 2026: purchase of a motor car (HSN 8703, fixture-blocked), intra-state. Taxable Rs
/// 10,000 gives CGST 900 and SGST 900.
fn v6_car_purchase() -> Voucher {
    voucher(
        6,
        BaseVoucherKind::Purchase,
        date!(2026 - 10 - 12),
        SUPPLIER,
        29,
        vec![
            dr(CAR, 1_000_000),
            dr(IN_CGST, 90_000),
            dr(IN_SGST, 90_000),
            cr(SUPPLIER, 1_180_000),
        ],
    )
}

fn post(api: &mut Api, v: Voucher) {
    value(api.execute(Command::PostVoucher(v)));
}

// ---------------------------------------------------------------------------------------------
// Helpers over results and rows
// ---------------------------------------------------------------------------------------------

fn value<T: Debug>(out: Result<Outcome<T>, ApiError>) -> T {
    match out {
        Ok(Outcome::Value(v)) => v,
        other => panic!("expected a value, got {other:?}"),
    }
}

fn unsupported<T: Debug>(out: Result<Outcome<T>, ApiError>) -> Unsupported {
    match out {
        Ok(Outcome::Unsupported(u)) => u,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

fn gstr1_return(api: &Api, period: Period) -> Gstr1Return {
    value(api.gstr1(&gstin_of(COMPANY), period))
}

fn bucket_of(buckets: &[BucketedVoucher], id: u64) -> Bucket {
    buckets
        .iter()
        .find(|b| b.voucher == VoucherId(id))
        .map(|b| b.bucket)
        .unwrap()
}

/// CGST and SGST of one item. An inter-state item must not come here.
fn cgst_sgst(tax: &ItemTax) -> (Paise, Paise) {
    match tax {
        ItemTax::CgstSgst { cgst, sgst } => (*cgst, *sgst),
        ItemTax::Igst(igst) => panic!("expected CGST and SGST, got IGST {igst:?}"),
    }
}

/// IGST of one item. An intra-state item must not come here.
fn igst(tax: &ItemTax) -> Paise {
    match tax {
        ItemTax::Igst(igst) => *igst,
        ItemTax::CgstSgst { cgst, sgst } => {
            panic!("expected IGST, got CGST {cgst:?} and SGST {sgst:?}")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// GSTR-1 tables, October 2026
// ---------------------------------------------------------------------------------------------

/// Derivation (all paise): outward taxable = 10,000,000 (V1) + 200,000 (V2) + 5,000,000 (V3)
/// = 15,200,000. IGST = 900,000 (V3). CGST = 900,000 (V1) + 18,000 (V2) = 918,000. SGST is the
/// same. The HSN total value is the sum of the three invoice values, 11,800,000 + 236,000 +
/// 5,900,000 = 17,936,000, which also equals taxable plus tax. Purchases (V4, V6) are not
/// outward supplies (CGST Act s.37), so they land in no table and are `NotRelevant`.
#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "one hand-derived table check per GSTR-1 section, kept together for review"
)]
fn gstr1_october_tables_match_hand_derivation() {
    let (_dir, mut api) = company_file(Some(true));
    for v in [
        v1_alpha_b2b(),
        v2_consumer_b2cs(),
        v3_gamma_b2b(),
        v4_purchase(),
        v6_car_purchase(),
    ] {
        post(&mut api, v);
    }
    let ret = gstr1_return(&api, oct());

    assert_eq!(bucket_of(&ret.buckets, 1), Bucket::Ready);
    assert_eq!(bucket_of(&ret.buckets, 2), Bucket::Ready);
    assert_eq!(bucket_of(&ret.buckets, 3), Bucket::Ready);
    assert_eq!(bucket_of(&ret.buckets, 4), Bucket::NotRelevant);
    assert_eq!(bucket_of(&ret.buckets, 6), Bucket::NotRelevant);
    assert!(ret.is_complete());

    // Table 4A: one party per registered buyer, one invoice each.
    assert_eq!(ret.gstr1.b2b.len(), 2);
    let alpha = ret
        .gstr1
        .b2b
        .iter()
        .find(|p| p.ctin == gstin_of(ALPHA_GSTIN))
        .unwrap();
    let inv = &alpha.invoices[0];
    assert_eq!(inv.num.as_str(), "1");
    assert_eq!(inv.date, date!(2026 - 10 - 05));
    assert_eq!(inv.value, Paise::new(11_800_000));
    assert_eq!(inv.pos, state(29));
    assert!(!inv.reverse_charge);
    assert!(matches!(inv.inv_type, B2bInvoiceType::Regular));
    assert_eq!(inv.items.len(), 1);
    let item = &inv.items[0];
    assert_eq!(item.rate, rate(18));
    assert_eq!(item.taxable, Paise::new(10_000_000));
    assert_eq!(
        cgst_sgst(&item.tax),
        (Paise::new(900_000), Paise::new(900_000))
    );

    let gamma = ret
        .gstr1
        .b2b
        .iter()
        .find(|p| p.ctin == gstin_of(GAMMA_GSTIN))
        .unwrap();
    let inv = &gamma.invoices[0];
    assert_eq!(inv.num.as_str(), "3");
    assert_eq!(inv.value, Paise::new(5_900_000));
    assert_eq!(inv.pos, state(27));
    let item = &inv.items[0];
    assert_eq!(item.taxable, Paise::new(5_000_000));
    assert_eq!(igst(&item.tax), Paise::new(900_000));

    // Table 7: the walk-in consumer's intra-state sale, B2CS.
    assert_eq!(ret.gstr1.b2cs.len(), 1);
    let row = &ret.gstr1.b2cs[0];
    assert!(row.intra_state);
    assert!(matches!(row.kind, B2csKind::Other));
    assert_eq!(row.pos, state(29));
    assert_eq!(row.line.rate, rate(18));
    assert_eq!(row.line.taxable, Paise::new(200_000));
    assert_eq!(
        cgst_sgst(&row.line.tax),
        (Paise::new(18_000), Paise::new(18_000))
    );

    // Table 12: one HSN row for 998313 at 18 per cent.
    assert_eq!(ret.gstr1.hsn.len(), 1);
    let h = &ret.gstr1.hsn[0];
    assert_eq!(String::from(h.hsn.clone()), "998313");
    assert_eq!(h.rate, rate(18));
    assert_eq!(h.taxable, Paise::new(15_200_000));
    assert_eq!(h.igst, Paise::new(900_000));
    assert_eq!(h.cgst, Paise::new(918_000));
    assert_eq!(h.sgst, Paise::new(918_000));
    assert_eq!(h.cess, Paise::ZERO);
    assert_eq!(h.value, Paise::new(17_936_000));
}

/// Derivation: the JSON is written because the October return is complete. Its shape is the
/// GSTN offline layout as the formats crate writes it: the period `fp` as MMYYYY (`102026`), the
/// `b2b` and `b2cs` tables, and an invoice carrying its number `inum`, date `idt` (DD-MM-YYYY)
/// and value `val` in rupees with two decimals. Key names are `pending` (see the module docs).
#[test]
fn gstr1_json_is_written_for_a_complete_return_in_the_documented_shape() {
    let (_dir, mut api) = company_file(Some(true));
    for v in [
        v1_alpha_b2b(),
        v2_consumer_b2cs(),
        v3_gamma_b2b(),
        v4_purchase(),
        v6_car_purchase(),
    ] {
        post(&mut api, v);
    }
    let json = value(api.gstr1_json(&gstin_of(COMPANY), oct()));

    assert!(json.starts_with('{'));
    assert!(json.trim_end().ends_with('}'));
    assert!(json.contains("\"fp\":\"102026\""));
    assert!(json.contains("\"b2b\":["));
    assert!(json.contains("\"b2cs\":["));
    assert!(json.contains("\"inum\":\"1\""));
    assert!(json.contains("\"idt\":\"05-10-2026\""));
    assert!(json.contains("\"val\":118000.00"));
}

/// Derivation: V5 is inter-state B2C. A pack with no `gstr1.b2cl_threshold` entry (the slab pack
/// alone) cannot say whether it is above the threshold, so the engine cannot place it and buckets
/// it `NeedsCorrection` (rule A1). The JSON is therefore refused, and the reason names the
/// voucher. Nothing is written for a wrong return.
#[test]
fn gstr1_json_is_refused_while_a_voucher_needs_correction() {
    let (_dir, mut api) = company_file(Some(true));
    post(&mut api, v5_delta_b2c_nov());
    let slabs_only = load_pack(BUNDLED_RATE_PACK).unwrap();

    let ret = value(api.gstr1_with_pack(&gstin_of(COMPANY), nov(), &slabs_only));
    assert_eq!(bucket_of(&ret.buckets, 5), Bucket::NeedsCorrection);
    assert!(!ret.is_complete());

    let u = unsupported(api.gstr1_json_with_pack(&gstin_of(COMPANY), nov(), &slabs_only));
    assert!(u.reason.contains("voucher 5"), "reason was: {}", u.reason);
}

/// Derivation: with the bundled packs (slabs plus `gstr-rules.toml`) the threshold is read. V5 is
/// an inter-state sale to an unregistered buyer, invoice value Rs 2,36,000 (23,600,000 paise),
/// which is more than the Rs 1 lakh of Rule 59(4) as amended by notification 12/2024-Central Tax
/// (read in `gstr-rules.toml`), so it is a B2CL invoice (Table 5) and the return is complete.
#[test]
fn an_inter_state_b2c_sale_above_one_lakh_is_ready_as_b2cl_on_the_bundled_packs() {
    let (_dir, mut api) = company_file(Some(true));
    post(&mut api, v5_delta_b2c_nov());

    let ret = gstr1_return(&api, nov());
    assert_eq!(bucket_of(&ret.buckets, 5), Bucket::Ready);
    assert!(ret.is_complete());
    assert_eq!(ret.gstr1.b2cl.len(), 1);
    value(api.gstr1_json(&gstin_of(COMPANY), nov()));
}

// ---------------------------------------------------------------------------------------------
// GSTR-3B working
// ---------------------------------------------------------------------------------------------

/// Derivation (paise). Outward taxable 15,200,000 (V1, V2, V3). Outward tax: integrated 900,000
/// (V3), central 900,000 + 18,000 = 918,000 (V1, V2), state/UT 918,000. Input credit on V4, an
/// ordinary intra-state purchase: central 360,000, state/UT 360,000, integrated 0. The car purchase
/// (V6) is marked ineligible on its ledger, so its CGST 90,000 and SGST 90,000 are not claimable
/// and are shown in the not-claimable row, with nothing in the eligible row.
#[test]
fn gstr3b_october_working_matches_hand_derivation() {
    let (_dir, mut api) = company_file(Some(true));
    for v in [
        v1_alpha_b2b(),
        v2_consumer_b2cs(),
        v3_gamma_b2b(),
        v4_purchase(),
        v6_car_purchase(),
    ] {
        post(&mut api, v);
    }
    let w = value(api.gstr3b_with_pack(&gstin_of(COMPANY), oct(), &fixture_pack()));

    assert_eq!(w.outward_taxable_value, Paise::new(15_200_000));
    assert_eq!(w.outward_tax.integrated, Paise::new(900_000));
    assert_eq!(w.outward_tax.central, Paise::new(918_000));
    assert_eq!(w.outward_tax.state_ut, Paise::new(918_000));
    assert_eq!(w.outward_tax.cess, Paise::ZERO);
    assert_eq!(w.outward_vouchers.len(), 3);
    for id in [1, 2, 3] {
        assert!(w.outward_vouchers.contains(&VoucherId(id)));
    }

    assert_eq!(w.itc_all_other.integrated, Paise::ZERO);
    assert_eq!(w.itc_all_other.central, Paise::new(360_000));
    assert_eq!(w.itc_all_other.state_ut, Paise::new(360_000));
    assert!(w.itc_vouchers.contains(&VoucherId(4)));

    assert_eq!(w.itc_not_claimable.integrated, Paise::ZERO);
    assert_eq!(w.itc_not_claimable.central, Paise::new(90_000));
    assert_eq!(w.itc_not_claimable.state_ut, Paise::new(90_000));

    assert_eq!(w.reverse_charge_purchases, Vec::<VoucherId>::new());
}

/// Derivation: a pack with no `itc.blocked.*` entries (the slab pack alone) cannot rule out credit
/// under CGST Act s.17(5) for a purchase dated in the period. The working is refused (rule A1), not
/// shown with an unchecked credit.
#[test]
fn gstr3b_refuses_a_period_with_a_purchase_when_the_pack_has_no_blocked_rules() {
    let (_dir, mut api) = company_file(Some(true));
    post(&mut api, v4_purchase());
    let slabs_only = load_pack(BUNDLED_RATE_PACK).unwrap();

    let u = unsupported(api.gstr3b_with_pack(&gstin_of(COMPANY), oct(), &slabs_only));
    assert!(u.what.contains("GSTR-3B"), "what was: {}", u.what);
}

/// Derivation: with the bundled packs the s.17(5) headings are read (`gstr-rules.toml`), and V4, an
/// ordinary intra-state purchase of goods with no HSN under any blocked heading, is eligible. Its
/// credit is central 360,000 and state/UT 360,000 paise, as in the October working above (V4 is
/// the only purchase here).
#[test]
fn gstr3b_gives_the_credit_of_an_ordinary_purchase_on_the_bundled_packs() {
    let (_dir, mut api) = company_file(Some(true));
    post(&mut api, v4_purchase());

    let w = value(api.gstr3b(&gstin_of(COMPANY), oct()));
    assert_eq!(w.itc_all_other.central, Paise::new(360_000));
    assert_eq!(w.itc_all_other.state_ut, Paise::new(360_000));
    assert_eq!(w.itc_all_other.integrated, Paise::ZERO);
}

/// Derivation: a November period with only V5 has no purchase, so the bundled pack gate passes.
/// Outward taxable Rs 2,00,000 = 20,000,000; IGST 36,000 = 3,600,000; no intra-state tax.
#[test]
fn gstr3b_november_outward_only_on_the_bundled_pack() {
    let (_dir, mut api) = company_file(Some(true));
    post(&mut api, v5_delta_b2c_nov());

    let w = value(api.gstr3b(&gstin_of(COMPANY), nov()));
    assert_eq!(w.outward_taxable_value, Paise::new(20_000_000));
    assert_eq!(w.outward_tax.integrated, Paise::new(3_600_000));
    assert_eq!(w.outward_tax.central, Paise::ZERO);
    assert_eq!(w.outward_tax.state_ut, Paise::ZERO);
    assert_eq!(w.outward_vouchers, vec![VoucherId(5)]);
    assert_eq!(w.itc_vouchers, Vec::<VoucherId>::new());
}

/// Derivation: the car purchase (V6) has HSN 8703, which the fixture pack blocks under CGST Act
/// s.17(5)(a). Nothing is marked on its ledger, its invoice line or the company, so the setting
/// that decides is absent and the HSN fallback is the only rule left. Credit on a blocked category
/// with nothing marked cannot be claimed or refused, so the working is `Unsupported` (rule A1) and
/// names the category, not a guessed credit.
#[test]
fn gstr3b_blocked_category_with_nothing_marked_is_unsupported() {
    let (_dir, mut api) = company_file(None);
    post(&mut api, v6_car_purchase());

    let u = unsupported(api.gstr3b_with_pack(&gstin_of(COMPANY), oct(), &fixture_pack()));
    assert_ne!(u.reason, "");
}
