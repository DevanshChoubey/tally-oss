//! Card G11 tests: GSTR-2B reconciliation against the books (spec 6.3).
//!
//! Provenance. The matching levels are the product policy of spec 6.3 ("exact, then fuzzy invoice
//! number normalisation, then amount/date tolerance, suggest Accept/Reject/Pending, the user
//! confirms every action"); they are not statute. The mapping of book voucher kinds to portal
//! document kinds is `pending` observed. Amounts are hand arithmetic written next to each case:
//! taxable 10,000.00 at 18 percent inter-State is IGST 1,800.00 = 180000 paise (IGST Act s.5);
//! none is an output of the engine.

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
use ledgerly_gst::GstError;
use ledgerly_gst::recon2b::{ImsAction, MatchLevel, ReconLine, match_2b, portal_documents};
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
const SUPPLIER_B: &str = "07AAACL0000A1ZP";
const PURCHASE: VoucherTypeId = VoucherTypeId(1);
const DEBIT_NOTE: VoucherTypeId = VoucherTypeId(2);
const PARTY_A: LedgerId = LedgerId(11);
const PARTY_B: LedgerId = LedgerId(12);
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
    masters
        .voucher_types
        .insert(DEBIT_NOTE, vtype(DEBIT_NOTE, BaseVoucherKind::DebitNote));
    masters.ledgers.insert(PARTY_A, party(PARTY_A, SUPPLIER_A));
    masters.ledgers.insert(PARTY_B, party(PARTY_B, SUPPLIER_B));
    Books {
        company: Company {
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

/// A regular purchase voucher from `party` carrying supplier number `reference` and IGST `igst`
/// paise.
fn purchase(id: u64, party: LedgerId, reference: &str, igst: i64) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: PURCHASE,
        number: Some(format!("P{id}")),
        series: None,
        date: date!(2026 - 10 - 20),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: Some(party),
        place_of_supply: None,
        reverse_charge: false,
        reference: Some(reference.to_owned()),
        reference_date: Some(date!(2026 - 10 - 15)),
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: Some(StoredGst {
            pack_version: "test".to_owned(),
            lines: vec![GstLine {
                ledger: IGST_LEDGER,
                kind: TaxLedgerKind::Igst,
                taxable: Paise::new(1_000_000),
                rate_percent: Rate(Decimal::new(18, 0)),
                tax: Paise::new(igst),
            }],
            round_off: Paise::ZERO,
        }),
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn item(igst: i64) -> InwardItem {
    InwardItem {
        num: Some(1),
        rate: Some(Decimal::new(18, 0)),
        amounts: TaxAmounts {
            taxable: Paise::new(1_000_000),
            igst: Paise::new(igst),
            cgst: Paise::ZERO,
            sgst: Paise::ZERO,
            cess: Paise::ZERO,
        },
        extra: empty(),
    }
}

fn doc(kind: InwardDocKind, number: &str, igst: i64) -> InwardDoc {
    InwardDoc {
        kind,
        number: number.to_owned(),
        date: date!(2026 - 10 - 15),
        value: Paise::new(1_180_000),
        invoice_type: None,
        place_of_supply: None,
        reverse_charge: None,
        itc_available: None,
        reason: None,
        diff_percent: None,
        irn: None,
        items: vec![item(igst)],
        extra: empty(),
    }
}

fn group(ctin: &str, docs: Vec<InwardDoc>) -> SupplierGroup<InwardDoc> {
    SupplierGroup {
        ctin: ctin.to_owned(),
        trade_name: None,
        supplier_period: None,
        supplier_filing_date: None,
        docs,
        extra: empty(),
    }
}

fn statement(b2b: Vec<SupplierGroup<InwardDoc>>, cdnr: Vec<SupplierGroup<InwardDoc>>) -> Gstr2b {
    Gstr2b {
        gstin: OURS.to_owned(),
        return_period: ReturnPeriod {
            year: 2026,
            month: Month::October,
        },
        version: None,
        generated_on: None,
        checksum: None,
        b2b,
        cdnr,
        extra: empty(),
        docdata_extra: empty(),
        root_extra: empty(),
    }
}

fn run(books: &Books, data: &Gstr2b, tolerance: i64) -> Vec<ReconLine> {
    match_2b(
        books,
        &Gstin::parse(OURS).unwrap(),
        data,
        Paise::new(tolerance),
    )
    .unwrap()
}

fn one_invoice(
    book_ref: &str,
    book_igst: i64,
    portal_no: &str,
    portal_igst: i64,
    tol: i64,
) -> ReconLine {
    let b = books(vec![purchase(1, PARTY_A, book_ref, book_igst)]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, portal_no, portal_igst)],
        )],
        vec![],
    );
    let lines = run(&b, &d, tol);
    assert_eq!(lines.len(), 1);
    lines.into_iter().next().unwrap()
}

