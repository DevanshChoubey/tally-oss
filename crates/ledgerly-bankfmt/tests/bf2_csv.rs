//! Card BF2 tests. Every expected value is `derived`: worked out by hand from the input text in the
//! test (rupees x 100 + paise; calendar dates read off the cell). None comes from running the parser.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "integration-test helpers; tests may unwrap and index (clippy.toml allows it only inside #[test] fns)"
)]
use ledgerly_bankfmt::BankFmtError;
use ledgerly_bankfmt::csv::{AmountColumns, CsvLayout, DateFormat, parse_csv_with_layout};
use ledgerly_core::{Date, Month, Paise};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn split_layout() -> CsvLayout {
    CsvLayout {
        format_id: "test-split".to_owned(),
        delimiter: b',',
        skip_rows: 1,
        date_col: 0,
        value_date_col: Some(1),
        description_col: 2,
        reference_col: Some(3),
        amounts: AmountColumns::Split {
            debit: 4,
            credit: 5,
        },
        balance_col: Some(6),
        date_format: DateFormat::DdMmYyyy,
        date_separator: '/',
    }
}

#[test]
fn split_debit_credit_columns() {
    // Input is a fictional statement; the letterhead/header row is skipped (skip_rows = 1).
    let csv = "Date,Value,Narration,Ref,Withdrawal,Deposit,Balance\n\
               05/04/2024,06/04/2024,\"RENT, APRIL\",CHQ001,\"10,000.50\",,\"89,999.50\"\n\
               07/04/2024,,SALARY,,,\"1,23,456.78\",\"2,13,456.28\"\n";
    let st = parse_csv_with_layout(csv.as_bytes(), &split_layout()).unwrap();
    assert_eq!(st.format_id, "test-split");
    assert_eq!(st.lines.len(), 2);
    let a = &st.lines[0];
    // derived: 10,000.50 rupees = 10000*100 + 50 = 1_000_050 paise.
    assert_eq!(a.debit, Paise::new(1_000_050));
    assert_eq!(a.credit, Paise::ZERO);
    // derived: 89,999.50 rupees = 89999*100 + 50 = 8_999_950 paise.
    assert_eq!(a.balance, Some(Paise::new(8_999_950)));
    assert_eq!(a.date, d(2024, Month::April, 5));
    assert_eq!(a.value_date, Some(d(2024, Month::April, 6)));
    assert_eq!(a.description, "RENT, APRIL");
    assert_eq!(a.reference.as_deref(), Some("CHQ001"));
    // derived: header is record 1, so this is record 2.
    assert_eq!(a.row, 2);
    let b = &st.lines[1];
    // derived: 1,23,456.78 rupees = 123456*100 + 78 = 12_345_678 paise.
    assert_eq!(b.credit, Paise::new(12_345_678));
    assert_eq!(b.debit, Paise::ZERO);
    // derived: 2,13,456.28 rupees = 213456*100 + 28 = 21_345_628 paise.
    assert_eq!(b.balance, Some(Paise::new(21_345_628)));
    assert_eq!(b.value_date, None);
    assert_eq!(b.reference, None);
    assert_eq!(b.row, 3);
    // raw cells kept verbatim.
    assert_eq!(b.raw.len(), 7);
    assert_eq!(b.raw.get(5).map(String::as_str), Some("1,23,456.78"));
}

#[test]
fn signed_single_column() {
    let mut l = split_layout();
    l.amounts = AmountColumns::Signed { amount: 4 };
    l.balance_col = None;
    l.value_date_col = None;
    l.reference_col = None;
    let csv = "h\n05/04/2024,x,A,,-500.00\n06/04/2024,x,B,,1\n";
    let st = parse_csv_with_layout(csv.as_bytes(), &l).unwrap();
    // derived: negative = money out; 500.00 rupees = 50_000 paise.
    assert_eq!(st.lines[0].debit, Paise::new(50_000));
    assert_eq!(st.lines[0].credit, Paise::ZERO);
    // derived: positive = money in; 1 rupee = 100 paise.
    assert_eq!(st.lines[1].credit, Paise::new(100));
    assert_eq!(st.lines[1].debit, Paise::ZERO);
}

