//! Card G4 tests: voucher tax computation and rounding (spec 6.3; CGST Act s.9, IGST Act s.5,
//! product rounding policy).
//!
//! Provenance. Slab percentages (5, 18 with CGST/SGST halves) come from `packs/rates-slabs.toml`,
//! which rests on `docs/research/04-gst.md` 2.2 row "GST 2.0 slabs" `[V]` (N.9/2025-CT(R) and
//! N.15/2025-CT(R), 17 Sep 2025, effective 22 Sep 2025). Intra-state supply is taxed CGST + SGST
//! (CGST Act s.9) and inter-state supply IGST (IGST Act s.5). Rounding: the invoice value is
//! rounded to the nearest rupee. That is a product policy; CGST Act s.170 is about amounts payable
//! to the government and is not cited for the invoice, and the exact-50-paise case is `pending`
//! (see its test). Every expected figure below is hand arithmetic written next to it; none is
//! copied from the engine.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration-test helpers outside #[test] fns may panic on a broken fixture"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, LedgerId, Outcome, Paise, Qty, Rate, SignedAmount,
    StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{
    GstBreakup, LineSource, RoundingPolicy, TaxHeads, compute_voucher_tax,
};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GstClassification, GstHistory,
    GstRegistration, InventoryLine, Ledger, LedgerLine, Masters, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem,
    TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use proptest::prelude::*;
use time::macros::date;

const KARNATAKA: u8 = 29;
const MAHARASHTRA: u8 = 27;
const CHANDIGARH: u8 = 4;

const ITEM: StockItemId = StockItemId(1);
const SALES_LEDGER: LedgerId = LedgerId(10);
const PARTY: LedgerId = LedgerId(11);
const CESS_LEDGER: LedgerId = LedgerId(12);
const SEZ_PARTY: LedgerId = LedgerId(13);

fn rules_on(d: Date) -> RuleSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let pack = load_pack(&fs::read_to_string(path).unwrap()).unwrap();
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn company_in(code: u8) -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(code),
        pan: None,
        registrations: vec![GstRegistration {
            // Fictional GSTIN from ledgerly-core's test vectors (Karnataka). The registration
            // state below is what the computation reads.
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: state(code),
            effective_from: date!(2017 - 07 - 01),
            composition: false,
        }],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2025 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn history(from: Date, taxability: Taxability, percent: Option<i64>) -> GstHistory {
    GstHistory {
        effective_from: from,
        taxability,
        rate_percent: percent.map(|p| Rate(Decimal::new(p, 0))),
    }
}

fn item_with(gst_history: Vec<GstHistory>) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: None,
        gst_history,
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        tally: None,
    }
}

fn ledger(
    id: LedgerId,
    registration: Option<RegistrationType>,
    tax: Option<TaxDetails>,
    gst: Option<GstClassification>,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: format!("Ledger {}", id.0),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: registration.map(|registration| PartyDetails {
            registration,
            gstin: None,
            pan: None,
            state: None,
            address: Address::default(),
        }),
        bank: None,
        tax,
        gst,
        reserved: None,
        tally: None,
    }
}

fn masters(base: BaseVoucherKind, gst_history: Vec<GstHistory>) -> Masters {
    let mut m = Masters::default();
    m.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Sales".to_owned(),
            base,
            abbreviation: "Sal".to_owned(),
            numbering: NumberingConfig {
                method: NumberingMethod::Automatic,
                prevent_renumbering: true,
                prefix: String::new(),
                suffix: String::new(),
                width: 0,
                restart: RestartPeriod::Yearly,
                series: vec![],
            },
            active: true,
            tally: None,
        },
    );
    m.units.insert(
        UnitId(1),
        Unit {
            id: UnitId(1),
            symbol: "pcs".to_owned(),
            formal_name: "Pieces".to_owned(),
            decimals: 0,
            kind: UnitKind::Simple,
            uqc: None,
            tally: None,
        },
    );
    m.stock_items.insert(ITEM, item_with(gst_history));
    let sales_class = GstClassification {
        hsn: None,
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::new(5, 0))),
    };
    m.ledgers.insert(
        SALES_LEDGER,
        ledger(SALES_LEDGER, None, None, Some(sales_class)),
    );
    m.ledgers.insert(
        PARTY,
        ledger(PARTY, Some(RegistrationType::Regular), None, None),
    );
    m.ledgers.insert(
        SEZ_PARTY,
        ledger(SEZ_PARTY, Some(RegistrationType::Sez), None, None),
    );
    m.ledgers.insert(
        CESS_LEDGER,
        ledger(
            CESS_LEDGER,
            None,
            Some(TaxDetails {
                kind: TaxLedgerKind::Cess,
                rate_percent: None,
            }),
            None,
        ),
    );
    m
}

fn eighteen_from_gst2() -> Vec<GstHistory> {
    vec![history(
        date!(2025 - 09 - 22),
        Taxability::Taxable,
        Some(18),
    )]
}

fn base_voucher(mode: VoucherMode, place: Option<u8>) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: date!(2026 - 01 - 15),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party: Some(PARTY),
        place_of_supply: place.map(state),
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn inv_line(paise: i64) -> InventoryLine {
    InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(paise, 2)),
        discount_percent: None,
        amount: Paise::new(paise),
        ledger: Some(SALES_LEDGER),
        godowns: vec![],
    }
}