/// derived: same supplier, number `INV-001` byte for byte, tax 180000 paise on both sides
/// (10,000.00 x 18 percent = 1,800.00): no difference, so the strictest level, `Exact`, and the
/// product policy suggests Accept, unconfirmed.
#[test]
fn identical_invoice_is_exact_and_suggests_accept() {
    let l = one_invoice("INV-001", 180_000, "INV-001", 180_000, 100);
    assert_eq!(l.level, MatchLevel::Exact);
    assert_eq!(l.suggested, ImsAction::Accept);
    assert_eq!(l.voucher, Some(VoucherId(1)));
    assert_eq!(l.portal_index, 0);
    assert!(!l.confirmed);
}

/// derived: spec 6.3 fuzzy number normalisation. `INV-001` and `INV/001` both reduce to `INV001`
/// (non-alphanumerics dropped); they differ as written, so not `Exact`. Tax is equal.
#[test]
fn invoice_number_differing_in_separator_is_normalised() {
    let l = one_invoice("INV/001", 180_000, "INV-001", 180_000, 100);
    assert_eq!(l.level, MatchLevel::Normalised);
    assert_eq!(l.suggested, ImsAction::Accept);
    assert_eq!(l.voucher, Some(VoucherId(1)));
}

/// derived: case and a space also normalise: `inv 001` -> `INV001`.
#[test]
fn case_and_spaces_normalise() {
    let l = one_invoice("inv 001", 180_000, "INV-001", 180_000, 100);
    assert_eq!(l.level, MatchLevel::Normalised);
}

/// derived: numbers identical; books tax 180000 paise, portal 180100 paise: a difference of 100
/// paise = Rs 1.00, within a tolerance of 100 paise. Not equal, so not `Exact`: `Tolerance`, and
/// the user must look at it, so Pending.
#[test]
fn one_rupee_difference_is_tolerance_not_exact() {
    let l = one_invoice("INV-001", 180_000, "INV-001", 180_100, 100);
    assert_eq!(l.level, MatchLevel::Tolerance);
    assert_eq!(l.suggested, ImsAction::Pending);
    assert_eq!(l.voucher, Some(VoucherId(1)));
    assert!(!l.confirmed);
}

/// derived: the tolerance is inclusive. |180000 - 180100| = 100 paise <= 100.
/// A difference of 101 paise (180101) exceeds it and the pair is not matched.
#[test]
fn tolerance_boundary_is_inclusive_and_beyond_is_unmatched() {
    let at = one_invoice("INV-001", 180_000, "INV-001", 180_100, 100);
    assert_eq!(at.level, MatchLevel::Tolerance);
    let over = one_invoice("INV-001", 180_000, "INV-001", 180_101, 100);
    assert_eq!(over.level, MatchLevel::Unmatched);
    assert_eq!(over.voucher, None);
    assert_eq!(over.suggested, ImsAction::Pending);
}

