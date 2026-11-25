//! Round s11 tests: the reconciliation rows (`reconcile_2b`) the 2B and IMS screen shows.
//!
//! Provenance. Amounts are hand arithmetic from the mock's own November 2B records
//! (`design/mock-v2/index.html`, `G2B` and `recon()`): Nilgiri Steel Traders NST/1145 is 2B taxable
//! 1,02,500.00 plus IGST 18,450.00 (= 1,20,950.00) against books taxable 1,00,000.00 plus IGST
//! 18,000.00 (= 1,18,000.00): a difference of 2,950.00 = 295000 paise, which the mock's side card
//! states. Kaveri KP/2231 is 64,000.00 + 11,520.00 = 75,520.00 on both sides. Which book voucher
//! kind pairs with which portal document kind is `pending` observed (see `recon2b`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Decimal, GroupId, Gstin, LedgerId, Paise, Rate, SignedAmount, StateCode, VoucherId,
    VoucherTypeId,
};
use ledgerly_gst::recon2b::{ImsAction, MatchLevel, ReconEntry, ReconState, reconcile_2b};
use ledgerly_gst_formats::gstr2b::{
    Gstr2b, InwardDoc, InwardDocKind, InwardItem, ReturnPeriod, SupplierGroup, TaxAmounts,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstLine, GstRegistration,
    Ledger, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails,
    RegistrationType, RestartPeriod, StoredGst, TaxLedgerKind, Voucher, VoucherMode, VoucherStatus,
    VoucherType,
};
use time::Month;
use time::macros::date;

const OURS: &str = "29ABCDE1234F1ZW";
const SUPPLIER_A: &str = "24AAACB1111C1ZT";
const PURCHASE: VoucherTypeId = VoucherTypeId(1);
const PARTY_A: LedgerId = LedgerId(11);
const IGST_LEDGER: LedgerId = LedgerId(20);

/// A defaulted value of the inferred type (the `extra` maps are `serde_json` types this crate does not name).
fn empty<T: Default>() -> T {
    T::default()
}
fn vtype(id: VoucherTypeId, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: format!("{base:?}"),
        base,
        abbreviation: "V".to_owned(),
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

fn party(id: LedgerId, gstin: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: format!("Supplier {}", id.0),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::cr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(Gstin::parse(gstin).unwrap()),
            pan: None,
            state: None,
            address: Address::default(),
        }),
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn books(vouchers: Vec<Voucher>) -> Books {
    let mut masters = Masters::default();
    masters
        .voucher_types
        .insert(PURCHASE, vtype(PURCHASE, BaseVoucherKind::Purchase));
    masters.ledgers.insert(PARTY_A, party(PARTY_A, SUPPLIER_A));
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Test Co".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(29).unwrap(),
            pan: None,
            registrations: vec![GstRegistration {
                gstin: Gstin::parse(OURS).unwrap(),
                state: StateCode::new(29).unwrap(),
                effective_from: date!(2017 - 07 - 01),
                composition: false,
            }],
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features {
                gst: true,
                ..Features::default()
            },
            negative_stock: NegativeStockPolicy::Warn,
        },
        masters,
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn rupees(n: i64) -> Paise {
    Paise::new(n * 100)
}

/// A purchase from `party` with supplier number `reference`, dated `on`, taxable `taxable` rupees
/// and IGST `igst` rupees.
fn purchase(id: u64, reference: &str, on: time::Date, taxable: i64, igst: i64) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: PURCHASE,
        number: Some(format!("P{id}")),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(PARTY_A),
        place_of_supply: None,
        reverse_charge: false,
        reference: Some(reference.to_owned()),
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: Some(StoredGst {
            pack_version: "test".to_owned(),
            lines: vec![GstLine {
                ledger: IGST_LEDGER,
                kind: TaxLedgerKind::Igst,
                taxable: rupees(taxable),
                rate_percent: Rate(Decimal::new(18, 0)),
                tax: rupees(igst),
            }],
            round_off: Paise::ZERO,
        }),
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn portal(number: &str, on: time::Date, taxable: i64, igst: i64, split: bool) -> InwardDoc {
    let items = if split {
        vec![InwardItem {
            num: Some(1),
            rate: Some(Decimal::new(18, 0)),
            amounts: TaxAmounts {
                taxable: rupees(taxable),
                igst: rupees(igst),
                cgst: Paise::ZERO,
                sgst: Paise::ZERO,
                cess: Paise::ZERO,
            },
            extra: empty(),
        }]
    } else {
        vec![]
    };
    InwardDoc {
        kind: InwardDocKind::Invoice,
        number: number.to_owned(),
        date: on,
        value: rupees(taxable + igst),
        invoice_type: None,
        place_of_supply: None,
        reverse_charge: None,
        itc_available: None,
        reason: None,
        diff_percent: None,
        irn: None,
        items,
        extra: empty(),
    }
}

fn statement(docs: Vec<InwardDoc>) -> Gstr2b {
    Gstr2b {
        gstin: OURS.to_owned(),
        return_period: ReturnPeriod {
            year: 2026,
            month: Month::November,
        },
        version: None,
        generated_on: None,
        checksum: None,
        b2b: vec![SupplierGroup {
            ctin: SUPPLIER_A.to_owned(),
            trade_name: None,
            supplier_period: None,
            supplier_filing_date: None,
            docs,
            extra: empty(),
        }],
        cdnr: vec![],
        extra: empty(),
        docdata_extra: empty(),
        root_extra: empty(),
    }
}

