//! Card BF3 tests. Every expected value is `derived`: worked out by hand from the cells written in
//! the test (rupees x 100 + paise; calendar dates read off the cell or computed from the Excel
//! serial rule stated next to it). None comes from running the parser.
//!
//! The workbooks are built here byte by byte (a stored, uncompressed zip holding the minimal
//! SpreadsheetML parts) because the crate has no XLSX writer dependency.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]
use ledgerly_bankfmt::BankFmtError;
use ledgerly_bankfmt::csv::DateFormat;
use ledgerly_bankfmt::xlsx::{XlsxAmounts, XlsxLayout, parse_xlsx_with_layout};
use ledgerly_core::{Date, Month, Paise};

// ---------------------------------------------------------------- workbook builder

/// One cell of a fixture sheet.
#[derive(Clone, Copy)]
enum C {
    /// Blank cell (not written).
    E,
    /// Text, written as an inline string.
    S(&'static str),
    /// Number, written as given (`<v>` text).
    N(&'static str),
    /// Date serial with the built-in date format (numFmtId 14) applied.
    D(&'static str),
}

fn col_letter(i: usize) -> char {
    char::from(b'A' + u8::try_from(i).unwrap())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `rows[0]` is sheet row 1. Entirely blank rows are written as absent rows.
fn sheet_xml(rows: &[Vec<C>]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    for (r, row) in rows.iter().enumerate() {
        let n = r + 1;
        let mut cells = String::new();
        for (c, cell) in row.iter().enumerate() {
            let at = format!("{}{n}", col_letter(c));
            match cell {
                C::E => {}
                C::S(t) => cells.push_str(&format!(
                    "<c r=\"{at}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
                    escape(t)
                )),
                C::N(v) => cells.push_str(&format!("<c r=\"{at}\"><v>{v}</v></c>")),
                C::D(v) => cells.push_str(&format!("<c r=\"{at}\" s=\"1\"><v>{v}</v></c>")),
            }
        }
        if !cells.is_empty() {
            out.push_str(&format!("<row r=\"{n}\">{cells}</row>"));
        }
    }
    out.push_str("</sheetData></worksheet>");
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn le16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A zip archive with stored (uncompressed) entries.
fn zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(out.len()).unwrap();
        let crc = crc32(data);
        let size = u32::try_from(data.len()).unwrap();
        let name_len = u16::try_from(name.len()).unwrap();
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20); // version needed
        le16(&mut out, 0); // flags
        le16(&mut out, 0); // method: stored
        le16(&mut out, 0); // time
        le16(&mut out, 33); // date: 1980-01-01
        le32(&mut out, crc);
        le32(&mut out, size);
        le32(&mut out, size);
        le16(&mut out, name_len);
        le16(&mut out, 0); // extra length
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20); // version made by
        le16(&mut central, 20); // version needed
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 33);
        le32(&mut central, crc);
        le32(&mut central, size);
        le32(&mut central, size);
        le16(&mut central, name_len);
        le16(&mut central, 0); // extra
        le16(&mut central, 0); // comment
        le16(&mut central, 0); // disk number
        le16(&mut central, 0); // internal attributes
        le32(&mut central, 0); // external attributes
        le32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = u32::try_from(out.len()).unwrap();
    let cd_size = u32::try_from(central.len()).unwrap();
    let count = u16::try_from(entries.len()).unwrap();
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, count);
    le16(&mut out, count);
    le32(&mut out, cd_size);
    le32(&mut out, cd_offset);
    le16(&mut out, 0);
    out
}

/// A workbook with the given sheets, in order. Style index 1 is the built-in date format.
fn workbook(sheets: &[(&str, Vec<Vec<C>>)]) -> Vec<u8> {
    let rel_ns = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let mut content_types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
    );
    let mut wb_sheets = String::new();
    let mut wb_rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        let n = i + 1;
        content_types.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        ));
        wb_sheets.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>",
            escape(name)
        ));
        wb_rels.push_str(&format!(
            "<Relationship Id=\"rId{n}\" Type=\"{rel_ns}/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"
        ));
        entries.push((
            format!("xl/worksheets/sheet{n}.xml"),
            sheet_xml(rows).into_bytes(),
        ));
    }
    content_types.push_str("</Types>");
    wb_rels.push_str("</Relationships>");
    let root_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{rel_ns}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
    );
    let workbook_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{rel_ns}\">\
         <sheets>{wb_sheets}</sheets></workbook>"
    );
    // Two cell formats: 0 = General, 1 = built-in numFmtId 14 (a date format).
    let styles = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
         <cellXfs count=\"2\"><xf numFmtId=\"0\"></xf><xf numFmtId=\"14\"></xf></cellXfs></styleSheet>";
    let mut all = vec![
        ("[Content_Types].xml".to_owned(), content_types.into_bytes()),
        ("_rels/.rels".to_owned(), root_rels.into_bytes()),
        ("xl/workbook.xml".to_owned(), workbook_xml.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels".to_owned(),
            wb_rels.into_bytes(),
        ),
        ("xl/styles.xml".to_owned(), styles.as_bytes().to_vec()),
    ];
    all.extend(entries);
    zip(&all)
}

