//! Card M2 tests: the default voucher types seed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeSet;

use ledgerly_model::seed_voucher_types::{affects_books, default_voucher_types};
use ledgerly_model::{BaseVoucherKind as K, NumberingMethod, RestartPeriod, VoucherType};

const ALL: [K; 18] = [
    K::Contra,
    K::Payment,
    K::Receipt,
    K::Journal,
    K::Sales,
    K::Purchase,
    K::CreditNote,
    K::DebitNote,
    K::Memorandum,
    K::ReversingJournal,
    K::ReceiptNote,
    K::DeliveryNote,
    K::RejectionIn,
    K::RejectionOut,
    K::StockJournal,
    K::PhysicalStock,
    K::SalesOrder,
    K::PurchaseOrder,
];

fn of(types: &[VoucherType], kind: K) -> &VoucherType {
    types
        .iter()
        .find(|t| t.base == kind)
        .expect("a default type exists for every kind")
}

#[test]
fn one_type_per_base_kind_with_ids_1_to_18() {
    // provenance: derived - spec section 5 lists 18 v1 base kinds (Contra .. PurchaseOrder);
    // the card asks for one default type per kind; ids are 1-based positions.
    let types = default_voucher_types();
    assert_eq!(types.len(), 18);
    for kind in ALL {
        assert_eq!(
            types.iter().filter(|t| t.base == kind).count(),
            1,
            "{kind:?}"
        );
    }
    let ids: BTreeSet<u64> = types.iter().map(|t| t.id.0).collect();
    assert_eq!(ids, (1..=18).collect::<BTreeSet<u64>>());
}

#[test]
fn names_follow_research_list() {
    // provenance: derived - R1 "Reserved voucher types" reconstructed list names.
    let expected = [
        (K::Contra, "Contra"),
        (K::Payment, "Payment"),
        (K::Receipt, "Receipt"),
        (K::Journal, "Journal"),
        (K::Sales, "Sales"),
        (K::Purchase, "Purchase"),
        (K::CreditNote, "Credit Note"),
        (K::DebitNote, "Debit Note"),
        (K::Memorandum, "Memorandum"),
        (K::ReversingJournal, "Reversing Journal"),
        (K::ReceiptNote, "Receipt Note"),
        (K::DeliveryNote, "Delivery Note"),
        (K::RejectionIn, "Rejection In"),
        (K::RejectionOut, "Rejection Out"),
        (K::StockJournal, "Stock Journal"),
        (K::PhysicalStock, "Physical Stock"),
        (K::SalesOrder, "Sales Order"),
        (K::PurchaseOrder, "Purchase Order"),
    ];
    let types = default_voucher_types();
    for (kind, name) in expected {
        assert_eq!(of(&types, kind).name, name);
    }
}

#[test]
fn names_and_abbreviations_are_unique_and_non_empty() {
    // provenance: derived - a voucher is picked by name or short form, so both must be distinct.
    let types = default_voucher_types();
    let names: BTreeSet<&str> = types.iter().map(|t| t.name.as_str()).collect();
    let abbrs: BTreeSet<&str> = types.iter().map(|t| t.abbreviation.as_str()).collect();
    assert_eq!(names.len(), 18);
    assert_eq!(abbrs.len(), 18);
    assert!(abbrs.iter().all(|a| !a.is_empty()));
}

#[test]
fn sales_and_credit_note_number_automatically_and_never_renumber() {
    // provenance: derived - card M2 text; spec section 5 "Sales-side and GST-document series
    // never renumber"; R1 proposal "sales-side series always retain".
    let types = default_voucher_types();
    for kind in [K::Sales, K::CreditNote] {
        let n = &of(&types, kind).numbering;
        assert_eq!(n.method, NumberingMethod::Automatic, "{kind:?}");
        assert!(n.prevent_renumbering, "{kind:?}");
    }
}

#[test]
fn debit_note_is_a_gst_document_and_never_renumbers() {
    // provenance: derived - a debit note is a GST document (CGST Act s.34), spec section 5
    // "GST-document series never renumber".
    let types = default_voucher_types();
    assert!(of(&types, K::DebitNote).numbering.prevent_renumbering);
}

#[test]
fn internal_types_may_renumber() {
    // provenance: derived - R1 "renumbering allowed only for internal types".
    let types = default_voucher_types();
    for kind in [
        K::Contra,
        K::Payment,
        K::Receipt,
        K::Journal,
        K::StockJournal,
    ] {
        assert!(!of(&types, kind).numbering.prevent_renumbering, "{kind:?}");
    }
}

#[test]
fn every_type_is_active_automatic_unimported_and_restarts_yearly() {
    // provenance: derived - a fresh company must be able to enter every v1 kind; sales-side
    // numbers must be unique per financial year (CGST Rules r.46: "unique for a financial year").
    for t in default_voucher_types() {
        assert!(t.active, "{:?}", t.base);
        assert!(t.tally.is_none(), "{:?}", t.base);
        assert_eq!(
            t.numbering.method,
            NumberingMethod::Automatic,
            "{:?}",
            t.base
        );
        assert_eq!(t.numbering.restart, RestartPeriod::Yearly, "{:?}", t.base);
        assert!(t.numbering.series.is_empty(), "{:?}", t.base);
    }
}

#[test]
fn memorandum_does_not_affect_books() {
    // provenance: derived - spec section 5 invariant 3: "memorandum, reversing ... vouchers never
    // affect base balances".
    assert!(!affects_books(K::Memorandum));
    assert!(!affects_books(K::ReversingJournal));
}

#[test]
fn accounting_kinds_affect_books() {
    // provenance: derived - spec section 5: these post Dr/Cr ledger lines (invariant 1).
    for kind in [
        K::Contra,
        K::Payment,
        K::Receipt,
        K::Journal,
        K::Sales,
        K::Purchase,
        K::CreditNote,
        K::DebitNote,
    ] {
        assert!(affects_books(kind), "{kind:?}");
    }
}

#[test]
fn seed_is_deterministic() {
    // provenance: derived - rule A7: same input, same output.
    assert_eq!(default_voucher_types(), default_voucher_types());
}
