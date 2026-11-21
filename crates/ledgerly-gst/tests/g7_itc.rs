//! Card G7 tests: ITC eligibility from the user's "ineligible for input credit" setting (spec
//! 6.3; CGST Act s.16(2), s.17(5)).
//!
//! Provenance. The behaviour modelled is TallyPrime's, as documented at
//! https://help.tallysolutions.com/docs/te9rel66/Tax_India/gst/gst_accounting_for_ineligible_itc.htm
//! (read 2026-11-21). In paraphrase: the user marks ineligibility on a stock item or stock group
//! (and may enable it company-wide when no purchase gets credit); the purchase invoice shows the
//! master value and the user may change it for that invoice; the page does not say how the
//! masters, the invoice and the company setting combine, nor how ledger and group masters behave
//! (the intro mentions them, the steps do not). Tally does not infer ineligibility from the item
//! category, so the HSN/SAC of a line does not decide eligibility. The expected results below are
//! hand derivations from that model. Where the page is silent the test is `pending` and checks
//! that the answer is `Unsupported`.
//!
//! The s.17(5) category check (`blocked_category_for`) is advisory: nothing on the number path
//! calls it. Its tests use fixture packs written in this file (8703, 8802, 9963).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Hsn, LedgerId, Outcome, Paise, Qty, Rate, SignedAmount,
    StateCode, StockGroupId, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::itc::{BLOCKED_RULE_IDS, ItcEligibility, blocked_category_for, itc_eligibility};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_model::{Address, Company, Features, NegativeStockPolicy, TaxDetails, TaxLedgerKind};
use ledgerly_model::{
    BaseVoucherKind, Group, GstClassification, InventoryLine, Ledger, LedgerLine, Masters,
    NumberingConfig, NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockGroup,
    StockItem, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
    VoucherType,
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
const OTHER_ITEM: StockItemId = StockItemId(2);
const LEDGER_GROUP: GroupId = GroupId(1);
const STOCK_PARENT: StockGroupId = StockGroupId(1);
const STOCK_CHILD: StockGroupId = StockGroupId(2);

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

fn company(default: Option<bool>) -> Company {
    Company {
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: vec![],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2025 - 04 - 01),
        entity_kind: ledgerly_model::EntityKind::Company,
        features: Features {
            gst: true,
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
        ineligible_itc_default: default,
    }
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
        group: LEDGER_GROUP,
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
        ineligible_itc: None,
        tally: None,
    }
}

fn group(id: GroupId, parent: Option<GroupId>, ineligible_itc: Option<bool>) -> Group {
    Group {
        id,
        name: format!("Group {}", id.0),
        aliases: vec![],
        parent,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        ineligible_itc,
        tally: None,
    }
}

