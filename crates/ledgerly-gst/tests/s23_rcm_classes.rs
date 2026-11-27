//! S23 tests: the notified reverse-charge classes of `packs/rcm.toml` and the recipient,
//! forward-charge and declaration rules of `rcm.rs`.
//!
//! Provenance: every expected answer is `derived` from the notification text quoted in
//! `packs/rcm.toml` (13/2017-CT(R) and 10/2017-IT(R) as amended up to 08/2026, read on CBIC's tax
//! information portal on 2026-11-27) and the SAC codes of the Annexure to 11/2017-CT(R). Rates come
//! from `packs/rates-slabs.toml` (`slab.five` 2.5% + 2.5%, `slab.eighteen` 9% + 9%, from
//! 22 September 2025). Tax per head is rounded once per voucher to the paisa (card G4 policy). The
//! hand working is next to each figure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Rate, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_gst::compute::TaxHeads;
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_gst::rcm::{NoRcmReason, RcmLiability, RcmPayable, reverse_charge_for_company};
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GstClassification, GstRegistration,
    Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod,
    PartyDetails, RegistrationType, RestartPeriod, TaxDetails, TaxLedgerKind, Taxability, Voucher,
    VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const SLABS: &str = include_str!("../packs/rates-slabs.toml");
const RULES: &str = include_str!("../packs/gstr-rules.toml");
const RCM: &str = include_str!("../packs/rcm.toml");

const KARNATAKA: u8 = 29;

const GTA: LedgerId = LedgerId(20);
const LEGAL: LedgerId = LedgerId(21);
const SPONSOR: LedgerId = LedgerId(22);
const CAB: LedgerId = LedgerId(23);
const CAB_FIVE: LedgerId = LedgerId(24);
const SHOP_RENT: LedgerId = LedgerId(25);
const HOUSE_RENT: LedgerId = LedgerId(26);
const UNREG: LedgerId = LedgerId(30);
const REG: LedgerId = LedgerId(31);
const CGST_IN: LedgerId = LedgerId(40);
const SGST_IN: LedgerId = LedgerId(41);

fn pack() -> RulePack {
    load_pack(SLABS)
        .unwrap()
        .merge(&load_pack(RULES).unwrap())
        .unwrap()
        .merge(&load_pack(RCM).unwrap())
        .unwrap()
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn company(kind: EntityKind) -> Company {
    Company {
        ineligible_itc_default: None,
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
            composition: false,
        }],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2022 - 04 - 01),
        entity_kind: kind,
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn ledger(id: LedgerId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: format!("Ledger {}", id.0),
        aliases: vec![],
        group: Some(GroupId(1)),
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

fn service(id: LedgerId, sac: &str, percent: i64) -> Ledger {
    let mut l = ledger(id);
    l.gst = Some(GstClassification {
        hsn: Some(Hsn::parse(sac).unwrap()),
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::new(percent, 0))),
    });
    l
}

fn masters() -> Masters {
    let mut m = Masters::default();
    m.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Purchase".to_owned(),
            base: BaseVoucherKind::Purchase,
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
    for l in [
        service(GTA, "996791", 5),
        service(LEGAL, "998212", 18),
        service(SPONSOR, "998397", 18),
        service(CAB, "996601", 18),
        service(CAB_FIVE, "996601", 5),
        service(SHOP_RENT, "997212", 18),
        service(HOUSE_RENT, "997211", 18),
    ] {
        m.ledgers.insert(l.id, l);
    }
    for (id, registration) in [
        (UNREG, RegistrationType::Unregistered),
        (REG, RegistrationType::Regular),
    ] {
        let mut l = ledger(id);
        l.party = Some(PartyDetails {
            registration,
            gstin: None,
            pan: None,
            state: Some(state(KARNATAKA)),
            address: Address::default(),
        });
        m.ledgers.insert(id, l);
    }
    for (id, kind) in [
        (CGST_IN, TaxLedgerKind::Cgst),
        (SGST_IN, TaxLedgerKind::Sgst),
    ] {
        let mut l = ledger(id);
        l.tax = Some(TaxDetails {
            kind,
            rate_percent: None,
        });
        m.ledgers.insert(id, l);
    }
    m
}

fn line(ledger: LedgerId, paise: i64) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: SignedAmount::dr(Paise::new(paise)),
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

