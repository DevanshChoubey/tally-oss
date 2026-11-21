//! Card RP8 tests: CSV, XLSX and JSON exports.
//!
//! Provenance for every expected value is `derived`, written beside it. The sources are:
//! - 1 rupee = 100 paise, so `Paise(100_000)` is 1000.00 rupees (the definition of `Paise`);
//! - the sign convention written in the `export` module docs (debit positive, credit negative);
//! - RFC 4180 for CSV quoting (fields with a comma or a double quote are quoted, a double quote is
//!   doubled);
//! - Excel's worksheet-name rules (at most 31 characters, none of `\ / ? * [ ] :`).
//!
//! The plan names an `insta` snapshot of the CSV for the RP2 fixture. The hand-derived string is
//! asserted by `csv_of_trial_balance_fixture`, and `csv_snapshot_of_trial_balance_fixture` keeps the
//! same text as an `insta` snapshot (`tests/snapshots/`). That snapshot file was written by hand
//! to hold the hand-derived text, and the first `cargo insta` run must confirm it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use calamine::{Data, Reader, Xlsx, open_workbook};
use ledgerly_core::{Decimal, Paise, Period, Qty, SignedAmount, Unsupported};
use ledgerly_model::Scenario;
use ledgerly_reports::export::{to_csv, to_json, to_xlsx};
use ledgerly_reports::{
    Cell, ColumnHeader, ColumnSpec, Detail, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};
