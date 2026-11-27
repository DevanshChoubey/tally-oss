//! Card G10 tests: GSTR-3B working, books against portal (spec 6.3; CGST Act s.9, s.16, s.17(5);
//! IGST Act s.5).
//!
//! Provenance. Every figure is hand arithmetic written next to the case. Rates: the 18% slab with
//! 9% + 9% halves (CGST Act s.9, IGST Act s.5; GST 2.0 slab per `docs/research/04-gst.md` 2.2 [V],
//! the same rows as `packs/rates-slabs.toml`). Rounding: each head is rounded once per voucher to
//! the paisa (G4's product policy). Table meanings: 3.1(a) is outward taxable supplies other than
//! zero rated, nil rated and exempted; 4(A)(5) is all other ITC (GSTR-3B form). Blocked credit:
//! CGST Act s.17(5)(a) motor vehicles; which codes are blocked is fixture pack data (prefix 8703).
//! A line in a blocked category makes the working Unsupported, because its exceptions depend on use.
//! The portal values in the comparison tests are typed in the test, as a user would type them; they
//! are not observed portal output. No test here is `observed`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_gst::GstError;
use ledgerly_gst::gstr3b::{
    Gstr3bField, Gstr3bPortalValues, Gstr3bTaxAmounts, Gstr3bWorking, build_gstr3b,
};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, Books, Company, EntityKind, Features,
    Group, GstClassification, GstHistory, GstRegistration, InventoryLine, Ledger, LedgerLine,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, StockItem, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use time::macros::date;

const KARNATAKA: u8 = 29;
const MAHARASHTRA: u8 = 27;

const SALES_TYPE: VoucherTypeId = VoucherTypeId(1);
const PURCHASE_TYPE: VoucherTypeId = VoucherTypeId(2);
const CREDIT_NOTE_TYPE: VoucherTypeId = VoucherTypeId(3);
const RECEIPT_TYPE: VoucherTypeId = VoucherTypeId(4);

const ITEM: StockItemId = StockItemId(1);
const CUSTOMER: LedgerId = LedgerId(11);
const SUPPLIER_KA: LedgerId = LedgerId(12);
const SUPPLIER_MH: LedgerId = LedgerId(13);
const SALES18: LedgerId = LedgerId(20);
const SALES_EXEMPT: LedgerId = LedgerId(21);
const PURCHASE18: LedgerId = LedgerId(22);
const CAR: LedgerId = LedgerId(23);
const PURCHASE_ITEM: LedgerId = LedgerId(24);
const SEZ_CUSTOMER: LedgerId = LedgerId(14);
const SALES_NIL: LedgerId = LedgerId(25);
const SALES_NON_GST: LedgerId = LedgerId(26);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn period() -> Period {
    Period::new(date!(2026 - 01 - 01), date!(2026 - 01 - 31)).unwrap()
}

fn gstin() -> Gstin {
    // Fictional GSTIN from ledgerly-core's test vectors (Karnataka).
    Gstin::parse("29ABCDE1234F1ZW").unwrap()
}

fn other_gstin() -> Gstin {
    // Same fictional PAN, Maharashtra; the check character comes from core's algorithm, which is
    // only a fixture identifier here (no expected value depends on it).
    let first14 = "27ABCDE1234F1Z";
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

/// A pack written for this file: the 18% and 5% GST 2.0 slabs, and optionally the three blocked
/// categories of the fixture (motor vehicles 8703, aircraft 8802, food 9963).
fn pack(with_itc: bool) -> RulePack {
    let mut text = "version = \"fixture-3b\"\n\
        [[entry]]\nid = \"slab.five\"\neffective_from = \"2025-09-22\"\nlegal_ref = \"fixture: GST 2.0 slab\"\n\
        [entry.params]\nigst_percent = \"5\"\ncgst_percent = \"2.5\"\nsgst_percent = \"2.5\"\n\
        [[entry]]\nid = \"slab.eighteen\"\neffective_from = \"2025-09-22\"\nlegal_ref = \"fixture: GST 2.0 slab\"\n\
        [entry.params]\nigst_percent = \"18\"\ncgst_percent = \"9\"\nsgst_percent = \"9\"\n"
        .to_owned();
    if with_itc {
        text.push_str(
            "[[entry]]\nid = \"itc.blocked.motor_vehicles\"\neffective_from = \"2017-07-01\"\nlegal_ref = \"CGST Act s.17(5)(a)\"\n\
             [entry.params]\nsection = \"17(5)(a)\"\nhsn_prefixes = \"8703\"\n\
             [[entry]]\nid = \"itc.blocked.vessels_aircraft\"\neffective_from = \"2017-07-01\"\nlegal_ref = \"CGST Act s.17(5)(aa)\"\n\
             [entry.params]\nsection = \"17(5)(aa)\"\nhsn_prefixes = \"8802\"\n\
             [[entry]]\nid = \"itc.blocked.food_beverages\"\neffective_from = \"2017-07-01\"\nlegal_ref = \"CGST Act s.17(5)(b)\"\n\
             [entry.params]\nsection = \"17(5)(b)\"\nhsn_prefixes = \"9963\"\n",
        );
    }
    load_pack(&text).unwrap()
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(KARNATAKA),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin(),
            state: state(KARNATAKA),
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

fn ledger(
    id: LedgerId,
    party: Option<(RegistrationType, Option<u8>)>,
    gst: Option<GstClassification>,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: format!("Ledger {}", id.0),
        aliases: vec![],
        group: Some(GroupId(1)),
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
        tax: None,
        gst,
        reserved: None,
        tally: None,
    }
}

fn class(hsn: Option<&str>, taxability: Taxability, percent: Option<i64>) -> GstClassification {
    GstClassification {
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        taxability,
        rate_percent: percent.map(|p| Rate(Decimal::new(p, 0))),
    }
}

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
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
    }
}

