//! Card G5 tests: reverse charge (CGST Act s.9(3), s.9(4); spec 6.3; research 04-gst 2.5).
//!
//! Provenance. The legal rule under test is that the recipient, not the supplier, pays the tax on
//! a notified class of supply (CGST Act s.9(3)), and on supplies by unregistered persons only for
//! the classes notified under s.9(4) (research 04-gst 2.5, tags `[V]`/`[K]`). The classes used
//! below (SAC prefix 9965 for transport of goods, HSN prefix 7204 for metal scrap) are **test
//! fixture entries written inside this file**, with the legal reference "TEST FIXTURE"; they
//! exercise the pack-driven machinery and are not statements of which supplies are notified. The
//! rates come from `packs/rates-slabs.toml` (`slab.eighteen` = 9% + 9%, 18% IGST; N.9/2025-CT(R)
//! and N.15/2025-CT(R), research 04-gst 2.2 `[V]`). Intra-state supply is CGST + SGST (CGST Act
//! s.9), inter-state IGST (IGST Act s.5), a union territory without legislature CGST + UTGST.
//! Tax per head is summed over the voucher and rounded once, half away from zero, to the paisa
//! (the product policy of card G4). Every expected figure is hand arithmetic written next to it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{LineSource, TaxHeads};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_gst::rcm::{
    NoRcmReason, RcmItc, RcmLiability, RcmPayable, reverse_charge, reverse_charge_for_company,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GstClassification, GstHistory,
    GstRegistration, InventoryLine, Ledger, LedgerLine, Masters, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem,
    TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use time::macros::date;

const KARNATAKA: u8 = 29;
const MAHARASHTRA: u8 = 27;
const CHANDIGARH: u8 = 4;

const ITEM: StockItemId = StockItemId(1);
const CESS_LEDGER: LedgerId = LedgerId(12);
const GTA_LEDGER: LedgerId = LedgerId(20);
const OTHER_LEDGER: LedgerId = LedgerId(21);
const SCRAP_LEDGER: LedgerId = LedgerId(22);
const EXEMPT_GTA_LEDGER: LedgerId = LedgerId(23);
const NO_HSN_LEDGER: LedgerId = LedgerId(24);
const SEVEN_PERCENT_LEDGER: LedgerId = LedgerId(25);
const UNREG_KA: LedgerId = LedgerId(30);
const UNREG_MH: LedgerId = LedgerId(31);
const UNREG_NO_STATE: LedgerId = LedgerId(32);
const REG_KA: LedgerId = LedgerId(33);
const COMPOSITION: LedgerId = LedgerId(34);
const OVERSEAS: LedgerId = LedgerId(35);
const UNREG_CH: LedgerId = LedgerId(36);

/// Test fixture classes: two classes, one for any supplier and one for unregistered suppliers.
const RCM_PACK: &str = r#"
[[entry]]
id = "rcm.classes"
effective_from = "2025-09-22"
legal_ref = "TEST FIXTURE: index of class ids for engine tests, not a statement of law"
[entry.params]
ids = "gta,scrap"

[[entry]]
id = "rcm.class.gta"
effective_from = "2025-09-22"
legal_ref = "TEST FIXTURE gta"
[entry.params]
hsn_prefixes = "9965"
supplier = "any"

[[entry]]
id = "rcm.class.scrap"
effective_from = "2025-09-22"
legal_ref = "TEST FIXTURE scrap"
[entry.params]
hsn_prefixes = "7204"
supplier = "unregistered"
"#;

