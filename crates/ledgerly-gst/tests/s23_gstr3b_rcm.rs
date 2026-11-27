//! S23 tests: GSTR-3B tables 3.1(d) and 4(A)(3), inward supplies liable to reverse charge.
//!
//! Provenance: `derived`. The table meanings are those of FORM GSTR-3B: 3.1(d) "inward supplies
//! (liable to reverse charge)" with taxable value and tax per head, 4(A)(3) "inward supplies
//! liable to reverse charge (other than 1 & 2 above)". Liability: CGST Act s.9(3) with the classes
//! of `packs/rcm.toml` (13/2017-CT(R), read on CBIC's tax information portal 2026-11-27). Period:
//! the time of supply, which under reverse charge is s.13(3) (read 2026-11-27 on the same portal:
//! the earlier of the payment date and "the date immediately following sixty days from the date of
//! issue of invoice"), entered by the user as the supply date. Credit: Rule 36(1)(b) (the
//! recipient's invoice, "subject to the payment of tax"), s.49(4) with s.2(82) (the credit ledger
//! pays output tax, which excludes reverse-charge tax), and the s.17(5) headings of
//! `packs/gstr-rules.toml`. Rates: `packs/rates-slabs.toml` (5% = 2.5% + 2.5%, 18% = 9% + 9%, from
//! 22 September 2025). Each head is rounded once per voucher to the paisa (card G4 policy).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Rate,
    SignedAmount, StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_gst::gstr3b::{Gstr3bTaxAmounts, Gstr3bWorking, InwardReverseCharge, build_gstr3b};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, GstClassification,
    GstRegistration, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, TaxDetails, TaxLedgerKind,
    Taxability, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const SLABS: &str = include_str!("../packs/rates-slabs.toml");
const RULES: &str = include_str!("../packs/gstr-rules.toml");
const RCM: &str = include_str!("../packs/rcm.toml");

const KARNATAKA: u8 = 29;

const GTA: LedgerId = LedgerId(20);
const LEGAL: LedgerId = LedgerId(21);
const CAB: LedgerId = LedgerId(23);
const PAPER: LedgerId = LedgerId(27);
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

fn gstin() -> Gstin {
    // Fictional GSTIN from ledgerly-core's test vectors.
    Gstin::parse("29ABCDE1234F1ZW").unwrap()
}

fn january() -> Period {
    Period::new(date!(2026 - 01 - 01), date!(2026 - 01 - 31)).unwrap()
}

fn february() -> Period {
    Period::new(date!(2026 - 02 - 01), date!(2026 - 02 - 28)).unwrap()
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

fn classified(id: LedgerId, code: &str, percent: i64) -> Ledger {
    let mut l = ledger(id);
    l.gst = Some(GstClassification {
        hsn: Some(Hsn::parse(code).unwrap()),
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::new(percent, 0))),
    });
    l
}

fn masters() -> Masters {
    let mut m = Masters::default();
    m.groups.insert(
        GroupId(1),
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
        },
    );
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
        classified(GTA, "996791", 5),
        classified(LEGAL, "998212", 18),
        classified(CAB, "996601", 5),
        classified(PAPER, "482010", 18),
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