fn item_voucher(place: Option<u8>, amounts: &[i64]) -> Voucher {
    Voucher {
        inventory_lines: amounts.iter().map(|a| inv_line(*a)).collect(),
        ..base_voucher(VoucherMode::ItemInvoice, place)
    }
}

fn compute(v: &Voucher, m: &Masters, company: &Company) -> Result<Outcome<GstBreakup>, GstError> {
    let tos = v.supply_date.unwrap_or(v.date);
    compute_voucher_tax(v, m, &rules_on(tos), company)
}

fn value(v: &Voucher, m: &Masters, company: &Company) -> GstBreakup {
    match compute(v, m, company).unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn assert_unsupported(v: &Voucher, m: &Masters, company: &Company) {
    match compute(v, m, company).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(b) => panic!("expected Unsupported, got {b:?}"),
    }
}

#[test]
fn intra_state_10000_at_18_percent_is_cgst_900_plus_sgst_900() {
    // provenance: derived — CGST Act s.9 (intra-state: CGST + SGST); slab.eighteen = 9% + 9%
    // (N.9/2025-CT(R) via research 04-gst 2.2 [V]).
    // ₹10,000.00 = 1,000,000 paise. CGST = 10,000 x 9% = 900.00 = 90,000 paise; SGST the same.
    // Invoice value = 10,000.00 + 900.00 + 900.00 = 11,800.00, already whole rupees: round-off 0.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let b = value(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.heads, TaxHeads::CgstSgst);
    assert_eq!(b.taxable_total, Paise::new(1_000_000));
    assert_eq!(b.totals.cgst, Paise::new(90_000));
    assert_eq!(b.totals.sgst, Paise::new(90_000));
    assert_eq!(b.totals.utgst, Paise::ZERO);
    assert_eq!(b.totals.igst, Paise::ZERO);
    assert_eq!(b.totals.cess, Paise::ZERO);
    assert_eq!(b.total_before_round_off, Paise::new(1_180_000));
    assert_eq!(b.round_off, Paise::ZERO);
    assert_eq!(b.pack_version, "2026.1-slabs");
    assert_eq!(b.policy, RoundingPolicy::HeadTotalsToPaiseInvoiceToRupee);
    assert_eq!(b.lines.len(), 1);
    let line = b.lines.first().unwrap();
    assert_eq!(line.source, LineSource::Inventory { index: 0 });
    assert_eq!(line.rate_percent, Decimal::new(18, 0));
    assert_eq!(line.cgst, Decimal::new(900, 0));
    assert_eq!(line.sgst, Decimal::new(900, 0));
}