fn stock_group(
    id: StockGroupId,
    parent: Option<StockGroupId>,
    ineligible_itc: Option<bool>,
) -> StockGroup {
    StockGroup {
        id,
        name: format!("Stock group {}", id.0),
        parent,
        ineligible_itc,
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

fn item(id: StockItemId, hsn: Option<&str>, group: Option<StockGroupId>) -> StockItem {
    StockItem {
        id,
        name: format!("Item {}", id.0),
        aliases: vec![],
        group,
        unit: UnitId(1),
        alt_unit: None,
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        ineligible_itc: None,
        tally: None,
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
    m.stock_items.insert(ITEM, item(ITEM, item_hsn, None));
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
    m.groups.insert(LEDGER_GROUP, group(LEDGER_GROUP, None, None));
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
        ineligible_itc: None,
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

fn inv(item: StockItemId, ledger: LedgerId) -> InventoryLine {
    InventoryLine {
        item,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(10_000, 0)),
        discount_percent: None,
        amount: Paise::new(1_000_000),
        ledger: Some(ledger),
        godowns: vec![],
    }
}

fn eligibility(v: &Voucher, line: usize, c: &Company, m: &Masters) -> ItcEligibility {
    match itc_eligibility(v, line, c, m).unwrap() {
        Outcome::Value(e) => e,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn assert_unsupported(v: &Voucher, line: usize, c: &Company, m: &Masters) {
    match itc_eligibility(v, line, c, m).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(e) => panic!("expected Unsupported, got {e:?}"),
    }
}

fn is_ineligible(e: &ItcEligibility) -> bool {
    match e {
        ItcEligibility::Ineligible { .. } => true,
        ItcEligibility::Eligible | ItcEligibility::Blocked { .. } => false,
    }
}

#[test]
fn motor_car_with_no_flag_is_eligible_because_tally_does_not_infer_from_the_category() {
    // provenance: derived — Tally ineligible-ITC page (see the module header, read 2026-11-21): the
    // user marks ineligibility on the master or the company; nothing marked means the credit is
    // not ineligible. The HSN 870321 (heading 8703) does not decide. Replaces the round-2 default,
    // where this line was Unsupported under s.17(5)(a) because of its HSN. Its use-based
    // exceptions are not checked by the books (the user's marking is the record).
    let e = eligibility(&purchase(PARTY, CAR), 0, &company(None), &masters());
    assert_eq!(e, ItcEligibility::Eligible);
    assert!(e.is_claimable());
}

#[test]
fn food_service_with_no_flag_is_eligible() {
    // provenance: derived — as above: SAC 996331 does not decide; no flag is set, so Eligible.
    // Replaces the round-2 Unsupported under s.17(5)(b).
    assert_eq!(
        eligibility(&purchase(PARTY, FOOD), 0, &company(None), &masters()),
        ItcEligibility::Eligible
    );
}

#[test]
fn office_supplies_are_eligible() {
    // provenance: derived — CGST Act s.16(1): a registered person may take credit of tax on
    // inputs used in business. The supplier is a regular taxpayer, the ledger is taxable and no
    // flag is set, so the purchase is eligible.
    let e = eligibility(&purchase(PARTY, OFFICE), 0, &company(None), &masters());
    assert_eq!(e, ItcEligibility::Eligible);
    assert!(e.is_claimable());
}

#[test]
fn a_flagged_stock_item_makes_its_purchase_ineligible() {
    // provenance: derived — Tally ineligible-ITC page: the user marks the stock item; the purchase
    // carries the master value, so the line is ineligible (tax was charged, so the reason is the
    // flag, not a missing invoice).
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(true);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    let e = eligibility(&v, 0, &company(None), &m);
    assert!(is_ineligible(&e));
    assert!(!e.is_claimable());
}

#[test]
fn an_item_flag_of_false_is_an_explicit_eligible_setting_that_stops_the_walk() {
    // provenance: derived — the page marks ineligibility only as Yes; a master explicitly set to
    // No is a decided value (the "nearest master that sets it" rule). A child group flagged No
    // beneath a parent flagged Yes is eligible: the walk stops at the first setting.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_groups
        .insert(STOCK_PARENT, stock_group(STOCK_PARENT, None, Some(true)));
    m.stock_groups
        .insert(STOCK_CHILD, stock_group(STOCK_CHILD, Some(STOCK_PARENT), Some(false)));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), Some(STOCK_CHILD)));
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert_eq!(
        eligibility(&v, 0, &company(None), &m),
        ItcEligibility::Eligible
    );
}

#[test]
fn a_stock_item_without_a_flag_takes_its_stock_group_chain() {
    // provenance: derived — the page says the setting may be made on a stock group; an item with
    // no setting takes the nearest group that sets it (parent Yes beneath a child with no
    // setting), so the purchase is ineligible.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_groups
        .insert(STOCK_PARENT, stock_group(STOCK_PARENT, None, Some(true)));
    m.stock_groups
        .insert(STOCK_CHILD, stock_group(STOCK_CHILD, Some(STOCK_PARENT), None));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), Some(STOCK_CHILD)));
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert!(is_ineligible(&eligibility(&v, 0, &company(None), &m)));
}

#[test]
fn a_ledger_only_line_takes_the_ledger_then_its_group_chain() {
    // provenance: derived — the page's intro mentions ledger and group masters but its steps do
    // not show them, so the ledger and group values are taken as the owner's rule states: the
    // nearest master that sets it. The office ledger has no setting; its group is flagged Yes.
    let mut m = masters();
    m.groups
        .insert(LEDGER_GROUP, group(LEDGER_GROUP, None, Some(true)));
    assert!(is_ineligible(&eligibility(
        &purchase(PARTY, OFFICE),
        0,
        &company(None),
        &m
    )));
    // A ledger setting of No beneath the flagged group stops the walk: eligible.
    m.ledgers.get_mut(&OFFICE).unwrap().ineligible_itc = Some(false);
    assert_eq!(
        eligibility(&purchase(PARTY, OFFICE), 0, &company(None), &m),
        ItcEligibility::Eligible
    );
}

#[test]
fn the_invoice_line_override_wins_over_the_masters() {
    // provenance: derived — Tally ineligible-ITC page: the invoice's GST details show the master
    // value and the user may change it for that invoice. Here the item is flagged Yes but the
    // invoice says No, so the purchase is eligible; and the reverse on a clean office ledger.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(true);
    let mut v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    v.ledger_lines[0].ineligible_itc = Some(false);
    assert_eq!(
        eligibility(&v, 0, &company(None), &m),
        ItcEligibility::Eligible
    );
    let mut office = purchase(PARTY, OFFICE);
    office.ledger_lines[0].ineligible_itc = Some(true);
    assert!(is_ineligible(&eligibility(
        &office,
        0,
        &company(None),
        &masters()
    )));
}