fn rules_with(extra: &str, d: Date) -> RuleSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let text = format!("{}\n{extra}", fs::read_to_string(path).unwrap());
    let pack = load_pack(&text).unwrap();
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn rules_on(d: Date) -> RuleSet {
    rules_with(RCM_PACK, d)
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn company(composition: bool, gst: bool) -> Company {
    Company {
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(KARNATAKA),
        pan: None,
        registrations: vec![GstRegistration {
            // Fictional GSTIN from ledgerly-core's test vectors.
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: state(KARNATAKA),
            effective_from: date!(2017 - 07 - 01),
            composition,
        }],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2025 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst,
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn class(hsn: Option<&str>, taxability: Taxability, percent: i64) -> GstClassification {
    GstClassification {
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        taxability,
        rate_percent: Some(Rate(Decimal::new(percent, 0))),
    }
}

fn ledger(
    id: LedgerId,
    party: Option<(RegistrationType, Option<u8>)>,
    tax: Option<TaxDetails>,
    gst: Option<GstClassification>,
) -> Ledger {
    Ledger {
        id,
        name: format!("Ledger {}", id.0),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: party.map(|(registration, st)| PartyDetails {
            registration,
            gstin: None,
            pan: None,
            state: st.map(state),
            address: Address::default(),
        }),
        bank: None,
        tax,
        gst,
        reserved: None,
        tally: None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one fixture table of ledgers; splitting it would hide the data"
)]
fn masters(base: BaseVoucherKind) -> Masters {
    let mut m = Masters::default();
    m.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Purchase".to_owned(),
            base,
            abbreviation: "Pur".to_owned(),
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
    m.stock_items.insert(
        ITEM,
        StockItem {
            id: ITEM,
            name: "Freight item".to_owned(),
            aliases: vec![],
            group: None,
            unit: UnitId(1),
            alt_unit: None,
            hsn: Some(Hsn::parse("996511").unwrap()),
            gst_history: vec![GstHistory {
                effective_from: date!(2025 - 09 - 22),
                taxability: Taxability::Taxable,
                rate_percent: Some(Rate(Decimal::new(18, 0))),
            }],
            valuation: ValuationMethod::AverageCost,
            batches_enabled: false,
            opening: vec![],
            standard_costs: vec![],
            tally: None,
        },
    );
    let t = Taxability::Taxable;
    let ledgers = [
        (GTA_LEDGER, None, class(Some("996511"), t, 18)),
        (OTHER_LEDGER, None, class(Some("998313"), t, 18)),
        (SCRAP_LEDGER, None, class(Some("7204"), t, 18)),
        (
            EXEMPT_GTA_LEDGER,
            None,
            class(Some("996511"), Taxability::Exempt, 0),
        ),
        (NO_HSN_LEDGER, None, class(None, t, 18)),
        (SEVEN_PERCENT_LEDGER, None, class(Some("996511"), t, 7)),
    ];
    for (id, party, gst) in ledgers {
        m.ledgers.insert(id, ledger(id, party, None, Some(gst)));
    }
    let parties = [
        (UNREG_KA, RegistrationType::Unregistered, Some(KARNATAKA)),
        (UNREG_MH, RegistrationType::Unregistered, Some(MAHARASHTRA)),
        (UNREG_NO_STATE, RegistrationType::Unregistered, None),
        (REG_KA, RegistrationType::Regular, Some(KARNATAKA)),
        (COMPOSITION, RegistrationType::Composition, Some(KARNATAKA)),
        (OVERSEAS, RegistrationType::Overseas, None),
        (UNREG_CH, RegistrationType::Unregistered, Some(CHANDIGARH)),
    ];
    for (id, reg, st) in parties {
        m.ledgers
            .insert(id, ledger(id, Some((reg, st)), None, None));
    }
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

fn line(ledger: LedgerId, paise: i64) -> LedgerLine {
    LedgerLine {
        ledger,
        amount: SignedAmount::dr(Paise::new(paise)),
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

/// A purchase dated 2026-01-15 with that supply date, from `party`, place of supply `place`.
fn purchase(party: LedgerId, place: Option<u8>, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: date!(2026 - 01 - 15),
        supply_date: Some(date!(2026 - 01 - 15)),
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(party),
        place_of_supply: place.map(state),
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn gta_purchase(party: LedgerId, place: Option<u8>) -> Voucher {
    purchase(party, place, vec![line(GTA_LEDGER, 1_000_000)])
}

fn run(v: &Voucher, m: &Masters) -> Result<Outcome<RcmLiability>, GstError> {
    reverse_charge(v, m, &rules_on(v.supply_date.unwrap_or(v.date)))
}

fn liability(v: &Voucher, m: &Masters) -> RcmLiability {
    match run(v, m).unwrap() {
        Outcome::Value(l) => l,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn payable(v: &Voucher, m: &Masters) -> RcmPayable {
    match liability(v, m) {
        RcmLiability::Payable(p) => p,
        RcmLiability::NotApplicable(r) => panic!("expected Payable, got NotApplicable({r:?})"),
    }
}

fn assert_not_applicable(v: &Voucher, m: &Masters, reason: NoRcmReason) {
    assert_eq!(liability(v, m), RcmLiability::NotApplicable(reason));
}

fn assert_unsupported(v: &Voucher, m: &Masters) {
    match run(v, m).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(l) => panic!("expected Unsupported, got {l:?}"),
    }
}

fn purchase_masters() -> Masters {
    masters(BaseVoucherKind::Purchase)
}

#[test]
fn notified_service_from_unregistered_supplier_intra_state_is_cgst_plus_sgst_on_the_recipient() {
    // provenance: derived — CGST Act s.9(3) (recipient pays on notified supplies); s.9 intra-state
    // is CGST + SGST; slab.eighteen = 9% + 9%. Fixture class `gta` (SAC prefix 9965) matches the
    // ledger's SAC 996511.
    // ₹10,000.00 = 1,000,000 paise. CGST = 10,000 x 9% = 900.00 = 90,000 paise; SGST the same.
    let p = payable(
        &gta_purchase(UNREG_KA, Some(KARNATAKA)),
        &purchase_masters(),
    );
    assert_eq!(p.heads, TaxHeads::CgstSgst);
    assert_eq!(p.class_id, "gta");
    assert_eq!(p.legal_ref, "TEST FIXTURE gta");
    assert_eq!(p.pack_version, "2026.1-slabs");
    assert_eq!(p.taxable_total, Paise::new(1_000_000));
    assert_eq!(p.totals.cgst, Paise::new(90_000));
    assert_eq!(p.totals.sgst, Paise::new(90_000));
    assert_eq!(p.totals.utgst, Paise::ZERO);
    assert_eq!(p.totals.igst, Paise::ZERO);
    assert_eq!(p.totals.cess, Paise::ZERO);
    assert_eq!(p.lines.len(), 1);
    let l = p.lines.first().unwrap();
    assert_eq!(l.source, LineSource::Ledger { ledger: GTA_LEDGER });
    assert_eq!(l.rate_percent, Decimal::new(18, 0));
    assert_eq!(l.cgst, Decimal::new(900, 0));
    assert_eq!(l.sgst, Decimal::new(900, 0));
}

#[test]
fn itc_on_the_rcm_tax_is_available_only_after_it_is_paid() {
    // provenance: derived — research 04-gst 2.5 (tag [K]): RCM tax is paid in cash and ITC follows
    // payment. The engine reports the timing only; eligibility is card G7's.
    let p = payable(
        &gta_purchase(UNREG_KA, Some(KARNATAKA)),
        &purchase_masters(),
    );
    assert_eq!(p.itc, RcmItc::AfterTaxPaid);
}

#[test]
fn inter_state_supply_is_igst_on_the_recipient() {
    // provenance: derived — IGST Act s.5; slab.eighteen igst_percent = 18.
    // Supplier in Maharashtra, place of supply Karnataka: IGST = 10,000.00 x 18% = 1,800.00
    // = 180,000 paise.
    let p = payable(
        &gta_purchase(UNREG_MH, Some(KARNATAKA)),
        &purchase_masters(),
    );
    assert_eq!(p.heads, TaxHeads::Igst);
    assert_eq!(p.totals.igst, Paise::new(180_000));
    assert_eq!(p.totals.cgst, Paise::ZERO);
    assert_eq!(p.totals.sgst, Paise::ZERO);
}

#[test]
fn union_territory_without_legislature_is_cgst_plus_utgst() {
    // provenance: derived — UTGST Act s.8 (intra-UT supply: CGST + UTGST); Chandigarh is such a
    // territory per ledgerly-core's table (recalled, not yet sourced: owner check pending).
    // 10,000 x 9% = 900.00 = 90,000 paise each.
    let p = payable(
        &gta_purchase(UNREG_CH, Some(CHANDIGARH)),
        &purchase_masters(),
    );
    assert_eq!(p.heads, TaxHeads::CgstUtgst);
    assert_eq!(p.totals.cgst, Paise::new(90_000));
    assert_eq!(p.totals.utgst, Paise::new(90_000));
    assert_eq!(p.totals.sgst, Paise::ZERO);
}

#[test]
fn head_is_rounded_once_over_the_voucher_not_per_line() {
    // provenance: derived — product rounding policy (card G4): sum the unrounded tax, round the
    // head once, half away from zero. Two lines of ₹33.33 at 9% CGST: each 33.33 x 9% = 2.9997;
    // sum 5.9994 rounds to 5.99 = 599 paise. (Rounding each line first would give 3.00 + 3.00.)
    // SGST is the same. Taxable total 66.66 = 6,666 paise.
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(GTA_LEDGER, 3_333), line(GTA_LEDGER, 3_333)],
    );
    let p = payable(&v, &purchase_masters());
    assert_eq!(p.taxable_total, Paise::new(6_666));
    assert_eq!(p.totals.cgst, Paise::new(599));
    assert_eq!(p.totals.sgst, Paise::new(599));
    assert_eq!(p.lines.len(), 2);
}

#[test]
fn item_invoice_lines_are_matched_by_the_items_hsn() {
    // provenance: derived — as the first test: ₹10,000.00 at 18% intra-state = 90,000 paise CGST
    // and SGST. The item's SAC 996511 matches fixture class `gta`.
    let mut v = purchase(UNREG_KA, Some(KARNATAKA), vec![]);
    v.mode = VoucherMode::ItemInvoice;
    v.inventory_lines = vec![InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(1_000_000, 2)),
        discount_percent: None,
        amount: Paise::new(1_000_000),
        ledger: Some(GTA_LEDGER),
        godowns: vec![],
    }];
    let p = payable(&v, &purchase_masters());
    assert_eq!(
        p.lines.first().unwrap().source,
        LineSource::Inventory { index: 0 }
    );
    assert_eq!(p.totals.cgst, Paise::new(90_000));
    assert_eq!(p.totals.sgst, Paise::new(90_000));
}

#[test]
fn non_notified_supply_has_no_reverse_charge() {
    // provenance: derived — CGST Act s.9(4) applies only to notified classes; SAC 998313 is in no
    // fixture class, so the supplier charges the tax and card G4 computes it.
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(OTHER_LEDGER, 1_000_000)],
    );
    assert_not_applicable(&v, &purchase_masters(), NoRcmReason::NotNotified);
}

#[test]
fn registered_supplier_of_an_any_supplier_class_is_still_reverse_charge() {
    // provenance: derived — CGST Act s.9(3) covers notified supplies whether or not the supplier
    // is registered; fixture class `gta` has supplier = "any". 10,000 x 9% = 90,000 paise each.
    let p = payable(&gta_purchase(REG_KA, Some(KARNATAKA)), &purchase_masters());
    assert_eq!(p.class_id, "gta");
    assert_eq!(p.totals.cgst, Paise::new(90_000));
}

#[test]
fn class_for_unregistered_suppliers_only_does_not_apply_to_a_registered_supplier() {
    // provenance: derived — CGST Act s.9(4) is about supplies by unregistered persons; fixture
    // class `scrap` has supplier = "unregistered". A registered supplier charges tax itself.
    let v = purchase(REG_KA, Some(KARNATAKA), vec![line(SCRAP_LEDGER, 1_000_000)]);
    assert_not_applicable(&v, &purchase_masters(), NoRcmReason::SupplierRegistered);
    let unreg = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(SCRAP_LEDGER, 1_000_000)],
    );
    let p = payable(&unreg, &purchase_masters());
    assert_eq!(p.class_id, "scrap");
    assert_eq!(p.totals.cgst, Paise::new(90_000));
}

