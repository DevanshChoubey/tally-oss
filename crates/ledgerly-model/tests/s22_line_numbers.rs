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

// S22 round 3: amounts in messages are written as the app writes money, with Indian digit
// grouping and two decimals (`ledgerly_core::format::format_inr`, the same text as the app's
// `ui::format::amount`: "1,00,000.00"), never as the debug form `Paise(10000000)`.
// provenance: derived. Rs 1,000 is 1,00,000 paise and Rs 900 is 90,000 paise; the mock writes
// amounts this way (design/mock-v2/index.html, for example "7,27,524.50").

#[test]
fn an_unbalanced_voucher_names_its_totals_as_money() {
    let e = VoucherError::Unbalanced {
        dr: Paise::new(100_000),
        cr: Paise::new(90_000),
    };
    assert_eq!(
        e.to_string(),
        "voucher does not balance: debits 1,000.00, credits 900.00"
    );
}

#[test]
fn a_bill_mismatch_names_its_amounts_as_money() {
    let e = VoucherError::BillAllocationMismatch {
        line: 1,
        expected: Paise::new(12_345_678),
        allocated: Paise::new(10_000_000),
    };
    assert_eq!(
        e.to_string(),
        "bill allocations on line 2 total 1,00,000.00, the line is 1,23,456.78"
    );
}