fn one_sheet(rows: Vec<Vec<C>>) -> Vec<u8> {
    workbook(&[("Statement", rows)])
}

// ---------------------------------------------------------------- layouts and helpers

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn s(text: &str) -> String {
    text.to_owned()
}

fn layout() -> XlsxLayout {
    XlsxLayout {
        format_id: s("test-xlsx"),
        sheet: None,
        header_markers: vec![s("Date"), s("Narration")],
        end_markers: vec![],
        date_col: s("Date"),
        value_date_col: Some(s("Value Date")),
        description_col: s("Narration"),
        reference_col: Some(s("Chq/Ref No")),
        amounts: XlsxAmounts::Split {
            debit: s("Withdrawal Amt"),
            credit: s("Deposit Amt"),
        },
        balance_col: Some(s("Closing Balance")),
        date_format: DateFormat::DdMmYyyy,
        date_separator: '/',
    }
}

fn header() -> Vec<C> {
    vec![
        C::S("Date"),
        C::S("Value Date"),
        C::S("Narration"),
        C::S("Chq/Ref No"),
        C::S("Withdrawal Amt"),
        C::S("Deposit Amt"),
        C::S("Closing Balance"),
    ]
}

fn row_error(err: &BankFmtError) -> (u32, &str) {
    match err {
        BankFmtError::Row { row, reason } => (*row, reason.as_str()),
        BankFmtError::Format { .. } | BankFmtError::Registry { .. } => {
            panic!("expected a row error, got {err:?}")
        }
    }
}

fn format_error(err: &BankFmtError) -> &str {
    match err {
        BankFmtError::Format { reason } => reason.as_str(),
        BankFmtError::Row { .. } | BankFmtError::Registry { .. } => {
            panic!("expected a format error, got {err:?}")
        }
    }
}

// ---------------------------------------------------------------- tests

#[test]
fn header_row_detection_skips_bank_letterhead_rows() {
    // Fictional letterhead occupies sheet rows 1-5 (bank name, address, title, account holder,
    // period) and row 6 is blank, so the header is sheet row 7 and the data starts at row 8.
    let rows = vec![
        vec![C::S("Example Co-operative Bank Ltd")],
        vec![C::S("12 Sample Road, Testville 000000")],
        vec![C::S("Statement of Account")],
        vec![
            C::S("Account Holder: A. Fictional"),
            C::E,
            C::S("A/c No: 000000000000"),
        ],
        vec![C::S("Period: 01/04/2024 to 30/04/2024")],
        vec![],
        header(),
        vec![
            C::S("05/04/2024"),
            C::S("06/04/2024"),
            C::S("RENT, APRIL"),
            C::S("CHQ001"),
            C::S("10,000.50"),
            C::E,
            C::S("89,999.50"),
        ],
        vec![
            C::S("07/04/2024"),
            C::E,
            C::S("SALARY"),
            C::E,
            C::E,
            C::S("1,23,456.78"),
            C::S("2,13,456.28"),
        ],
    ];
    let st = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap();
    assert_eq!(st.format_id, "test-xlsx");
    assert_eq!(st.lines.len(), 2);
    let a = &st.lines[0];
    // derived: the header is on sheet row 7, so the first data line is sheet row 8.
    assert_eq!(a.row, 8);
    // derived: 10,000.50 rupees = 10000*100 + 50 = 1_000_050 paise.
    assert_eq!(a.debit, Paise::new(1_000_050));
    assert_eq!(a.credit, Paise::ZERO);
    // derived: 89,999.50 rupees = 89999*100 + 50 = 8_999_950 paise.
    assert_eq!(a.balance, Some(Paise::new(8_999_950)));
    assert_eq!(a.date, d(2024, Month::April, 5));
    assert_eq!(a.value_date, Some(d(2024, Month::April, 6)));
    assert_eq!(a.description, "RENT, APRIL");
    assert_eq!(a.reference.as_deref(), Some("CHQ001"));
    let b = &st.lines[1];
    assert_eq!(b.row, 9);
    // derived: 1,23,456.78 rupees = 123456*100 + 78 = 12_345_678 paise.
    assert_eq!(b.credit, Paise::new(12_345_678));
    assert_eq!(b.debit, Paise::ZERO);
    // derived: 2,13,456.28 rupees = 213456*100 + 28 = 21_345_628 paise.
    assert_eq!(b.balance, Some(Paise::new(21_345_628)));
    assert_eq!(b.value_date, None);
    assert_eq!(b.reference, None);
    // The raw cells of the line are kept; the cell under "Deposit Amt" is the sixth cell.
    assert_eq!(b.raw.get(5).map(String::as_str), Some("1,23,456.78"));
}

