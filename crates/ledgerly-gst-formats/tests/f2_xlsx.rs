//! Card F2 tests. Expected values are derived in each test (hand arithmetic or the rule stated in
//! the comment); none is an output of the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use calamine::{Data, DataType, Reader, Xlsx, open_workbook};
use ledgerly_core::Paise;
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::gstr1_xlsx::{
    Cell, SheetData, SheetSpec, check_against_template, read_layout, write_sheets,
};

fn out(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("f2_{name}.xlsx"))
}

fn spec(name: &str, headers: &[&str]) -> SheetSpec {
    SheetSpec {
        name: name.to_owned(),
        header_row: 0,
        headers: headers.iter().map(|h| (*h).to_owned()).collect(),
    }
}

fn sample() -> Vec<SheetData> {
    vec![
        SheetData {
            spec: spec("alpha", &["Name", "Count", "Amount"]),
            rows: vec![
                vec![
                    Cell::Text("A1".to_owned()),
                    Cell::Integer(7),
                    // derived: 123456 paise = 1234.56 rupees
                    Cell::Money(Paise::new(123_456)),
                ],
                vec![
                    Cell::Text("=1+1".to_owned()),
                    Cell::Blank,
                    // derived: -5 paise = -0.05 rupees
                    Cell::Money(Paise::new(-5)),
                ],
            ],
        },
        SheetData {
            spec: spec("beta", &["X"]),
            rows: vec![],
        },
    ]
}

#[test]
fn sheets_headers_and_cells_round_trip() {
    let path = out("roundtrip");
    write_sheets(&sample(), &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    // derived: sheets are written in the order given.
    assert_eq!(book.sheet_names(), vec!["alpha", "beta"]);
    let range = book.worksheet_range("alpha").unwrap();
    let rows: Vec<&[Data]> = range.rows().collect();
    // derived: header row first, then one row per input row.
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0].as_string().as_deref(), Some("Name"));
    assert_eq!(rows[0][2].as_string().as_deref(), Some("Amount"));
    assert_eq!(rows[1][0].as_string().as_deref(), Some("A1"));
    assert_eq!(rows[1][1].as_f64(), Some(7.0));
    assert_eq!(rows[1][2].as_f64(), Some(1234.56));
    // derived: text beginning with '=' is stored as text, not as a formula (no evaluated 2).
    assert_eq!(rows[2][0].as_string().as_deref(), Some("=1+1"));
    // derived: a blank cell stays empty.
    assert!(rows[2][1].is_empty());
    assert_eq!(rows[2][2].as_f64(), Some(-0.05));
}

#[test]
fn money_beyond_exact_double_range_is_an_error() {
    // derived: a spreadsheet number is an IEEE double, exact for 15 digits; 10^15 paise has 16.
    let sheets = vec![SheetData {
        spec: spec("m", &["Amount"]),
        rows: vec![vec![Cell::Money(Paise::new(1_000_000_000_000_000))]],
    }];
    let err = write_sheets(&sheets, &out("too_big")).unwrap_err();
    assert!(matches!(err, FormatError::Field { .. }));
}

#[test]
fn the_most_negative_amount_is_an_error_not_a_panic() {
    // derived: i64::MIN has no positive counterpart; it must be refused, not wrapped.
    let sheets = vec![SheetData {
        spec: spec("m", &["Amount"]),
        rows: vec![vec![Cell::Money(Paise::new(i64::MIN))]],
    }];
    assert!(write_sheets(&sheets, &out("min")).is_err());
}

#[test]
fn largest_exact_amount_is_written_with_two_decimals() {
    // derived: 999_999_999_999_999 paise = 9_999_999_999_999.99 rupees (15 digits).
    let path = out("max_exact");
    let sheets = vec![SheetData {
        spec: spec("m", &["Amount"]),
        rows: vec![vec![Cell::Money(Paise::new(999_999_999_999_999))]],
    }];
    write_sheets(&sheets, &path).unwrap();
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let range = book.worksheet_range("m").unwrap();
    let v = range.get_value((1, 0)).unwrap().as_f64().unwrap();
    assert_eq!(v, 9_999_999_999_999.99);
}

#[test]
fn duplicate_sheet_names_are_an_error_ignoring_case() {
    // derived: spreadsheet sheet names are unique case-insensitively.
    let sheets = vec![
        SheetData {
            spec: spec("b2b", &[]),
            rows: vec![],
        },
        SheetData {
            spec: spec("B2B", &[]),
            rows: vec![],
        },
    ];
    assert!(matches!(
        write_sheets(&sheets, &out("dup")).unwrap_err(),
        FormatError::Field { .. }
    ));
}

#[test]
fn invalid_sheet_name_is_an_error() {
    // derived: a spreadsheet sheet name may not contain '/' (and is at most 31 characters).
    let sheets = vec![SheetData {
        spec: spec("a/b", &[]),
        rows: vec![],
    }];
    assert!(matches!(
        write_sheets(&sheets, &out("badname")).unwrap_err(),
        FormatError::Spreadsheet(_)
    ));
}

#[test]
fn read_layout_returns_names_and_headers_with_trailing_blanks_dropped() {
    let path = out("layout");
    write_sheets(&sample(), &path).unwrap();
    let got = read_layout(&path, 0).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], spec("alpha", &["Name", "Count", "Amount"]));
    assert_eq!(got[1], spec("beta", &["X"]));
}

#[test]
fn template_check_accepts_matching_and_names_the_first_difference() {
    let template = out("tpl");
    write_sheets(&sample(), &template).unwrap();
    // derived: same sheets and headers, different data rows, still matches.
    let mut same = sample();
    same[0].rows.clear();
    let ok = out("tpl_ok");
    write_sheets(&same, &ok).unwrap();
    check_against_template(&ok, &template, 0).unwrap();

    // derived: a changed header is reported against its sheet.
    let mut bad_header = sample();
    bad_header[0].spec.headers[1] = "Qty".to_owned();
    let p = out("tpl_bad_header");
    write_sheets(&bad_header, &p).unwrap();
    match check_against_template(&p, &template, 0).unwrap_err() {
        FormatError::Field { field, .. } => assert_eq!(field, "alpha"),
        other => panic!("unexpected error {other:?}"),
    }

    // derived: a renamed sheet is reported as a sheet-name difference.
    let mut bad_name = sample();
    bad_name[1].spec.name = "gamma".to_owned();
    let p = out("tpl_bad_name");
    write_sheets(&bad_name, &p).unwrap();
    match check_against_template(&p, &template, 0).unwrap_err() {
        FormatError::Field { field, .. } => assert_eq!(field, "sheet name"),
        other => panic!("unexpected error {other:?}"),
    }

    // derived: a missing sheet is reported by count.
    let mut fewer = sample();
    fewer.pop();
    let p = out("tpl_fewer");
    write_sheets(&fewer, &p).unwrap();
    assert!(check_against_template(&p, &template, 0).is_err());
}

#[test]
fn a_missing_template_is_an_error_never_a_pass() {
    // derived: the official template is not supplied yet, so the official-layout test is pending;
    // a missing template file must fail the check rather than skip it.
    let written = out("missing_template_written");
    write_sheets(&sample(), &written).unwrap();
    let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/gstr1_offline.xlsx");
    assert!(matches!(
        check_against_template(&written, &missing, 0).unwrap_err(),
        FormatError::Spreadsheet(_)
    ));
}