fn group_one() -> Group {
    Group {
        id: GroupId(1),
        name: "Group 1".to_owned(),
        aliases: vec![],
        parent: None,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        ineligible_itc: None,
        tally: None,
    }
}

fn masters() -> Masters {
    let mut m = Masters::default();
    // Every ledger here sits under GroupId(1). Round 3: ITC eligibility walks a ledger's group
    // chain for the ineligible-for-input-credit setting, so the group must exist (it was
    // referenced but missing before). Nothing is set on it, so no expected value changes.
    m.groups.insert(GroupId(1), group_one());
    for (id, name, base) in [
        (SALES_TYPE, "Sales", BaseVoucherKind::Sales),
        (PURCHASE_TYPE, "Purchase", BaseVoucherKind::Purchase),
        (CREDIT_NOTE_TYPE, "Credit Note", BaseVoucherKind::CreditNote),
        (RECEIPT_TYPE, "Receipt", BaseVoucherKind::Receipt),
    ] {
        m.voucher_types.insert(id, voucher_type(id, name, base));
    }
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
            ineligible_itc: None,
            id: ITEM,
            name: "Widget".to_owned(),
            aliases: vec![],
            group: None,
            unit: UnitId(1),
            alt_unit: None,
            hsn: Some(Hsn::parse("847130").unwrap()),
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
    let parties = [
        (CUSTOMER, (RegistrationType::Regular, Some(KARNATAKA))),
        (SUPPLIER_KA, (RegistrationType::Regular, Some(KARNATAKA))),
        (SUPPLIER_MH, (RegistrationType::Regular, Some(MAHARASHTRA))),
        (SEZ_CUSTOMER, (RegistrationType::Sez, Some(KARNATAKA))),
    ];
    for (id, party) in parties {
        m.ledgers.insert(id, ledger(id, Some(party), None));
    }
    let goods = [
        (SALES18, None, Taxability::Taxable, Some(18)),
        (SALES_EXEMPT, None, Taxability::Exempt, None),
        (SALES_NIL, None, Taxability::NilRated, None),
        (SALES_NON_GST, None, Taxability::NonGst, None),
        (PURCHASE18, Some("482010"), Taxability::Taxable, Some(18)),
        (CAR, Some("870321"), Taxability::Taxable, Some(18)),
        (PURCHASE_ITEM, None, Taxability::Taxable, Some(18)),
    ];
    for (id, hsn, taxability, percent) in goods {
        m.ledgers
            .insert(id, ledger(id, None, Some(class(hsn, taxability, percent))));
    }
    m
}

fn voucher(id: u64, type_id: VoucherTypeId, mode: VoucherMode, on: Date) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: Some(id.to_string()),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party: None,
        place_of_supply: Some(state(KARNATAKA)),
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

fn line(ledger: LedgerId, paise: i64, credit: bool) -> LedgerLine {
    let amount = Paise::new(paise);
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: if credit {
            SignedAmount::cr(amount)
        } else {
            SignedAmount::dr(amount)
        },
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

fn inv_line(paise: i64, ledger: LedgerId) -> InventoryLine {
    InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(paise, 2)),
        discount_percent: None,
        amount: Paise::new(paise),
        ledger: Some(ledger),
        godowns: vec![],
    }
}

