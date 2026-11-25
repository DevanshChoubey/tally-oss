//! Round S12: the trial balance, the outstanding bills and the stock summary list what the mock's
//! states list (design/mock-v2/index.html: `trial-balance.default`, `outstanding.receivables`,
//! `stock-summary.default`), while the screen keeps the engine's own report as it is.
//!
//! Provenance (derived): the sample company mirrors the mock's data. The mock's trial balance
//! starts with "Ananya Kulkarni, capital" under "Capital account"; its receivables list starts,
//! in the engine's party order, with Lakshmi Farm Supplies bill SI/0381 of 96,250.00 dated 20 Sep
//! 2026 (the mock's row "Lakshmi Farm Supplies SI/0381 20 Sep 2026 20 Oct 2026 96,250.00"); its
//! stock summary starts with "Drip lateral 16 mm, 400 m roll", HSN 3917, 187 roll.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::Duration;

use ledgerly_api::ReportKind;
use ledgerly_app::screens::report_view::{ReportViewScreen, ViewCell};
use ledgerly_app::shell::navigate::report_screen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

fn shown(name: &str, kind: ReportKind) -> ReportViewScreen {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (api, _path) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = report_screen(kind, &books).unwrap();
    for _ in 0..4000 {
        screen.tick(Some(&worker));
        if !screen.is_loading() {
            return screen;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the report did not arrive within the bound");
}

#[test]
fn the_trial_balance_cursor_starts_on_a_ledger_with_its_group() {
    let screen = shown("s12_look_tb", ReportKind::TrialBalance);
    let row = screen.selected_row().unwrap();
    assert_eq!(row.label, "Ananya Kulkarni, capital");
    assert_eq!(row.cells[0], ViewCell::Text("Capital Account".to_owned()));
    // The screen's own report is still the engine's: groups, condensed.
    assert!(
        screen
            .report()
            .rows
            .iter()
            .any(|r| r.label == "Capital Account")
    );
}

#[test]
fn the_receivables_list_one_line_per_bill() {
    let screen = shown(
        "s12_look_recv",
        ReportKind::Outstanding { receivable: true },
    );
    let row = screen.selected_row().unwrap();
    assert_eq!(row.label, "Lakshmi Farm Supplies");
    assert_eq!(row.cells[0], ViewCell::Text("SI/0381".to_owned()));
    assert_eq!(row.cells[1], ViewCell::Text("20 Sep 2026".to_owned()));
    assert_eq!(row.cells[2], ViewCell::Text("20 Oct 2026".to_owned()));
}

#[test]
fn the_stock_summary_lists_items_with_their_hsn_and_unit() {
    let screen = shown("s12_look_stock", ReportKind::StockSummary);
    let row = screen.selected_row().unwrap();
    assert_eq!(row.label, "Drip lateral 16 mm, 400 m roll");
    assert_eq!(row.cells[0], ViewCell::Text("3917".to_owned()));
    assert_eq!(row.cells[2], ViewCell::Text("roll".to_owned()));
}

mod headings {
    //! The label column's heading and the bare figures of the mock's tables.
    //!
    //! provenance: derived from design/mock-v2/index.html: the trial balance table is headed
    //! Ledger, Group, Debit, Credit; the receivables Customer, Bill, Bill date, Due, Open,
    //! Overdue; the payables Supplier; the stock summary Item, HSN, Quantity, Unit, Value. Its
    //! open amounts print bare (`96,250.00`, row SI/0381).

    use ledgerly_app::screens::report_view::generic::{column_cell_text, label_heading};
    use ledgerly_app::screens::report_view::{
        ReportToken, ViewCell, ViewDetail, ViewParams, ViewReport,
    };
    use ledgerly_core::{Paise, Period, SignedAmount};
    use time::macros::date;

    fn report(columns: &[&str]) -> ViewReport {
        ViewReport {
            token: ReportToken("t".to_owned()),
            title: "t".to_owned(),
            params: ViewParams {
                period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
                detail: ViewDetail::Detailed,
            },
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            rows: Vec::new(),
            totals: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn each_table_names_what_its_lines_are() {
        assert_eq!(
            label_heading(&report(&["Group", "Debit", "Credit"])),
            "Ledger"
        );
        assert_eq!(
            label_heading(&report(&["Bill", "Bill date", "Due", "Open Dr", "Overdue"])),
            "Customer"
        );
        assert_eq!(
            label_heading(&report(&["Bill", "Bill date", "Due", "Open Cr", "Overdue"])),
            "Supplier"
        );
        assert_eq!(
            label_heading(&report(&["HSN", "Quantity", "Unit", "Value Dr"])),
            "Item"
        );
        assert_eq!(label_heading(&report(&["Debit", "Credit"])), "Particulars");
    }

    #[test]
    fn an_open_amount_prints_bare_on_its_own_side_and_keeps_the_word_on_the_other() {
        let dr = ViewCell::Amount(SignedAmount::dr(Paise::new(9_625_000)));
        let cr = ViewCell::Amount(SignedAmount::cr(Paise::new(9_625_000)));
        assert_eq!(column_cell_text("Open Dr", &dr), "96,250.00");
        assert_eq!(column_cell_text("Open Dr", &cr), "96,250.00 Cr");
        assert_eq!(column_cell_text("Open Cr", &cr), "96,250.00");
        assert_eq!(column_cell_text("Value Dr", &dr), "96,250.00");
    }
}