#[test]
fn letterhead_text_that_mentions_a_marker_is_not_the_header() {
    // Row 1 contains the cell "Date" but not "Narration", so it is not the header; row 2 has both.
    let rows = vec![
        vec![C::S("Date")],
        header(),
        vec![
            C::S("01/04/2024"),
            C::E,
            C::S("X"),
            C::E,
            C::S("1"),
            C::E,
            C::E,
        ],
    ];
    let st = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap();
    assert_eq!(st.lines.len(), 1);
    // derived: header on sheet row 2, data on row 3; 1 rupee = 100 paise.
    assert_eq!(st.lines[0].row, 3);
    assert_eq!(st.lines[0].debit, Paise::new(100));
    assert_eq!(st.lines[0].balance, None);
}

#[test]
fn header_matching_ignores_case_and_spacing() {
    let rows = vec![
        vec![
            C::S("  DATE "),
            C::E,
            C::S("narration"),
            C::E,
            C::S("withdrawal   amt"),
            C::S("DEPOSIT AMT"),
        ],
        vec![C::S("01/04/2024"), C::E, C::S("X"), C::E, C::N("5"), C::E],
    ];
    let mut l = layout();
    l.value_date_col = None;
    l.reference_col = None;
    l.balance_col = None;
    let st = parse_xlsx_with_layout(&one_sheet(rows), &l).unwrap();
    // derived: 5 rupees = 500 paise.
    assert_eq!(st.lines[0].debit, Paise::new(500));
}

#[test]
fn numeric_amount_cells_are_read_from_their_decimal_text() {
    let rows = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("10000.5"),
            C::E,
            C::N("89999.5"),
        ],
        vec![
            C::S("06/04/2024"),
            C::E,
            C::S("B"),
            C::E,
            C::E,
            C::N("250"),
            C::N("90249.5"),
        ],
    ];
    let st = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap();
    // derived: 10000.5 rupees = 10000*100 + 50 = 1_000_050 paise (one decimal digit means tens).
    assert_eq!(st.lines[0].debit, Paise::new(1_000_050));
    // derived: 89999.5 rupees = 8_999_950 paise.
    assert_eq!(st.lines[0].balance, Some(Paise::new(8_999_950)));
    // derived: 250 rupees = 25_000 paise; 90249.5 rupees = 90249*100 + 50 = 9_024_950 paise.
    assert_eq!(st.lines[1].credit, Paise::new(25_000));
    assert_eq!(st.lines[1].balance, Some(Paise::new(9_024_950)));
}

#[test]
fn float_noise_in_an_amount_is_an_error_not_a_rounded_value() {
    // 0.30000000000000004 has more than two decimals; it must be refused, never rounded to 0.30.
    let rows = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("0.30000000000000004"),
            C::E,
            C::E,
        ],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    let (row, reason) = row_error(&err);
    assert_eq!(row, 2);
    assert!(reason.contains("debit"), "{reason}");
}

#[test]
fn real_date_cells_use_the_excel_serial() {
    // derived: Excel 1900 system serial 36526 = 1 Jan 2000 (epoch 30 Dec 1899 + 36526 days: 100
    // years = 36524 days incl. 24 leap days 1904..1996, plus 2 days). 1 Jan 2000 to 1 Jan 2024 is
    // 24*365 + 6 leap days (2000, 04, 08, 12, 16, 20) = 8766 days, so 1 Jan 2024 = 45292.
    // January 31 + February 29 + March 31 = 91 days, so 1 Apr 2024 = 45383 and 5 Apr 2024 = 45387;
    // 6 Apr 2024 = 45388.
    let rows = vec![
        header(),
        vec![
            C::D("45387"),
            C::D("45388"),
            C::S("A"),
            C::E,
            C::N("1"),
            C::E,
            C::E,
        ],
    ];
    let st = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap();
    assert_eq!(st.lines[0].date, d(2024, Month::April, 5));
    assert_eq!(st.lines[0].value_date, Some(d(2024, Month::April, 6)));
}

