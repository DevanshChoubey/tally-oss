//! Card S14 follow-up: the printed text of a report. Provenance: `derived`. Every expected string
//! is written by hand from the rows given in each test (Indian grouping: last three digits, then
//! pairs), never read back from the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Paise, Period, SignedAmount};
use ledgerly_model::Scenario;
use ledgerly_reports::print_text::{PrintRowStyle, print_text};
use ledgerly_reports::{
    Cell, ColumnHeader, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams,
    ReportRow, RowKind,
};
use time::macros::date;

fn params(kind: ReportKind) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
        as_of: date!(2026 - 11 - 20),
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn dr(rupees: i64) -> Cell {
    Cell::Amount(SignedAmount::dr(Paise::new(rupees * 100)))
}
fn cr(rupees: i64) -> Cell {
    Cell::Amount(SignedAmount::cr(Paise::new(rupees * 100)))
}
fn row(label: &str, kind: RowKind, cells: Vec<Cell>) -> ReportRow {
    ReportRow {
        key: None,
        label: label.to_owned(),
        depth: 0,
        kind,
        cells,
    }
}
fn column(label: &str) -> ColumnHeader {
    ColumnHeader {
        label: label.to_owned(),
        period: None,
    }
}

fn hand_set_rows() -> Vec<ReportRow> {
    let signed = |n: i64| Cell::Signed(Paise::new(n * 100));
    vec![
        row("Trading account", RowKind::Text, vec![Cell::Empty]),
        row("Opening stock", RowKind::Item, vec![dr(20_000)]),
        row("Purchase Accounts", RowKind::Group, vec![dr(60_000)]),
        row("Direct Expenses", RowKind::Group, vec![dr(5_000)]),
        row("Sales Accounts", RowKind::Group, vec![cr(100_000)]),
        row("Closing stock", RowKind::Item, vec![cr(30_000)]),
        row("Gross profit", RowKind::Subtotal, vec![signed(45_000)]),
        row("Gross profit c/o", RowKind::Item, vec![dr(45_000)]),
        row("Trading total", RowKind::Subtotal, vec![dr(130_000)]),
        row("Profit and loss account", RowKind::Text, vec![Cell::Empty]),
        row("Gross profit b/f", RowKind::Item, vec![cr(45_000)]),
        row("Indirect Expenses", RowKind::Group, vec![dr(10_000)]),
        row("Indirect Incomes", RowKind::Group, vec![cr(2_000)]),
        row("Net profit", RowKind::Subtotal, vec![signed(37_000)]),
        row("Net profit c/o", RowKind::Item, vec![dr(37_000)]),
        row("Profit and loss total", RowKind::Subtotal, vec![dr(47_000)]),
    ]
}

#[test]
fn the_profit_and_loss_prints_as_two_sides_with_both_totals() {
    // The hand set of the engine's profit and loss tests: opening stock 20,000, purchases 60,000,
    // direct expenses 5,000, sales 1,00,000, closing stock 30,000. Gross profit 45,000 (c/o on the
    // debit side). Trading total 1,30,000 on each side. Profit and loss: indirect expenses 10,000,
    // indirect incomes 2,000, net profit 37,000 c/o; both sides 47,000.
    let report = Report {
        title: "Profit and loss".to_owned(),
        params: params(ReportKind::ProfitAndLoss),
        columns: vec![column("period")],
        rows: hand_set_rows(),
        totals: Vec::new(),
        notes: Vec::new(),
    };
    let t = print_text(&report).unwrap();
    assert_eq!(
        t.headings,
        ["Particulars", "Debit", "Particulars", "Credit"]
    );
    let got: Vec<(Vec<&str>, PrintRowStyle)> = t
        .rows
        .iter()
        .map(|r| (r.cells.iter().map(String::as_str).collect(), r.style))
        .collect();
    let want: Vec<(Vec<&str>, PrintRowStyle)> = vec![
        (vec!["Trading account", "", "", ""], PrintRowStyle::Band),
        (
            vec![
                "Opening stock",
                "20,000.00",
                "Sales Accounts",
                "1,00,000.00",
            ],
            PrintRowStyle::Plain,
        ),
        (
            vec![
                "Purchase Accounts",
                "60,000.00",
                "Closing stock",
                "30,000.00",
            ],
            PrintRowStyle::Plain,
        ),
        (
            vec!["Direct Expenses", "5,000.00", "", ""],
            PrintRowStyle::Plain,
        ),
        (
            vec!["Gross profit c/o", "45,000.00", "", ""],
            PrintRowStyle::Plain,
        ),
        (
            vec!["Total", "1,30,000.00", "Total", "1,30,000.00"],
            PrintRowStyle::Total,
        ),
        (
            vec!["Profit and loss account", "", "", ""],
            PrintRowStyle::Band,
        ),
        (
            vec![
                "Indirect Expenses",
                "10,000.00",
                "Gross profit b/f",
                "45,000.00",
            ],
            PrintRowStyle::Plain,
        ),
        (
            vec![
                "Net profit c/o",
                "37,000.00",
                "Indirect Incomes",
                "2,000.00",
            ],
            PrintRowStyle::Plain,
        ),
        (
            vec!["Total", "47,000.00", "Total", "47,000.00"],
            PrintRowStyle::Total,
        ),
    ];
    assert_eq!(got, want);
}

#[test]
fn a_figure_off_its_columns_side_keeps_dr_or_cr_and_nothing_is_a_bare_minus() {
    // A trial balance: Debit column 12,34,567.00 (bare), a credit figure in the Debit column
    // (an odd balance) keeps its `Cr`, a debit in the Credit column keeps `Dr`; zero is bare.
    let report = Report {
        title: "Trial Balance".to_owned(),
        params: params(ReportKind::TrialBalance),
        columns: vec![column("Debit"), column("Credit")],
        rows: vec![
            row("Rent", RowKind::Ledger, vec![dr(1_234_567), Cell::Empty]),
            row("Odd", RowKind::Ledger, vec![cr(5), Cell::Empty]),
            row("Odd2", RowKind::Ledger, vec![Cell::Empty, dr(7)]),
            row("Nil", RowKind::Ledger, vec![dr(0), Cell::Empty]),
        ],
        totals: vec![dr(1_234_562), dr(7)],
        notes: Vec::new(),
    };
    let t = print_text(&report).unwrap();
    let cells: Vec<Vec<&str>> = t
        .rows
        .iter()
        .map(|r| r.cells.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        cells,
        vec![
            vec!["Rent", "12,34,567.00", ""],
            vec!["Odd", "5.00 Cr", ""],
            vec!["Odd2", "", "7.00 Dr"],
            vec!["Nil", "0.00", ""],
            vec!["Total", "12,34,562.00", "7.00 Dr"],
        ]
    );
}