#[test]
fn the_mark_on_the_voucher_is_reported_not_trusted() {
    // provenance: derived — liability arises from the law (s.9(3)), not from the user's mark.
    let m = purchase_masters();
    let unmarked = gta_purchase(UNREG_KA, Some(KARNATAKA));
    assert!(!payable(&unmarked, &m).marked_on_voucher);
    let marked = Voucher {
        reverse_charge: true,
        ..unmarked
    };
    assert!(payable(&marked, &m).marked_on_voucher);
}

#[test]
fn a_voucher_marked_reverse_charge_that_the_pack_says_is_not_is_unsupported() {
    // provenance: rule A1 — a conflict between the user's mark and the rule pack is neither a
    // liability nor "no liability"; it is reported as Unsupported.
    let m = purchase_masters();
    let other = Voucher {
        reverse_charge: true,
        ..purchase(
            UNREG_KA,
            Some(KARNATAKA),
            vec![line(OTHER_LEDGER, 1_000_000)],
        )
    };
    assert_unsupported(&other, &m);
    let registered_scrap = Voucher {
        reverse_charge: true,
        ..purchase(REG_KA, Some(KARNATAKA), vec![line(SCRAP_LEDGER, 1_000_000)])
    };
    assert_unsupported(&registered_scrap, &m);
}