/// Item-invoice sale to the customer, place of supply `place`.
fn item_sale(id: u64, on: Date, place: u8, amounts: &[i64]) -> Voucher {
    Voucher {
        party: Some(CUSTOMER),
        place_of_supply: Some(state(place)),
        inventory_lines: amounts.iter().map(|a| inv_line(*a, SALES18)).collect(),
        ..voucher(id, SALES_TYPE, VoucherMode::ItemInvoice, on)
    }
}

/// Accounting-invoice sale: credit lines to the given sales ledgers.
fn ledger_sale(id: u64, on: Date, place: u8, lines: &[(LedgerId, i64)]) -> Voucher {
    Voucher {
        party: Some(CUSTOMER),
        place_of_supply: Some(state(place)),
        ledger_lines: lines.iter().map(|(l, a)| line(*l, *a, true)).collect(),
        ..voucher(id, SALES_TYPE, VoucherMode::AccountingInvoice, on)
    }
}

/// Accounting-invoice purchase from `supplier`, place of supply Karnataka.
fn ledger_purchase(id: u64, on: Date, supplier: LedgerId, ledger: LedgerId, paise: i64) -> Voucher {
    Voucher {
        party: Some(supplier),
        ledger_lines: vec![line(ledger, paise, false)],
        ..voucher(id, PURCHASE_TYPE, VoucherMode::AccountingInvoice, on)
    }
}

fn books_with(vouchers: Vec<Voucher>) -> Books {
    Books {
        company: company(),
        masters: masters(),
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: vec![],
        stock_moves: vec![],
    }
}

/// The books used by most tests (January 2026).
fn january_books() -> Books {
    let jan = date!(2026 - 01 - 15);
    // Item-invoice sale, two lines, intra-state.
    let s1 = item_sale(1, jan, KARNATAKA, &[600_000, 400_000]);
    // Accounting-invoice sale: taxable line and an exempt line, inter-state.
    let s2 = ledger_sale(
        2,
        jan,
        MAHARASHTRA,
        &[(SALES18, 250_050), (SALES_EXEMPT, 100_000)],
    );
    // Dated in February: not in January.
    let s3 = item_sale(3, date!(2026 - 02 - 02), KARNATAKA, &[5_000_000]);
    // Cancelled and optional vouchers are not in the books.
    let s4 = Voucher {
        status: VoucherStatus::Cancelled,
        ..item_sale(4, jan, KARNATAKA, &[7_000_000])
    };
    let s5 = Voucher {
        status: VoucherStatus::Optional,
        ..item_sale(5, jan, KARNATAKA, &[8_000_000])
    };
    // Voucher date in January, supply date in February: time of supply is in February.
    let s6 = Voucher {
        supply_date: Some(date!(2026 - 02 - 01)),
        ..ledger_sale(6, date!(2026 - 01 - 31), KARNATAKA, &[(SALES18, 9_000_000)])
    };
    // Voucher date in February, supply date in January: time of supply is in January.
    let s7 = Voucher {
        supply_date: Some(date!(2026 - 01 - 31)),
        ..ledger_sale(7, date!(2026 - 02 - 03), KARNATAKA, &[(SALES18, 100_000)])
    };
    // Eligible accounting-invoice purchase, intra-state.
    let p11 = ledger_purchase(11, jan, SUPPLIER_KA, PURCHASE18, 5_000_000);
    // Eligible item-invoice purchase, inter-state supplier.
    let p13 = Voucher {
        party: Some(SUPPLIER_MH),
        ledger_lines: vec![line(PURCHASE_ITEM, 2_000_050, false)],
        inventory_lines: vec![inv_line(2_000_050, PURCHASE_ITEM)],
        ..voucher(13, PURCHASE_TYPE, VoucherMode::ItemInvoice, jan)
    };
    // Reverse-charge purchase: neither 3.1(a) nor 4(A)(5).
    let p14 = Voucher {
        reverse_charge: true,
        ..ledger_purchase(14, jan, SUPPLIER_KA, PURCHASE18, 3_000_000)
    };
    // February purchase: not in January.
    let p15 = ledger_purchase(
        15,
        date!(2026 - 02 - 10),
        SUPPLIER_KA,
        PURCHASE18,
        6_000_000,
    );
    books_with(vec![s1, s2, s3, s4, s5, s6, s7, p11, p13, p14, p15])
}