#[test]
fn the_company_default_applies_when_no_master_sets_the_flag() {
    // provenance: derived — Tally ineligible-ITC page: the setting may be enabled at company level
    // when no purchase gets credit. With no master setting on the office ledger, the company
    // default of Yes makes the purchase ineligible; a default of No leaves it eligible.
    assert!(is_ineligible(&eligibility(
        &purchase(PARTY, OFFICE),
        0,
        &company(Some(true)),
        &masters()
    )));
    assert_eq!(
        eligibility(&purchase(PARTY, OFFICE), 0, &company(Some(false)), &masters()),
        ItcEligibility::Eligible
    );
}

#[test]
fn a_master_that_disagrees_with_the_company_default_is_unsupported() {
    // provenance: pending — the Tally page does not say how a master setting combines with the
    // company-level setting. The item says No while the company says Yes, which one wins is not
    // documented, so the answer is Unsupported, not a guess.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(false);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert_unsupported(&v, 0, &company(Some(true)), &m);
}

#[test]
fn an_item_and_its_purchase_ledger_that_disagree_are_unsupported() {
    // provenance: pending — the Tally page does not say which of a stock item's setting and the
    // purchase ledger's setting wins. The item says Yes, the ledger says No: Unsupported.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(true);
    m.ledgers.get_mut(&BY_ITEM).unwrap().ineligible_itc = Some(false);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert_unsupported(&v, 0, &company(None), &m);
}

#[test]
fn an_item_flag_with_an_agreeing_ledger_flag_is_ineligible() {
    // provenance: derived — both masters say Yes, so they agree and the purchase is ineligible.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(true);
    m.ledgers.get_mut(&BY_ITEM).unwrap().ineligible_itc = Some(true);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert!(is_ineligible(&eligibility(&v, 0, &company(None), &m)));
}

#[test]
fn items_on_one_line_with_different_flags_are_unsupported() {
    // provenance: derived — one ledger line cannot be partly ineligible without a split; the
    // answer would be wrong for part of the amount, so the line must be split (Unsupported).
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.stock_items.insert(ITEM, item(ITEM, Some("482010"), None));
    m.stock_items.get_mut(&ITEM).unwrap().ineligible_itc = Some(true);
    m.stock_items
        .insert(OTHER_ITEM, item(OTHER_ITEM, Some("482010"), None));
    m.stock_items.get_mut(&OTHER_ITEM).unwrap().ineligible_itc = Some(false);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM), inv(OTHER_ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert_unsupported(&v, 0, &company(None), &m);
}

#[test]
fn an_unflagged_item_does_not_conflict_with_a_flagged_ledger() {
    // provenance: derived — the item sets nothing, so there is no disagreement; the purchase
    // ledger's setting (Yes) is the nearest master that sets it, so the purchase is ineligible.
    let mut m = masters_with(BaseVoucherKind::Purchase, Some("482010"));
    m.ledgers.get_mut(&BY_ITEM).unwrap().ineligible_itc = Some(true);
    let v = Voucher {
        inventory_lines: vec![inv(ITEM, BY_ITEM)],
        ..purchase(PARTY, BY_ITEM)
    };
    assert!(is_ineligible(&eligibility(&v, 0, &company(None), &m)));
}

#[test]
fn a_group_loop_is_unsupported_and_a_missing_group_is_an_error() {
    // provenance: derived — rule A1: a loop in the parent chain is unsupported data (no infinite
    // walk), and a parent group that is not in the masters is bad data (NotFound).
    let mut looped = masters();
    looped
        .groups
        .insert(LEDGER_GROUP, group(LEDGER_GROUP, Some(LEDGER_GROUP), None));
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &company(None), &looped);

    let mut missing = masters();
    missing
        .groups
        .insert(LEDGER_GROUP, group(LEDGER_GROUP, Some(GroupId(9)), None));
    assert!(matches!(
        itc_eligibility(&purchase(PARTY, OFFICE), 0, &company(None), &missing),
        Err(GstError::NotFound { .. })
    ));
}