#[test]
fn a_bare_number_in_a_date_column_is_an_error() {
    // No date style on the cell: it may be an amount typed in the wrong column.
    let rows = vec![
        header(),
        vec![C::N("45387"), C::E, C::S("A"), C::E, C::N("1"), C::E, C::E],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    let (row, reason) = row_error(&err);
    assert_eq!(row, 2);
    assert!(reason.contains("date"), "{reason}");
}

#[test]
fn excel_serial_60_is_rejected() {
    // derived: serial 60 is Excel's 29 Feb 1900, a date that never existed.
    let rows = vec![
        header(),
        vec![C::D("60"), C::E, C::S("A"), C::E, C::N("1"), C::E, C::E],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    assert_eq!(row_error(&err).0, 2);
}

#[test]
fn signed_single_column() {
    let rows = vec![
        vec![C::S("Date"), C::S("Narration"), C::S("Amount")],
        vec![C::S("05/04/2024"), C::S("A"), C::S("-500.00")],
        vec![C::S("06/04/2024"), C::S("B"), C::N("1")],
    ];
    let mut l = layout();
    l.amounts = XlsxAmounts::Signed {
        amount: s("Amount"),
    };
    l.value_date_col = None;
    l.reference_col = None;
    l.balance_col = None;
    let st = parse_xlsx_with_layout(&one_sheet(rows), &l).unwrap();
    // derived: negative = money out; 500.00 rupees = 50_000 paise.
    assert_eq!(st.lines[0].debit, Paise::new(50_000));
    assert_eq!(st.lines[0].credit, Paise::ZERO);
    // derived: positive = money in; 1 rupee = 100 paise.
    assert_eq!(st.lines[1].credit, Paise::new(100));
    assert_eq!(st.lines[1].debit, Paise::ZERO);
}

#[test]
fn amount_with_dr_cr_indicator() {
    let rows = vec![
        vec![
            C::S("Date"),
            C::S("Narration"),
            C::S("Amount"),
            C::S("Dr/Cr"),
        ],
        vec![
            C::S("05/04/2024"),
            C::S("A"),
            C::S("1,00,000.00"),
            C::S("DR"),
        ],
        vec![C::S("06/04/2024"), C::S("B"), C::S("2.5"), C::S("cr")],
        vec![C::S("07/04/2024"), C::S("C"), C::S("2"), C::S("??")],
    ];
    let mut l = layout();
    l.amounts = XlsxAmounts::WithIndicator {
        amount: s("Amount"),
        indicator: s("Dr/Cr"),
    };
    l.value_date_col = None;
    l.reference_col = None;
    l.balance_col = None;
    let err = parse_xlsx_with_layout(&one_sheet(rows.clone()), &l).unwrap_err();
    // derived: the third data line is sheet row 4 and its marker is neither Dr nor Cr.
    assert_eq!(row_error(&err).0, 4);

    let ok_rows = rows.into_iter().take(3).collect::<Vec<_>>();
    let st = parse_xlsx_with_layout(&one_sheet(ok_rows), &l).unwrap();
    // derived: 1,00,000.00 rupees = 100000*100 = 10_000_000 paise, debit.
    assert_eq!(st.lines[0].debit, Paise::new(10_000_000));
    // derived: 2.5 rupees = 250 paise, credit.
    assert_eq!(st.lines[1].credit, Paise::new(250));
}

#[test]
fn both_debit_and_credit_filled_is_a_row_error() {
    let rows = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("1"),
            C::N("1"),
            C::E,
        ],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    let (row, reason) = row_error(&err);
    assert_eq!(row, 2);
    assert!(reason.contains("both"), "{reason}");
}

#[test]
fn bad_amount_text_names_the_sheet_row() {
    let rows = vec![
        vec![C::S("Letterhead")],
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("1"),
            C::E,
            C::E,
        ],
        vec![
            C::S("06/04/2024"),
            C::E,
            C::S("B"),
            C::E,
            C::S("12,34.00"),
            C::E,
            C::E,
        ],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    // derived: the bad cell is on the fourth sheet row.
    assert_eq!(row_error(&err).0, 4);
}

#[test]
fn blank_rows_are_ignored_and_end_markers_stop_the_table() {
    let rows = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("1"),
            C::E,
            C::E,
        ],
        vec![],
        vec![
            C::S("06/04/2024"),
            C::E,
            C::S("B"),
            C::E,
            C::E,
            C::N("2"),
            C::E,
        ],
        vec![
            C::S("  Closing   Balance as on 30/04/2024"),
            C::E,
            C::E,
            C::E,
            C::E,
            C::E,
            C::N("9"),
        ],
        vec![C::S("not a date"), C::E, C::S("legend")],
    ];
    let mut l = layout();
    l.end_markers = vec![s("closing balance")];
    let st = parse_xlsx_with_layout(&one_sheet(rows), &l).unwrap();
    assert_eq!(st.lines.len(), 2);
    // derived: the blank third sheet row is skipped, so the second line keeps sheet row 4.
    assert_eq!(st.lines[0].row, 2);
    assert_eq!(st.lines[1].row, 4);
}

