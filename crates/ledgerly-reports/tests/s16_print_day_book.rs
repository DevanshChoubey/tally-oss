//! Card S16: the printed day book keeps what a day book is (date, voucher type, voucher number,
//! particulars, amount, and the status and narration when there are any) and leaves the
//! screen-only audit columns (entered by, origin) off the page.
//!
//! Provenance: `derived`. The help page "Day Book"
//! (<https://help.tallysolutions.com/day-book-tally/>, read 2026-11-26) says the day book shows
//! "all your transactions that were recorded on a particular day or a period" and that narration
//! comes in through its F12 configuration; it says nothing of who entered a voucher, which is
//! Ledgerly's audit information. The cell texts are written by hand from the rows given here
//! (Indian grouping: last three digits, then pairs): `9,12,000.00` for `91_200_000` paise.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Paise, Period, SignedAmount};
use ledgerly_model::Scenario;
use ledgerly_reports::print_text::print_text;
use ledgerly_reports::{
    Cell, ColumnHeader, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams,
    ReportRow, RowKind,
};
use time::macros::date;

fn column(label: &str) -> ColumnHeader {
    ColumnHeader {
        label: label.to_owned(),
        period: None,
    }
}

fn day_book(narration: &str, status: &str) -> Report {
    let amount = Cell::Amount(SignedAmount::dr(Paise::new(91_200_000)));
    Report {
        title: "Day Book".to_owned(),
        params: ReportParams {
            kind: ReportKind::DayBook,
            period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
            as_of: date!(2026 - 06 - 30),
            scenario: Scenario::Actual,
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        },
        columns: [
            "Date",
            "Voucher type",
            "Voucher no.",
            "Amount",
            "Status",
            "Narration",
            "Entered by",
            "Origin",
        ]
        .iter()
        .map(|l| column(l))
        .collect(),
        rows: vec![ReportRow {
            key: None,
            label: "Rent".to_owned(),
            depth: 0,
            kind: RowKind::Voucher,
            cells: vec![
                Cell::Date(date!(2026 - 06 - 01)),
                Cell::Text("Payment".to_owned()),
                Cell::Text("PV/0001".to_owned()),
                amount.clone(),
                if status.is_empty() {
                    Cell::Empty
                } else {
                    Cell::Text(status.to_owned())
                },
                if narration.is_empty() {
                    Cell::Empty
                } else {
                    Cell::Text(narration.to_owned())
                },
                Cell::Text("User 1".to_owned()),
                Cell::Text("Typed".to_owned()),
            ],
        }],
        totals: vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            amount,
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
        ],
        notes: Vec::new(),
    }
}

#[test]
fn the_audit_columns_stay_off_the_printed_day_book() {
    let text = print_text(&day_book("", "")).unwrap();
    assert_eq!(
        text.headings,
        [
            "Particulars",
            "Date",
            "Voucher type",
            "Voucher no.",
            "Amount"
        ]
    );
    assert_eq!(text.rows[0].cells[0], "Rent");
    assert_eq!(text.rows[0].cells[3], "PV/0001");
    assert_eq!(text.rows[0].cells[4], "9,12,000.00 Dr");
    assert_eq!(text.rows.len(), 2, "one voucher and the total");
    assert_eq!(text.rows[1].cells[4], "9,12,000.00 Dr");
}

#[test]
fn narration_and_status_print_when_a_voucher_has_them() {
    let text = print_text(&day_book("June rent", "Cancelled")).unwrap();
    assert_eq!(
        text.headings,
        [
            "Particulars",
            "Date",
            "Voucher type",
            "Voucher no.",
            "Amount",
            "Status",
            "Narration"
        ]
    );
    assert_eq!(text.rows[0].cells[5], "Cancelled");
    assert_eq!(text.rows[0].cells[6], "June rent");
    assert_eq!(text.aligns.len(), 7);
}