fn purchase(on: Date, party: LedgerId, lines: Vec<LedgerLine>, marked: bool) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: on,
        supply_date: Some(on),
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(party),
        place_of_supply: Some(state(KARNATAKA)),
        reverse_charge: marked,
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

fn run(v: &Voucher, kind: EntityKind) -> Outcome<RcmLiability> {
    let on = v.supply_date.unwrap();
    let rules = match pack().for_time_of_supply(on) {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => return Outcome::Unsupported(u),
    };
    reverse_charge_for_company(v, &masters(), &rules, &company(kind)).unwrap()
}

fn payable(v: &Voucher, kind: EntityKind) -> RcmPayable {
    match run(v, kind) {
        Outcome::Value(RcmLiability::Payable(p)) => p,
        other => panic!("expected Payable, got {other:?}"),
    }
}

fn not_applicable(v: &Voucher, kind: EntityKind) -> NoRcmReason {
    match run(v, kind) {
        Outcome::Value(RcmLiability::NotApplicable(r)) => r,
        other => panic!("expected NotApplicable, got {other:?}"),
    }
}

fn unsupported(v: &Voucher, kind: EntityKind) -> String {
    match run(v, kind) {
        Outcome::Unsupported(u) => format!("{}: {}", u.what, u.reason),
        Outcome::Value(l) => panic!("expected Unsupported, got {l:?}"),
    }
}

fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

const JAN_2026: Date = date!(2026 - 01 - 15);
const NOV_2026: Date = date!(2026 - 11 - 15);

#[test]
fn the_bundled_rcm_pack_loads_and_merges_with_the_other_packs() {
    let merged = pack();
    assert!(merged.version().ends_with("+2026.1-rcm"));
}

#[test]
fn gta_from_an_unregistered_supplier_is_payable_by_the_registered_recipient() {
    // 13/2017-CT(R) sl.1: GTA (SAC 996791) to "any person registered under the CGST Act": the
    // recipient pays. Intra-state, 5%: 10,000 x 2.5% = 250 CGST and 250 SGST.
    let v = purchase(JAN_2026, UNREG, vec![line(GTA, 1_000_000)], false);
    let p = payable(&v, EntityKind::Company);
    assert_eq!(p.class_id, "gta");
    assert_eq!(p.heads, TaxHeads::CgstSgst);
    assert_eq!(p.totals.cgst, rupees(250));
    assert_eq!(p.totals.sgst, rupees(250));
    assert!(p.legal_ref.contains("13/2017-CT(R) sl.1"));
    assert_eq!(p.declared, None);
}

#[test]
fn a_registered_gta_that_charges_tax_on_its_invoice_has_opted_for_forward_charge() {
    // Second proviso to sl.1 (05/2022-CT(R)): no reverse charge where the registered GTA issues a
    // tax invoice charging central tax. The invoice carries 250 CGST and 250 SGST.
    let v = purchase(
        JAN_2026,
        REG,
        vec![
            line(GTA, 1_000_000),
            line(CGST_IN, 25_000),
            line(SGST_IN, 25_000),
        ],
        false,
    );
    assert_eq!(
        not_applicable(&v, EntityKind::Company),
        NoRcmReason::SupplierChargedTax
    );
}

#[test]
fn a_registered_gta_whose_invoice_charges_no_tax_is_reverse_charge() {
    let v = purchase(JAN_2026, REG, vec![line(GTA, 1_000_000)], false);
    assert_eq!(payable(&v, EntityKind::Company).totals.cgst, rupees(250));
}

#[test]
fn legal_services_need_the_users_mark_because_the_books_do_not_say_who_the_supplier_is() {
    // sl.2: only an advocate or a firm of advocates. Unmarked and untaxed: the engine cannot tell.
    let unmarked = purchase(JAN_2026, REG, vec![line(LEGAL, 5_000_000)], false);
    let why = unsupported(&unmarked, EntityKind::Company);
    assert!(why.contains("advocate"), "{why}");
    // Marked: the user declares the condition. 50,000 x 9% = 4,500 CGST and 4,500 SGST.
    let marked = purchase(JAN_2026, REG, vec![line(LEGAL, 5_000_000)], true);
    let p = payable(&marked, EntityKind::Company);
    assert_eq!(p.class_id, "legal");
    assert_eq!(p.totals.cgst, rupees(4_500));
    assert_eq!(p.totals.sgst, rupees(4_500));
    assert!(p.declared.as_deref().unwrap().contains("advocate"));
}

