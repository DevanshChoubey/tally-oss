//! Card C3: a statement's stated end date and rebuilding a stored statement.
//!
//! Provenance `derived`: every expected value is worked out by hand from the figures written in
//! each test. The end-date rule (a file that states its end date lists every transaction up to
//! and including that day) is our rule; TallyPrime's Bank Reconciliation page
//! <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! (accessed 2026-11-21) does not mention a statement end date, so that rule is `pending`
//! confirmation against Tally.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_bank::statement::{
    StatementError, check_statement, check_statement_with_end, restore_statement,
};
use ledgerly_bankfmt::{RawLine, RawStatement};
use ledgerly_core::{Date, LedgerId, Paise};
use time::macros::date;

const BANK: LedgerId = LedgerId(10);

fn credit(row: u32, on: Date, paise: i64) -> RawLine {
    RawLine {
        row,
        date: on,
        value_date: None,
        description: "FICTIONAL CREDIT".to_owned(),
        reference: None,
        debit: Paise::ZERO,
        credit: Paise::new(paise),
        balance: None,
        raw: Vec::new(),
    }
}

/// Opening 1000, one credit of 500 on 5 Apr, closing 1500 (all in paise x 100 below).
fn raw() -> RawStatement {
    RawStatement {
        format_id: "fictional".to_owned(),
        account_hint: None,
        opening: Some(Paise::new(100_000)),
        closing: Some(Paise::new(150_000)),
        lines: vec![credit(2, date!(2026 - 04 - 05), 50_000)],
    }
}

#[test]
fn check_statement_has_no_end_date() {
    let st = check_statement(raw(), b"a", BANK, &[]).unwrap();
    assert_eq!(st.end_date(), None);
}

#[test]
fn a_stated_end_date_is_kept() {
    let st = check_statement_with_end(raw(), Some(date!(2026 - 04 - 30)), b"a", BANK, &[]).unwrap();
    assert_eq!(st.end_date(), Some(date!(2026 - 04 - 30)));
}

#[test]
fn an_end_date_equal_to_the_last_line_is_accepted() {
    let st = check_statement_with_end(raw(), Some(date!(2026 - 04 - 05)), b"a", BANK, &[]).unwrap();
    assert_eq!(st.end_date(), Some(date!(2026 - 04 - 05)));
}

#[test]
fn an_end_date_before_the_last_line_is_refused() {
    let err = check_statement_with_end(raw(), Some(date!(2026 - 04 - 04)), b"a", BANK, &[])
        .unwrap_err();
    assert_eq!(
        err,
        StatementError::EndBeforeLastLine {
            end: date!(2026 - 04 - 04),
            last_line: date!(2026 - 04 - 05),
        }
    );
}

#[test]
fn a_restored_statement_equals_the_checked_one() {
    let original =
        check_statement_with_end(raw(), Some(date!(2026 - 04 - 30)), b"a", BANK, &[]).unwrap();
    let restored = restore_statement(
        raw(),
        Some(date!(2026 - 04 - 30)),
        original.file_hash().to_owned(),
        BANK,
    )
    .unwrap();
    assert_eq!(restored, original);
}

#[test]
fn a_stored_statement_that_does_not_balance_is_refused_on_restore() {
    let mut bad = raw();
    // derived: 1000 + 500 = 1500, but this says 1600.
    bad.closing = Some(Paise::new(160_000));
    let err = restore_statement(bad, None, "00".to_owned(), BANK).unwrap_err();
    assert!(matches!(err, StatementError::BalanceMismatch { .. }), "{err:?}");
}
