//! Card C3: CSV opening and closing balance from the balance column, and the MT940 end date.
//!
//! Provenance `derived`: values are worked out by hand from the file text in each test. Amounts
//! are paise (Rs 1.00 = 100).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_bankfmt::csv::{AmountColumns, CsvLayout, DateFormat, parse_csv_with_layout};
use ledgerly_bankfmt::mt940::{parse_mt940, parse_mt940_with_end};
use ledgerly_core::Paise;
use time::macros::date;

fn layout(balance_col: Option<usize>) -> CsvLayout {
    CsvLayout {
        format_id: "fictional".to_owned(),
        delimiter: b',',
        skip_rows: 1,
        date_col: 0,
        value_date_col: None,
        description_col: 1,
        reference_col: None,
        amounts: AmountColumns::Split {
            debit: 2,
            credit: 3,
        },
        balance_col,
        date_format: DateFormat::DdMmYyyy,
        date_separator: '/',
    }
}

#[test]
fn csv_balances_come_from_the_first_and_last_balance_cells() {
    // derived: row 2 deposit 500.00 leaves 1500.00, so the opening is 1500.00 - 500.00 = 1000.00;
    // row 3 withdrawal 300.00 leaves 1200.00, which is the closing.
    let text =
        "Date,Narration,Dr,Cr,Bal\n05/04/2026,A,,500.00,1500.00\n12/04/2026,B,300.00,,1200.00\n";
    let st = parse_csv_with_layout(text.as_bytes(), &layout(Some(4))).unwrap();
    assert_eq!(st.opening, Some(Paise::new(100_000)));
    assert_eq!(st.closing, Some(Paise::new(120_000)));
}

#[test]
fn csv_opening_is_taken_back_over_a_first_line_withdrawal() {
    // derived: a withdrawal of 200.00 leaving 800.00 means the opening was 800.00 + 200.00.
    let text = "Date,Narration,Dr,Cr,Bal\n05/04/2026,A,200.00,,800.00\n";
    let st = parse_csv_with_layout(text.as_bytes(), &layout(Some(4))).unwrap();
    assert_eq!(st.opening, Some(Paise::new(100_000)));
    assert_eq!(st.closing, Some(Paise::new(80_000)));
}

#[test]
fn csv_without_a_balance_column_states_no_balances() {
    let text = "Date,Narration,Dr,Cr\n05/04/2026,A,,500.00\n";
    let st = parse_csv_with_layout(text.as_bytes(), &layout(None)).unwrap();
    assert_eq!(st.opening, None);
    assert_eq!(st.closing, None);
}

#[test]
fn csv_with_an_empty_last_balance_cell_states_no_balances() {
    let text = "Date,Narration,Dr,Cr,Bal\n05/04/2026,A,,500.00,1500.00\n12/04/2026,B,300.00,,\n";
    let st = parse_csv_with_layout(text.as_bytes(), &layout(Some(4))).unwrap();
    assert_eq!(st.opening, None);
    assert_eq!(st.closing, None);
}

const MT940: &str = ":20:S1\r\n:25:123\r\n:60F:C260401INR1000,00\r\n:61:260405C500,00NTRFNONREF//B1\r\n:62F:C260430INR1500,00\r\n";

#[test]
fn mt940_keeps_the_62f_date_as_the_end_date() {
    // derived: the :62F: field reads C, 260430 (30 April 2026), INR, 1500,00.
    let parsed = parse_mt940_with_end(MT940).unwrap();
    assert_eq!(parsed.end_date, Some(date!(2026 - 04 - 30)));
    assert_eq!(parsed.raw.closing, Some(Paise::new(150_000)));
}

#[test]
fn parse_mt940_is_the_same_statement_without_the_end_date() {
    let parsed = parse_mt940_with_end(MT940).unwrap();
    assert_eq!(parse_mt940(MT940).unwrap(), parsed.raw);
}