#[test]
fn inter_state_10000_at_18_percent_is_igst_1800() {
    // provenance: derived — IGST Act s.5 (inter-state: IGST); slab.eighteen igst_percent = 18.
    // IGST = 10,000.00 x 18% = 1,800.00 = 180,000 paise. Invoice value 11,800.00, round-off 0.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let b = value(
        &item_voucher(Some(MAHARASHTRA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.heads, TaxHeads::Igst);
    assert_eq!(b.totals.igst, Paise::new(180_000));
    assert_eq!(b.totals.cgst, Paise::ZERO);
    assert_eq!(b.totals.sgst, Paise::ZERO);
    assert_eq!(b.total_before_round_off, Paise::new(1_180_000));
    assert_eq!(b.round_off, Paise::ZERO);
    assert_eq!(b.lines.first().unwrap().igst, Decimal::new(1800, 0));
}

fn five_percent_history() -> Vec<GstHistory> {
    vec![history(date!(2025 - 09 - 22), Taxability::Taxable, Some(5))]
}

#[test]
fn three_lines_of_333_33_at_5_percent_intra_keep_line_tax_unrounded_and_add_round_off() {
    // provenance: derived — slab.five = CGST 2.5% + SGST 2.5% (N.9/2025-CT(R) via research [V]);
    // the round-off is to the nearest rupee, and the fractions here (.99 and .01 of a rupee) are
    // far from the half rupee, so every nearest-rupee convention gives the same answer.
    // Each line: 333.33 x 2.5% = 8.33325 (unrounded, kept as 8.33325). Three lines: 24.99975,
    // rounded to the paisa = 25.00; SGST likewise.
    // Taxable total = 3 x 333.33 = 999.99. Invoice value = 999.99 + 25.00 + 25.00 = 1,049.99.
    // nearest rupee: 99 paise is far above the half rupee, so it rounds up under every convention, so the value becomes 1,050.00 and round-off = +0.01.
    let m = masters(BaseVoucherKind::Sales, five_percent_history());
    let b = value(
        &item_voucher(Some(KARNATAKA), &[33_333, 33_333, 33_333]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.lines.len(), 3);
    for line in &b.lines {
        assert_eq!(line.cgst, Decimal::new(833_325, 5));
        assert_eq!(line.sgst, Decimal::new(833_325, 5));
    }
    assert_eq!(b.taxable_total, Paise::new(99_999));
    assert_eq!(b.totals.cgst, Paise::new(2_500));
    assert_eq!(b.totals.sgst, Paise::new(2_500));
    assert_eq!(b.total_before_round_off, Paise::new(104_999));
    assert_eq!(b.round_off, Paise::new(1));
}

#[test]
fn three_lines_of_333_33_at_5_percent_inter_keep_line_tax_unrounded_and_add_round_off() {
    // provenance: derived — slab.five igst_percent = 5; nearest-rupee round-off (fractions far from the half rupee).
    // Each line: 333.33 x 5% = 16.6665 unrounded. Three lines: 49.9995, rounded to the paisa
    // = 50.00. Invoice value = 999.99 + 50.00 = 1,049.99 -> 1,050.00, round-off +0.01.
    let m = masters(BaseVoucherKind::Sales, five_percent_history());
    let b = value(
        &item_voucher(Some(MAHARASHTRA), &[33_333, 33_333, 33_333]),
        &m,
        &company_in(KARNATAKA),
    );
    for line in &b.lines {
        assert_eq!(line.igst, Decimal::new(166_665, 4));
    }
    assert_eq!(b.totals.igst, Paise::new(5_000));
    assert_eq!(b.total_before_round_off, Paise::new(104_999));
    assert_eq!(b.round_off, Paise::new(1));
}

#[test]
fn round_off_can_be_negative() {
    // provenance: derived — slab.eighteen 9% + 9%; nearest rupee (1 paisa is far below the half
    // rupee, so every nearest-rupee convention drops it).
    // Taxable 100.01. CGST = 100.01 x 9% = 9.0009 -> 9.00; SGST 9.00. Invoice value
    // = 100.01 + 9.00 + 9.00 = 118.01; nearest rupee is 118.00, round-off -0.01.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let b = value(
        &item_voucher(Some(KARNATAKA), &[10_001]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.lines.first().unwrap().cgst, Decimal::new(90_009, 4));
    assert_eq!(b.totals.cgst, Paise::new(900));
    assert_eq!(b.total_before_round_off, Paise::new(11_801));
    assert_eq!(b.round_off, Paise::new(-1));
}

#[test]
fn exactly_fifty_paise_rounds_up() {
    // provenance: pending — no verified source for how an invoice total of exactly 50 paise is
    // rounded. The research note (04-gst 2.x, tagged [K]) says "50 paise or more rounds up" for
    // CGST s.170, but s.170 covers amounts payable to the government, and its wording for exactly
    // 50 paise was not checked. This test pins the stated product policy (half away from zero),
    // not a legal fact; the feature stays unverified in the conformance report until an
    // observed invoice from an accounting tool or the Act text is recorded.
    // Taxable 100.50 at 0% (slab.zero: igst/cgst/sgst all 0): tax 0, value 100.50 -> 101.00 under
    // the policy, round-off = +0.50 = 50 paise.
    let zero = vec![history(date!(2025 - 09 - 22), Taxability::Taxable, Some(0))];
    let m = masters(BaseVoucherKind::Sales, zero);
    let b = value(
        &item_voucher(Some(KARNATAKA), &[10_050]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.totals.cgst, Paise::ZERO);
    assert_eq!(b.total_before_round_off, Paise::new(10_050));
    assert_eq!(b.round_off, Paise::new(50));
}

#[test]
fn pending_pack_entry_is_unsupported() {
    // provenance: derived — engineering rule A8: an entry without a verified legal reference is
    // pending and evaluates to Unsupported. packs/rates-slabs.toml marks slab.eighteen pending
    // for 2017-07-01..=2025-09-21 (no notification cited for the pre-GST-2.0 slabs).
    let old = vec![history(
        date!(2017 - 07 - 01),
        Taxability::Taxable,
        Some(18),
    )];
    let m = masters(BaseVoucherKind::Sales, old);
    let mut v = item_voucher(Some(KARNATAKA), &[1_000_000]);
    v.date = date!(2025 - 09 - 21);
    assert_unsupported(&v, &m, &company_in(KARNATAKA));
}

#[test]
fn rate_is_chosen_by_time_of_supply_not_voucher_date() {
    // provenance: derived — engineering rule A8 (select by time of supply). The item rate history
    // says 12% before 2025-09-22 and 18% from it. Voucher dated 2025-09-20 with supply date
    // 2025-09-22 is taxed at 18%: CGST = 1,000.00 x 9% = 90.00 = 9,000 paise.
    let hist = vec![
        history(date!(2017 - 07 - 01), Taxability::Taxable, Some(12)),
        history(date!(2025 - 09 - 22), Taxability::Taxable, Some(18)),
    ];
    let m = masters(BaseVoucherKind::Sales, hist);
    let mut v = item_voucher(Some(KARNATAKA), &[100_000]);
    v.date = date!(2025 - 09 - 20);
    v.supply_date = Some(date!(2025 - 09 - 22));
    let b = value(&v, &m, &company_in(KARNATAKA));
    assert_eq!(b.totals.cgst, Paise::new(9_000));
    assert_eq!(b.totals.sgst, Paise::new(9_000));
}

#[test]
fn rules_selected_for_another_date_is_an_error() {
    // provenance: derived — rules must be selected by the time of supply (spec 6.3). Voucher
    // supply date is 2026-01-15; passing the rule set of 2025-10-01 is a caller bug, not a value.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let v = item_voucher(Some(KARNATAKA), &[1_000_000]);
    let r = compute_voucher_tax(
        &v,
        &m,
        &rules_on(date!(2025 - 10 - 01)),
        &company_in(KARNATAKA),
    );
    assert!(matches!(r, Err(GstError::Invalid { .. })), "{r:?}");
}

#[test]
fn rate_with_no_verified_slab_is_unsupported() {
    // provenance: derived — a 7% rate matches no slab of the pack (0, 5, 18, 40 and the two
    // special rates), so there is no verified CGST/SGST split; never guessed (rule A1).
    let hist = vec![history(date!(2025 - 09 - 22), Taxability::Taxable, Some(7))];
    let m = masters(BaseVoucherKind::Sales, hist);
    assert_unsupported(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
}

#[test]
fn taxable_item_without_rate_is_unsupported() {
    // provenance: derived — rule A1: a taxable item with no rate has no tax figure.
    let hist = vec![history(date!(2025 - 09 - 22), Taxability::Taxable, None)];
    let m = masters(BaseVoucherKind::Sales, hist);
    assert_unsupported(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
}

#[test]
fn item_without_gst_details_on_the_date_is_unsupported() {
    // provenance: derived — the only history entry starts after the supply date.
    let hist = vec![history(
        date!(2026 - 06 - 01),
        Taxability::Taxable,
        Some(18),
    )];
    let m = masters(BaseVoucherKind::Sales, hist);
    assert_unsupported(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
}

#[test]
fn exempt_nil_rated_and_non_gst_lines_carry_no_tax() {
    // provenance: derived — CGST Act s.9 levies tax only on taxable supplies; an exempt,
    // nil-rated or non-GST line has tax 0. ₹500.00 -> value 500.00, round-off 0.
    for t in [Taxability::Exempt, Taxability::NilRated, Taxability::NonGst] {
        let hist = vec![history(date!(2025 - 09 - 22), t, None)];
        let m = masters(BaseVoucherKind::Sales, hist);
        let b = value(
            &item_voucher(Some(MAHARASHTRA), &[50_000]),
            &m,
            &company_in(KARNATAKA),
        );
        assert_eq!(b.totals.igst, Paise::ZERO);
        assert_eq!(b.total_before_round_off, Paise::new(50_000));
        assert_eq!(b.round_off, Paise::ZERO);
        assert_eq!(b.lines.first().unwrap().taxability, t);
    }
}

#[test]
fn accounting_invoice_uses_the_ledger_rate() {
    // provenance: derived — slab.five 2.5% + 2.5%. Sales ledger credited ₹1,000.00 at 5%:
    // CGST = 1,000.00 x 2.5% = 25.00 = 2,500 paise; SGST 25.00. Value 1,050.00, round-off 0.
    let m = masters(BaseVoucherKind::Sales, vec![]);
    let v = Voucher {
        ledger_lines: vec![
            LedgerLine {
                ineligible_itc: None,
                ledger: PARTY,
                amount: SignedAmount::dr(Paise::new(105_000)),
                bills: vec![],
                cost: vec![],
                bank: None,
            },
            LedgerLine {
                ineligible_itc: None,
                ledger: SALES_LEDGER,
                amount: SignedAmount::cr(Paise::new(100_000)),
                bills: vec![],
                cost: vec![],
                bank: None,
            },
        ],
        ..base_voucher(VoucherMode::AccountingInvoice, Some(KARNATAKA))
    };
    let b = value(&v, &m, &company_in(KARNATAKA));
    assert_eq!(b.lines.len(), 1);
    assert_eq!(
        b.lines.first().unwrap().source,
        LineSource::Ledger {
            ledger: SALES_LEDGER
        }
    );
    assert_eq!(b.totals.cgst, Paise::new(2_500));
    assert_eq!(b.totals.sgst, Paise::new(2_500));
    assert_eq!(b.total_before_round_off, Paise::new(105_000));
    assert_eq!(b.round_off, Paise::ZERO);
}

#[test]
fn union_territory_without_legislature_charges_utgst() {
    // provenance: derived — UTGST Act s.8: intra-UT supply bears CGST and UTGST at the same
    // rates. Chandigarh (04) is a UT without a legislature (UTGST Act s.2). ₹10,000.00 at 18%:
    // CGST 900.00 + UTGST 900.00, no SGST.
    // The fixture sets the registration state directly; its GSTIN is the Karnataka test vector.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let b = value(
        &item_voucher(Some(CHANDIGARH), &[1_000_000]),
        &m,
        &company_in(CHANDIGARH),
    );
    assert_eq!(b.heads, TaxHeads::CgstUtgst);
    assert_eq!(b.totals.cgst, Paise::new(90_000));
    assert_eq!(b.totals.utgst, Paise::new(90_000));
    assert_eq!(b.totals.sgst, Paise::ZERO);
}

#[test]
fn unsupported_situations_are_reported_not_guessed() {
    // provenance: derived — each case is outside this card's scope (module docs); engineering
    // rule A1 requires Unsupported, never a default tax.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let company = company_in(KARNATAKA);
    let ok = item_voucher(Some(KARNATAKA), &[1_000_000]);

    let rcm = Voucher {
        reverse_charge: true,
        ..ok.clone()
    };
    assert_unsupported(&rcm, &m, &company);

    let no_place = Voucher {
        place_of_supply: None,
        ..ok.clone()
    };
    assert_unsupported(&no_place, &m, &company);

    let cancelled = Voucher {
        status: VoucherStatus::Cancelled,
        ..ok.clone()
    };
    assert_unsupported(&cancelled, &m, &company);

    let plain = Voucher {
        mode: VoucherMode::AsVoucher,
        ..ok.clone()
    };
    assert_unsupported(&plain, &m, &company);

    let sez = Voucher {
        party: Some(SEZ_PARTY),
        ..ok.clone()
    };
    assert_unsupported(&sez, &m, &company);

    let early_payment = Voucher {
        payment_date: Some(date!(2026 - 01 - 10)),
        ..ok.clone()
    };
    assert_unsupported(&early_payment, &m, &company);

    let mut no_gst = company_in(KARNATAKA);
    no_gst.features.gst = false;
    assert_unsupported(&ok, &m, &no_gst);

    let mut composition = company_in(KARNATAKA);
    if let Some(r) = composition.registrations.first_mut() {
        r.composition = true;
    }
    assert_unsupported(&ok, &m, &composition);

    let mut unregistered = company_in(KARNATAKA);
    unregistered.registrations.clear();
    assert_unsupported(&ok, &m, &unregistered);

    let journal = masters(BaseVoucherKind::Journal, eighteen_from_gst2());
    assert_unsupported(&ok, &journal, &company);

    let with_cess = Voucher {
        ledger_lines: vec![LedgerLine {
            ineligible_itc: None,
            ledger: CESS_LEDGER,
            amount: SignedAmount::cr(Paise::new(100)),
            bills: vec![],
            cost: vec![],
            bank: None,
        }],
        ..ok.clone()
    };
    assert_unsupported(&with_cess, &m, &company);
}

fn history_at(from: Date, percent: &str) -> GstHistory {
    GstHistory {
        effective_from: from,
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(percent.parse::<Decimal>().unwrap())),
    }
}

fn classified_line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

#[test]
fn purchase_and_debit_note_are_unsupported_not_taxed_with_the_company_state() {
    // provenance: derived — CGST Act s.9 / IGST Act s.5: the tax heads depend on the supplier's
    // location against the place of supply. On a purchase the supplier is the party, not the
    // company, so comparing the place of supply with the company's state would be wrong (an
    // out-of-state vendor delivering to the company's state is IGST, not CGST + SGST). The
    // supplier's state and registration are not applied here, so the answer is Unsupported
    // (engineering rule A1), never a figure.
    for base in [BaseVoucherKind::Purchase, BaseVoucherKind::DebitNote] {
        let m = masters(base, eighteen_from_gst2());
        let v = item_voucher(Some(KARNATAKA), &[1_000_000]);
        assert_unsupported(&v, &m, &company_in(KARNATAKA));
        assert_unsupported(&v, &m, &company_in(MAHARASHTRA));
    }
}

#[test]
fn credit_note_is_taxed_like_the_sale_it_corrects() {
    // provenance: derived — CGST Act s.9 (intra-state: CGST + SGST); slab.five = 2.5% + 2.5%
    // (N.9/2025-CT(R) via research [V]). The company is the supplier on a credit note.
    // Intra, taxable ₹1,000.00 = 100,000 paise: CGST = 1,000.00 x 2.5% = 25.00 = 2,500 paise;
    // SGST the same; value 1,050.00, already whole rupees, round-off 0.
    // Inter (place Maharashtra, supplier Karnataka): IGST 5% = 50.00 = 5,000 paise.
    let m = masters(BaseVoucherKind::CreditNote, five_percent_history());
    let intra = value(
        &item_voucher(Some(KARNATAKA), &[100_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(intra.heads, TaxHeads::CgstSgst);
    assert_eq!(intra.totals.cgst, Paise::new(2_500));
    assert_eq!(intra.totals.sgst, Paise::new(2_500));
    assert_eq!(intra.total_before_round_off, Paise::new(105_000));
    assert_eq!(intra.round_off, Paise::ZERO);
    let inter = value(
        &item_voucher(Some(MAHARASHTRA), &[100_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(inter.heads, TaxHeads::Igst);
    assert_eq!(inter.totals.igst, Paise::new(5_000));
}

#[test]
fn forty_percent_slab_splits_twenty_and_twenty() {
    // provenance: derived — slab.forty = CGST 20% + SGST 20% (N.9/2025-CT(R) via research [V]).
    // Taxable ₹1,000.00 = 100,000 paise intra: CGST = 1,000.00 x 20% = 200.00 = 20,000 paise;
    // SGST 200.00. Value = 1,000.00 + 200.00 + 200.00 = 1,400.00, round-off 0.
    let m = masters(
        BaseVoucherKind::Sales,
        vec![history(
            date!(2025 - 09 - 22),
            Taxability::Taxable,
            Some(40),
        )],
    );
    let b = value(
        &item_voucher(Some(KARNATAKA), &[100_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.totals.cgst, Paise::new(20_000));
    assert_eq!(b.totals.sgst, Paise::new(20_000));
    assert_eq!(b.total_before_round_off, Paise::new(140_000));
    assert_eq!(b.round_off, Paise::ZERO);
    assert_eq!(b.lines.first().unwrap().rate_percent, Decimal::new(40, 0));
}

#[test]
fn gold_jewellery_three_percent_splits_one_and_a_half_each() {
    // provenance: derived — special.gold_jewellery = IGST 3% = CGST 1.5% + SGST 1.5%
    // (N.9/2025-CT(R) via research [V]). Taxable ₹1,00,000.00 = 10,000,000 paise intra:
    // CGST = 100,000.00 x 1.5% = 1,500.00 = 150,000 paise; SGST 1,500.00.
    // Value = 100,000.00 + 1,500.00 + 1,500.00 = 103,000.00, round-off 0.
    let m = masters(
        BaseVoucherKind::Sales,
        vec![history(date!(2025 - 09 - 22), Taxability::Taxable, Some(3))],
    );
    let b = value(
        &item_voucher(Some(KARNATAKA), &[10_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.totals.cgst, Paise::new(150_000));
    assert_eq!(b.totals.sgst, Paise::new(150_000));
    assert_eq!(b.total_before_round_off, Paise::new(10_300_000));
    assert_eq!(b.round_off, Paise::ZERO);
}

#[test]
fn rough_diamonds_quarter_percent_splits_into_eighths() {
    // provenance: derived — special.rough_diamonds = IGST 0.25% = CGST 0.125% + SGST 0.125%
    // (N.9/2025-CT(R) via research [V]). Taxable ₹1,00,000.00 = 10,000,000 paise intra:
    // CGST = 100,000.00 x 0.125% = 125.00 = 12,500 paise; SGST 125.00. Value = 100,250.00.
    // Inter: IGST = 100,000.00 x 0.25% = 250.00 = 25,000 paise.
    let m = masters(
        BaseVoucherKind::Sales,
        vec![history_at(date!(2025 - 09 - 22), "0.25")],
    );
    let intra = value(
        &item_voucher(Some(KARNATAKA), &[10_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(intra.totals.cgst, Paise::new(12_500));
    assert_eq!(intra.totals.sgst, Paise::new(12_500));
    assert_eq!(intra.total_before_round_off, Paise::new(10_025_000));
    assert_eq!(intra.round_off, Paise::ZERO);
    let inter = value(
        &item_voucher(Some(MAHARASHTRA), &[10_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(inter.totals.igst, Paise::new(25_000));
}

#[test]
fn place_of_supply_decides_not_the_party_state() {
    // provenance: derived — IGST Act s.10(1)(a): for goods the place of supply is where the
    // movement ends; CGST s.9 / IGST s.5 compare it with the supplier's location. A buyer whose
    // registered state is Maharashtra but who takes delivery in Karnataka from a Karnataka
    // supplier is an intra-state supply (CGST + SGST); delivery in Maharashtra is inter-state.
    let mut m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    if let Some(p) = m.ledgers.get_mut(&PARTY).and_then(|l| l.party.as_mut()) {
        p.state = Some(state(MAHARASHTRA));
    }
    let company = company_in(KARNATAKA);
    let delivered_in_karnataka = value(&item_voucher(Some(KARNATAKA), &[1_000_000]), &m, &company);
    assert_eq!(delivered_in_karnataka.heads, TaxHeads::CgstSgst);
    let delivered_in_maharashtra =
        value(&item_voucher(Some(MAHARASHTRA), &[1_000_000]), &m, &company);
    assert_eq!(delivered_in_maharashtra.heads, TaxHeads::Igst);
}

#[test]
fn more_unsupported_branches_are_reported_not_guessed() {
    // provenance: derived — engineering rule A1; each case is outside the module's scope.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let company = company_in(KARNATAKA);

    // Centre jurisdiction (99) is not a state to charge CGST/SGST or IGST for.
    assert_unsupported(&item_voucher(Some(99), &[1_000_000]), &m, &company);

    // Taxable lines on both sides of one accounting invoice are not netted.
    let both_sides = Voucher {
        ledger_lines: vec![
            classified_line(SALES_LEDGER, SignedAmount::cr(Paise::new(100_000))),
            classified_line(SALES_LEDGER, SignedAmount::dr(Paise::new(50_000))),
        ],
        ..base_voucher(VoucherMode::AccountingInvoice, Some(KARNATAKA))
    };
    assert_unsupported(&both_sides, &m, &company);

    // An item invoice whose classified ledger lines (freight 500.00) differ from the inventory
    // sum (10,000.00) is not guessed.
    let freight = Voucher {
        ledger_lines: vec![classified_line(
            SALES_LEDGER,
            SignedAmount::cr(Paise::new(50_000)),
        )],
        ..item_voucher(Some(KARNATAKA), &[1_000_000])
    };
    assert_unsupported(&freight, &m, &company);
}

#[test]
fn two_item_gst_entries_from_the_same_date_are_an_error() {
    // provenance: derived — engineering rule A1: ambiguous master data (two rates effective
    // from the same date) must not be resolved by vector order.
    let both = vec![
        history(date!(2025 - 09 - 22), Taxability::Taxable, Some(5)),
        history(date!(2025 - 09 - 22), Taxability::Taxable, Some(18)),
    ];
    let m = masters(BaseVoucherKind::Sales, both);
    let r = compute(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert!(matches!(r, Err(GstError::Invalid { .. })), "{r:?}");
}

#[test]
fn every_verified_slab_in_the_pack_is_found() {
    // provenance: derived — rule A8: every verified rate slab in force on 2025-09-22 must price a
    // taxable line. A slab id missing from the module's lookup list would make its vouchers
    // Unsupported; this guards the coupling (see docs/interface-requests.md, G4).
    let d = date!(2025 - 09 - 22);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let pack = load_pack(&fs::read_to_string(path).unwrap()).unwrap();
    let mut checked = 0;
    for entry in pack.entries() {
        let in_force = entry.effective_from() <= d && entry.effective_to().is_none_or(|t| d <= t);
        let Some(percent) = entry.params().get("igst_percent") else {
            continue;
        };
        if entry.is_pending() || !in_force {
            continue;
        }
        let m = masters(BaseVoucherKind::Sales, vec![history_at(d, percent)]);
        let b = value(
            &item_voucher(Some(KARNATAKA), &[1_000_000]),
            &m,
            &company_in(KARNATAKA),
        );
        assert_eq!(
            b.lines.first().unwrap().rate_percent,
            percent.parse::<Decimal>().unwrap(),
            "slab {}",
            entry.id()
        );
        checked += 1;
    }
    assert!(
        checked >= 6,
        "expected the six verified slab entries, saw {checked}"
    );
}

#[test]
fn payment_after_invoice_does_not_change_time_of_supply() {
    // provenance: derived — a payment dated after the invoice cannot make the supply earlier
    // (CGST s.12(2)(a): earlier of invoice date and payment date), so the voucher date stands.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let v = Voucher {
        payment_date: Some(date!(2026 - 02 - 01)),
        ..item_voucher(Some(KARNATAKA), &[1_000_000])
    };
    let b = value(&v, &m, &company_in(KARNATAKA));
    assert_eq!(b.totals.cgst, Paise::new(90_000));
}

#[test]
fn missing_references_are_errors() {
    // provenance: derived — a voucher naming a stock item or party that does not exist is bad
    // data, a typed error rather than a tax figure.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let mut v = item_voucher(Some(KARNATAKA), &[1_000_000]);
    if let Some(l) = v.inventory_lines.first_mut() {
        l.item = StockItemId(999);
    }
    let r = compute(&v, &m, &company_in(KARNATAKA));
    assert!(matches!(r, Err(GstError::NotFound { .. })), "{r:?}");

    let v = Voucher {
        party: Some(LedgerId(999)),
        ..item_voucher(Some(KARNATAKA), &[1_000_000])
    };
    let r = compute(&v, &m, &company_in(KARNATAKA));
    assert!(matches!(r, Err(GstError::NotFound { .. })), "{r:?}");
}

#[test]
fn overflowing_amounts_are_errors_not_wrapped() {
    // provenance: derived — rule A3: overflow is an error. Two lines of i64::MAX / 2 + 1 paise
    // cannot be summed in i64.
    let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
    let big = i64::MAX / 2 + 1;
    let r = compute(
        &item_voucher(Some(KARNATAKA), &[big, big]),
        &m,
        &company_in(KARNATAKA),
    );
    assert!(r.is_err(), "{r:?}");
}

proptest! {
    #[test]
    fn invoice_value_after_round_off_is_whole_rupees_and_off_by_at_most_50_paise(
        amounts in proptest::collection::vec(1_i64..=1_000_000_000, 1..=4),
        inter in any::<bool>(),
    ) {
        // provenance: derived — the invoice value is rounded to the nearest rupee, so value +
        // round-off is a multiple of 100 paise and |round-off| <= 50 paise (true under every
        // nearest-rupee convention).
        // Also value = taxable + the five rounded heads by definition.
        let m = masters(BaseVoucherKind::Sales, eighteen_from_gst2());
        let place = if inter { MAHARASHTRA } else { KARNATAKA };
        let b = value(&item_voucher(Some(place), &amounts), &m, &company_in(KARNATAKA));
        let sum: i64 = amounts.iter().sum();
        prop_assert_eq!(b.taxable_total, Paise::new(sum));
        let heads = b.totals.cgst.get() + b.totals.sgst.get() + b.totals.utgst.get()
            + b.totals.igst.get() + b.totals.cess.get();
        prop_assert_eq!(b.total_before_round_off.get(), sum + heads);
        prop_assert_eq!((b.total_before_round_off.get() + b.round_off.get()) % 100, 0);
        prop_assert!(b.round_off.get().abs() <= 50);
    }
}

// ---- purchases and debit notes: the party supplies -------------------------------------------

/// Makes `PARTY` the supplier with this registration, state and GSTIN.
fn supplier(m: &mut Masters, registration: RegistrationType, st: Option<u8>, gstin: Option<&str>) {
    let p = m
        .ledgers
        .get_mut(&PARTY)
        .and_then(|l| l.party.as_mut())
        .unwrap();
    p.registration = registration;
    p.state = st.map(state);
    p.gstin = gstin.map(|g| Gstin::parse(g).unwrap());
}

#[test]
fn purchase_from_a_supplier_in_the_place_of_supply_state_is_cgst_plus_sgst() {
    // provenance: derived — CGST Act s.9 / IGST Act s.8(1): a supply whose supplier's location
    // and place of supply are in the same state is intra-state, CGST + SGST. On a purchase the
    // supplier is the party. Karnataka supplier, delivered in Karnataka: slab.eighteen 9% + 9%.
    // ₹10,000.00 x 9% = ₹900.00 = 90,000 paise CGST, the same SGST; invoice 11,800.00, round-off 0.
    for base in [BaseVoucherKind::Purchase, BaseVoucherKind::DebitNote] {
        let mut m = masters(base, eighteen_from_gst2());
        supplier(&mut m, RegistrationType::Regular, Some(KARNATAKA), None);
        let b = value(
            &item_voucher(Some(KARNATAKA), &[1_000_000]),
            &m,
            &company_in(KARNATAKA),
        );
        assert_eq!(b.heads, TaxHeads::CgstSgst, "{base:?}");
        assert_eq!(b.totals.cgst, Paise::new(90_000));
        assert_eq!(b.totals.sgst, Paise::new(90_000));
        assert_eq!(b.totals.igst, Paise::ZERO);
        assert_eq!(b.total_before_round_off, Paise::new(1_180_000));
        assert_eq!(b.round_off, Paise::ZERO);
    }
}

#[test]
fn purchase_from_another_state_is_igst_whatever_the_company_state() {
    // provenance: derived — IGST Act s.7(1): supplier's location and place of supply in different
    // states is inter-state, IGST (IGST Act s.5). A Maharashtra supplier delivering in Karnataka
    // to a Karnataka company is IGST although the place of supply equals the company's state; the
    // company's state is not the supplier's. IGST = ₹10,000.00 x 18% = ₹1,800.00 = 180,000 paise.
    for base in [BaseVoucherKind::Purchase, BaseVoucherKind::DebitNote] {
        let mut m = masters(base, eighteen_from_gst2());
        supplier(&mut m, RegistrationType::Regular, Some(MAHARASHTRA), None);
        let b = value(
            &item_voucher(Some(KARNATAKA), &[1_000_000]),
            &m,
            &company_in(KARNATAKA),
        );
        assert_eq!(b.heads, TaxHeads::Igst, "{base:?}");
        assert_eq!(b.totals.igst, Paise::new(180_000));
        assert_eq!(b.totals.cgst, Paise::ZERO);
        assert_eq!(b.totals.sgst, Paise::ZERO);
        assert_eq!(b.total_before_round_off, Paise::new(1_180_000));
    }
}

#[test]
fn supplier_state_is_read_from_its_gstin_when_the_ledger_has_none() {
    // provenance: derived — a GSTIN's first two digits are the registration's state code; the
    // ledgerly-core test vector 07AAACL0000A1ZP is a Delhi (07) registration. Delhi supplier,
    // delivered in Karnataka: inter-state, IGST 180,000 paise on ₹10,000.00 at 18%.
    let mut m = masters(BaseVoucherKind::Purchase, eighteen_from_gst2());
    supplier(
        &mut m,
        RegistrationType::Regular,
        None,
        Some("07AAACL0000A1ZP"),
    );
    let b = value(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
    assert_eq!(b.heads, TaxHeads::Igst);
    assert_eq!(b.totals.igst, Paise::new(180_000));
}

#[test]
fn purchase_supplier_whose_state_and_gstin_disagree_is_unsupported() {
    // provenance: derived — the ledger says Karnataka and the GSTIN says Delhi (07); which state
    // supplies decides CGST + SGST against IGST, so neither is guessed (rule A1).
    let mut m = masters(BaseVoucherKind::Purchase, eighteen_from_gst2());
    supplier(
        &mut m,
        RegistrationType::Regular,
        Some(KARNATAKA),
        Some("07AAACL0000A1ZP"),
    );
    assert_unsupported(
        &item_voucher(Some(KARNATAKA), &[1_000_000]),
        &m,
        &company_in(KARNATAKA),
    );
}

#[test]
fn purchase_from_a_supplier_that_is_not_a_regular_taxpayer_is_unsupported() {
    // provenance: derived — a composition supplier issues a bill of supply without tax (CGST Act
    // s.10, s.31(3)(c)); an unregistered supplier charges no GST and may bring reverse charge
    // (s.9(4)); an overseas or SEZ supplier is an import, taxed IGST under the Customs Tariff Act
    // (IGST Act s.5(1) proviso), not by this module. None of these is a CGST/SGST/IGST invoice
    // that this module can price, so all are Unsupported, never a figure.
    for registration in [
        RegistrationType::Composition,
        RegistrationType::Unregistered,
        RegistrationType::Consumer,
        RegistrationType::Sez,
        RegistrationType::Overseas,
        RegistrationType::Uin,
    ] {
        let mut m = masters(BaseVoucherKind::Purchase, eighteen_from_gst2());
        supplier(&mut m, registration, Some(KARNATAKA), None);
        match compute(
            &item_voucher(Some(KARNATAKA), &[1_000_000]),
            &m,
            &company_in(KARNATAKA),
        )
        .unwrap()
        {
            Outcome::Unsupported(_) => {}
            Outcome::Value(b) => panic!("{registration:?}: expected Unsupported, got {b:?}"),
        }
    }
}

#[test]
fn purchase_without_a_party_or_party_details_is_unsupported() {
    // provenance: derived — without the supplier's ledger details its state is unknown, and the
    // tax heads depend on it (IGST Act s.7, s.8); the state is not guessed (rule A1).
    let m = masters(BaseVoucherKind::Purchase, eighteen_from_gst2());
    let no_party = Voucher {
        party: None,
        ..item_voucher(Some(KARNATAKA), &[1_000_000])
    };
    assert_unsupported(&no_party, &m, &company_in(KARNATAKA));
    let not_a_party = Voucher {
        party: Some(SALES_LEDGER),
        ..item_voucher(Some(KARNATAKA), &[1_000_000])
    };
    assert_unsupported(&not_a_party, &m, &company_in(KARNATAKA));
}