#[test]
fn amount_with_dr_cr_indicator_column() {
    let mut l = split_layout();
    l.amounts = AmountColumns::WithIndicator {
        amount: 4,
        indicator: 5,
    };
    l.balance_col = None;
    let csv = "h\n05/04/2024,,A,,\"1,000.00\",DR,\n06/04/2024,,B,,\"2,000.00\",Cr,\n";
    let st = parse_csv_with_layout(csv.as_bytes(), &l).unwrap();
    assert_eq!(st.lines[0].debit, Paise::new(100_000)); // derived: 1000*100
    assert_eq!(st.lines[1].credit, Paise::new(200_000)); // derived: 2000*100
    let bad = "h\n05/04/2024,,A,,100.00,X,\n";
    assert!(matches!(
        parse_csv_with_layout(bad.as_bytes(), &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
}

fn one_credit(cell: &str) -> Result<Paise, BankFmtError> {
    let csv = format!("h\n05/04/2024,,A,,,\"{cell}\",\n");
    let mut l = split_layout();
    l.balance_col = None;
    parse_csv_with_layout(csv.as_bytes(), &l).map(|s| s.lines[0].credit)
}

#[test]
fn indian_and_western_grouping() {
    // derived: 12,34,56,789.00 = 123456789*100 = 12_345_678_900 paise (Indian grouping 2-2-2-3).
    assert_eq!(
        one_credit("12,34,56,789.00").unwrap(),
        Paise::new(12_345_678_900)
    );
    // derived: 123,456.78 (western grouping) = 123456*100+78 = 12_345_678 paise.
    assert_eq!(one_credit("123,456.78").unwrap(), Paise::new(12_345_678));
    // derived: ungrouped 123456.78 is the same amount.
    assert_eq!(one_credit("123456.78").unwrap(), Paise::new(12_345_678));
    // derived: 5 rupees = 500; 5.5 = 550 (one decimal digit is 50 paise); 0.07 = 7.
    assert_eq!(one_credit("5").unwrap(), Paise::new(500));
    assert_eq!(one_credit("5.5").unwrap(), Paise::new(550));
    assert_eq!(one_credit("0.07").unwrap(), Paise::new(7));
}

#[test]
fn malformed_amounts_are_errors_never_rounded() {
    for bad in [
        "1,2345.00",
        "12,3.00",
        "5.555",
        "5.",
        ".5",
        "Rs 5",
        "1,,000",
        "1e3",
        "--5",
    ] {
        assert!(
            matches!(one_credit(bad), Err(BankFmtError::Row { row: 2, .. })),
            "{bad} must be rejected"
        );
    }
}

#[test]
fn both_debit_and_credit_or_neither_is_an_error() {
    let l = split_layout();
    let both = "h\n05/04/2024,,A,,10.00,20.00,\n";
    assert!(matches!(
        parse_csv_with_layout(both.as_bytes(), &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
    let neither = "h\n05/04/2024,,A,,,,\n";
    assert!(matches!(
        parse_csv_with_layout(neither.as_bytes(), &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
}

#[test]
fn date_formats() {
    let mut l = split_layout();
    l.balance_col = None;
    l.value_date_col = None;
    let row = |date: &str| format!("h\n{date},,A,,1.00,,\n");
    l.date_format = DateFormat::DdMmYy { century_base: 2000 };
    l.date_separator = '-';
    // derived: dd-mm-yy with century base 2000: 31-12-24 is 31 December 2024.
    let st = parse_csv_with_layout(row("31-12-24").as_bytes(), &l).unwrap();
    assert_eq!(st.lines[0].date, d(2024, Month::December, 31));
    l.date_format = DateFormat::YyyyMmDd;
    l.date_separator = '-';
    let st = parse_csv_with_layout(row("2024-02-29").as_bytes(), &l).unwrap();
    assert_eq!(st.lines[0].date, d(2024, Month::February, 29)); // derived: 2024 is a leap year
    l.date_format = DateFormat::DdMonYyyy;
    let st = parse_csv_with_layout(row("05-Apr-2024").as_bytes(), &l).unwrap();
    assert_eq!(st.lines[0].date, d(2024, Month::April, 5));
    // derived: 2023 is not a leap year, so 29-02-2023 does not exist.
    l.date_format = DateFormat::DdMmYyyy;
    l.date_separator = '/';
    assert!(matches!(
        parse_csv_with_layout(row("29/02/2023").as_bytes(), &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
    // wrong separator for the layout is an error, not a guess.
    assert!(parse_csv_with_layout(row("05-04-2024").as_bytes(), &l).is_err());
}

#[test]
fn structural_errors_and_edge_cases() {
    let l = split_layout();
    // too few cells for the mapped columns.
    let short = "h\n05/04/2024,,A\n";
    assert!(matches!(
        parse_csv_with_layout(short.as_bytes(), &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
    // invalid UTF-8 in a data row is a format error.
    let mut bytes = b"h\n05/04/2024,,A".to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe]);
    bytes.extend_from_slice(b",,1.00,,\n");
    assert!(matches!(
        parse_csv_with_layout(&bytes, &l),
        Err(BankFmtError::Row { row: 2, .. })
    ));
    // BOM, CRLF, blank and trailing all-empty rows are tolerated; letterhead junk is skipped.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"h\r\n05/04/2024,,A,,1.00,,\r\n\r\n,,,,,,\r\n");
    let st = parse_csv_with_layout(&bytes, &l).unwrap();
    assert_eq!(st.lines.len(), 1);
    // empty file -> no lines, not an error.
    assert_eq!(parse_csv_with_layout(b"", &l).unwrap().lines, vec![]);
    // tab delimiter.
    let mut t = split_layout();
    t.delimiter = b'\t';
    let tsv = "h\n05/04/2024\t\tA\t\t1.00\t\t\n";
    assert_eq!(
        parse_csv_with_layout(tsv.as_bytes(), &t).unwrap().lines[0].debit,
        Paise::new(100)
    );
}

#[test]
fn skip_rows_counts_toward_row_numbers() {
    let mut l = split_layout();
    l.skip_rows = 3;
    let csv = "Bank Name\nAccount 123\nDate,Value\n05/04/2024,,A,,1.00,,\nbad,,A,,1.00,,\n";
    // derived: the bad date is on record 5 (3 skipped + 1 good + this one).
    assert!(matches!(
        parse_csv_with_layout(csv.as_bytes(), &l),
        Err(BankFmtError::Row { row: 5, .. })
    ));
}
