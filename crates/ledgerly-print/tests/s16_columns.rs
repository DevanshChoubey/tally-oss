//! Card S16: a figure column is as wide as its figures, so a printed amount never breaks across
//! two lines (the day book printed "9,12,00" over "0.00 Dr" before).
//!
//! Provenance: `derived`. The property is that a right-aligned figure is drawn as one text item:
//! the figure "9,12,000.00 Dr" is 14 characters, about 65 points at 9 point Helvetica (digits 556
//! thousandths of an em each, comma and full stop 278, space 278, D 722, r 333), which is more than
//! the 2/21 share of a 523 point A4 text width (about 50 points) the old fixed weights gave it in
//! a seven column table.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::pdf::{ColumnAlign, PageItem, ReportPage, render_report_pages};

#[test]
fn a_figure_is_never_broken_across_lines_even_among_many_columns() {
    let headings: Vec<String> = [
        "Particulars",
        "Date",
        "Voucher type",
        "Voucher no.",
        "Amount",
        "Status",
        "Narration",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let page = ReportPage {
        company: "Test Traders".to_owned(),
        title: "Day Book".to_owned(),
        period: "01-04-2026 to 31-03-2027".to_owned(),
        headings,
        rows: vec![vec![
            "Rent".to_owned(),
            "01-Jun-2026".to_owned(),
            "Payment".to_owned(),
            "N/0001".to_owned(),
            "9,12,000.00 Dr".to_owned(),
            String::new(),
            "June rent for the shop".to_owned(),
        ]],
        notes: Vec::new(),
        aligns: vec![
            ColumnAlign::Left,
            ColumnAlign::Left,
            ColumnAlign::Left,
            ColumnAlign::Left,
            ColumnAlign::Right,
            ColumnAlign::Left,
            ColumnAlign::Left,
        ],
        styles: Vec::new(),
    };
    let doc = render_report_pages(&page).unwrap();
    let texts: Vec<&str> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            PageItem::Text { text, .. } => Some(text.as_str()),
            PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect();
    assert!(texts.contains(&"9,12,000.00 Dr"), "{texts:?}");
}
