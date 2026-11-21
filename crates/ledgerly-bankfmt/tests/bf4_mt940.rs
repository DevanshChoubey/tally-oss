//! Card BF4 tests. Every expected value is `derived`: worked out by hand from the SWIFT MT940
//! field layout (`:60F:`/`:62F:` = mark, YYMMDD, currency, amount with decimal comma; `:61:` =
//! value date YYMMDD, optional entry date MMDD, mark D/C/RD/RC, amount, type, references) and from
//! the sample text below. None comes from running the parser. The sample is fictional.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]
use ledgerly_bankfmt::BankFmtError;
use ledgerly_bankfmt::mt940::{parse_mt940, verify_balances};
use ledgerly_core::{Date, Month, Paise};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

// File line numbers (1-based): envelope 1, :20: 2, :25: 3, :28C: 4, :60F: 5, :61: 6, :86: 7-8,
// :61: 9, :86: 10, :61: 11, :62F: 12, "-}" 13.
// Closing, derived: opening 100000,00 = 10_000_000 paise
//   - 2500,50 (250_050)  + 12345,6 = 12345,60 (1_234_560)  - 100,00 (10_000)
//   = 10_000_000 - 250_050 = 9_749_950; + 1_234_560 = 10_984_510; - 10_000 = 10_974_510
//   = 109745,10 rupees.
const SAMPLE: &str = "{1:F01TESTINBBAXXX0000000000}{2:O9400000000000TESTINBBXXXX00000000000000000000N}{4:\r\n\
:20:STMT0001\r\n\
:25:0123456789\r\n\
:28C:00001/001\r\n\
:60F:C240401INR100000,00\r\n\
:61:2404050405D2500,50NTRFNONREF//BK123\r\n\
:86:UPI/RENT PAYMENT\r\n\
APRIL\r\n\
:61:240406C12345,6NCHK000123\r\n\
:86:CHEQUE DEPOSIT\r\n\
:61:2404300430RC100,00NMSC1234//BK9\r\n\
:62F:C240430INR109745,10\r\n\
-}\r\n";

#[test]
fn sample_header_and_balances_parsed() {
    let st = parse_mt940(SAMPLE).unwrap();
    // derived: the id of formats/generic-mt940.toml, the registry entry this parser serves.
    assert_eq!(st.format_id, "generic-mt940");
    assert_eq!(st.account_hint.as_deref(), Some("0123456789"));
    // derived: C240401INR100000,00 = credit 100000 rupees = 10_000_000 paise.
    assert_eq!(st.opening, Some(Paise::new(10_000_000)));
    // derived: see the arithmetic above SAMPLE.
    assert_eq!(st.closing, Some(Paise::new(10_974_510)));
    assert_eq!(st.lines.len(), 3);
}

#[test]
fn line_one_debit_with_entry_date_and_bank_reference() {
    let st = parse_mt940(SAMPLE).unwrap();
    let l = &st.lines[0];
    // derived: ":61:2404050405D2500,50NTRFNONREF//BK123": value 24-04-05, entry 04-05, D = debit,
    // 2500,50 = 250_050 paise, type NTRF, customer ref NONREF (= none) so bank ref BK123 is used.
    assert_eq!(l.date, d(2024, Month::April, 5));
    assert_eq!(l.value_date, Some(d(2024, Month::April, 5)));
    assert_eq!(l.debit, Paise::new(250_050));
    assert_eq!(l.credit, Paise::ZERO);
    assert_eq!(l.reference.as_deref(), Some("BK123"));
    // derived: the :86: has two lines, "UPI/RENT PAYMENT" and "APRIL", joined with a space.
    assert_eq!(l.description, "UPI/RENT PAYMENT APRIL");
    // derived: the :61: is on line 6 of the sample.
    assert_eq!(l.row, 6);
    assert_eq!(l.balance, None);
    assert_eq!(l.raw[0], ":61:2404050405D2500,50NTRFNONREF//BK123");
    assert_eq!(l.raw[1], ":86:UPI/RENT PAYMENT\nAPRIL");
}

