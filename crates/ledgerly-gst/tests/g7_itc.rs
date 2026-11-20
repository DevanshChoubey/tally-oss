//! Card G7 tests: ITC eligibility and s.17(5) blocked credit (spec 6.3; CGST Act s.16(2), s.17(5)).
//!
//! Provenance. The legal rules are those listed in `docs/research/04-gst.md` 2.9 ("Blocked credits
//! under s.17(5)": (a) motor vehicles with 13 seats or fewer, (b) food and beverages, among
//! others; composition dealers cannot collect tax, CGST Act s.10(4); an invoice with tax is needed,
//! s.16(2)(a)). Which HSN/SAC codes belong to a category is rule-pack data. The packs below are
//! fixtures written in this file: each category lists the 4-digit HSN heading or SAC the test
//! uses (8703 motor cars, 9963 food and beverage services, 8802 aircraft). Every expected result
//! is what the stated rule gives for those fixture codes, worked by hand next to the case; none
//! is copied from engine output. Whether the real pack's codes are right is the pack's own
//! provenance (the shipped pack has no `itc.blocked.*` entries yet, so production is `Unsupported`,
//! see the last tests).

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
    Date, Decimal, GroupId, Hsn, LedgerId, Outcome, Paise, Qty, Rate, SignedAmount, StockItemId,
    UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::itc::{BLOCKED_RULE_IDS, ItcEligibility, itc_eligibility};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_model::{Address, TaxDetails, TaxLedgerKind};