/// derived: tolerance 0 means only equal tax matches; 1 paise apart is unmatched.
#[test]
fn zero_tolerance_requires_equal_tax() {
    let l = one_invoice("INV-001", 180_000, "INV-001", 180_001, 0);
    assert_eq!(l.level, MatchLevel::Unmatched);
}

/// derived: the portal number is entirely different (`X77` vs `INV-001`) but the dates are equal
/// (books reference date 2026-10-15, portal date 2026-10-15) and tax is 1 paise apart, within a
/// tolerance of 100: date tolerance, `Tolerance`, Pending.
#[test]
fn different_number_same_date_within_tolerance_is_tolerance() {
    let l = one_invoice("INV-001", 180_000, "X77", 180_001, 100);
    assert_eq!(l.level, MatchLevel::Tolerance);
    assert_eq!(l.suggested, ImsAction::Pending);
}

/// derived: number `X77` vs `INV-001` does not match even normalised, and the portal date
/// 2026-10-16 differs from the books reference date 2026-10-15, so neither rule holds: Unmatched.
#[test]
fn different_number_and_date_is_unmatched() {
    let b = books(vec![purchase(1, PARTY_A, "INV-001", 180_000)]);
    let mut d0 = doc(InwardDocKind::Invoice, "X77", 180_000);
    d0.date = date!(2026 - 10 - 16);
    let d = statement(vec![group(SUPPLIER_A, vec![d0])], vec![]);
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[0].voucher, None);
}

/// derived: a document that is nowhere in the books is `Unmatched` and left Pending (rejecting
/// is the user's call).
#[test]
fn invoice_missing_from_books_is_unmatched_pending() {
    let b = books(vec![]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[0].suggested, ImsAction::Pending);
    assert_eq!(l[0].voucher, None);
}

/// derived: the same number from a different supplier GSTIN is a different document.
#[test]
fn same_number_other_supplier_is_not_a_match() {
    let b = books(vec![purchase(1, PARTY_B, "INV-001", 180_000)]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![],
    );
    assert_eq!(run(&b, &d, 100)[0].level, MatchLevel::Unmatched);
}

/// derived: a supplier credit note not yet booked stays Pending (spec 6.3: credit notes may stay
/// pending one period), and portal indexes run b2b first then cdnr.
#[test]
fn unbooked_credit_note_stays_pending() {
    let b = books(vec![purchase(1, PARTY_A, "INV-001", 180_000)]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::CreditNote, "CN-9", 18_000)],
        )],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].portal_index, 0);
    assert_eq!(l[0].level, MatchLevel::Exact);
    assert_eq!(l[1].portal_index, 1);
    assert_eq!(l[1].level, MatchLevel::Unmatched);
    assert_eq!(l[1].suggested, ImsAction::Pending);
    assert_eq!(l[1].voucher, None);
}

/// derived: a supplier credit note booked as a debit note voucher with the same number and
/// tax (1,000.00 x 18 percent = 180.00 = 18000 paise) matches exactly. An invoice-numbered
/// purchase voucher does not pair with a credit note.
#[test]
fn booked_credit_note_matches_debit_note_voucher() {
    let mut dn = purchase(2, PARTY_A, "CN-9", 18_000);
    dn.type_id = DEBIT_NOTE;
    let b = books(vec![dn, purchase(3, PARTY_A, "CN-9", 18_000)]);
    let d = statement(
        vec![],
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::CreditNote, "CN-9", 18_000)],
        )],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Exact);
    assert_eq!(l[0].voucher, Some(VoucherId(2)));
}

/// derived: two book vouchers equally fit one portal document, so no match is claimed.
#[test]
fn two_equal_candidates_are_ambiguous() {
    let b = books(vec![
        purchase(1, PARTY_A, "INV-001", 180_000),
        purchase(2, PARTY_A, "INV-001", 180_000),
    ]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[0].voucher, None);
    assert_eq!(l[0].suggested, ImsAction::Pending);
}