/// A motor car purchase in January 2026. Its s.17(5)(a) exceptions depend on use, so the working
/// is Unsupported whenever it is in the period. It is kept out of [`january_books`] for that reason.
fn car_purchase() -> Voucher {
    ledger_purchase(12, date!(2026 - 01 - 15), SUPPLIER_KA, CAR, 10_000_000)
}

fn run(
    books: &Books,
    pack: &RulePack,
    portal: Option<&Gstr3bPortalValues>,
) -> Result<Outcome<Gstr3bWorking>, GstError> {
    build_gstr3b(books, &gstin(), period(), pack, portal)
}

fn value(books: &Books, portal: Option<&Gstr3bPortalValues>) -> Gstr3bWorking {
    match run(books, &pack(true), portal).unwrap() {
        Outcome::Value(w) => w,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn assert_unsupported(books: &Books) {
    match run(books, &pack(true), None).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(w) => panic!("expected Unsupported, got {w:?}"),
    }
}

fn amounts(integrated: i64, central: i64, state_ut: i64) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: Paise::new(integrated),
        central: Paise::new(central),
        state_ut: Paise::new(state_ut),
        cess: Paise::ZERO,
    }
}

/// The portal values that agree with the books of `january_books`.
fn agreeing_portal() -> Gstr3bPortalValues {
    Gstr3bPortalValues {
        outward_taxable_value: Paise::new(1_350_050),
        outward_tax: amounts(45_009, 99_000, 99_000),
        itc_all_other: amounts(360_009, 450_000, 450_000),
    }
}

#[test]
fn table_3_1_a_taxable_value_is_the_sum_of_outward_taxable_lines() {
    // provenance: derived — 3B table 3.1(a) = outward taxable supplies other than zero rated,
    // nil rated and exempted. January 2026 vouchers:
    //   S1 item invoice 6,000.00 + 4,000.00 = 10,000.00 taxable at 18%   -> 1,000,000 paise
    //   S2 accounting invoice: 2,500.50 taxable + 1,000.00 exempt        ->   250,050 paise taxable
    //   S7 supplied 31 Jan (voucher dated 3 Feb): 1,000.00 taxable       ->   100,000 paise
    // Left out: S3 (dated Feb), S4 (cancelled), S5 (optional), S6 (supplied 1 Feb).
    // Sum = 1,000,000 + 250,050 + 100,000 = 1,350,050 paise = Rs 13,500.50.
    let w = value(&january_books(), None);
    assert_eq!(w.outward_taxable_value, Paise::new(1_350_050));
    assert_eq!(
        w.outward_vouchers,
        vec![VoucherId(1), VoucherId(2), VoucherId(7)]
    );
}

#[test]
fn table_3_1_a_tax_is_per_head_over_the_taxable_vouchers() {
    // provenance: derived — CGST Act s.9 (intra-state: CGST + SGST) and IGST Act s.5 (inter-state:
    // IGST), 18% = 9% + 9%.
    //   S1 intra-state: 10,000.00 x 9% = 900.00 CGST and 900.00 SGST   -> 90,000 + 90,000 paise
    //   S2 inter-state: 2,500.50 x 18% = 450.09 IGST (the exempt line bears none) -> 45,009 paise
    //   S7 intra-state: 1,000.00 x 9% = 90.00 CGST and 90.00 SGST       ->  9,000 +  9,000 paise
    // Central = 90,000 + 9,000 = 99,000; state = 99,000; integrated = 45,009; cess = 0.
    let w = value(&january_books(), None);
    assert_eq!(w.outward_tax, amounts(45_009, 99_000, 99_000));
    assert_eq!(w.pack_version, "fixture-3b");
    assert_eq!(w.gstin, gstin());
    assert_eq!(w.period, period());
}

#[test]
fn table_4_a_5_is_the_tax_on_eligible_purchases_only() {
    // provenance: derived — 3B table 4(A)(5) = all other ITC; CGST Act s.16(1) allows credit of
    // tax on inputs. The motor car is not in january_books (see a_blocked_motor_car_...).
    //   P11 purchase 50,000.00 (HSN 482010) from a Karnataka supplier: CGST 4,500.00, SGST 4,500.00
    //       -> 450,000 + 450,000 paise, eligible.
    //   P13 item purchase 20,000.50 from a Maharashtra supplier (item HSN 847130): IGST
    //       20,000.50 x 18% = 3,600.09 -> 360,009 paise, eligible.
    // Left out: P14 (reverse charge), P15 (dated February).
    // 4(A)(5): integrated 360,009; central 450,000; state 450,000.
    // Nothing is in the not-claimable row: a line that is blocked or ineligible makes the working
    // Unsupported (blocked) or is rejected by compute (composition and unregistered suppliers),
    // so no line can be reported as a known, not-claimable amount.
    let w = value(&january_books(), None);
    assert_eq!(w.itc_all_other, amounts(360_009, 450_000, 450_000));
    assert_eq!(w.itc_vouchers, vec![VoucherId(11), VoucherId(13)]);
    assert_eq!(w.itc_not_claimable, Gstr3bTaxAmounts::default());
}

