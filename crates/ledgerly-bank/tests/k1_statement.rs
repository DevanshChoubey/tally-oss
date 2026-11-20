//! Card K1: statement model and balance check (spec 6.5).
//!
//! Balance convention (from `ledgerly_bankfmt::RawLine`): balances are from the account holder's
//! view, credit balance positive; running balance after a line = previous + credit - debit.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use ledgerly_bank::statement::{StatementError, check_statement};
use ledgerly_bankfmt::{RawLine, RawStatement};
use ledgerly_core::{Date, LedgerId, MoneyError, Month, Paise};
use proptest::prelude::*;

const LEDGER: LedgerId = LedgerId(7);

// provenance: derived - FIPS 180-2 Appendix B.1 published SHA-256 test vector for the message
// "abc" (3 bytes).
const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
// provenance: derived - the published SHA-256 value of the empty message (FIPS 180-4 examples).
const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn d(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::April, day).unwrap()
}

fn line(row: u32, day: u8, debit: i64, credit: i64, balance: Option<i64>) -> RawLine {
    RawLine {
        row,
        date: d(day),
        value_date: None,
        description: format!("fictional line {row}"),
        reference: None,
        debit: Paise::new(debit),
        credit: Paise::new(credit),
        balance: balance.map(Paise::new),
        raw: vec![format!("cell{row}")],
    }
}

fn raw(opening: Option<i64>, closing: Option<i64>, lines: Vec<RawLine>) -> RawStatement {
    RawStatement {
        format_id: "fictional-csv".to_string(),
        account_hint: Some("XXXX0001".to_string()),
        opening: opening.map(Paise::new),
        closing: closing.map(Paise::new),
        lines,
    }
}

// Opening 1_000_000 paise (Rs 10,000.00), rows numbered 5, 6, 7 on purpose (row numbers are the
// file's, not indices).
//
// provenance: derived - hand arithmetic, no engine output:
//   row 5 debit  250_050 (Rs 2,500.50): 1_000_000 - 250_050 = 749_950
//   row 6 credit 120_000 (Rs 1,200.00):   749_950 + 120_000 = 869_950
//   row 7 debit    9_999 (Rs    99.99):   869_950 -   9_999 = 859_951
// so closing = 859_951 (Rs 8,599.51); Σ credit - Σ debit = 120_000 - 260_049 = -140_049 and
// 1_000_000 - 140_049 = 859_951.
fn good_lines() -> Vec<RawLine> {
    vec![
        line(5, 1, 250_050, 0, Some(749_950)),
        line(6, 2, 0, 120_000, Some(869_950)),
        line(7, 3, 9_999, 0, Some(859_951)),
    ]
}

#[test]
fn consistent_statement_is_accepted() {
    let raw = raw(Some(1_000_000), Some(859_951), good_lines());
    let stmt = check_statement(raw, b"abc", LEDGER, &[]).unwrap();
    assert_eq!(stmt.ledger(), LEDGER);
    assert_eq!(stmt.opening(), Paise::new(1_000_000));
    assert_eq!(stmt.closing(), Paise::new(859_951));
    assert_eq!(stmt.lines().len(), 3);
    assert_eq!(stmt.file_hash(), SHA256_ABC);
    // Period is the first and last transaction dates: 1 and 3 April 2025 (read off the inputs).
    assert_eq!(stmt.period(), Some((d(1), d(3))));
}

#[test]
fn opening_plus_lines_must_equal_closing_else_balance_mismatch() {
    // Same lines, no running-balance column, stated closing one paisa off.
    // provenance: derived - 1_000_000 + 120_000 - 250_050 - 9_999 = 859_951, stated 859_952.
    let lines = vec![
        line(5, 1, 250_050, 0, None),
        line(6, 2, 0, 120_000, None),
        line(7, 3, 9_999, 0, None),
    ];
    let err = check_statement(
        raw(Some(1_000_000), Some(859_952), lines),
        b"abc",
        LEDGER,
        &[],
    )
    .unwrap_err();
    assert_eq!(
        err,
        StatementError::BalanceMismatch {
            opening: Paise::new(1_000_000),
            lines_net: Paise::new(-140_049),
            computed_closing: Paise::new(859_951),
            stated_closing: Paise::new(859_952),
        }
    );
}

#[test]
fn running_balance_break_names_first_bad_row_and_stores_nothing() {
    // Rows 6 and 7 both carry a wrong running balance; row 6 is the first.
    // provenance: derived - after row 5 the balance is 749_950; row 6 credit 120_000 must give
    // 869_950, the file says 869_951 (found). The error is reported for file row 6, not index 1.
    let lines = vec![
        line(5, 1, 250_050, 0, Some(749_950)),
        line(6, 2, 0, 120_000, Some(869_951)),
        line(7, 3, 9_999, 0, Some(859_000)),
    ];
    let result = check_statement(
        raw(Some(1_000_000), Some(859_951), lines),
        b"abc",
        LEDGER,
        &[],
    );
    // All-or-nothing: the Err carries no statement and no partially accepted rows.
    assert_eq!(
        result.unwrap_err(),
        StatementError::RunningBalance {
            row: 6,
            expected: Paise::new(869_950),
            found: Paise::new(869_951),
        }
    );
}