use ledgerly_model::{
    BaseVoucherKind, GstClassification, InventoryLine, Ledger, LedgerLine, Masters,
    NumberingConfig, NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem,
    Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const PARTY: LedgerId = LedgerId(11);
const COMPOSITION_PARTY: LedgerId = LedgerId(12);
const UNREGISTERED_PARTY: LedgerId = LedgerId(13);
const OVERSEAS_PARTY: LedgerId = LedgerId(14);
const CAR: LedgerId = LedgerId(20);
const FOOD: LedgerId = LedgerId(21);
const OFFICE: LedgerId = LedgerId(22);
const EXEMPT: LedgerId = LedgerId(23);
const BY_ITEM: LedgerId = LedgerId(24);
const AIRCRAFT: LedgerId = LedgerId(25);
const CGST: LedgerId = LedgerId(30);
const ITEM: StockItemId = StockItemId(1);

fn pack_text(motor_to: Option<&str>, pending_food: bool) -> String {
    let to = motor_to.map_or(String::new(), |d| format!("effective_to = \"{d}\"\n"));
    let food = if pending_food {
        "pending = true\n[entry.params]\nsource_note = \"fixture: unverified\"\n".to_owned()
    } else {
        "legal_ref = \"CGST Act s.17(5)(b)\"\n[entry.params]\nsection = \"17(5)(b)\"\nhsn_prefixes = \"9963\"\n".to_owned()
    };
    format!(
        "version = \"fixture\"\n\
         [[entry]]\nid = \"itc.blocked.motor_vehicles\"\neffective_from = \"2017-07-01\"\n{to}\
         legal_ref = \"CGST Act s.17(5)(a)\"\n[entry.params]\nsection = \"17(5)(a)\"\nhsn_prefixes = \"8703\"\n\
         [[entry]]\nid = \"itc.blocked.vessels_aircraft\"\neffective_from = \"2017-07-01\"\n\
         legal_ref = \"CGST Act s.17(5)(aa)\"\n[entry.params]\nsection = \"17(5)(aa)\"\nhsn_prefixes = \"8802, 8901\"\n\
         [[entry]]\nid = \"itc.blocked.food_beverages\"\neffective_from = \"2017-07-01\"\n{food}"
    )
}

fn rules_from(text: &str, d: Date) -> RuleSet {
    match load_pack(text).unwrap().for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn rules(d: Date) -> RuleSet {
    rules_from(&pack_text(None, false), d)
}

fn purchase_date() -> Date {
    date!(2026 - 01 - 15)
}

fn ledger(
    id: LedgerId,
    registration: Option<RegistrationType>,
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

fn class(hsn: Option<&str>, taxability: Taxability) -> GstClassification {
    GstClassification {
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        taxability,
        rate_percent: Some(Rate(Decimal::new(18, 0))),
    }
}

fn masters_with(base: BaseVoucherKind, item_hsn: Option<&str>) -> Masters {
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
            name: "Widget".to_owned(),
            aliases: vec![],
            group: None,
            unit: UnitId(1),
            alt_unit: None,
            hsn: item_hsn.map(|h| Hsn::parse(h).unwrap()),
            gst_history: vec![],
            valuation: ValuationMethod::AverageCost,
            batches_enabled: false,
            opening: vec![],
            standard_costs: vec![],
            tally: None,
        },
    );
    for (id, reg) in [
        (PARTY, RegistrationType::Regular),
        (COMPOSITION_PARTY, RegistrationType::Composition),
        (UNREGISTERED_PARTY, RegistrationType::Unregistered),
        (OVERSEAS_PARTY, RegistrationType::Overseas),
    ] {
        m.ledgers.insert(id, ledger(id, Some(reg), None, None));
    }
    for (id, hsn, tax) in [
        (CAR, Some("870321"), Taxability::Taxable),
        (FOOD, Some("996331"), Taxability::Taxable),
        (OFFICE, Some("482010"), Taxability::Taxable),
        (EXEMPT, Some("482010"), Taxability::Exempt),
        (BY_ITEM, None, Taxability::Taxable),
        (AIRCRAFT, Some("880240"), Taxability::Taxable),
    ] {
        m.ledgers
            .insert(id, ledger(id, None, None, Some(class(hsn, tax))));
    }
    m.ledgers.insert(
        CGST,
        ledger(
            CGST,
            None,
            Some(TaxDetails {
                kind: TaxLedgerKind::Cgst,
                rate_percent: None,
            }),
            None,
        ),
    );
    m
}

fn masters() -> Masters {
    masters_with(BaseVoucherKind::Purchase, Some("482010"))
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

fn purchase(party: LedgerId, expense: LedgerId) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: purchase_date(),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(party),
        place_of_supply: None,
        reverse_charge: false,
        reference: Some("S-1".to_owned()),
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![
            line(expense, 1_000_000),
            line(CGST, 90_000),
            line(party, 1_180_000),
        ],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn eligibility(v: &Voucher, line: usize, m: &Masters, r: &RuleSet) -> ItcEligibility {
    match itc_eligibility(v, line, m, r).unwrap() {
        Outcome::Value(e) => e,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn assert_unsupported(v: &Voucher, line: usize, m: &Masters, r: &RuleSet) {
    match itc_eligibility(v, line, m, r).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(e) => panic!("expected Unsupported, got {e:?}"),
    }
}

#[test]
fn motor_car_is_blocked_under_17_5_a() {
    // provenance: derived — CGST Act s.17(5)(a) (research 04-gst 2.9: motor vehicles with 13 seats
    // or fewer). Fixture pack: category `itc.blocked.motor_vehicles` lists HSN prefix 8703; the
    // ledger's code 870321 starts with 8703, so the line is in the category and the section shown
    // is the pack's "17(5)(a)".
    let e = eligibility(
        &purchase(PARTY, CAR),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert_eq!(
        e,
        ItcEligibility::Blocked {
            section: "17(5)(a)".to_owned()
        }
    );
    assert!(!e.is_claimable());
}

#[test]
fn food_and_beverage_service_is_blocked_under_17_5_b() {
    // provenance: derived — CGST Act s.17(5)(b) (research 04-gst 2.9: food and beverages, outdoor
    // catering). Fixture pack lists SAC prefix 9963; the ledger's code 996331 starts with 9963.
    let e = eligibility(
        &purchase(PARTY, FOOD),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert_eq!(
        e,
        ItcEligibility::Blocked {
            section: "17(5)(b)".to_owned()
        }
    );
}

#[test]
fn aircraft_prefix_list_is_split_on_commas_and_trimmed() {
    // provenance: derived — CGST Act s.17(5)(aa) (vessels and aircraft). The fixture lists
    // "8802, 8901" (with a space); 880240 starts with 8802, so it is blocked under "17(5)(aa)".
    let e = eligibility(
        &purchase(PARTY, AIRCRAFT),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert_eq!(
        e,
        ItcEligibility::Blocked {
            section: "17(5)(aa)".to_owned()
        }
    );
}

#[test]
fn office_supplies_are_eligible() {
    // provenance: derived — CGST Act s.16(1): a registered person may take credit of tax on
    // inputs used in the course of business; stationery (HSN 4820) is in none of the fixture
    // pack's blocked categories (8703, 8802, 8901, 9963), the supplier is a regular taxpayer and
    // the ledger is taxable, so nothing blocks or disqualifies it.
    let e = eligibility(
        &purchase(PARTY, OFFICE),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert_eq!(e, ItcEligibility::Eligible);
    assert!(e.is_claimable());
}

#[test]
fn a_prefix_only_matches_its_own_heading() {
    // provenance: derived — a category lists heading 8703; a goods code in heading 8704
    // (8704 is a different heading) does not start with "8703", so it is not in the category.
    let mut m = masters();
    m.ledgers.insert(
        CAR,
        ledger(
            CAR,
            None,
            None,
            Some(class(Some("870421"), Taxability::Taxable)),
        ),
    );
    let e = eligibility(&purchase(PARTY, CAR), 0, &m, &rules(purchase_date()));
    assert_eq!(e, ItcEligibility::Eligible);
}

#[test]
fn blocked_and_ineligible_credit_is_never_claimable() {
    // provenance: derived — the card's rule "ineligible ITC never appears in 3B 4(A)": only
    // `Eligible` is claimable, so a 3B builder that filters on `is_claimable` cannot include the
    // other two.
    assert!(ItcEligibility::Eligible.is_claimable());
    assert!(
        !ItcEligibility::Blocked {
            section: "17(5)(a)".to_owned()
        }
        .is_claimable()
    );
    assert!(
        !ItcEligibility::Ineligible {
            reason: "x".to_owned()
        }
        .is_claimable()
    );
}

#[test]
fn the_rule_set_for_the_time_of_supply_decides() {
    // provenance: derived — engineering rule A8 (rules are effective-dated and picked by time of
    // supply). Fixture: the motor vehicle category ends 2025-12-31. A purchase with time of supply
    // 2025-12-31 is blocked; one on 2026-01-15, after the entry ended, has no entry for that
    // category in force, so the rule set has no such rule and the answer is Unsupported (never
    // "eligible by default").
    let text = pack_text(Some("2025-12-31"), false);
    let before = eligibility(
        &purchase(PARTY, CAR),
        0,
        &masters(),
        &rules_from(&text, date!(2025 - 12 - 31)),
    );
    assert_eq!(
        before,
        ItcEligibility::Blocked {
            section: "17(5)(a)".to_owned()
        }
    );
    assert_unsupported(
        &purchase(PARTY, CAR),
        0,
        &masters(),
        &rules_from(&text, date!(2026 - 01 - 15)),
    );
}

#[test]
fn a_pending_category_makes_the_answer_unsupported() {
    // provenance: derived — engineering rule A8: a pending entry evaluates to Unsupported. Fixture:
    // the food category is pending. Office supplies cannot be called eligible while a blocked
    // category is unverified, and the pending category itself is Unsupported too.
    let r = rules_from(&pack_text(None, true), purchase_date());
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &masters(), &r);
    assert_unsupported(&purchase(PARTY, FOOD), 0, &masters(), &r);
}

#[test]
fn the_shipped_slab_pack_has_no_itc_categories_so_everything_is_unsupported() {
    // provenance: derived — the shipped `packs/rates-slabs.toml` has no `itc.blocked.*` entries
    // (this card does not own pack files), so no blocked category can be verified: Unsupported.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let r = rules_from(&fs::read_to_string(path).unwrap(), purchase_date());
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &masters(), &r);
    assert_eq!(BLOCKED_RULE_IDS.len(), 3);
}

#[test]
fn exempt_purchase_has_no_credit() {
    // provenance: derived — an exempt supply carries no tax (CGST Act s.11, s.2(47)), so there is
    // nothing to claim: Ineligible, not Eligible.
    let e = eligibility(
        &purchase(PARTY, EXEMPT),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert!(matches!(e, ItcEligibility::Ineligible { .. }));
}

#[test]
fn composition_supplier_gives_no_credit() {
    // provenance: derived — CGST Act s.10(4): a composition dealer shall not collect tax from the
    // recipient, so the recipient has no tax invoice with tax (s.16(2)(a)).
    let e = eligibility(
        &purchase(COMPOSITION_PARTY, OFFICE),
        0,
        &masters(),
        &rules(purchase_date()),
    );
    assert!(matches!(e, ItcEligibility::Ineligible { .. }));
}

#[test]
fn unregistered_supplier_gives_no_credit_unless_reverse_charge() {
    // provenance: derived — without reverse charge an unregistered supplier cannot issue a tax
    // invoice with tax (CGST Act s.16(2)(a)): Ineligible. Under reverse charge the recipient pays
    // the tax itself (s.9(4)) and takes credit (s.16(1)), so a non-blocked line is Eligible.
    let m = masters();
    let r = rules(purchase_date());
    let plain = purchase(UNREGISTERED_PARTY, OFFICE);
    assert!(matches!(
        eligibility(&plain, 0, &m, &r),
        ItcEligibility::Ineligible { .. }
    ));
    let rcm = Voucher {
        reverse_charge: true,
        ..plain
    };
    assert_eq!(eligibility(&rcm, 0, &m, &r), ItcEligibility::Eligible);
}

#[test]
fn blocked_credit_stays_blocked_under_reverse_charge() {
    // provenance: derived — s.17(5) applies to "input tax" however it was paid; a motor car
    // bought under reverse charge is still in clause (a).
    let v = Voucher {
        reverse_charge: true,
        ..purchase(UNREGISTERED_PARTY, CAR)
    };
    let e = eligibility(&v, 0, &masters(), &rules(purchase_date()));
    assert!(matches!(e, ItcEligibility::Blocked { .. }));
}

#[test]
fn overseas_supplier_is_unsupported() {
    // provenance: pending — import of goods and services (bill of entry, s.5 IGST Act) is not
    // handled by this card, so the answer is Unsupported rather than a guess.
    assert_unsupported(
        &purchase(OVERSEAS_PARTY, OFFICE),
        0,
        &masters(),
        &rules(purchase_date()),
    );
}

#[test]
fn hsn_comes_from_the_stock_items_when_the_ledger_has_none() {
    // provenance: derived — the ledger has no code; the inventory line posted to it is for an item
    // with HSN 870321 (motor car, heading 8703) in one case and 482010 (stationery) in the other.
    let r = rules(purchase_date());
    let inv = |ledger| InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(10_000, 0)),
        discount_percent: None,
        amount: Paise::new(1_000_000),
        ledger: Some(ledger),
        godowns: vec![],
    };
    let v = Voucher {
        inventory_lines: vec![inv(BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    let car = masters_with(BaseVoucherKind::Purchase, Some("870321"));
    assert!(matches!(
        eligibility(&v, 0, &car, &r),
        ItcEligibility::Blocked { .. }
    ));
    let office = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    assert_eq!(eligibility(&v, 0, &office, &r), ItcEligibility::Eligible);
    // No code anywhere: Unsupported.
    let none = masters_with(BaseVoucherKind::Purchase, None);
    assert_unsupported(&v, 0, &none, &r);
    // No inventory line at all and no ledger code: Unsupported.
    assert_unsupported(&purchase(PARTY, BY_ITEM), 0, &office, &r);
}

#[test]
fn items_in_different_categories_on_one_line_are_unsupported() {
    // provenance: derived — one ledger line cannot be both blocked and eligible; the answer would
    // be wrong for part of the amount, so the line must be split (Unsupported).
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("870321"));
    let second = StockItemId(2);
    let mut other = m.stock_items.get(&ITEM).unwrap().clone();
    other.id = second;
    other.hsn = Some(Hsn::parse("482010").unwrap());
    m.stock_items.insert(second, other);
    let inv = |item| InventoryLine {
        item,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(100, 0)),
        discount_percent: None,
        amount: Paise::new(10_000),
        ledger: Some(BY_ITEM),
        godowns: vec![],
    };
    let v = Voucher {
        inventory_lines: vec![inv(ITEM), inv(second)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert_unsupported(&v, 0, &m, &rules(purchase_date()));
}

#[test]
fn only_regular_purchase_vouchers_are_classified() {
    // provenance: derived — this card classifies purchase credit; a sales voucher or a cancelled
    // voucher has no input credit to classify, so the answer is Unsupported.
    let r = rules(purchase_date());
    let sales = masters_with(BaseVoucherKind::Sales, Some("482010"));
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &sales, &r);
    let debit = masters_with(BaseVoucherKind::DebitNote, Some("482010"));
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &debit, &r);
    let cancelled = Voucher {
        status: VoucherStatus::Cancelled,
        ..purchase(PARTY, OFFICE)
    };
    assert_unsupported(&cancelled, 0, &masters(), &r);
}

#[test]
fn a_party_or_tax_line_is_not_a_purchase_line() {
    // provenance: derived — the tax ledger line (index 1) and the party line (index 2) have no GST
    // classification, so there is no category to decide: Unsupported.
    let v = purchase(PARTY, OFFICE);
    let r = rules(purchase_date());
    assert_unsupported(&v, 1, &masters(), &r);
    assert_unsupported(&v, 2, &masters(), &r);
}

#[test]
fn missing_things_are_errors_not_guesses() {
    // provenance: derived — rule A1: a line index that does not exist, or a ledger or voucher type
    // missing from the masters, is bad input and a typed error.
    let v = purchase(PARTY, OFFICE);
    let r = rules(purchase_date());
    assert!(matches!(
        itc_eligibility(&v, 3, &masters(), &r),
        Err(GstError::Invalid { .. })
    ));
    let mut no_ledger = masters();
    no_ledger.ledgers.remove(&OFFICE);
    assert!(matches!(
        itc_eligibility(&v, 0, &no_ledger, &r),
        Err(GstError::NotFound { .. })
    ));
    assert!(matches!(
        itc_eligibility(&v, 0, &Masters::default(), &r),
        Err(GstError::NotFound { .. })
    ));
}

#[test]
fn a_malformed_pack_entry_is_a_pack_error() {
    // provenance: derived — rule A1: a category with a non-digit prefix, or without `section`,
    // is a broken pack (typed error), not "not blocked".
    let bad = "version = \"fixture\"\n\
        [[entry]]\nid = \"itc.blocked.motor_vehicles\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(a)\"\n[entry.params]\nsection = \"17(5)(a)\"\nhsn_prefixes = \"87x3\"\n\
        [[entry]]\nid = \"itc.blocked.vessels_aircraft\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(aa)\"\n[entry.params]\nsection = \"17(5)(aa)\"\nhsn_prefixes = \"8802\"\n\
        [[entry]]\nid = \"itc.blocked.food_beverages\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(b)\"\n[entry.params]\nhsn_prefixes = \"9963\"\n";
    let r = rules_from(bad, purchase_date());
    assert!(matches!(
        itc_eligibility(&purchase(PARTY, OFFICE), 0, &masters(), &r),
        Err(GstError::Pack { .. })
    ));
}