#[test]
fn a_blocked_motor_car_makes_the_working_unsupported_not_an_understated_4_a_5() {
    // provenance: derived — CGST Act s.17(5)(a): a motor car's exceptions (further supply,
    // passenger or goods transport, driving training) depend on use, which is not recorded. The
    // credit cannot be called blocked, so the working is Unsupported. A silent 4(A)(5) of zero
    // would be a wrong number, because the car may be claimable.
    let books = books_with(vec![car_purchase()]);
    assert_unsupported(&books);
}

#[test]
fn a_blocked_motor_car_is_unsupported_even_next_to_eligible_purchases() {
    // provenance: derived — the same rule with an eligible purchase in the period: the working as
    // a whole is Unsupported, so the eligible line's credit is not shown as the full 4(A)(5).
    let books = books_with(vec![
        ledger_purchase(
            11,
            date!(2026 - 01 - 15),
            SUPPLIER_KA,
            PURCHASE18,
            5_000_000,
        ),
        car_purchase(),
    ]);
    assert_unsupported(&books);
}

#[test]
fn reverse_charge_purchases_are_listed_not_added() {
    // provenance: derived — reverse-charge purchases are reported in 3B tables 3.1(d) and 4(A)(3),
    // not in 3.1(a) or 4(A)(5); the voucher is named so nothing is dropped silently. P14 is the
    // only purchase under reverse charge in January.
    let w = value(&january_books(), None);
    assert_eq!(w.reverse_charge_purchases, vec![VoucherId(14)]);
}

#[test]
fn no_portal_values_means_no_comparison() {
    // provenance: derived — the card: the comparison column is populated from an injected portal
    // value; with none injected there is nothing to compare.
    let w = value(&january_books(), None);
    assert_eq!(w.comparison, None);
    assert_eq!(
        w.differences(),
        [] as [&ledgerly_gst::gstr3b::ComparisonRow; 0]
    );
}

#[test]
fn agreeing_portal_values_list_no_differences() {
    // provenance: derived — books figures are those of the cases above (taxable value 1,350,050;
    // 3.1(a) tax 45,009 / 99,000 / 99,000; 4(A)(5) 360,009 / 450,000 / 450,000), so a portal that
    // shows the same numbers differs by 0 in every row.
    let portal = agreeing_portal();
    let w = value(&january_books(), Some(&portal));
    let rows = w.comparison.as_ref().unwrap();
    assert_eq!(rows.len(), 9);
    assert_eq!(
        w.differences(),
        [] as [&ledgerly_gst::gstr3b::ComparisonRow; 0]
    );
}

#[test]
fn differences_are_books_minus_portal_and_only_differing_rows_are_listed() {
    // provenance: derived — books taxable value 1,350,050; portal typed as 1,350,000 -> books minus
    // portal = +50 paise. Books 4(A)(5) central 450,000; portal typed as 400,000 -> +50,000.
    // Portal state/UT ITC typed as 460,000 against books 450,000 -> -10,000. Every other row
    // agrees, so exactly three rows are listed, in table order.
    let mut portal = agreeing_portal();
    portal.outward_taxable_value = Paise::new(1_350_000);
    portal.itc_all_other.central = Paise::new(400_000);
    portal.itc_all_other.state_ut = Paise::new(460_000);
    let w = value(&january_books(), Some(&portal));
    let listed: Vec<(Gstr3bField, i64, i64, i64)> = w
        .differences()
        .iter()
        .map(|r| (r.field, r.books.get(), r.portal.get(), r.difference.get()))
        .collect();
    assert_eq!(
        listed,
        vec![
            (Gstr3bField::OutwardTaxableValue, 1_350_050, 1_350_000, 50),
            (Gstr3bField::ItcCentral, 450_000, 400_000, 50_000),
            (Gstr3bField::ItcStateUt, 450_000, 460_000, -10_000),
        ]
    );
}