#[test]
fn running_balance_is_checked_against_opening_for_first_row() {
    // provenance: derived - opening 1_000_000, row 5 debit 250_050 gives 749_950; file says 750_000.
    let lines = vec![line(5, 1, 250_050, 0, Some(750_000))];
    let err = check_statement(
        raw(Some(1_000_000), Some(750_000), lines),
        b"abc",
        LEDGER,
        &[],
    )
    .unwrap_err();
    assert_eq!(
        err,
        StatementError::RunningBalance {
            row: 5,
            expected: Paise::new(749_950),
            found: Paise::new(750_000),
        }
    );
}

#[test]
fn rows_without_a_balance_do_not_hide_a_later_break() {
    // provenance: derived - 100_000 + 50_000 (row 1, no balance) = 150_000; row 2 debit 10_000
    // gives 140_000; the file says 141_000.
    let lines = vec![
        line(1, 1, 0, 50_000, None),
        line(2, 2, 10_000, 0, Some(141_000)),
    ];
    let err = check_statement(
        raw(Some(100_000), Some(141_000), lines),
        b"abc",
        LEDGER,
        &[],
    )
    .unwrap_err();
    assert_eq!(
        err,
        StatementError::RunningBalance {
            row: 2,
            expected: Paise::new(140_000),
            found: Paise::new(141_000),
        }
    );
}

#[test]
fn duplicate_file_hash_is_rejected() {
    let known = vec!["0000".to_string(), SHA256_ABC.to_string()];
    let err = check_statement(
        raw(Some(1_000_000), Some(859_951), good_lines()),
        b"abc",
        LEDGER,
        &known,
    )
    .unwrap_err();
    assert_eq!(
        err,
        StatementError::Duplicate {
            file_hash: SHA256_ABC.to_string()
        }
    );
}

#[test]
fn duplicate_check_ignores_hex_case() {
    let known = vec![SHA256_ABC.to_uppercase()];
    let err = check_statement(raw(Some(0), Some(0), vec![]), b"abc", LEDGER, &known).unwrap_err();
    assert_eq!(
        err,
        StatementError::Duplicate {
            file_hash: SHA256_ABC.to_string()
        }
    );
}

#[test]
fn different_hash_is_not_a_duplicate_and_empty_file_hash_is_the_known_vector() {
    let known = vec![SHA256_ABC.to_string()];
    let stmt = check_statement(raw(Some(5), Some(5), vec![]), b"", LEDGER, &known).unwrap();
    assert_eq!(stmt.file_hash(), SHA256_EMPTY);
    assert_eq!(stmt.period(), None);
}

#[test]
fn missing_opening_or_closing_cannot_be_verified_and_is_rejected() {
    let e1 = check_statement(raw(None, Some(1), vec![]), b"abc", LEDGER, &[]).unwrap_err();
    assert_eq!(e1, StatementError::MissingBalance { which: "opening" });
    let e2 = check_statement(raw(Some(1), None, vec![]), b"abc", LEDGER, &[]).unwrap_err();
    assert_eq!(e2, StatementError::MissingBalance { which: "closing" });
}

#[test]
fn negative_amounts_and_two_sided_lines_are_rejected() {
    let neg = vec![line(3, 1, -5, 0, None)];
    let e = check_statement(raw(Some(0), Some(5), neg), b"abc", LEDGER, &[]).unwrap_err();
    assert!(
        matches!(e, StatementError::InvalidLine { row: 3, .. }),
        "{e:?}"
    );
    let both = vec![line(4, 1, 5, 5, None)];
    let e = check_statement(raw(Some(0), Some(0), both), b"abc", LEDGER, &[]).unwrap_err();
    assert!(
        matches!(e, StatementError::InvalidLine { row: 4, .. }),
        "{e:?}"
    );
}

#[test]
fn sum_overflow_is_an_error_not_a_wrap() {
    // provenance: derived - i64::MAX + 1 does not fit in i64.
    let lines = vec![line(1, 1, 0, 1, None)];
    let e = check_statement(
        raw(Some(i64::MAX), Some(i64::MAX), lines),
        b"abc",
        LEDGER,
        &[],
    )
    .unwrap_err();
    assert_eq!(e, StatementError::Money(MoneyError::Overflow));
}

proptest! {
    // Invariant (spec 6.5): a statement built so that opening + Σ(credit - debit) = closing, with
    // each running balance equal to the cumulative sum, is accepted; moving the stated closing by
    // any non-zero amount is rejected as BalanceMismatch.
    #[test]
    fn consistent_accepted_and_perturbed_closing_rejected(
        opening in -1_000_000_000i64..1_000_000_000,
        moves in proptest::collection::vec((any::<bool>(), 0i64..10_000_000), 0..20),
        delta in prop_oneof![-1_000i64..0, 1i64..1_000],
    ) {
        let mut running = opening;
        let mut lines = Vec::new();
        for (i, (is_credit, amt)) in moves.iter().enumerate() {
            let (debit, credit) = if *is_credit { (0, *amt) } else { (*amt, 0) };
            running = running + credit - debit;
            lines.push(line(u32::try_from(i).unwrap() + 1, 1, debit, credit, Some(running)));
        }
        let ok = check_statement(raw(Some(opening), Some(running), lines.clone()), b"x", LEDGER, &[]);
        prop_assert!(ok.is_ok());
        let no_balances: Vec<RawLine> =
            lines.into_iter().map(|mut l| { l.balance = None; l }).collect();
        let bad = check_statement(
            raw(Some(opening), Some(running + delta), no_balances), b"x", LEDGER, &[]);
        let is_mismatch = matches!(bad, Err(StatementError::BalanceMismatch { .. }));
        prop_assert!(is_mismatch);
    }
}