fn run(vouchers: Vec<Voucher>, docs: Vec<InwardDoc>) -> Vec<ReconEntry> {
    reconcile_2b(
        &books(vouchers),
        &Gstin::parse(OURS).unwrap(),
        &statement(docs),
        Paise::ZERO,
    )
    .unwrap()
}

#[test]
fn an_equal_invoice_is_matched_exact_with_totals_from_taxable_and_tax() {
    // derived: KP/2231 is 64,000.00 + 11,520.00 = 75,520.00 on both sides, same date.
    let e = run(
        vec![purchase(
            1,
            "KP/2231",
            date!(2026 - 11 - 08),
            64_000,
            11_520,
        )],
        vec![portal(
            "KP/2231",
            date!(2026 - 11 - 08),
            64_000,
            11_520,
            true,
        )],
    );
    assert_eq!(e.len(), 1);
    let e = &e[0];
    assert_eq!(e.state, ReconState::Matched);
    assert_eq!(e.line.level, MatchLevel::Exact);
    assert_eq!(e.line.suggested, ImsAction::Accept);
    assert_eq!(e.line.voucher, Some(VoucherId(1)));
    let books = e.books.unwrap();
    assert_eq!(books.taxable, Some(rupees(64_000)));
    assert_eq!(books.tax, Some(rupees(11_520)));
    assert_eq!(books.total, Some(rupees(75_520)));
    assert_eq!(e.portal.total, Some(rupees(75_520)));
    assert_eq!(e.difference, Some(Paise::ZERO));
}

#[test]
fn the_same_number_with_other_amounts_is_value_differs_not_missing() {
    // derived: NST/1145, 2B 1,02,500.00 + 18,450.00 = 1,20,950.00; books 1,00,000.00 + 18,000.00
    // = 1,18,000.00; 2B less books = 2,950.00 (the mock's side card). The tax gap alone is 450.00,
    // outside a zero tolerance, so the matcher gives no voucher; the row still pairs by number.
    let e = run(
        vec![purchase(
            2,
            "NST/1145",
            date!(2026 - 11 - 20),
            100_000,
            18_000,
        )],
        vec![portal(
            "NST/1145",
            date!(2026 - 11 - 20),
            102_500,
            18_450,
            true,
        )],
    );
    let e = &e[0];
    assert_eq!(e.state, ReconState::ValueDiffers);
    assert_eq!(e.line.voucher, Some(VoucherId(2)));
    assert_eq!(e.line.suggested, ImsAction::Pending);
    assert_eq!(e.portal.total, Some(rupees(120_950)));
    assert_eq!(e.books.unwrap().total, Some(rupees(118_000)));
    assert_eq!(e.difference, Some(rupees(2_950)));
}

#[test]
fn an_invoice_that_is_not_booked_has_no_books_side() {
    // derived: VL/889 is in 2B and nowhere in the books.
    let e = run(
        vec![purchase(
            1,
            "KP/2231",
            date!(2026 - 11 - 08),
            64_000,
            11_520,
        )],
        vec![portal("VL/889", date!(2026 - 11 - 11), 12_000, 2_160, true)],
    );
    let e = &e[0];
    assert_eq!(e.state, ReconState::NotInBooks);
    assert_eq!(e.line.voucher, None);
    assert!(e.books.is_none());
    assert_eq!(e.difference, None);
    assert_eq!(e.line.suggested, ImsAction::Pending);
}

#[test]
fn equal_amounts_on_another_date_are_date_differs() {
    // derived: the amounts agree to the paisa, so the number match is Exact, but the books say 20
    // Nov and 2B says 21 Nov.
    let e = run(
        vec![purchase(
            1,
            "NST/1145",
            date!(2026 - 11 - 20),
            100_000,
            18_000,
        )],
        vec![portal(
            "NST/1145",
            date!(2026 - 11 - 21),
            100_000,
            18_000,
            true,
        )],
    );
    let e = &e[0];
    assert_eq!(e.state, ReconState::DateDiffers);
    assert_eq!(e.line.voucher, Some(VoucherId(1)));
    assert_eq!(e.difference, Some(Paise::ZERO));
}

#[test]
fn two_books_with_one_number_are_a_possible_duplicate() {
    // derived: two purchases carry the supplier number NST/1145 and the amounts fit neither, so
    // no single voucher is the match.
    let e = run(
        vec![
            purchase(1, "NST/1145", date!(2026 - 11 - 20), 100_000, 18_000),
            purchase(2, "NST/1145", date!(2026 - 11 - 21), 90_000, 16_200),
        ],
        vec![portal(
            "NST/1145",
            date!(2026 - 11 - 20),
            102_500,
            18_450,
            true,
        )],
    );
    let e = &e[0];
    assert_eq!(e.state, ReconState::PossibleDuplicate);
    assert_eq!(e.line.voucher, None);
}

#[test]
fn a_document_without_a_tax_split_is_not_compared() {
    // derived: the file carries no item lines, so no taxable value or tax is known. A paired
    // number must not be shown as matched or as differing.
    let e = run(
        vec![purchase(
            1,
            "NST/1145",
            date!(2026 - 11 - 20),
            100_000,
            18_000,
        )],
        vec![portal("NST/1145", date!(2026 - 11 - 20), 0, 0, false)],
    );
    let e = &e[0];
    assert_eq!(e.state, ReconState::NotComparable);
    assert_eq!(e.line.voucher, Some(VoucherId(1)));
    assert_eq!(e.portal.total, None);
    assert_eq!(e.difference, None);
}