/// derived: one book voucher (`INV/001`) fits two portal documents (`INV-001`, `inv-001`) equally
/// at the normalised level, so it is matched to neither.
#[test]
fn one_voucher_claimed_twice_is_matched_to_neither() {
    let b = books(vec![purchase(1, PARTY_A, "INV/001", 180_000)]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![
                doc(InwardDocKind::Invoice, "INV-001", 180_000),
                doc(InwardDocKind::Invoice, "inv-001", 180_000),
            ],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[1].level, MatchLevel::Unmatched);
}

/// derived: the exact pairing is made first, so a second invoice with a normalised-only number
/// takes the other voucher: INV-001 (exact, voucher 1) and INV/002 vs `INV-002` (normalised,
/// voucher 2).
#[test]
fn several_documents_pair_independently() {
    let b = books(vec![
        purchase(1, PARTY_A, "INV-001", 180_000),
        purchase(2, PARTY_A, "INV-002", 90_000),
    ]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![
                doc(InwardDocKind::Invoice, "INV/002", 90_000),
                doc(InwardDocKind::Invoice, "INV-001", 180_000),
            ],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Normalised);
    assert_eq!(l[0].voucher, Some(VoucherId(2)));
    assert_eq!(l[1].level, MatchLevel::Exact);
    assert_eq!(l[1].voucher, Some(VoucherId(1)));
}

/// derived: nothing is auto-accepted: across every level every line has `confirmed == false`.
#[test]
fn nothing_is_confirmed() {
    let b = books(vec![
        purchase(1, PARTY_A, "INV-001", 180_000),
        purchase(2, PARTY_A, "INV-002", 90_000),
        purchase(3, PARTY_B, "B-1", 10_000),
    ]);
    let d = statement(
        vec![
            group(
                SUPPLIER_A,
                vec![
                    doc(InwardDocKind::Invoice, "INV-001", 180_000),
                    doc(InwardDocKind::Invoice, "INV/002", 90_050),
                    doc(InwardDocKind::Invoice, "INV-003", 5),
                ],
            ),
            group(SUPPLIER_B, vec![doc(InwardDocKind::Invoice, "B-1", 10_000)]),
        ],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l.len(), 4);
    assert!(l.iter().all(|x| !x.confirmed));
    assert_eq!(l[0].level, MatchLevel::Exact);
    assert_eq!(l[1].level, MatchLevel::Tolerance);
    assert_eq!(l[2].level, MatchLevel::Unmatched);
    assert_eq!(l[3].level, MatchLevel::Exact);
}

/// derived: the supplier reported a document whose number matches a voucher the user cancelled:
/// the suggestion is Reject (still unconfirmed), level Unmatched, carrying the cancelled voucher.
#[test]
fn document_matching_a_cancelled_voucher_suggests_reject() {
    let mut v = purchase(1, PARTY_A, "INV-001", 180_000);
    v.status = VoucherStatus::Cancelled;
    let b = books(vec![v]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[0].suggested, ImsAction::Reject);
    assert_eq!(l[0].voucher, Some(VoucherId(1)));
    assert!(!l[0].confirmed);
}

/// derived: optional and post-dated vouchers are not in the books, so they do not match.
#[test]
fn optional_and_postdated_vouchers_do_not_match() {
    let mut a = purchase(1, PARTY_A, "INV-001", 180_000);
    a.status = VoucherStatus::Optional;
    let mut p = purchase(2, PARTY_A, "INV-002", 180_000);
    p.status = VoucherStatus::PostDated;
    let b = books(vec![a, p]);
    let d = statement(
        vec![group(
            SUPPLIER_A,
            vec![
                doc(InwardDocKind::Invoice, "INV-001", 180_000),
                doc(InwardDocKind::Invoice, "INV-002", 180_000),
            ],
        )],
        vec![],
    );
    let l = run(&b, &d, 100);
    assert_eq!(l[0].level, MatchLevel::Unmatched);
    assert_eq!(l[1].level, MatchLevel::Unmatched);
}

