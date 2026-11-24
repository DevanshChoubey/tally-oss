//! Report screens drawn to the mock (card S8): the drill levels' words, the figures of the
//! Debit and Credit columns, a column that repeats the label, and the loading state.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap"
)]

use ledgerly_api::ReportKind;
use ledgerly_app::screens::report_view::balance;
use ledgerly_app::screens::report_view::generic;
use ledgerly_app::screens::report_view::{
    ReportToken, ViewCell, ViewDetail, ViewParams, ViewReport, ViewRow, ViewRowKind,
};
use ledgerly_app::screens::report_view::{
    VoucherView, drill_subtitle, drill_title, posting_columns, voucher_title,
};
use ledgerly_core::{GroupId, LedgerId, Paise, Period, SignedAmount};
use time::macros::date;

fn dr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

fn ledger_vouchers(dup_label: &str) -> ViewReport {
    let row = |label: &str, particulars: ViewCell| ViewRow {
        key: None,
        label: label.to_owned(),
        depth: 0,
        kind: ViewRowKind::Voucher,
        cells: vec![
            ViewCell::Date(date!(2026 - 11 - 10)),
            particulars,
            ViewCell::Text("Sales".to_owned()),
            dr(11_800_000),
        ],
    };
    ViewReport {
        token: ReportToken("lv".to_owned()),
        title: "Ledger Vouchers: Sample Buyer".to_owned(),
        params: ViewParams {
            period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
            detail: ViewDetail::Detailed,
        },
        columns: ["Date", "Particulars", "Voucher Type", "Debit"]
            .map(str::to_owned)
            .to_vec(),
        rows: vec![row(
            "Sales, Output CGST",
            ViewCell::Text(dup_label.to_owned()),
        )],
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

#[test]
fn a_particulars_column_that_repeats_the_label_is_not_drawn_twice() {
    // derived: the engine's ledger voucher row carries its counter ledgers as the row label and
    // again as the "Particulars" cell (ledgerly-reports ledger.rs `voucher_row`). The label
    // column is headed "Particulars" too, so the same words twice side by side is one column too
    // many (the 2026-11-24 capture of balance-sheet.ledger shows it clipped).
    let same = ledger_vouchers("Sales, Output CGST");
    assert!(generic::repeats_label(&same, 1));
    assert!(!generic::repeats_label(&same, 0));
    // A column headed alike whose text differs from the label is data and stays.
    let different = ledger_vouchers("Narration text");
    assert!(!generic::repeats_label(&different, 1));
}

#[test]
fn figures_in_the_debit_and_credit_columns_drop_the_side_word() {
    // provenance: index.html:1604-1605 (ledger drill) and 1647: the Debit and Credit columns
    // print `inr(x)` with no Dr or Cr. Derived: the column says the side, so a debit figure in
    // Debit and a credit figure in Credit are bare, and a figure on the other side of its column
    // keeps its word. Zero has no side. A column of another name keeps the word.
    assert_eq!(
        generic::column_cell_text("Debit", &dr(11_800_000)),
        "1,18,000.00"
    );
    assert_eq!(generic::column_cell_text("Credit", &cr(5_000)), "50.00");
    assert_eq!(generic::column_cell_text("Debit", &cr(5_000)), "50.00 Cr");
    assert_eq!(generic::column_cell_text("Credit", &dr(0)), "0.00");
    assert_eq!(generic::column_cell_text("Balance", &dr(5_000)), "50.00 Dr");
}

#[test]
fn a_drill_level_is_titled_by_what_was_drilled() {
    // provenance: index.html:1570-1574: the crumbs and the title of a drill level are the group's
    // or ledger's own name ("Sundry debtors", "Shree Ganesh Traders"), not a report name with a
    // colon. Derived: the engine titles them "Group Summary: <name>" and "Ledger Vouchers:
    // <name>" (ledgerly-reports ledger.rs); the screen drops the report's kind and keeps the name
    // the engine gave. Another kind, or a title without the prefix, is unchanged.
    let group = ReportKind::GroupSummary(GroupId(1));
    let ledger = ReportKind::LedgerVouchers(LedgerId(1));
    assert_eq!(
        drill_title("Group Summary: Sundry Debtors", &group),
        "Sundry Debtors"
    );
    assert_eq!(
        drill_title("Ledger Vouchers: Sample Buyer Pvt Ltd", &ledger),
        "Sample Buyer Pvt Ltd"
    );
    assert_eq!(drill_title("Group Summary: X", &ledger), "Group Summary: X");
    assert_eq!(
        drill_title("Trial Balance", &ReportKind::TrialBalance),
        "Trial Balance"
    );
}

#[test]
fn a_drill_level_says_enter_goes_deeper() {
    // provenance: index.html:1577: "As at 21 Nov 2026 · FY 2026-27 · Enter goes deeper, Esc goes
    // back up" for every level below the balance sheet.
    assert_eq!(
        drill_subtitle(date!(2026 - 11 - 21)),
        "As at 21 Nov 2026 \u{00B7} FY 2026-27 \u{00B7} Enter goes deeper, Esc goes back up"
    );
    // the balance sheet's own subtitle is unchanged
    assert!(
        balance::subtitle(date!(2026 - 11 - 21)).ends_with("Enter opens a row, Esc goes back up")
    );
}

#[test]
fn a_posting_sits_in_the_column_of_its_side_without_a_side_word() {
    // provenance: index.html:1621: a posting's figure is under Debit when positive and under
    // Credit when negative, printed as `inr(x)`, with the other column empty.
    assert_eq!(
        posting_columns(SignedAmount::dr(Paise::new(15_350_000))),
        ("1,53,500.00".to_owned(), String::new())
    );
    assert_eq!(
        posting_columns(SignedAmount::cr(Paise::new(1_170_765))),
        (String::new(), "11,707.65".to_owned())
    );
}

#[test]
fn a_voucher_is_titled_by_its_number() {
    // provenance: index.html:1574: "Voucher SI/0412" over a crumb "SI/0412".
    let view = |number: Option<&str>| VoucherView {
        type_name: "Sales".to_owned(),
        number: number.map(str::to_owned),
        date: date!(2026 - 11 - 14),
        status: "Regular",
        party: None,
        narration: String::new(),
        lines: Vec::new(),
        not_shown: Vec::new(),
    };
    assert_eq!(voucher_title(&view(Some("SI/0412"))), "Voucher SI/0412");
    assert_eq!(voucher_title(&view(None)), "Voucher (no number yet)");
}
