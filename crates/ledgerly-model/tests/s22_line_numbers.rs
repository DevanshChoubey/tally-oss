//! S22: messages name a voucher's lines as the user counts them, from 1.
//!
//! provenance: derived. The voucher screen numbers its lines from 1 (the app's own messages, e.g.
//! `u4_voucher_entry` "line 1" for the first line); the error fields stay 0-based indexes for
//! code (`VoucherError::BillAllocationMismatch::line`, `NegativeAmount::line`, "Line index
//! (0-based)"), and only the text shown to people adds one.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::Paise;
use ledgerly_model::VoucherError;

#[test]
fn a_negative_amount_on_the_first_line_says_line_1() {
    let e = VoucherError::NegativeAmount { line: 0 };
    assert_eq!(e.to_string(), "line 1 has a negative amount");
}

#[test]
fn a_bill_mismatch_on_the_first_line_says_line_1() {
    let e = VoucherError::BillAllocationMismatch {
        line: 0,
        expected: Paise::new(100),
        allocated: Paise::new(50),
    };
    assert!(
        e.to_string().starts_with("bill allocations on line 1 "),
        "{e}"
    );
}