#[test]
fn line_two_credit_without_entry_date_and_one_decimal() {
    let st = parse_mt940(SAMPLE).unwrap();
    let l = &st.lines[1];
    // derived: ":61:240406C12345,6NCHK000123": no entry date (next char is the mark), C = credit,
    // 12345,6 = 12345.60 rupees = 1_234_560 paise, customer ref 000123, no bank ref.
    assert_eq!(l.date, d(2024, Month::April, 6));
    assert_eq!(l.value_date, Some(d(2024, Month::April, 6)));
    assert_eq!(l.credit, Paise::new(1_234_560));
    assert_eq!(l.debit, Paise::ZERO);
    assert_eq!(l.reference.as_deref(), Some("000123"));
    assert_eq!(l.description, "CHEQUE DEPOSIT");
    assert_eq!(l.row, 9);
}

#[test]
fn line_three_reversal_of_credit_is_a_debit() {
    let st = parse_mt940(SAMPLE).unwrap();
    let l = &st.lines[2];
    // derived: SWIFT field 61 mark RC = "reversal of credit" = a debit entry; 100,00 = 10_000 paise.
    assert_eq!(l.debit, Paise::new(10_000));
    assert_eq!(l.credit, Paise::ZERO);
    // derived: customer reference MSC... the type is NMSC, so the customer reference is "1234"
    // and the bank reference after "//" is BK9; the customer reference wins.
    assert_eq!(l.reference.as_deref(), Some("1234"));
    // derived: no :86: follows, so the description is empty (no supplementary details either).
    assert_eq!(l.description, "");
    assert_eq!(l.row, 11);
}

#[test]
fn balance_check_passes_on_the_sample() {
    let st = parse_mt940(SAMPLE).unwrap();
    // derived: 10_000_000 - 250_050 + 1_234_560 - 10_000 = 10_974_510 = the :62F: amount.
    verify_balances(&st).unwrap();
}

#[test]
fn balance_check_fails_when_closing_is_wrong() {
    // derived: the true closing is 109745,10; 109745,11 is one paise more.
    let text = SAMPLE.replace("109745,10", "109745,11");
    let st = parse_mt940(&text).unwrap();
    assert!(matches!(
        verify_balances(&st),
        Err(BankFmtError::Format { .. })
    ));
}

#[test]
fn debit_balances_are_negative() {
    // derived: D mark = debit (overdrawn) balance: D000,50 -> -50 paise; closing = -50 + 200,00
    // (credit 20_000) = 19_950 = C199,50.
    let text = ":20:X\n:60F:D240401INR0,50\n:61:240402C200,NMSC\n:62F:C240402INR199,50\n";
    let st = parse_mt940(text).unwrap();
    assert_eq!(st.opening, Some(Paise::new(-50)));
    assert_eq!(st.lines[0].credit, Paise::new(20_000));
    assert_eq!(st.closing, Some(Paise::new(19_950)));
    verify_balances(&st).unwrap();
}

#[test]
fn entry_date_year_rolls_over_to_the_nearest_year() {
    // derived: value 2024-12-30, entry date 0102: candidates 2023-01-02, 2024-01-02, 2025-01-02;
    // 2025-01-02 is 3 days from the value date, the others are far more.
    let text = ":60F:C241201INR1,00\n:61:2412300102C1,00NMSC\n:62F:C241230INR2,00\n";
    let st = parse_mt940(text).unwrap();
    assert_eq!(st.lines[0].date, d(2025, Month::January, 2));
    assert_eq!(st.lines[0].value_date, Some(d(2024, Month::December, 30)));
}

#[test]
fn funds_code_and_supplementary_details() {
    // derived: "CR" = credit mark C then funds code R; the second line is supplementary details and
    // becomes the description when no :86: follows.
    let text =
        ":60F:C240401INR0,00\n:61:240407CR50,00NTRFABC\nSUPP DETAILS\n:62F:C240407INR50,00\n";
    let st = parse_mt940(text).unwrap();
    let l = &st.lines[0];
    assert_eq!(l.credit, Paise::new(5_000));
    assert_eq!(l.reference.as_deref(), Some("ABC"));
    assert_eq!(l.description, "SUPP DETAILS");
}