#[test]
fn exempt_lines_carry_no_reverse_charge_tax() {
    // provenance: derived — tax is only on taxable supplies. An exempt line of SAC 996511 beside a
    // taxable one adds nothing: taxable total stays 10,000.00, CGST 90,000 paise.
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![
            line(GTA_LEDGER, 1_000_000),
            line(EXEMPT_GTA_LEDGER, 500_000),
        ],
    );
    let p = payable(&v, &purchase_masters());
    assert_eq!(p.taxable_total, Paise::new(1_000_000));
    assert_eq!(p.totals.cgst, Paise::new(90_000));
    let only_exempt = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(EXEMPT_GTA_LEDGER, 500_000)],
    );
    assert_not_applicable(
        &only_exempt,
        &purchase_masters(),
        NoRcmReason::NoTaxableLine,
    );
}

#[test]
fn the_notified_list_is_pack_data_and_effective_dated() {
    // provenance: derived — engineering rule A8 and the time-of-supply rule: the list in force on
    // the supply date decides. Fixture: class `gta` is listed and in force only from 2026-02-01.
    let pack = r#"
[[entry]]
id = "rcm.classes"
effective_from = "2025-09-22"
effective_to = "2026-01-31"
legal_ref = "TEST FIXTURE index v1"
[entry.params]
ids = "scrap"

[[entry]]
id = "rcm.classes"
effective_from = "2026-02-01"
legal_ref = "TEST FIXTURE index v2"
[entry.params]
ids = "scrap,gta"

[[entry]]
id = "rcm.class.scrap"
effective_from = "2025-09-22"
legal_ref = "TEST FIXTURE scrap"
[entry.params]
hsn_prefixes = "7204"
supplier = "unregistered"

[[entry]]
id = "rcm.class.gta"
effective_from = "2026-02-01"
legal_ref = "TEST FIXTURE gta"
[entry.params]
hsn_prefixes = "9965"
supplier = "any"
"#;
    let m = purchase_masters();
    let before = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let l = reverse_charge(&before, &m, &rules_with(pack, date!(2026 - 01 - 15)))
        .unwrap()
        .value()
        .unwrap();
    assert_eq!(l, RcmLiability::NotApplicable(NoRcmReason::NotNotified));
    let after = Voucher {
        date: date!(2026 - 02 - 15),
        supply_date: Some(date!(2026 - 02 - 15)),
        ..before
    };
    let l = reverse_charge(&after, &m, &rules_with(pack, date!(2026 - 02 - 15)))
        .unwrap()
        .value()
        .unwrap();
    match l {
        RcmLiability::Payable(p) => assert_eq!(p.class_id, "gta"),
        RcmLiability::NotApplicable(r) => panic!("expected Payable, got {r:?}"),
    }
}