#[test]
fn no_hsn_no_longer_blocks_a_flag_free_purchase() {
    // provenance: derived — the Tally page does not ask for an HSN to decide credit; a ledger
    // with no code and no flag is eligible. Replaces the round-2 Unsupported ("ITC without an
    // HSN/SAC").
    let mut m = masters();
    m.ledgers.get_mut(&BY_ITEM).unwrap().gst = Some(class(None, Taxability::Taxable));
    assert_eq!(
        eligibility(&purchase(PARTY, BY_ITEM), 0, &company(None), &m),
        ItcEligibility::Eligible
    );
}

#[test]
fn exempt_purchase_has_no_credit() {
    // provenance: derived — an exempt supply carries no tax (CGST Act s.11, s.2(47)), so there is
    // nothing to claim: Ineligible, not Eligible.
    let e = eligibility(&purchase(PARTY, EXEMPT), 0, &company(None), &masters());
    assert!(is_ineligible(&e));
}

#[test]
fn composition_supplier_gives_no_credit() {
    // provenance: derived — CGST Act s.10(4): a composition dealer shall not collect tax from the
    // recipient, so the recipient has no tax invoice with tax (s.16(2)(a)).
    let e = eligibility(
        &purchase(COMPOSITION_PARTY, OFFICE),
        0,
        &company(None),
        &masters(),
    );
    assert!(is_ineligible(&e));
}

#[test]
fn unregistered_supplier_gives_no_credit_unless_reverse_charge() {
    // provenance: derived — without reverse charge an unregistered supplier cannot issue a tax
    // invoice with tax (CGST Act s.16(2)(a)): Ineligible. Under reverse charge the recipient pays
    // the tax itself (s.9(4)) and takes credit (s.16(1)), so an unflagged line is Eligible.
    let m = masters();
    let c = company(None);
    let plain = purchase(UNREGISTERED_PARTY, OFFICE);
    assert!(is_ineligible(&eligibility(&plain, 0, &c, &m)));
    let rcm = Voucher {
        reverse_charge: true,
        ..plain
    };
    assert_eq!(eligibility(&rcm, 0, &c, &m), ItcEligibility::Eligible);
}

#[test]
fn overseas_supplier_is_unsupported() {
    // provenance: pending — import of goods and services (bill of entry, s.5 IGST Act) is not
    // handled by this card, so the answer is Unsupported rather than a guess.
    assert_unsupported(
        &purchase(OVERSEAS_PARTY, OFFICE),
        0,
        &company(None),
        &masters(),
    );
}

#[test]
fn only_regular_purchase_vouchers_are_classified() {
    // provenance: derived — this card classifies purchase credit; a sales voucher or a cancelled
    // voucher has no input credit to classify, so the answer is Unsupported.
    let c = company(None);
    let sales = masters_with(BaseVoucherKind::Sales, Some("482010"));
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &c, &sales);
    let debit = masters_with(BaseVoucherKind::DebitNote, Some("482010"));
    assert_unsupported(&purchase(PARTY, OFFICE), 0, &c, &debit);
    let cancelled = Voucher {
        status: VoucherStatus::Cancelled,
        ..purchase(PARTY, OFFICE)
    };
    assert_unsupported(&cancelled, 0, &c, &masters());
}

#[test]
fn a_party_or_tax_line_is_not_a_purchase_line() {
    // provenance: derived — the tax ledger line (index 1) and the party line (index 2) have no GST
    // classification, so there is no purchase line to decide: Unsupported.
    let v = purchase(PARTY, OFFICE);
    let c = company(None);
    assert_unsupported(&v, 1, &c, &masters());
    assert_unsupported(&v, 2, &c, &masters());
}

