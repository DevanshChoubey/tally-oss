//! Card P2: report PDFs. The expected text is written by hand in each test, as the engine would
//! return it; no output of the renderer is used as an expected value.
//!
//! Provenance: `derived`. The page count in the long-report test is bounded by the row height
//! (9 point table text plus padding, about 17 points a row) on an A4 page of 842 points less
//! margins and footer: 150 rows need at least four pages, so "Page 1 of N" must have N of four
//! or more.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::pdf::{ReportPage, render_report};
use ledgerly_print::template::PrintError;

/// The strings a PDF writes as text, read back from the uncompressed content streams.
fn texts(pdf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pdf.len() {
        if pdf[i] != b'(' {
            i += 1;
            continue;
        }
        let mut depth = 1;
        let mut s = Vec::new();
        i += 1;
        while i < pdf.len() && depth > 0 {
            match pdf[i] {
                b'\\' => {
                    i += 1;
                    s.push(pdf[i]);
                }
                b'(' => {
                    depth += 1;
                    s.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth > 0 {
                        s.push(b')');
                    }
                }
                other => s.push(other),
            }
            i += 1;
        }
        out.push(String::from_utf8(s).unwrap());
    }
    out
}

fn page(rows: Vec<Vec<String>>) -> ReportPage {
    ReportPage {
        company: "Test Traders".to_owned(),
        title: "Balance Sheet".to_owned(),
        period: "01-04-2026 to 31-03-2027".to_owned(),
        headings: vec!["Particulars".to_owned(), "As on 30-06-2026".to_owned()],
        rows,
        notes: Vec::new(),
    }
}

#[test]
fn the_figures_the_engine_gave_are_printed_as_given() {
    // Derived: the two figures and the labels are written out here, and the page is the first of
    // one.
    let p = page(vec![
        vec!["Cash".to_owned(), "1,000.00 Dr".to_owned()],
        vec!["Capital".to_owned(), "1,000.00 Cr".to_owned()],
    ]);
    let pdf = render_report(&p).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let t = texts(&pdf);
    for want in [
        "Test Traders",
        "Balance Sheet",
        "01-04-2026 to 31-03-2027",
        "Particulars",
        "As on 30-06-2026",
        "Cash",
        "1,000.00 Dr",
        "Capital",
        "1,000.00 Cr",
        "Page 1 of 1",
    ] {
        assert!(t.iter().any(|s| s == want), "missing {want:?} in {t:?}");
    }
}

#[test]
fn a_long_report_repeats_its_heading_on_every_page() {
    let rows: Vec<Vec<String>> = (1..=150)
        .map(|n| vec![format!("Ledger {n:03}"), format!("{n}.00 Dr")])
        .collect();
    let pdf = render_report(&page(rows)).unwrap();
    let t = texts(&pdf);
    let pages: usize = t
        .iter()
        .find_map(|s| s.strip_prefix("Page 1 of "))
        .expect("the first page is numbered")
        .parse()
        .unwrap();
    assert!(pages >= 4, "150 rows need at least four pages, got {pages}");
    let headings = t.iter().filter(|s| *s == "Particulars").count();
    assert_eq!(headings, pages, "the heading is repeated on every page");
    let last = t.iter().filter(|s| *s == "150.00 Dr").count();
    assert_eq!(last, 1, "the last row is printed once");
}

#[test]
fn a_heading_or_figure_that_is_not_ascii_is_unsupported_not_blank() {
    let p = page(vec![vec!["Caf\u{e9} Ltd".to_owned(), "1.00 Dr".to_owned()]]);
    match render_report(&p) {
        Err(PrintError::Unsupported { reason }) => {
            assert!(reason.contains("ASCII"), "reason was {reason:?}");
        }
        other => panic!("expected Unsupported for a non-ASCII label, got {other:?}"),
    }
}

#[test]
fn a_row_with_the_wrong_number_of_cells_is_an_error_not_a_shifted_figure() {
    let p = page(vec![vec!["Cash".to_owned()]]);
    match render_report(&p) {
        Err(PrintError::Render { reason }) => {
            assert!(reason.contains("cells"), "reason was {reason:?}");
        }
        other => panic!("expected a Render error for a short row, got {other:?}"),
    }
}