use proptest::prelude::*;
use time::macros::date;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn params() -> ReportParams {
    let period = Period::new(date!(2025 - 04 - 01), date!(2026 - 03 - 31)).unwrap();
    ReportParams {
        kind: ReportKind::TrialBalance,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn header(label: &str) -> ColumnHeader {
    ColumnHeader {
        label: label.to_owned(),
        period: None,
    }
}

fn row(label: &str, depth: u8, kind: RowKind, cells: Vec<Cell>) -> ReportRow {
    ReportRow {
        key: None,
        label: label.to_owned(),
        depth,
        kind,
        cells,
    }
}

fn dr(p: Paise) -> Cell {
    Cell::Amount(SignedAmount::dr(p))
}

fn cr(p: Paise) -> Cell {
    Cell::Amount(SignedAmount::cr(p))
}

/// A small trial balance: Debit and Credit columns, two groups, totals and one note.
fn trial_balance_fixture() -> Report {
    Report {
        title: "Trial Balance".to_owned(),
        params: params(),
        columns: vec![header("Debit"), header("Credit")],
        rows: vec![
            row(
                "Capital Account",
                0,
                RowKind::Group,
                vec![Cell::Empty, cr(rs(1000))],
            ),
            row(
                "Proprietor Capital",
                1,
                RowKind::Ledger,
                vec![Cell::Empty, cr(rs(1000))],
            ),
            row(
                "Current Assets",
                0,
                RowKind::Group,
                vec![dr(rs(1000)), Cell::Empty],
            ),
            row("Cash", 1, RowKind::Ledger, vec![dr(rs(1000)), Cell::Empty]),
        ],
        totals: vec![dr(rs(1000)), cr(rs(1000))],
        notes: vec!["Actual vouchers as of 2026-03-31".to_owned()],
    }
}

/// One-column report holding the given rows, for cell-kind tests.
fn one_column(rows: Vec<ReportRow>) -> Report {
    Report {
        title: "Cells".to_owned(),
        params: params(),
        columns: vec![header("Value")],
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

fn one_cell(cell: Cell) -> Report {
    one_column(vec![row("r", 0, RowKind::Text, vec![cell])])
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rp8");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

// ---------------------------------------------------------------- CSV

#[test]
fn csv_of_trial_balance_fixture() {
    // derived: header is `Particulars` + column labels; labels indent 2 spaces per depth; a credit
    // is negative (module docs); Paise(100_000) = 1000.00 rupees (1 rupee = 100 paise); empty cells
    // are empty fields; the totals line is labelled `Total`; each note is `Note: ...`; every
    // record ends with a line feed (csv crate default terminator).
    let expected = "\
Particulars,Debit,Credit
Capital Account,,-1000.00
  Proprietor Capital,,-1000.00
Current Assets,1000.00,
  Cash,1000.00,
Total,1000.00,-1000.00
Note: Actual vouchers as of 2026-03-31
";
    assert_eq!(to_csv(&trial_balance_fixture()).unwrap(), expected);
}

#[test]
fn csv_snapshot_of_trial_balance_fixture() {
    // provenance: derived. The snapshot holds the text that `csv_of_trial_balance_fixture` derives
    // by hand. A change to CSV layout shows up as a snapshot diff, which needs a reason in the
    // commit (rule E6).
    insta::assert_snapshot!(to_csv(&trial_balance_fixture()).unwrap());
}

#[test]
fn csv_quotes_commas_and_doubles_quotes() {
    // derived: RFC 4180: a field containing a comma or a double quote is wrapped in double quotes
    // and each inner double quote is doubled.
    let report = one_column(vec![row(
        "Smith, John \"JJ\"",
        0,
        RowKind::Ledger,
        vec![dr(rs(1))],
    )]);
    assert_eq!(
        to_csv(&report).unwrap(),
        "Particulars,Value\n\"Smith, John \"\"JJ\"\"\",1.00\n"
    );
}

#[test]
fn csv_text_that_looks_like_a_formula_gets_a_leading_apostrophe() {
    // derived: module docs: text cells starting with = + - @ tab or CR are prefixed with `'`.
    let report = one_column(vec![
        row("=1+1", 0, RowKind::Text, vec![Cell::Text("@x".to_owned())]),
        row("+a", 0, RowKind::Text, vec![Cell::Text("-b".to_owned())]),
        row("safe", 0, RowKind::Text, vec![Cell::Text("x=y".to_owned())]),
    ]);
    assert_eq!(
        to_csv(&report).unwrap(),
        "Particulars,Value\n'=1+1,'@x\n'+a,'-b\nsafe,x=y\n"
    );
}

#[test]
fn csv_negative_numbers_are_not_prefixed() {
    // derived: only text is guarded; a credit amount is a number and must stay `-5.00`.
    assert_eq!(
        to_csv(&one_cell(cr(rs(5)))).unwrap(),
        "Particulars,Value\nr,-5.00\n"
    );
}

#[test]
fn csv_row_wider_than_the_report_is_an_error() {
    // derived: rule A1: figures that would not line up under their headers are never exported.
    let report = one_column(vec![row(
        "wide",
        0,
        RowKind::Ledger,
        vec![dr(rs(1)), dr(rs(2))],
    )]);
    assert!(matches!(to_csv(&report), Err(ReportError::Invalid { .. })));
}

#[test]
fn csv_short_row_is_padded_with_empty_fields() {
    // derived: module docs: a row with fewer cells than columns is padded on the right.
    let report = Report {
        columns: vec![header("A"), header("B")],
        ..one_column(vec![row("Heading", 0, RowKind::Text, Vec::new())])
    };
    assert_eq!(to_csv(&report).unwrap(), "Particulars,A,B\nHeading,,\n");
}

#[test]
fn csv_cell_kinds() {
    // derived: Signed(-12345 paise) = -123.45 rupees; Qty keeps the exact decimal text
    // (Decimal::new(2500, 3) is 2.500); a date is ISO; a provisional amount is text with its side
    // and reason (never a bare number); an unsupported cell names what and why (never blank).
    let rows = vec![
        row(
            "signed",
            0,
            RowKind::Text,
            vec![Cell::Signed(Paise::new(-12_345))],
        ),
        row(
            "qty",
            0,
            RowKind::Item,
            vec![Cell::Qty(Qty(Decimal::new(2500, 3)))],
        ),
        row(
            "date",
            0,
            RowKind::Voucher,
            vec![Cell::Date(date!(2025 - 04 - 01))],
        ),
        row(
            "prov",
            0,
            RowKind::Item,
            vec![Cell::Provisional {
                amount: SignedAmount::dr(Paise::new(50_050)),
                reason: "closing stock not final".to_owned(),
            }],
        ),
        row(
            "unsup",
            0,
            RowKind::Item,
            vec![Cell::Unsupported(Unsupported {
                what: "stock valuation".to_owned(),
                reason: "method not built".to_owned(),
            })],
        ),
        row("empty", 0, RowKind::Text, vec![Cell::Empty]),
    ];
    let expected = "\
Particulars,Value
signed,-123.45
qty,2.500
date,2025-04-01
prov,500.50 Dr (provisional: closing stock not final)
unsup,UNSUPPORTED stock valuation: method not built
empty,
";
    assert_eq!(to_csv(&one_column(rows)).unwrap(), expected);
}

#[test]
fn csv_amount_above_2_pow_53_keeps_every_paisa() {
    // derived: 2^53 = 9_007_199_254_740_992, so 9_007_199_254_740_993 paise is the first count of
    // paise an f64 cannot hold. In rupees: 9007199254740993 / 100 = 90071992547409.93.
    let report = one_cell(dr(Paise::new(9_007_199_254_740_993)));
    assert_eq!(
        to_csv(&report).unwrap(),
        "Particulars,Value\nr,90071992547409.93\n"
    );
}

proptest! {
    #[test]
    fn csv_amount_text_matches_integer_division(p in 0i64..1_000_000_000_000_000, credit in any::<bool>()) {
        // derived: rupees = p / 100 and paise = p % 100 (two digits); a credit is negative unless
        // zero; this is computed with integer arithmetic, independent of the exporter.
        let amount = if credit { cr(Paise::new(p)) } else { dr(Paise::new(p)) };
        let sign = if credit && p != 0 { "-" } else { "" };
        let expected = format!("{sign}{}.{:02}", p / 100, p % 100);
        let csv = to_csv(&one_cell(amount)).unwrap();
        prop_assert_eq!(csv, format!("Particulars,Value\nr,{expected}\n"));
    }
}

// ---------------------------------------------------------------- XLSX

fn cell_string(data: Option<&Data>) -> String {
    let Some(data) = data else {
        return "empty".to_owned();
    };
    if let Data::Float(f) = data {
        format!("number {f:.2}")
    } else if let Data::String(s) = data {
        format!("text {s}")
    } else if matches!(data, Data::Empty) {
        "empty".to_owned()
    } else {
        format!("other {data:?}")
    }
}

#[test]
fn xlsx_opens_with_calamine_and_amounts_are_numbers_with_two_decimals() {
    // derived: layout and sign convention as in the CSV test; header texts are strings, amounts are
    // numeric cells (calamine reports Data::Float), shown here with two decimals; blanks are empty.
    let path = scratch("tb.xlsx");
    to_xlsx(&trial_balance_fixture(), &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(book.sheet_names(), vec!["Trial Balance".to_owned()]);
    let range = book.worksheet_range("Trial Balance").unwrap();
    let at = |r: usize, c: usize| cell_string(range.get((r, c)));
    assert_eq!(at(0, 0), "text Particulars");
    assert_eq!(at(0, 1), "text Debit");
    assert_eq!(at(0, 2), "text Credit");
    assert_eq!(at(1, 0), "text Capital Account");
    assert_eq!(at(1, 1), "empty");
    assert_eq!(at(1, 2), "number -1000.00");
    // indentation is two spaces per depth level; only the label is asserted here because a reader
    // may trim leading spaces.
    assert!(at(2, 0).ends_with("Proprietor Capital"));
    assert_eq!(at(3, 1), "number 1000.00");
    assert_eq!(at(4, 1), "number 1000.00");
    assert_eq!(at(5, 0), "text Total");
    assert_eq!(at(5, 1), "number 1000.00");
    assert_eq!(at(5, 2), "number -1000.00");
    assert_eq!(at(6, 0), "text Note: Actual vouchers as of 2026-03-31");
}

#[test]
fn xlsx_amount_with_paise_keeps_two_decimals() {
    // derived: 123_456_789_12 paise = 123456789.12 rupees; the nearest f64 prints as 123456789.12
    // with two decimals; Signed(-5 paise) = -0.05 rupees.
    let report = one_column(vec![
        row(
            "big",
            0,
            RowKind::Ledger,
            vec![dr(Paise::new(12_345_678_912))],
        ),
        row("tiny", 0, RowKind::Text, vec![Cell::Signed(Paise::new(-5))]),
    ]);
    let path = scratch("paise.xlsx");
    to_xlsx(&report, &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let range = book.worksheet_range("Cells").unwrap();
    assert_eq!(cell_string(range.get((1, 1))), "number 123456789.12");
    assert_eq!(cell_string(range.get((2, 1))), "number -0.05");
}

#[test]
fn xlsx_provisional_and_unsupported_are_text_not_numbers() {
    // derived: module docs: a provisional amount and an unsupported cell are written as text.
    let report = one_column(vec![
        row(
            "prov",
            0,
            RowKind::Item,
            vec![Cell::Provisional {
                amount: SignedAmount::cr(Paise::new(1_000)),
                reason: "not final".to_owned(),
            }],
        ),
        row(
            "unsup",
            0,
            RowKind::Item,
            vec![Cell::Unsupported(Unsupported {
                what: "w".to_owned(),
                reason: "r".to_owned(),
            })],
        ),
    ]);
    let path = scratch("text_cells.xlsx");
    to_xlsx(&report, &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let range = book.worksheet_range("Cells").unwrap();
    assert_eq!(
        cell_string(range.get((1, 1))),
        "text 10.00 Cr (provisional: not final)"
    );
    assert_eq!(cell_string(range.get((2, 1))), "text UNSUPPORTED w: r");
}

#[test]
fn xlsx_sheet_name_is_made_valid() {
    // derived: Excel allows at most 31 characters and none of \ / ? * [ ] :. Each forbidden
    // character becomes a space, the result is cut to 31 characters:
    // "Profit & Loss  FY  2025 26  and" is exactly 31 characters long.
    let mut report = one_cell(Cell::Empty);
    report.title = "Profit & Loss: FY [2025/26] and a very long title".to_owned();
    let path = scratch("name.xlsx");
    to_xlsx(&report, &path).unwrap();
    let book: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(
        book.sheet_names(),
        vec!["Profit & Loss  FY  2025 26  and".to_owned()]
    );
}

#[test]
fn xlsx_empty_title_gets_a_default_sheet_name() {
    // derived: Excel rejects an empty sheet name, so the export falls back to "Report".
    let mut report = one_cell(Cell::Empty);
    report.title = String::new();
    let path = scratch("untitled.xlsx");
    to_xlsx(&report, &path).unwrap();
    let book: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(book.sheet_names(), vec!["Report".to_owned()]);
}

#[test]
fn xlsx_write_replaces_atomically_and_leaves_no_temp_file() {
    // derived: rule D3: the new content replaces the old at the same path in one rename, and the
    // temporary file (`.<name>.ledgerly-tmp`, same directory) is gone afterwards.
    let path = scratch("atomic.xlsx");
    to_xlsx(&one_cell(dr(rs(1))), &path).unwrap();
    to_xlsx(&one_cell(dr(rs(2))), &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let range = book.worksheet_range("Cells").unwrap();
    assert_eq!(cell_string(range.get((1, 1))), "number 2.00");
    let leftovers = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(".atomic.xlsx.") && name.ends_with(".ledgerly-tmp")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn xlsx_amount_above_2_pow_53_paise_is_refused_not_rounded() {
    // derived: 2^53 = 9_007_199_254_740_992, so 9_007_199_254_740_993 paise (90071992547409.93
    // rupees) has no exact f64 cell; the export must fail with a typed error and write nothing.
    let report = one_cell(dr(Paise::new(9_007_199_254_740_993)));
    let path = scratch("too_big.xlsx");
    let _ = std::fs::remove_file(&path);
    let result = to_xlsx(&report, &path);
    assert!(
        matches!(&result, Err(ReportError::Export { .. })),
        "{result:?}"
    );
    assert!(!path.exists());
}

#[test]
fn xlsx_quantity_with_more_than_15_significant_digits_is_refused() {
    // derived: 1234567890123456789 has 19 significant digits; an f64 holds about 15-17, and the
    // neighbouring doubles near 1.2e18 are 256 apart, so the cell cannot be exact.
    let report = one_cell(Cell::Qty(Qty(Decimal::new(1_234_567_890_123_456_789, 0))));
    let path = scratch("big_qty.xlsx");
    let _ = std::fs::remove_file(&path);
    let result = to_xlsx(&report, &path);
    assert!(
        matches!(&result, Err(ReportError::Export { .. })),
        "{result:?}"
    );
    assert!(!path.exists());
}

#[test]
fn xlsx_failure_creates_nothing() {
    // derived: rule D3: a write into a directory that does not exist fails with a typed export
    // error naming the path, and no file appears there.
    let path = scratch("no_such_dir").join("out.xlsx");
    let result = to_xlsx(&one_cell(dr(rs(1))), &path);
    assert!(
        matches!(&result, Err(ReportError::Export { .. })),
        "{result:?}"
    );
    if let Err(ReportError::Export { path: shown, .. }) = result {
        assert_eq!(shown, path.display().to_string());
    }
    assert!(!path.exists());
}

#[test]
fn xlsx_wide_row_is_an_error_and_writes_nothing() {
    // derived: the same width rule as CSV; nothing is written when it fails.
    let report = one_column(vec![row(
        "wide",
        0,
        RowKind::Ledger,
        vec![dr(rs(1)), dr(rs(2))],
    )]);
    let path = scratch("wide.xlsx");
    let _ = std::fs::remove_file(&path);
    assert!(matches!(
        to_xlsx(&report, &path),
        Err(ReportError::Invalid { .. })
    ));
    assert!(!path.exists());
}

// ---------------------------------------------------------------- JSON

#[test]
fn json_round_trips_exactly() {
    // derived: the JSON is the serde form of Report, so reading it back gives an equal Report
    // (every amount keeps its side and every paisa).
    let mut report = trial_balance_fixture();
    report.rows.push(row(
        "odd",
        2,
        RowKind::Difference,
        vec![
            Cell::Signed(Paise::new(-1)),
            Cell::Provisional {
                amount: SignedAmount::cr(Paise::new(7)),
                reason: "r".to_owned(),
            },
        ],
    ));
    let json = to_json(&report).unwrap();
    let back: Report = serde_json::from_str(&json).unwrap();
    assert_eq!(back, report);
}

#[test]
fn json_is_deterministic_and_carries_the_side() {
    // derived: serialising the same value twice gives the same text, and a credit amount is
    // tagged "Cr" in it (SignedAmount derives Serialize with a `side` field holding DrCr::Cr).
    let report = one_cell(cr(rs(5)));
    let first = to_json(&report).unwrap();
    assert_eq!(first, to_json(&report).unwrap());
    assert!(first.contains("\"Cr\""));
}