#[test]
fn reversal_of_debit_is_a_credit() {
    // derived: SWIFT mark RD = reversal of debit = a credit entry.
    let text = ":60F:C240401INR0,00\n:61:240407RD7,00NTRF\n:62F:C240407INR7,00\n";
    let st = parse_mt940(text).unwrap();
    assert_eq!(st.lines[0].credit, Paise::new(700));
    assert_eq!(st.lines[0].debit, Paise::ZERO);
}

#[test]
fn rejects_three_decimals() {
    // derived: INR has two decimals; "1,005" would need rounding, which is never done silently.
    let text = ":60F:C240401INR0,00\n:61:240407C1,005NTRF\n:62F:C240407INR1,00\n";
    assert!(matches!(
        parse_mt940(text),
        Err(BankFmtError::Row { row: 2, .. })
    ));
}

#[test]
fn rejects_impossible_date() {
    // derived: 2024-02-30 does not exist.
    let text = ":60F:C240401INR0,00\n:61:240230C1,00NTRF\n:62F:C240401INR1,00\n";
    assert!(matches!(
        parse_mt940(text),
        Err(BankFmtError::Row { row: 2, .. })
    ));
}

#[test]
fn rejects_other_currency() {
    // derived: the product is rupee-only; a USD balance is refused, not read as rupees.
    let text = ":60F:C240401USD1,00\n:62F:C240401USD1,00\n";
    assert!(matches!(
        parse_mt940(text),
        Err(BankFmtError::Format { .. })
    ));
}

#[test]
fn rejects_missing_closing_and_unknown_tags_and_stray_86() {
    for text in [
        ":60F:C240401INR1,00\n",
        ":60F:C240401INR1,00\n:99:X\n:62F:C240401INR1,00\n",
        ":60F:C240401INR1,00\n:86:ORPHAN\n:62F:C240401INR1,00\n",
        ":60M:C240401INR1,00\n",
    ] {
        assert!(
            matches!(parse_mt940(text), Err(BankFmtError::Format { .. })),
            "{text}"
        );
    }
}

#[test]
fn rejects_two_statements_in_one_file() {
    // derived: one RawStatement per file; a second message after "-}" is refused, not dropped.
    let text = format!("{SAMPLE}{SAMPLE}");
    assert!(matches!(
        parse_mt940(&text),
        Err(BankFmtError::Format { .. })
    ));
}

#[test]
fn parse_is_deterministic() {
    assert_eq!(parse_mt940(SAMPLE).unwrap(), parse_mt940(SAMPLE).unwrap());
}

#[test]
fn two_digit_year_outside_the_window_is_an_error_not_2099() {
    // derived: "99" would read as 2099. The window is 2000 to 2050, so a value date of 990102 (a
    // 1999 statement) is refused with a Row error on its :61: line (line 2 here), never read as
    // 2099-01-02.
    let text = ":60F:C240401INR0,00\n:61:990102C1,00NTRF\n:62F:C240401INR1,00\n";
    let err = parse_mt940(text).unwrap_err();
    assert!(
        matches!(err, BankFmtError::Row { row: 2, .. }),
        "expected a Row error on line 2, got {err:?}"
    );
    assert!(err.to_string().contains("2099"), "{err}");
}

#[test]
fn two_digit_year_window_bounds_are_2000_and_2050() {
    // derived: 2050 is the last year accepted and 2051 the first refused. 500101 is 2050-01-01 and
    // 510101 would be 2051-01-01. Both are checked on the :61: value date.
    let accepted = ":60F:C500101INR0,00\n:61:500101C1,00NTRF\n:62F:C500101INR1,00\n";
    let parsed = parse_mt940(accepted).unwrap();
    assert_eq!(parsed.lines[0].value_date, Some(d(2050, Month::January, 1)));
    let refused = ":60F:C500101INR0,00\n:61:510101C1,00NTRF\n:62F:C500101INR1,00\n";
    assert!(matches!(
        parse_mt940(refused),
        Err(BankFmtError::Row { row: 2, .. })
    ));
}