#[test]
fn an_empty_period_is_a_zero_working() {
    // provenance: derived — no voucher touches the period, so every sum over an empty set is 0.
    let books = books_with(vec![]);
    let w = value(&books, None);
    assert_eq!(w.outward_taxable_value, Paise::ZERO);
    assert_eq!(w.outward_tax, Gstr3bTaxAmounts::default());
    assert_eq!(w.itc_all_other, Gstr3bTaxAmounts::default());
    assert_eq!(w.outward_vouchers, [] as [ledgerly_core::VoucherId; 0]);
}

#[test]
fn the_gstin_must_be_a_registration_of_the_company() {
    // provenance: derived — a working for a GSTIN the company does not hold has no vouchers to
    // draw on; it is an error, not an empty working.
    let books = books_with(vec![]);
    let r = build_gstr3b(&books, &other_gstin(), period(), &pack(true), None);
    assert!(matches!(r, Err(GstError::NotFound { .. })));
}

#[test]
fn a_company_with_two_registrations_is_unsupported() {
    // provenance: derived — vouchers carry no GSTIN, so with two registrations the figures of one
    // cannot be told from the other's (never guess).
    let mut books = books_with(vec![]);
    books.company.registrations.push(GstRegistration {
        gstin: other_gstin(),
        state: state(MAHARASHTRA),
        effective_from: date!(2017 - 07 - 01),
        composition: false,
    });
    assert_unsupported(&books);
}

#[test]
fn a_composition_registration_is_unsupported() {
    // provenance: derived — composition taxpayers file CMP-08 and GSTR-4, not GSTR-3B.
    let mut books = books_with(vec![]);
    books.company.registrations[0].composition = true;
    assert_unsupported(&books);
}

#[test]
fn gst_disabled_is_unsupported() {
    // provenance: derived — with the GST feature off the books carry no GST, so no working.
    let mut books = books_with(vec![]);
    books.company.features.gst = false;
    assert_unsupported(&books);
}

#[test]
fn a_registration_that_starts_after_the_period_is_unsupported() {
    // provenance: derived — the registration is effective 1 Mar 2026; January 2026 is before it,
    // so there is no return for the period.
    let mut books = books_with(vec![]);
    books.company.registrations[0].effective_from = date!(2026 - 03 - 01);
    assert_unsupported(&books);
}

#[test]
fn a_credit_note_in_the_period_is_unsupported() {
    // provenance: pending — how a credit note nets against 3.1(a) (CGST Act s.34) has not been
    // checked against the Act text, so the working must not guess: Unsupported.
    let note = Voucher {
        party: Some(CUSTOMER),
        ledger_lines: vec![line(SALES18, 100_000, false)],
        ..voucher(
            20,
            CREDIT_NOTE_TYPE,
            VoucherMode::AccountingInvoice,
            date!(2026 - 01 - 20),
        )
    };
    assert_unsupported(&books_with(vec![note]));
}

#[test]
fn a_credit_note_outside_the_period_does_not_matter() {
    // provenance: derived — a credit note dated in February is not in the January working.
    let note = Voucher {
        party: Some(CUSTOMER),
        ledger_lines: vec![line(SALES18, 100_000, false)],
        ..voucher(
            20,
            CREDIT_NOTE_TYPE,
            VoucherMode::AccountingInvoice,
            date!(2026 - 02 - 20),
        )
    };
    let w = value(&books_with(vec![note]), None);
    assert_eq!(w.outward_taxable_value, Paise::ZERO);
}

#[test]
fn an_advance_receipt_in_the_period_is_unsupported() {
    // provenance: pending — tax on an advance depends on the time-of-supply rules (card G12), so
    // the working does not guess whether it belongs in 3.1(a).
    let mut advance = line(CUSTOMER, 100_000, true);
    advance.bills.push(BillAllocation {
        kind: BillRefKind::Advance,
        name: "ADV-1".to_owned(),
        due_date: None,
        credit_days: None,
        amount: SignedAmount::cr(Paise::new(100_000)),
    });
    let receipt = Voucher {
        party: Some(CUSTOMER),
        ledger_lines: vec![advance],
        ..voucher(
            21,
            RECEIPT_TYPE,
            VoucherMode::AsVoucher,
            date!(2026 - 01 - 20),
        )
    };
    assert_unsupported(&books_with(vec![receipt]));
}

