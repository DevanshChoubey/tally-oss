//! Card S14: the pages the preview shows come from the same layout as the PDF.
//!
//! Provenance: `derived`. The contract is the module docs of `ledgerly_print::pdf`: a report is
//! an A4 page (210 x 297 mm = 595 x 842 points), the heading repeats on each page and "Page n of
//! N" is at the foot. The page count is bounded as in `p2_report_pdf.rs`: a table row is at
//! least 9 point text plus 2 x 3 padding, so 150 rows cannot fit on fewer than four pages.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::pdf::{PageItem, ReportPage, render_report, render_report_pages};

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

fn texts(page: &ledgerly_print::pdf::PageLayout) -> Vec<&str> {
    page.items
        .iter()
        .filter_map(|i| match i {
            PageItem::Text { text, .. } => Some(text.as_str()),
            PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn the_pages_come_with_the_same_pdf_bytes_the_report_renderer_writes() {
    // An identity check between two entry points of one renderer, not a copied figure.
    let page = report(150);
    let doc = render_report_pages(&page).unwrap();
    assert_eq!(doc.pdf, render_report(&page).unwrap());
}

#[test]
fn a_long_report_has_the_pages_the_pdf_has_each_numbered_and_with_the_heading() {
    let doc = render_report_pages(&report(150)).unwrap();
    assert!(doc.pages.len() >= 4, "{} pages", doc.pages.len());
    let total = doc.pages.len();
    for (n, page) in doc.pages.iter().enumerate() {
        assert_eq!((page.width, page.height), (595, 842));
        let foot = format!("Page {} of {total}", n + 1);
        assert!(
            texts(page).contains(&foot.as_str()),
            "page {} has no {foot:?}",
            n + 1
        );
        assert!(
            texts(page).contains(&"Particulars"),
            "page {} lost the heading",
            n + 1
        );
    }
    // The first page opens with the company, the title and the period, in that order.
    assert_eq!(
        texts(&doc.pages[0])[..3],
        ["Test Traders", "Day Book", "01-04-2026 to 20-11-2026"]
    );
}

#[test]
fn a_short_report_is_one_page_and_text_carries_the_width_the_engine_aligned_with() {
    let doc = render_report_pages(&report(3)).unwrap();
    assert_eq!(doc.pages.len(), 1);
    // The amount column is right aligned: its text ends at the column's right edge less the
    // padding, so x + width is the same for every amount (the preview anchors there).
    let ends: Vec<i32> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            PageItem::Text { x, width, text, .. } if text == "1,00.00" => Some(x + width),
            PageItem::Text { .. } | PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect();
    assert_eq!(ends.len(), 3);
    assert!(ends.iter().all(|e| *e == ends[0]), "{ends:?}");
}