#[test]
fn a_class_missing_from_the_index_is_not_notified() {
    // provenance: derived — the index is the list in force; an entry that exists but is not listed
    // does not count. Index lists only `scrap`, so SAC 9965 is not notified.
    let pack = RCM_PACK.replace(r#"ids = "gta,scrap""#, r#"ids = "scrap""#);
    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let l = reverse_charge(
        &v,
        &purchase_masters(),
        &rules_with(&pack, date!(2026 - 01 - 15)),
    )
    .unwrap()
    .value()
    .unwrap();
    assert_eq!(l, RcmLiability::NotApplicable(NoRcmReason::NotNotified));
}

#[test]
fn pack_without_a_list_or_with_a_pending_or_missing_class_is_unsupported() {
    // provenance: rules A1 and A8 — no default list, and a pending class cannot be read, so the
    // answer is Unsupported, never "not notified".
    let m = purchase_masters();
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(OTHER_LEDGER, 1_000_000)],
    );
    let d = date!(2026 - 01 - 15);

    let no_list = reverse_charge(&v, &m, &rules_with("", d)).unwrap();
    assert!(no_list.is_unsupported());

    let pending = RCM_PACK.replace("legal_ref = \"TEST FIXTURE gta\"", "pending = true");
    let r = reverse_charge(&v, &m, &rules_with(&pending, d)).unwrap();
    assert!(r.is_unsupported());

    let ghost = RCM_PACK.replace(r#"ids = "gta,scrap""#, r#"ids = "gta,scrap,ghost""#);
    let r = reverse_charge(&v, &m, &rules_with(&ghost, d)).unwrap();
    assert!(r.is_unsupported());
}