#[test]
fn legal_services_on_an_invoice_that_charges_tax_are_the_suppliers_forward_charge() {
    let v = purchase(
        JAN_2026,
        REG,
        vec![
            line(LEGAL, 5_000_000),
            line(CGST_IN, 450_000),
            line(SGST_IN, 450_000),
        ],
        false,
    );
    assert_eq!(
        not_applicable(&v, EntityKind::Company),
        NoRcmReason::SupplierChargedTax
    );
}

#[test]
fn sponsorship_reaches_only_a_body_corporate_or_a_partnership_firm() {
    // sl.4 column (4): "Any body corporate or partnership firm". A proprietor is neither.
    // The recipient test comes first, so an unmarked voucher is answered for a proprietor.
    let unmarked = purchase(JAN_2026, UNREG, vec![line(SPONSOR, 1_000_000)], false);
    assert_eq!(
        not_applicable(&unmarked, EntityKind::Proprietorship),
        NoRcmReason::RecipientNotCovered
    );
    // From 16.1.2025 the supplier must not be a body corporate, so the mark is needed.
    // 10,000 x 9% = 900 each head.
    let v = purchase(JAN_2026, UNREG, vec![line(SPONSOR, 1_000_000)], true);
    // An LLP is a partnership firm for this notification (Explanation (e), 22/2017-CT(R)).
    assert_eq!(payable(&v, EntityKind::Llp).totals.cgst, rupees(900));
    // A trust, society or HUF ("Other") may or may not be a body corporate: not decided.
    let why = unsupported(&v, EntityKind::Other);
    assert!(why.contains("body corporate"), "{why}");
}

#[test]
fn sponsorship_before_16_january_2025_did_not_depend_on_the_supplier() {
    // Before 07/2025-CT(R) the supplier was "Any person", so the recipient test alone decides.
    // 2024-12-10 is checked on a proprietor recipient (no rate needed for NotApplicable).
    let v = purchase(
        date!(2024 - 12 - 10),
        REG,
        vec![line(SPONSOR, 1_000_000)],
        false,
    );
    assert_eq!(
        not_applicable(&v, EntityKind::Proprietorship),
        NoRcmReason::RecipientNotCovered
    );
}

#[test]
fn renting_a_cab_with_fuel_goes_to_a_body_corporate_only() {
    let unmarked = purchase(NOV_2026, UNREG, vec![line(CAB_FIVE, 1_000_000)], false);
    // sl.15 column (4): "Any body corporate". A partnership firm is not one.
    assert_eq!(
        not_applicable(&unmarked, EntityKind::Partnership),
        NoRcmReason::RecipientNotCovered
    );
    // Whether an LLP is a body corporate under Companies Act s.2(11) is not settled here.
    let v = purchase(NOV_2026, UNREG, vec![line(CAB_FIVE, 1_000_000)], true);
    let why = unsupported(&v, EntityKind::Llp);
    assert!(why.contains("body corporate"), "{why}");
    // Marked, company recipient: 10,000 x 2.5% = 250 each head.
    assert_eq!(payable(&v, EntityKind::Company).totals.sgst, rupees(250));
}

#[test]
fn a_cab_invoice_charging_the_notified_rate_is_forward_charge_and_another_rate_is_not() {
    // From 1.11.2026 (08/2026-CT(R)) the supplier escapes sl.15 by an invoice charging central tax
    // at 9 per cent: 18% in all. 10,000 x 9% = 900 CGST and 900 SGST on the invoice.
    let at_18 = purchase(
        NOV_2026,
        REG,
        vec![
            line(CAB, 1_000_000),
            line(CGST_IN, 90_000),
            line(SGST_IN, 90_000),
        ],
        false,
    );
    assert_eq!(
        not_applicable(&at_18, EntityKind::Company),
        NoRcmReason::SupplierChargedTax
    );
    // An invoice charging 5% is not the notified rate, but the supplier's own invoice charges tax
    // and the user has not marked the voucher, so the books record a forward-charge supply.
    let at_5_unmarked = purchase(
        NOV_2026,
        REG,
        vec![
            line(CAB_FIVE, 1_000_000),
            line(CGST_IN, 25_000),
            line(SGST_IN, 25_000),
        ],
        false,
    );
    assert_eq!(
        not_applicable(&at_5_unmarked, EntityKind::Company),
        NoRcmReason::SupplierChargedTax
    );
    // Marked as well as taxed: the mark and the supplier's invoice disagree.
    let mut both = at_5_unmarked.clone();
    both.reverse_charge = true;
    let why = unsupported(&both, EntityKind::Company);
    assert!(why.contains("charges tax"), "{why}");
}

