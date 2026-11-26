//! Card S16: what a person can set before printing (paper, copies, pages to print, first page
//! number) and what the engine writes for it.
//!
//! Provenance: `derived`. The settings are those of the help page "Configure for Print, Export,
//! Share" (<https://help.tallysolutions.com/configure-for-print-export-share/>, read 2026-11-26):
//! "Number of copies"; "Pages to Print": "Number to start from first page ... sets the starting
//! page number, which then increments" and "Page Range ... prints only the pages you specify".
//! A5 is 148 x 210 mm = 420 x 595 points (mm x 72 / 25.4, rounded), A4 595 x 842 (as `s14_pages`).
//! Page counts follow the row arithmetic of `s14_pages`: 150 rows need at least 4 pages.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::pdf::{PageItem, PageLayout, ReportPage, render_report_pages_with};
use ledgerly_print::setup::{PageSelection, Paper, PrintSetup};
use ledgerly_print::template::PrintError;

fn report(rows: usize) -> ReportPage {
    ReportPage {
        company: "Test Traders".to_owned(),
        title: "Day Book".to_owned(),
        period: "01-04-2026 to 20-11-2026".to_owned(),
        headings: vec!["Particulars".to_owned(), "Amount".to_owned()],
        rows: (0..rows)
            .map(|n| vec![format!("Row {n}"), "1,00.00".to_owned()])
            .collect(),
        notes: Vec::new(),
        aligns: Vec::new(),
        styles: Vec::new(),
    }
}

fn texts(page: &PageLayout) -> Vec<&str> {
    page.items
        .iter()
        .filter_map(|i| match i {
            PageItem::Text { text, .. } => Some(text.as_str()),
            PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect()
}

fn pdf_page_count(pdf: &[u8]) -> usize {
    // Every page object of the writer carries /Type /Page (the tree is /Type /Pages).
    String::from_utf8_lossy(pdf)
        .matches("/Type /Page\n")
        .count()
}

fn setup(f: impl FnOnce(&mut PrintSetup)) -> PrintSetup {
    let mut s = PrintSetup::default();
    f(&mut s);
    s
}

#[test]
fn the_default_setup_is_what_the_plain_renderer_writes() {
    let doc = render_report_pages_with(&report(150), &PrintSetup::default()).unwrap();
    assert_eq!((doc.pages[0].width, doc.pages[0].height), (595, 842));
    assert_eq!(doc.copies, 1);
    assert!(texts(&doc.pages[0]).contains(&format!("Page 1 of {}", doc.pages.len()).as_str()));
}

#[test]
fn a5_paper_lays_the_pages_out_on_148_by_210_mm() {
    let doc = render_report_pages_with(&report(3), &setup(|s| s.paper = Some(Paper::A5))).unwrap();
    assert_eq!((doc.pages[0].width, doc.pages[0].height), (420, 595));
    // A smaller page holds fewer rows: 150 rows need more pages than on A4 (at least 4 there).
    let long =
        render_report_pages_with(&report(150), &setup(|s| s.paper = Some(Paper::A5))).unwrap();
    let a4 = render_report_pages_with(&report(150), &PrintSetup::default()).unwrap();
    assert!(long.pages.len() > a4.pages.len());
}

#[test]
fn a_page_range_prints_only_those_pages_numbered_as_they_were() {
    let full = render_report_pages_with(&report(150), &PrintSetup::default()).unwrap();
    let total = full.pages.len();
    let doc = render_report_pages_with(
        &report(150),
        &setup(|s| s.pages = PageSelection::parse("2-3").unwrap()),
    )
    .unwrap();
    assert_eq!(doc.pages.len(), 2);
    assert!(texts(&doc.pages[0]).contains(&format!("Page 2 of {total}").as_str()));
    assert!(texts(&doc.pages[1]).contains(&format!("Page 3 of {total}").as_str()));
    // The heading repeats on each page, so a printed page 2 is still readable alone.
    assert!(texts(&doc.pages[0]).contains(&"Particulars"));
    assert_eq!(pdf_page_count(&doc.pdf), 2);
}

#[test]
fn a_range_that_selects_no_page_of_the_document_is_refused_not_printed_blank() {
    let err = render_report_pages_with(
        &report(3),
        &setup(|s| s.pages = PageSelection::parse("5-6").unwrap()),
    )
    .unwrap_err();
    let PrintError::Setup { reason } = err else {
        panic!("a range outside the document must be a setup error, got {err:?}");
    };
    assert!(reason.contains("5-6"), "{reason}");
    assert!(reason.contains("pages 1 to 1"), "{reason}");
}

#[test]
fn the_first_page_number_sets_the_number_of_the_first_page_and_counts_up() {
    // "Number to start from first page ... sets the starting page number, which then increments."
    let full = render_report_pages_with(&report(150), &PrintSetup::default()).unwrap();
    let total = full.pages.len();
    let doc = render_report_pages_with(&report(150), &setup(|s| s.first_page_number = 5)).unwrap();
    let last = 5 + total - 1;
    assert!(texts(&doc.pages[0]).contains(&format!("Page 5 of {last}").as_str()));
    assert!(texts(&doc.pages[1]).contains(&format!("Page 6 of {last}").as_str()));
    // A range is by the numbers on the pages: 6 is the second page here.
    let one = render_report_pages_with(
        &report(150),
        &setup(|s| {
            s.first_page_number = 5;
            s.pages = PageSelection::parse("6").unwrap();
        }),
    )
    .unwrap();
    assert_eq!(one.pages.len(), 1);
    assert!(texts(&one.pages[0]).contains(&format!("Page 6 of {last}").as_str()));
}

#[test]
fn copies_repeat_the_selected_pages_in_the_file_and_the_preview_shows_one() {
    let one = render_report_pages_with(&report(3), &PrintSetup::default()).unwrap();
    let three = render_report_pages_with(&report(3), &setup(|s| s.copies = 3)).unwrap();
    assert_eq!(three.copies, 3);
    assert_eq!(three.pages, one.pages);
    assert_eq!(pdf_page_count(&three.pdf), 3 * one.pages.len());
    assert_eq!(pdf_page_count(&one.pdf), one.pages.len());
}

#[test]
fn settings_that_cannot_be_used_are_refused_with_the_reason() {
    for bad in [
        setup(|s| s.copies = 0),
        setup(|s| s.copies = 100),
        setup(|s| s.first_page_number = 0),
    ] {
        assert!(
            matches!(
                render_report_pages_with(&report(3), &bad),
                Err(PrintError::Setup { .. })
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn a_page_range_is_read_as_numbers_and_from_to_ranges() {
    assert_eq!(PageSelection::parse("").unwrap(), PageSelection::All);
    assert_eq!(PageSelection::parse("  ").unwrap(), PageSelection::All);
    assert_eq!(
        PageSelection::parse("1, 3-5").unwrap(),
        PageSelection::Ranges(vec![(1, 1), (3, 5)])
    );
    for bad in ["x", "0", "4-2", "1-", "-3", "1,,2", "2-3-4"] {
        assert!(
            matches!(PageSelection::parse(bad), Err(PrintError::Setup { .. })),
            "{bad:?} should be refused"
        );
    }
}