#[test]
fn a_receipt_without_an_advance_is_ignored() {
    // provenance: derived — a receipt against an existing bill settles a debtor; it is not a
    // supply, so it is not in 3.1(a).
    let mut against = line(CUSTOMER, 100_000, true);
    against.bills.push(BillAllocation {
        kind: BillRefKind::Against,
        name: "INV-1".to_owned(),
        due_date: None,
        credit_days: None,
        amount: SignedAmount::cr(Paise::new(100_000)),
    });
    let receipt = Voucher {
        party: Some(CUSTOMER),
        ledger_lines: vec![against],
        ..voucher(
            21,
            RECEIPT_TYPE,
            VoucherMode::AsVoucher,
            date!(2026 - 01 - 20),
        )
    };
    let w = value(&books_with(vec![receipt]), None);
    assert_eq!(w.outward_taxable_value, Paise::ZERO);
}

#[test]
fn an_outward_supply_under_reverse_charge_is_unsupported() {
    // provenance: pending — where tax paid by the recipient is reported is not derived here.
    let sale = Voucher {
        reverse_charge: true,
        ..item_sale(30, date!(2026 - 01 - 15), KARNATAKA, &[100_000])
    };
    assert_unsupported(&books_with(vec![sale]));
}

#[test]
fn a_supply_to_an_sez_party_is_unsupported() {
    // provenance: derived — a supply to an SEZ is zero-rated (IGST Act s.16(1)(b)) and belongs in
    // table 3.1(b), not 3.1(a); this working does not fill 3.1(b), so it must not drop the
    // voucher silently: Unsupported.
    let sale = Voucher {
        party: Some(SEZ_CUSTOMER),
        ..item_sale(31, date!(2026 - 01 - 15), KARNATAKA, &[100_000])
    };
    assert_unsupported(&books_with(vec![sale]));
}

#[test]
fn a_payment_dated_before_the_invoice_without_a_supply_date_is_unsupported() {
    // provenance: pending — the time of supply moves with an earlier payment (CGST Act s.12/s.13),
    // which card G12 will decide; until then the period of the supply is unknown.
    let sale = Voucher {
        payment_date: Some(date!(2025 - 12 - 20)),
        ..item_sale(32, date!(2026 - 01 - 15), KARNATAKA, &[100_000])
    };
    // The payment is in December and the invoice in January: January's working cannot place it.
    assert_unsupported(&books_with(vec![sale]));
}