/// An accounting-invoice purchase dated `on`, supplied (time of supply) on `supplied`.
fn purchase(
    id: u64,
    on: Date,
    supplied: Option<Date>,
    party: LedgerId,
    lines: Vec<LedgerLine>,
    marked: bool,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(id.to_string()),
        series: None,
        date: on,
        supply_date: supplied,
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

fn books(vouchers: Vec<Voucher>) -> Books {
    Books {
        company: company(),
        masters: masters(),
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn working(vouchers: Vec<Voucher>, period: Period) -> Gstr3bWorking {
    match build_gstr3b(&books(vouchers), &gstin(), period, &pack(), None).unwrap() {
        Outcome::Value(w) => w,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn amounts(central: i64, state_ut: i64) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: Paise::ZERO,
        central: Paise::new(central),
        state_ut: Paise::new(state_ut),
        cess: Paise::ZERO,
    }
}

/// GTA 10,000.00 from an unregistered transporter, marked, supplied 10 January 2026.
fn gta(id: u64) -> Voucher {
    purchase(
        id,
        date!(2026 - 01 - 10),
        Some(date!(2026 - 01 - 10)),
        UNREG,
        vec![line(GTA, 1_000_000)],
        true,
    )
}

/// An advocate's fee of 50,000.00, marked (declaring the supplier is an advocate), 20 January.
fn legal(id: u64) -> Voucher {
    purchase(
        id,
        date!(2026 - 01 - 20),
        Some(date!(2026 - 01 - 20)),
        REG,
        vec![line(LEGAL, 5_000_000)],
        true,
    )
}

/// Paper 20,000.00 from a registered supplier who charged 9% + 9% (forward charge).
fn paper(id: u64) -> Voucher {
    purchase(
        id,
        date!(2026 - 01 - 15),
        None,
        REG,
        vec![
            line(PAPER, 2_000_000),
            line(CGST_IN, 180_000),
            line(SGST_IN, 180_000),
        ],
        false,
    )
}

#[test]
fn table_3_1_d_sums_value_and_tax_of_reverse_charge_purchases_in_the_period() {
    // provenance: derived — 13/2017-CT(R) sl.1 (GTA, any supplier, registered recipient) and sl.2
    // (advocate, declared by the mark); 3.1(d) is their taxable value and tax.
    //   GTA 10,000.00 at 5%: CGST 2.5% = 250.00, SGST 250.00
    //   Legal 50,000.00 at 18%: CGST 9% = 4,500.00, SGST 4,500.00
    //   Value 60,000.00 = 6,000,000 paise; CGST 4,750.00 = 475,000; SGST 475,000.
    let w = working(vec![gta(1), legal(2)], january());
    let Outcome::Value(d) = w.inward_reverse_charge else {
        panic!("3.1(d) should be worked out: {:?}", w.inward_reverse_charge)
    };
    assert_eq!(d.taxable_value, Paise::new(6_000_000));
    assert_eq!(d.tax, amounts(475_000, 475_000));
    assert_eq!(d.vouchers, vec![VoucherId(1), VoucherId(2)]);
    assert_eq!(d.unmarked, Vec::<VoucherId>::new());
    assert_eq!(w.reverse_charge_purchases, vec![VoucherId(1), VoucherId(2)]);
}

#[test]
fn table_4_a_3_is_the_credit_on_the_claimable_reverse_charge_tax() {
    // provenance: derived — Rule 36(1)(b): credit on the recipient's invoice subject to payment of
    // the tax; neither SAC 996791 nor 998212 is a s.17(5) heading of the pack, so all of the
    // 3.1(d) tax is creditable: CGST 475,000, SGST 475,000 paise.
    let w = working(vec![gta(1), legal(2)], january());
    let Outcome::Value(itc) = w.itc_reverse_charge else {
        panic!("4(A)(3) should be worked out: {:?}", w.itc_reverse_charge)
    };
    assert_eq!(itc.itc, amounts(475_000, 475_000));
    assert_eq!(itc.not_claimable, Gstr3bTaxAmounts::default());
    assert_eq!(itc.vouchers, vec![VoucherId(1), VoucherId(2)]);
}

#[test]
fn reverse_charge_tax_is_in_neither_3_1_a_nor_4_a_5() {
    // provenance: derived — s.2(82): output tax excludes tax payable on reverse charge, so it is
    // not in 3.1(a); its credit is 4(A)(3), not 4(A)(5). The paper purchase is the only
    // forward-charge credit: 20,000.00 x 9% = 1,800.00 CGST and 1,800.00 SGST (180,000 paise).
    let w = working(vec![gta(1), legal(2), paper(3)], january());
    assert_eq!(w.outward_taxable_value, Paise::ZERO);
    assert_eq!(w.outward_tax, Gstr3bTaxAmounts::default());
    assert_eq!(w.itc_all_other, amounts(180_000, 180_000));
    assert_eq!(w.itc_vouchers, vec![VoucherId(3)]);
}

#[test]
fn the_period_is_the_time_of_supply_not_the_voucher_date() {
    // provenance: derived — s.13(3): the time of supply under reverse charge is the earlier of the
    // payment and the 61st day after the invoice; the user enters it as the supply date. A GTA
    // voucher dated 25 January supplied (paid) 3 February is in February's 3.1(d), not January's.
    let v = purchase(
        1,
        date!(2026 - 01 - 25),
        Some(date!(2026 - 02 - 03)),
        UNREG,
        vec![line(GTA, 1_000_000)],
        true,
    );
    let jan = working(vec![v.clone()], january());
    assert_eq!(
        jan.inward_reverse_charge,
        Outcome::Value(InwardReverseCharge::default())
    );
    assert_eq!(jan.reverse_charge_purchases, Vec::<VoucherId>::new());
    let feb = working(vec![v], february());
    let Outcome::Value(d) = feb.inward_reverse_charge else {
        panic!("February 3.1(d) should be worked out")
    };
    // 10,000.00 at 5%: 250.00 + 250.00.
    assert_eq!(d.taxable_value, Paise::new(1_000_000));
    assert_eq!(d.tax, amounts(25_000, 25_000));
}

#[test]
fn an_unmarked_unregistered_gta_is_still_liable_and_is_flagged() {
    // provenance: derived — sl.1 names "any person" as supplier and needs no declaration, so the
    // liability exists without the mark (s.9(3) is the law, not the mark). 250.00 + 250.00.
    let mut v = gta(1);
    v.reverse_charge = false;
    let w = working(vec![v], january());
    let Outcome::Value(d) = w.inward_reverse_charge else {
        panic!("3.1(d) should be worked out")
    };
    assert_eq!(d.tax, amounts(25_000, 25_000));
    assert_eq!(d.unmarked, vec![VoucherId(1)]);
    let Outcome::Value(itc) = w.itc_reverse_charge else {
        panic!("4(A)(3) should be worked out")
    };
    assert_eq!(itc.itc, amounts(25_000, 25_000));
}

#[test]
fn a_registered_gta_invoice_without_tax_goes_to_3_1_d_not_4_a_5() {
    // provenance: derived — sl.1: a registered GTA that did not opt for forward charge charges no
    // tax and the recipient pays it. Unmarked, no GST posted: the forward path would compute a
    // credit of 250.00 + 250.00 that no supplier charged, so the voucher goes to 3.1(d).
    let v = purchase(
        1,
        date!(2026 - 01 - 10),
        Some(date!(2026 - 01 - 10)),
        REG,
        vec![line(GTA, 1_000_000)],
        false,
    );
    let w = working(vec![v], january());
    assert_eq!(w.itc_all_other, Gstr3bTaxAmounts::default());
    let Outcome::Value(d) = w.inward_reverse_charge else {
        panic!("3.1(d) should be worked out")
    };
    assert_eq!(d.tax, amounts(25_000, 25_000));
}

#[test]
fn a_registered_gta_invoice_that_charges_tax_is_forward_charge() {
    // provenance: derived — the forward-charge proviso of sl.1 (05/2022-CT(R)): the GTA that
    // charges tax pays it; the recipient takes 4(A)(5) credit of 250.00 + 250.00.
    let v = purchase(
        1,
        date!(2026 - 01 - 10),
        None,
        REG,
        vec![
            line(GTA, 1_000_000),
            line(CGST_IN, 25_000),
            line(SGST_IN, 25_000),
        ],
        false,
    );
    let w = working(vec![v], january());
    assert_eq!(w.itc_all_other, amounts(25_000, 25_000));
    assert_eq!(
        w.inward_reverse_charge,
        Outcome::Value(InwardReverseCharge::default())
    );
}

#[test]
fn a_reverse_charge_purchase_without_a_supply_date_leaves_the_forward_tables_worked_out() {
    // provenance: derived — s.13(3) needs the payment date; without a supply date the time of
    // supply is unknown, so 3.1(d) and 4(A)(3) are Unsupported. 3.1(a) and 4(A)(5) do not depend
    // on it: paper's 1,800.00 + 1,800.00 is still shown.
    let mut v = gta(1);
    v.supply_date = None;
    let w = working(vec![v, paper(3)], january());
    assert!(matches!(w.inward_reverse_charge, Outcome::Unsupported(_)));
    assert!(matches!(w.itc_reverse_charge, Outcome::Unsupported(_)));
    assert_eq!(w.itc_all_other, amounts(180_000, 180_000));
    assert_eq!(w.reverse_charge_purchases, vec![VoucherId(1)]);
}

#[test]
fn an_undated_reverse_charge_purchase_from_the_previous_60_days_is_unsupported() {
    // provenance: derived — s.13(3)(b): the 61st day after an invoice of 15 December 2025 is
    // 14 February 2026, so a payment in January would put the time of supply in January. Without
    // a supply date, January's 3.1(d) cannot be told.
    let v = purchase(
        1,
        date!(2025 - 12 - 15),
        None,
        UNREG,
        vec![line(GTA, 1_000_000)],
        true,
    );
    let w = working(vec![v], january());
    assert!(matches!(w.inward_reverse_charge, Outcome::Unsupported(_)));
}

#[test]
fn a_hired_cab_is_liable_but_its_credit_is_undecided() {
    // provenance: derived — sl.15 (cab with fuel to a body corporate, declared by the mark): 3.1(d)
    // 10,000.00 at 5% = 250.00 + 250.00. Its credit falls under s.17(5)(b) (heading 9966 in the
    // pack), whose exceptions depend on use, so 4(A)(3) is Unsupported while 3.1(d) is known.
    let v = purchase(
        1,
        date!(2026 - 01 - 12),
        Some(date!(2026 - 01 - 12)),
        REG,
        vec![line(CAB, 1_000_000)],
        true,
    );
    let w = working(vec![v], january());
    let Outcome::Value(d) = w.inward_reverse_charge else {
        panic!("3.1(d) should be worked out: {:?}", w.inward_reverse_charge)
    };
    assert_eq!(d.tax, amounts(25_000, 25_000));
    assert!(matches!(w.itc_reverse_charge, Outcome::Unsupported(_)));
}

#[test]
fn an_unregistered_suppliers_goods_outside_every_class_carry_no_tax() {
    // provenance: derived — paper (HSN 482010) is in no class of 13/2017-CT(R) and the supplier is
    // unregistered, so no tax is charged or payable and no table holds it.
    let v = purchase(
        1,
        date!(2026 - 01 - 15),
        Some(date!(2026 - 01 - 15)),
        UNREG,
        vec![line(PAPER, 2_000_000)],
        false,
    );
    let w = working(vec![v], january());
    assert_eq!(
        w.inward_reverse_charge,
        Outcome::Value(InwardReverseCharge::default())
    );
    assert_eq!(w.itc_all_other, Gstr3bTaxAmounts::default());
    assert_eq!(w.reverse_charge_purchases, Vec::<VoucherId>::new());
}