/// derived: with no tax split on either side amounts are not comparable, so a number match alone
/// is never reported as a match (rule: never a wrong number).
#[test]
fn missing_tax_split_is_never_matched() {
    let b = books(vec![purchase(1, PARTY_A, "INV-001", 180_000)]);
    let mut portal = doc(InwardDocKind::Invoice, "INV-001", 180_000);
    portal.items.clear();
    let d = statement(vec![group(SUPPLIER_A, vec![portal])], vec![]);
    assert_eq!(run(&b, &d, 100)[0].level, MatchLevel::Unmatched);

    let mut v = purchase(1, PARTY_A, "INV-001", 180_000);
    v.gst = None;
    let b2 = books(vec![v]);
    let d2 = statement(
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::Invoice, "INV-001", 180_000)],
        )],
        vec![],
    );
    assert_eq!(run(&b2, &d2, 100)[0].level, MatchLevel::Unmatched);
}

/// derived: tax is the sum of heads: books CGST 90000 + SGST 90000 = 180000 paise (9 percent
/// each of 10,000.00) against portal CGST 90000 + SGST 90000 is Exact.
#[test]
fn split_heads_are_summed() {
    let mut v = purchase(1, PARTY_A, "INV-001", 0);
    v.gst = Some(StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![
            GstLine {
                ledger: LedgerId(21),
                kind: TaxLedgerKind::Cgst,
                taxable: Paise::new(1_000_000),
                rate_percent: Rate(Decimal::new(9, 0)),
                tax: Paise::new(90_000),
            },
            GstLine {
                ledger: LedgerId(22),
                kind: TaxLedgerKind::Sgst,
                taxable: Paise::new(1_000_000),
                rate_percent: Rate(Decimal::new(9, 0)),
                tax: Paise::new(90_000),
            },
        ],
        round_off: Paise::ZERO,
    });
    let b = books(vec![v]);
    let mut portal = doc(InwardDocKind::Invoice, "INV-001", 0);
    portal.items = vec![InwardItem {
        num: Some(1),
        rate: Some(Decimal::new(18, 0)),
        amounts: TaxAmounts {
            taxable: Paise::new(1_000_000),
            igst: Paise::ZERO,
            cgst: Paise::new(90_000),
            sgst: Paise::new(90_000),
            cess: Paise::ZERO,
        },
        extra: empty(),
    }];
    let d = statement(vec![group(SUPPLIER_A, vec![portal])], vec![]);
    assert_eq!(run(&b, &d, 0)[0].level, MatchLevel::Exact);
}

/// derived: a statement for another GSTIN is refused, and so is a negative tolerance.
#[test]
fn wrong_gstin_and_negative_tolerance_are_errors() {
    let b = books(vec![]);
    let d = statement(vec![], vec![]);
    let other = Gstin::parse(SUPPLIER_A).unwrap();
    assert!(matches!(
        match_2b(&b, &other, &d, Paise::ZERO),
        Err(GstError::Invalid { .. })
    ));
    let ours = Gstin::parse(OURS).unwrap();
    assert!(matches!(
        match_2b(&b, &ours, &d, Paise::new(-1)),
        Err(GstError::Invalid { .. })
    ));
}

/// derived: `portal_documents` lists b2b documents then cdnr documents in file order.
#[test]
fn portal_documents_order_is_b2b_then_cdnr() {
    let d = statement(
        vec![
            group(SUPPLIER_A, vec![doc(InwardDocKind::Invoice, "A1", 1)]),
            group(SUPPLIER_B, vec![doc(InwardDocKind::Invoice, "B1", 1)]),
        ],
        vec![group(
            SUPPLIER_A,
            vec![doc(InwardDocKind::CreditNote, "C1", 1)],
        )],
    );
    let numbers: Vec<&str> = portal_documents(&d)
        .iter()
        .map(|(_, x)| x.number.as_str())
        .collect();
    assert_eq!(numbers, vec!["A1", "B1", "C1"]);
}