#[test]
fn a_malformed_class_entry_is_a_pack_error() {
    // provenance: rule A1 (typed errors) — a verified entry whose params cannot be read is a
    // broken pack, not an unsupported voucher.
    let m = purchase_masters();
    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let d = date!(2026 - 01 - 15);
    let bad_supplier = RCM_PACK.replacen(r#"supplier = "any""#, r#"supplier = "sometimes""#, 1);
    assert!(matches!(
        reverse_charge(&v, &m, &rules_with(&bad_supplier, d)),
        Err(GstError::Pack { .. })
    ));
    let bad_prefix = RCM_PACK.replacen(r#"hsn_prefixes = "9965""#, r#"hsn_prefixes = "99x5""#, 1);
    assert!(matches!(
        reverse_charge(&v, &m, &rules_with(&bad_prefix, d)),
        Err(GstError::Pack { .. })
    ));
    let no_prefix = RCM_PACK.replacen(r#"hsn_prefixes = "9965""#, "", 1);
    assert!(matches!(
        reverse_charge(&v, &m, &rules_with(&no_prefix, d)),
        Err(GstError::Pack { .. })
    ));
}

#[test]
fn overlapping_classes_for_one_hsn_are_unsupported() {
    // provenance: rule A1 — if two classes claim the same SAC the pack is ambiguous; no guess.
    let pack = format!(
        "{RCM_PACK}\n[[entry]]\nid = \"rcm.class.dup\"\neffective_from = \"2025-09-22\"\nlegal_ref = \"TEST FIXTURE dup\"\n[entry.params]\nhsn_prefixes = \"99651\"\nsupplier = \"any\"\n"
    )
    .replace(r#"ids = "gta,scrap""#, r#"ids = "gta,scrap,dup""#);
    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let r = reverse_charge(
        &v,
        &purchase_masters(),
        &rules_with(&pack, date!(2026 - 01 - 15)),
    )
    .unwrap();
    assert!(r.is_unsupported());
}

#[test]
fn mixed_or_unclassifiable_lines_are_unsupported() {
    // provenance: rule A1 — one voucher is one liability; mixed lines are not split by guessing.
    let m = purchase_masters();
    let mixed = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(GTA_LEDGER, 1_000_000), line(OTHER_LEDGER, 1_000_000)],
    );
    assert_unsupported(&mixed, &m);
    let two_classes = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(GTA_LEDGER, 1_000_000), line(SCRAP_LEDGER, 1_000_000)],
    );
    assert_unsupported(&two_classes, &m);
    let no_hsn = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(NO_HSN_LEDGER, 1_000_000)],
    );
    assert_unsupported(&no_hsn, &m);
}

#[test]
fn rate_without_a_verified_slab_is_unsupported() {
    // provenance: rule A1/A8 — 7% is not a slab of the 2025-09-22 schedule in the pack, so there
    // is no verified percentage to apply.
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(SEVEN_PERCENT_LEDGER, 1_000_000)],
    );
    assert_unsupported(&v, &purchase_masters());
}

#[test]
fn suppliers_whose_status_or_state_is_unknown_are_unsupported() {
    // provenance: rule A1 — composition suppliers, overseas suppliers (import of services) and a
    // supplier whose state is unknown (CGST+SGST against IGST cannot be decided) are not guessed.
    let m = purchase_masters();
    assert_unsupported(&gta_purchase(COMPOSITION, Some(KARNATAKA)), &m);
    assert_unsupported(&gta_purchase(OVERSEAS, Some(KARNATAKA)), &m);
    assert_unsupported(&gta_purchase(UNREG_NO_STATE, Some(KARNATAKA)), &m);
    let no_party = Voucher {
        party: None,
        ..gta_purchase(UNREG_KA, Some(KARNATAKA))
    };
    assert_unsupported(&no_party, &m);
}

#[test]
fn missing_place_of_supply_or_centre_jurisdiction_is_unsupported() {
    // provenance: rule A1 — the place of supply is never guessed; state code 99 is not a state.
    let m = purchase_masters();
    assert_unsupported(&gta_purchase(UNREG_KA, None), &m);
    assert_unsupported(&gta_purchase(UNREG_KA, Some(99)), &m);
}

