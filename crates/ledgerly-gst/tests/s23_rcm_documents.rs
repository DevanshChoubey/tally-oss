//! S23 tests: the documents a recipient issues under reverse charge (CGST Act s.31(3)(f) and (g))
//! and the time limit of Rule 47A.
//!
//! Provenance: `derived`, from the text read on CBIC's tax information portal on 2026-11-27:
//! * s.31(3)(f): a registered person liable under s.9(3) or s.9(4) "shall, within the period as
//!   may be prescribed, issue an invoice in respect of goods or services or both received by him
//!   from the supplier who is not registered on the date of receipt"; the Explanation counts a
//!   supplier registered only to deduct tax under s.51 as not registered (both inserted by s.122
//!   of the Finance (No. 2) Act, 2024).
//! * s.31(3)(g): the same person "shall issue a payment voucher at the time of making payment to
//!   the supplier" (any supplier, registered or not).
//! * Rule 47A (inserted w.e.f. 01.11.2024 by Notification 20/2024-CT): the s.31(3)(f) invoice is
//!   issued "within a period of thirty days from the date of receipt" of the supply.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::Outcome;
use ledgerly_gst::rcm_documents::{RcmDocuments, SupplierStatus, rcm_documents, self_invoice_due};
use time::macros::date;

#[test]
fn the_self_invoice_is_due_thirty_days_after_receipt() {
    // provenance: derived — Rule 47A: within thirty days from the date of receipt. Received
    // 10 January 2026: 10 Jan + 30 days = 9 February 2026 is the last day.
    assert_eq!(
        self_invoice_due(date!(2026 - 01 - 10)),
        Outcome::Value(date!(2026 - 02 - 09))
    );
    // Across a month end: 15 November 2024 + 30 days = 15 December 2024.
    assert_eq!(
        self_invoice_due(date!(2024 - 11 - 15)),
        Outcome::Value(date!(2024 - 12 - 15))
    );
}

#[test]
fn a_receipt_on_the_day_rule_47a_starts_is_covered() {
    // provenance: derived — Rule 47A w.e.f. 01.11.2024: 1 November 2024 + 30 days = 1 December.
    assert_eq!(
        self_invoice_due(date!(2024 - 11 - 01)),
        Outcome::Value(date!(2024 - 12 - 01))
    );
}

#[test]
fn before_rule_47a_no_time_limit_was_read_so_it_is_unsupported() {
    // provenance: derived — Rule 47A applies from 01.11.2024 and the period in s.31(3)(f) was
    // inserted by the Finance (No. 2) Act, 2024; the earlier text was not read, so a receipt of
    // 31 October 2024 has no known limit.
    assert!(matches!(
        self_invoice_due(date!(2024 - 10 - 31)),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn an_unregistered_supplier_needs_a_self_invoice_and_a_payment_voucher() {
    // provenance: derived — s.31(3)(f) (supplier not registered) and s.31(3)(g) (always).
    let d = rcm_documents(SupplierStatus::Unregistered, date!(2026 - 01 - 10));
    assert_eq!(
        d,
        RcmDocuments {
            self_invoice_due: Some(Outcome::Value(date!(2026 - 02 - 09))),
            payment_voucher: true,
        }
    );
}

#[test]
fn a_supplier_registered_only_to_deduct_tax_counts_as_unregistered() {
    // provenance: derived — the Explanation to s.31(3)(f).
    let d = rcm_documents(SupplierStatus::TaxDeductorOnly, date!(2026 - 01 - 10));
    assert_eq!(
        d.self_invoice_due,
        Some(Outcome::Value(date!(2026 - 02 - 09)))
    );
    assert!(d.payment_voucher);
}

#[test]
fn a_registered_supplier_needs_only_the_payment_voucher() {
    // provenance: derived — s.31(3)(f) covers only suppliers that are not registered; the
    // registered supplier issues its own invoice. s.31(3)(g) still requires a payment voucher.
    let d = rcm_documents(SupplierStatus::Registered, date!(2026 - 01 - 10));
    assert_eq!(
        d,
        RcmDocuments {
            self_invoice_due: None,
            payment_voucher: true,
        }
    );
}