#[test]
fn renting_commercial_property_from_an_unregistered_landlord_is_reverse_charge_from_10_october_2024()
 {
    // sl.5AB (09/2024-CT(R), from 10.10.2024): 50,000 x 9% = 4,500 CGST and 4,500 SGST.
    let v = purchase(JAN_2026, UNREG, vec![line(SHOP_RENT, 5_000_000)], false);
    let p = payable(&v, EntityKind::Proprietorship);
    assert_eq!(p.class_id, "commercial_rent_unregistered");
    assert_eq!(p.totals.cgst, rupees(4_500));
    // A registered landlord charges the tax itself.
    let reg = purchase(JAN_2026, REG, vec![line(SHOP_RENT, 5_000_000)], false);
    assert_eq!(
        not_applicable(&reg, EntityKind::Company),
        NoRcmReason::SupplierRegistered
    );
    // The day before 09/2024 came into force the class did not exist.
    let before = purchase(
        date!(2024 - 10 - 09),
        UNREG,
        vec![line(SHOP_RENT, 5_000_000)],
        false,
    );
    assert_eq!(
        not_applicable(&before, EntityKind::Company),
        NoRcmReason::NotNotified
    );
}

#[test]
fn renting_a_residential_dwelling_to_a_registered_person_is_reverse_charge_from_any_landlord() {
    // sl.5AA (05/2022-CT(R)): supplier "Any", recipient "Any registered person".
    let v = purchase(JAN_2026, REG, vec![line(HOUSE_RENT, 2_000_000)], false);
    let p = payable(&v, EntityKind::Proprietorship);
    assert_eq!(p.class_id, "residential_rent");
    // 20,000 x 9% = 1,800 each head.
    assert_eq!(p.totals.cgst, rupees(1_800));
}

#[test]
fn an_unregistered_suppliers_invoice_that_carries_tax_is_unsupported() {
    let v = purchase(
        JAN_2026,
        UNREG,
        vec![
            line(SHOP_RENT, 5_000_000),
            line(CGST_IN, 450_000),
            line(SGST_IN, 450_000),
        ],
        false,
    );
    let why = unsupported(&v, EntityKind::Company);
    assert!(why.contains("unregistered"), "{why}");
}

#[test]
fn dates_before_the_index_starts_are_unsupported() {
    // The index starts on 18.7.2022, when 05/2022-CT(R) came into force.
    let v = purchase(
        date!(2022 - 07 - 17),
        UNREG,
        vec![line(GTA, 1_000_000)],
        false,
    );
    let why = unsupported(&v, EntityKind::Company);
    assert!(why.contains("rcm.classes"), "{why}");
}

fn bad_pack(from: &str, to: &str) -> String {
    let text = RCM.replacen(from, to, 1);
    assert_ne!(text, RCM, "the replacement must change the pack");
    text
}

#[test]
fn unknown_tokens_and_a_blank_declaration_are_pack_errors() {
    let on = JAN_2026;
    let v = purchase(on, UNREG, vec![line(GTA, 1_000_000)], false);
    for text in [
        bad_pack(r#"recipient = "registered""#, r#"recipient = "anyone""#),
        bad_pack(
            r#"forward_charge = "any_tax""#,
            r#"forward_charge = "sometimes""#,
        ),
        bad_pack(
            r#"forward_charge = "rate:12""#,
            r#"forward_charge = "rate:twelve""#,
        ),
        bad_pack(
            r#"requires = "the supplier is an arbitral tribunal""#,
            r#"requires = " ""#,
        ),
    ] {
        let merged = load_pack(SLABS)
            .unwrap()
            .merge(&load_pack(&text).unwrap())
            .unwrap();
        let rules = match merged.for_time_of_supply(on) {
            Outcome::Value(r) => r,
            Outcome::Unsupported(u) => panic!("{u:?}"),
        };
        let got = reverse_charge_for_company(&v, &masters(), &rules, &company(EntityKind::Company));
        assert!(got.is_err(), "expected a pack error, got {got:?}");
    }
}