#[test]
fn a_non_notified_supply_needs_no_place_of_supply() {
    // provenance: derived — with no reverse charge there is no tax to split, so a missing place of
    // supply does not matter to this module (card G4 asks for it when it taxes the voucher).
    let v = purchase(UNREG_NO_STATE, None, vec![line(OTHER_LEDGER, 1_000_000)]);
    assert_not_applicable(&v, &purchase_masters(), NoRcmReason::NotNotified);
}

#[test]
fn supply_date_is_required_and_must_match_the_rule_set() {
    // provenance: CGST Act s.12(3), s.13(3) — under reverse charge the time of supply is not the
    // voucher date; without an explicit supply date the answer is Unsupported, and a rule set for
    // another date is a caller error.
    let m = purchase_masters();
    let no_supply_date = Voucher {
        supply_date: None,
        ..gta_purchase(UNREG_KA, Some(KARNATAKA))
    };
    let r = reverse_charge(&no_supply_date, &m, &rules_on(date!(2026 - 01 - 15))).unwrap();
    assert!(r.is_unsupported());

    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let wrong_date = reverse_charge(&v, &m, &rules_on(date!(2026 - 01 - 16)));
    assert!(matches!(wrong_date, Err(GstError::Invalid { .. })));
}

#[test]
fn only_purchase_vouchers_in_scope() {
    // provenance: rule A1 — sales, notes and non-trading vouchers are not decided by this module.
    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    for base in [
        BaseVoucherKind::Sales,
        BaseVoucherKind::DebitNote,
        BaseVoucherKind::CreditNote,
        BaseVoucherKind::Journal,
        BaseVoucherKind::Payment,
    ] {
        assert_unsupported(&v, &masters(base));
    }
    let cancelled = Voucher {
        status: VoucherStatus::Cancelled,
        ..v.clone()
    };
    assert_unsupported(&cancelled, &purchase_masters());
    let plain = Voucher {
        mode: VoucherMode::AsVoucher,
        ..v
    };
    assert_unsupported(&plain, &purchase_masters());
}

#[test]
fn cess_ledgers_are_unsupported() {
    // provenance: rule A1/A8 — no verified cess rate exists in the packs.
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(GTA_LEDGER, 1_000_000), line(CESS_LEDGER, 100)],
    );
    assert_unsupported(&v, &purchase_masters());
}

#[test]
fn unknown_references_are_errors_not_unsupported() {
    // provenance: rule A1 — a voucher naming a ledger that does not exist is broken data.
    let m = purchase_masters();
    let v = purchase(
        UNREG_KA,
        Some(KARNATAKA),
        vec![line(LedgerId(999), 1_000_000)],
    );
    assert!(matches!(run(&v, &m), Err(GstError::NotFound { .. })));
    let no_such_party = gta_purchase(LedgerId(998), Some(KARNATAKA));
    assert!(matches!(
        run(&no_such_party, &m),
        Err(GstError::NotFound { .. })
    ));
}

#[test]
fn recipient_checks_with_the_company() {
    // provenance: derived — the recipient must be a registered regular taxpayer (research 04-gst
    // 2.5: composition recipients are excluded; s.9(3) speaks of a registered recipient).
    let m = purchase_masters();
    let v = gta_purchase(UNREG_KA, Some(KARNATAKA));
    let rules = rules_on(date!(2026 - 01 - 15));
    let ok = reverse_charge_for_company(&v, &m, &rules, &company(false, true)).unwrap();
    assert_eq!(ok, reverse_charge(&v, &m, &rules).unwrap());
    assert!(matches!(ok, Outcome::Value(RcmLiability::Payable(_))));
    let composition = reverse_charge_for_company(&v, &m, &rules, &company(true, true)).unwrap();
    assert!(composition.is_unsupported());
    let no_gst = reverse_charge_for_company(&v, &m, &rules, &company(false, false)).unwrap();
    assert!(no_gst.is_unsupported());
    let mut unregistered = company(false, true);
    unregistered.registrations.clear();
    let r = reverse_charge_for_company(&v, &m, &rules, &unregistered).unwrap();
    assert!(r.is_unsupported());
}

#[test]
fn the_same_voucher_gives_the_same_answer() {
    // provenance: rule A7 — deterministic output.
    let m = purchase_masters();
    let v = gta_purchase(UNREG_MH, Some(KARNATAKA));
    assert_eq!(run(&v, &m).unwrap(), run(&v, &m).unwrap());
}