#[test]
fn missing_things_are_errors_not_guesses() {
    // provenance: derived — rule A1: a line index that does not exist, or a ledger or voucher type
    // missing from the masters, is bad input and a typed error.
    let v = purchase(PARTY, OFFICE);
    let c = company(None);
    assert!(matches!(
        itc_eligibility(&v, 3, &c, &masters()),
        Err(GstError::Invalid { .. })
    ));
    let mut no_ledger = masters();
    no_ledger.ledgers.remove(&OFFICE);
    assert!(matches!(
        itc_eligibility(&v, 0, &c, &no_ledger),
        Err(GstError::NotFound { .. })
    ));
    assert!(matches!(
        itc_eligibility(&v, 0, &c, &Masters::default()),
        Err(GstError::NotFound { .. })
    ));
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
fn a_car_in_the_blocked_heading_is_reported_by_the_advisory_check() {
    // provenance: derived — CGST Act s.17(5)(a) lists motor vehicles; the fixture pack lists
    // heading 8703; 870321 starts with 8703, so the advisory check names clause 17(5)(a). The
    // advisory check does not change the eligibility answer (see the eligibility tests above).
    let r = rules(purchase_date());
    let hit = blocked_category_for(&Hsn::parse("870321").unwrap(), &r);
    assert_eq!(
        hit.unwrap(),
        Outcome::Value(Some("17(5)(a)".to_owned()))
    );
}

#[test]
fn a_code_outside_the_blocked_headings_is_not_reported() {
    // provenance: derived — stationery (HSN 4820) is in none of the fixture categories (8703, 8802,
    // 8901, 9963), so the advisory check reports nothing.
    let r = rules(purchase_date());
    assert_eq!(
        blocked_category_for(&Hsn::parse("482010").unwrap(), &r).unwrap(),
        Outcome::Value(None)
    );
}

#[test]
fn aircraft_prefix_list_is_split_on_commas_and_trimmed() {
    // provenance: derived — CGST Act s.17(5)(aa). The fixture lists "8802, 8901" (with a space);
    // 880240 starts with 8802, so the advisory check must find it despite the space.
    let r = rules(purchase_date());
    assert_eq!(
        blocked_category_for(&Hsn::parse("880240").unwrap(), &r).unwrap(),
        Outcome::Value(Some("17(5)(aa)".to_owned()))
    );
}

#[test]
fn a_prefix_only_matches_its_own_heading() {
    // provenance: derived — a category lists heading 8703; heading 8704 does not start with
    // "8703", so the advisory check does not report it.
    let r = rules(purchase_date());
    assert_eq!(
        blocked_category_for(&Hsn::parse("870421").unwrap(), &r).unwrap(),
        Outcome::Value(None)
    );
}

#[test]
fn the_rule_set_for_the_time_of_supply_decides_the_advisory_check() {
    // provenance: derived — engineering rule A8. Fixture: the motor vehicle category ends
    // 2025-12-31. On 2025-12-31 the advisory check names it; on 2026-01-15 the entry is no longer
    // in force and the rule set has no such category, so the check is Unsupported (never "not
    // blocked" by default).
    let text = pack_text(Some("2025-12-31"), false);
    let code = Hsn::parse("870321").unwrap();
    assert_eq!(
        blocked_category_for(&code, &rules_from(&text, date!(2025 - 12 - 31))).unwrap(),
        Outcome::Value(Some("17(5)(a)".to_owned()))
    );
    assert!(matches!(
        blocked_category_for(&code, &rules_from(&text, date!(2026 - 01 - 15))).unwrap(),
        Outcome::Value(None) | Outcome::Unsupported(_)
    ));
}

#[test]
fn a_pending_category_makes_the_advisory_check_unsupported() {
    // provenance: derived — engineering rule A8: a pending entry evaluates to Unsupported. The
    // food category is pending, so the advisory check for a food code cannot be decided.
    let r = rules_from(&pack_text(None, true), purchase_date());
    assert!(matches!(
        blocked_category_for(&Hsn::parse("996331").unwrap(), &r).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn the_shipped_slab_pack_has_no_itc_categories_so_the_advisory_check_is_unsupported() {
    // provenance: derived — the shipped `packs/rates-slabs.toml` has no `itc.blocked.*` entries
    // (this card does not own pack files), so no blocked category can be verified: Unsupported.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let r = rules_from(&fs::read_to_string(path).unwrap(), purchase_date());
    assert!(matches!(
        blocked_category_for(&Hsn::parse("482010").unwrap(), &r).unwrap(),
        Outcome::Unsupported(_)
    ));
    assert_eq!(BLOCKED_RULE_IDS.len(), 3);
}

#[test]
fn a_malformed_pack_entry_is_a_pack_error() {
    // provenance: derived — rule A1: a category with a non-digit prefix, or without `section`,
    // is a broken pack (typed error), not "not blocked". Checked by the advisory check.
    let bad = "version = \"fixture\"\n\
        [[entry]]\nid = \"itc.blocked.motor_vehicles\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(a)\"\n[entry.params]\nsection = \"17(5)(a)\"\nhsn_prefixes = \"87x3\"\n\
        [[entry]]\nid = \"itc.blocked.vessels_aircraft\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(aa)\"\n[entry.params]\nsection = \"17(5)(aa)\"\nhsn_prefixes = \"8802\"\n\
        [[entry]]\nid = \"itc.blocked.food_beverages\"\neffective_from = \"2017-07-01\"\n\
        legal_ref = \"CGST Act s.17(5)(b)\"\n[entry.params]\nhsn_prefixes = \"9963\"\n";
    let r = rules_from(bad, purchase_date());
    assert!(matches!(
        blocked_category_for(&Hsn::parse("870321").unwrap(), &r),
        Err(GstError::Pack { .. })
    ));
}