#[test]
fn a_pack_without_blocked_categories_makes_a_taxable_purchase_unsupported() {
    // provenance: derived — engineering rule A8: an unverifiable blocked category means "not
    // blocked" cannot be claimed, so any taxable purchase in the period makes the working
    // Unsupported (the fixture pack has the slabs but no itc.blocked.* entries).
    let books = books_with(vec![ledger_purchase(
        1,
        date!(2026 - 01 - 15),
        SUPPLIER_KA,
        PURCHASE18,
        5_000_000,
    )]);
    assert!(matches!(
        run(&books, &pack(false), None).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn a_sale_in_a_period_with_no_verified_rate_is_unsupported() {
    // provenance: derived — the fixture item's GST details and the pack's slabs both start 2025-09-22, so a supply on
    // 2025-06-01 has no verified rate and G4 returns Unsupported; the working passes it on.
    let books = books_with(vec![item_sale(
        1,
        date!(2025 - 06 - 01),
        KARNATAKA,
        &[100_000],
    )]);
    let june = Period::new(date!(2025 - 06 - 01), date!(2025 - 06 - 30)).unwrap();
    let r = build_gstr3b(&books, &gstin(), june, &pack(true), None).unwrap();
    assert!(matches!(r, Outcome::Unsupported(_)));
}

#[test]
fn voucher_order_does_not_change_the_working() {
    // provenance: derived — rule A7: same input, same output. The books keep vouchers in a
    // BTreeMap by id, so inserting them in reverse gives the same working.
    let books = january_books();
    let mut reversed = books_with(vec![]);
    for (id, v) in books.vouchers.iter().rev() {
        reversed.vouchers.insert(*id, v.clone());
    }
    assert_eq!(value(&books, None), value(&reversed, None));
}

// Explicit settings on the motor car (round-3 fix). Provenance: derived, Tally ineligible-ITC page
// (https://help.tallysolutions.com/docs/te9rel66/Tax_India/gst/gst_accounting_for_ineligible_itc.htm,
// read 2026-11-21). The car's tax is 9,000.00 CGST and 9,000.00 SGST = 900,000 paise each.

#[test]
fn a_motor_car_marked_ineligible_on_its_ledger_goes_to_the_not_claimable_row() {
    // provenance: derived — the purchase ledger is marked Yes, so the credit is ineligible.
    let mut books = books_with(vec![car_purchase()]);
    books.masters.ledgers.get_mut(&CAR).unwrap().ineligible_itc = Some(true);
    let w = value(&books, None);
    assert_eq!(w.itc_all_other, Gstr3bTaxAmounts::default());
    assert_eq!(w.itc_vouchers, Vec::<VoucherId>::new());
    assert_eq!(w.itc_not_claimable, amounts(0, 900_000, 900_000));
}

#[test]
fn a_motor_car_marked_eligible_on_its_ledger_is_claimed() {
    // provenance: derived — the purchase ledger is marked No (explicit user setting), so the HSN
    // rule (s.17(5)(a) fixture) does not apply and the credit is claimed in 4(A)(5).
    let mut books = books_with(vec![car_purchase()]);
    books.masters.ledgers.get_mut(&CAR).unwrap().ineligible_itc = Some(false);
    let w = value(&books, None);
    assert_eq!(w.itc_all_other, amounts(0, 900_000, 900_000));
    assert_eq!(w.itc_vouchers, vec![VoucherId(12)]);
}

// ---------------------------------------------------------------- round s23: 3.1(c), 3.1(e), deemed exports
//
// provenance (all three tests): derived — the GST portal's GSTR-3B help page
// (https://tutorial.gst.gov.in/userguide/returns/GSTR3B.htm, read 2026-11-28) fills 3.1(a) from
// GSTR-1 tables 4A and 6C ("Deemed exports"), 3.1(c) with "Nil rated and exempted supplies
// (columns 2 and 3)" of GSTR-1 table 8, and 3.1(e) with "Non-GST outward supplies (column 4)".

#[test]
fn table_3_1_c_is_the_value_of_nil_rated_and_exempt_lines() {
    // provenance: derived — January books: S2 has an exempt line of 1,000.00 = 100,000 paise and
    // no other voucher in the period has a nil-rated, exempt or non-GST line.
    let w = value(&january_books(), None);
    assert_eq!(w.outward_nil_exempt_value, Paise::new(100_000));
    assert_eq!(w.outward_non_gst_value, Paise::ZERO);
    assert_eq!(w.other_outward_vouchers, vec![VoucherId(2)]);
    // 3.1(a) is unchanged by the exempt line (see the 3.1(a) tests above).
    assert_eq!(w.outward_taxable_value, Paise::new(1_350_050));
}

#[test]
fn nil_rated_goes_to_3_1_c_and_non_gst_to_3_1_e() {
    // provenance: derived — one January sale with nil-rated 500.00, exempt 100.00 and non-GST
    // 700.00 lines: 3.1(c) = 50,000 + 10,000 = 60,000 paise; 3.1(e) = 70,000 paise; nothing
    // in 3.1(a) and no tax.
    let s = ledger_sale(
        30,
        date!(2026 - 01 - 20),
        KARNATAKA,
        &[
            (SALES_NIL, 50_000),
            (SALES_EXEMPT, 10_000),
            (SALES_NON_GST, 70_000),
        ],
    );
    let w = value(&books_with(vec![s]), None);
    assert_eq!(w.outward_nil_exempt_value, Paise::new(60_000));
    assert_eq!(w.outward_non_gst_value, Paise::new(70_000));
    assert_eq!(w.other_outward_vouchers, vec![VoucherId(30)]);
    assert_eq!(w.outward_taxable_value, Paise::ZERO);
    assert_eq!(w.outward_tax, amounts(0, 0, 0));
}

#[test]
fn a_deemed_export_is_in_3_1_a() {
    // provenance: derived — the help page puts GSTR-1 6C (deemed exports) in 3.1(a). A deemed
    // export of 1,000.00 at 18% to the regular customer in Karnataka: 1,000.00 x 9% = 90.00 CGST
    // and 90.00 SGST (CGST Act s.9; the recipient's refund route does not change the tax).
    let mut s = ledger_sale(31, date!(2026 - 01 - 20), KARNATAKA, &[(SALES18, 100_000)]);
    s.extra.insert(
        ledgerly_gst::classify::SUPPLY_OPTION_KEY.to_owned(),
        "deemed_export".to_owned(),
    );
    let w = value(&books_with(vec![s]), None);
    assert_eq!(w.outward_taxable_value, Paise::new(100_000));
    assert_eq!(w.outward_tax, amounts(0, 9_000, 9_000));
    assert_eq!(w.outward_vouchers, vec![VoucherId(31)]);
}