#[test]
fn without_an_end_marker_a_footer_row_is_an_error_not_silently_dropped() {
    let rows = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("1"),
            C::E,
            C::E,
        ],
        vec![
            C::S("Closing Balance"),
            C::E,
            C::E,
            C::E,
            C::E,
            C::E,
            C::N("9"),
        ],
    ];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    assert_eq!(row_error(&err).0, 3);
}

#[test]
fn missing_header_row_is_a_format_error() {
    let rows = vec![vec![C::S("Just"), C::S("letterhead")]];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    assert!(format_error(&err).contains("no header row"));
}

#[test]
fn missing_column_header_is_a_format_error_naming_it() {
    let rows = vec![vec![C::S("Date"), C::S("Narration")]];
    let err = parse_xlsx_with_layout(&one_sheet(rows), &layout()).unwrap_err();
    assert!(format_error(&err).contains("Withdrawal Amt"));
}

#[test]
fn duplicate_column_header_is_a_format_error() {
    let mut h = header();
    h.push(C::S("deposit amt"));
    let err = parse_xlsx_with_layout(&one_sheet(vec![h]), &layout()).unwrap_err();
    assert!(format_error(&err).contains("more than once"));
}

#[test]
fn sheet_can_be_chosen_by_name() {
    let first = vec![vec![C::S("Cover page")]];
    let second = vec![
        header(),
        vec![
            C::S("05/04/2024"),
            C::E,
            C::S("A"),
            C::E,
            C::N("1"),
            C::E,
            C::E,
        ],
    ];
    let bytes = workbook(&[("Cover", first), ("Transactions", second)]);
    let mut l = layout();
    // The first sheet has no header row.
    assert!(parse_xlsx_with_layout(&bytes, &l).is_err());
    l.sheet = Some(s("Transactions"));
    let st = parse_xlsx_with_layout(&bytes, &l).unwrap();
    assert_eq!(st.lines.len(), 1);
    l.sheet = Some(s("Nope"));
    let err = parse_xlsx_with_layout(&bytes, &l).unwrap_err();
    assert!(format_error(&err).contains("Nope"));
}

#[test]
fn text_dates_follow_the_layout_format() {
    let rows = vec![
        vec![C::S("Date"), C::S("Narration"), C::S("Amount")],
        vec![C::S("05-Apr-2024"), C::S("A"), C::N("1")],
    ];
    let mut l = layout();
    l.amounts = XlsxAmounts::Signed {
        amount: s("Amount"),
    };
    l.value_date_col = None;
    l.reference_col = None;
    l.balance_col = None;
    l.date_format = DateFormat::DdMonYyyy;
    l.date_separator = '-';
    let st = parse_xlsx_with_layout(&one_sheet(rows), &l).unwrap();
    assert_eq!(st.lines[0].date, d(2024, Month::April, 5));
}

#[test]
fn bytes_that_are_not_a_workbook_are_a_format_error() {
    let err = parse_xlsx_with_layout(b"not a zip file", &layout()).unwrap_err();
    assert!(format_error(&err).contains("not a readable XLSX"));
    let err = parse_xlsx_with_layout(&[], &layout()).unwrap_err();
    assert!(format_error(&err).contains("not a readable XLSX"));
}

#[test]
fn an_empty_sheet_is_a_format_error() {
    let err = parse_xlsx_with_layout(&one_sheet(vec![]), &layout()).unwrap_err();
    assert!(format_error(&err).contains("empty"));
}
