//! Wave 2, item S-D: the balance sheet and the day book on the report viewer, and the footnote
//! fix (B7).
//!
//! Provenance of the expected values:
//! * the mock screens: `design/mock-v2/index.html`, the `balance-sheet` and `day-book` renders,
//!   cited by line where a string is copied;
//! * hand derivations written next to each figure. No figure is taken from the code under test.
//! * the Tally help page for the balance sheet, read 2026-11-22:
//!   <https://help.tallysolutions.com/tally/balance-sheet-tally/>.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap"
)]

use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::report_view::balance::{VerticalBalance, sides_agree, vertical_balance};
use ledgerly_app::screens::report_view::day_book::{
    DAY_TAB, DayColumns, MONTH_TAB, day_columns, period_of_tab, subtitle, tab_of,
};
use ledgerly_app::screens::report_view::{
    ReportSource, ReportToken, ReportViewScreen, ViewCell, ViewDetail, ViewError, ViewKey,
    ViewParams, ViewReport, ViewRow, ViewRowKind,
};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Outcome, Paise, Period, SignedAmount};
use time::macros::date;

// ---- fixtures --------------------------------------------------------------------------------

fn period_fy() -> Period {
    Period::new(date!(2025 - 04 - 01), date!(2026 - 03 - 31)).unwrap()
}

fn params() -> ViewParams {
    ViewParams {
        period: period_fy(),
        detail: ViewDetail::Condensed,
    }
}

fn dr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

fn row(label: &str, cells: Vec<ViewCell>) -> ViewRow {
    ViewRow {
        key: None,
        label: label.to_owned(),
        depth: 0,
        kind: ViewRowKind::Group,
        cells,
    }
}

/// The balance sheet of the mock's condensed view, as the engine lays it out: a figure in the
/// liabilities column or the assets column, never both.
///
/// Derivation of the totals: liabilities 5,00,000.00 + 27,000.00 + 1,25,000.00 = 6,52,000.00 Cr,
/// that is `50_000_000` + `2_700_000` + `12_500_000` = `65_200_000` paise. Assets 6,52,000.00 Dr, which is
/// `65_200_000` paise. The figures match the mock's condensed statement (balance-sheet.default).
fn balance_sheet() -> ViewReport {
    ViewReport {
        token: ReportToken("bs".to_owned()),
        title: "Balance Sheet".to_owned(),
        params: params(),
        columns: vec!["Liabilities".to_owned(), "Assets".to_owned()],
        rows: vec![
            row("Capital Account", vec![cr(50_000_000), ViewCell::Empty]),
            row("Current Liabilities", vec![cr(2_700_000), ViewCell::Empty]),
            row("Profit and Loss A/c", vec![cr(12_500_000), ViewCell::Empty]),
            row("Current Assets", vec![ViewCell::Empty, dr(65_200_000)]),
        ],
        totals: vec![cr(65_200_000), dr(65_200_000)],
        notes: Vec::new(),
    }
}

fn host_source() -> ReportSource {
    ReportSource::new(
        |_: &ReportToken, _: ViewParams| -> Result<Outcome<ViewReport>, ViewError> {
            Err(ViewError::Source {
                message: "not used in this test".to_owned(),
            })
        },
        |_: &ReportToken, _: &ViewKey, _: ViewParams| {
            Err(ViewError::Source {
                message: "not used in this test".to_owned(),
            })
        },
    )
}

// ---- B7: the footnote stays inside the window ------------------------------------------------

#[test]
fn a_tall_report_in_a_short_window_keeps_its_footnote_inside_the_window() {
    // Window: 1280 by 300 points. The report has 60 rows (22 points each, 1,320 points of body)
    // and three notes. Before the fix the table took the window height less 48 points, so the
    // notes were drawn below the bottom edge, under the key bar. After the fix the notes are a
    // bottom panel, so their bottom edge is the window's bottom edge.
    let rows: Vec<ViewRow> = (0..60)
        .map(|i| {
            row(
                &format!("Ledger {}", i + 1),
                vec![ViewCell::Text(format!("{}", i + 1))],
            )
        })
        .collect();
    let report = ViewReport {
        token: ReportToken("tall".to_owned()),
        title: "Tall".to_owned(),
        params: params(),
        columns: vec!["Amount".to_owned()],
        rows,
        totals: Vec::new(),
        notes: vec![
            "Scenario: Actual. Balances as of 2026-03-31.".to_owned(),
            "Profit and Loss A/c is the profit from the beginning of the books.".to_owned(),
            "The figures are provisional because stock is provisional.".to_owned(),
        ],
    };
    let mut screen = ReportViewScreen::new(report, host_source()).unwrap();
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 300.0));
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    for _ in 0..3 {
        let input = RawInput {
            screen_rect: Some(window),
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            screen.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    }
    let footer = screen
        .footer_rect()
        .expect("the footnote is drawn in the first frame");
    assert!(
        footer.min.y >= window.min.y && footer.max.y <= window.max.y + 0.5,
        "the footnote {footer:?} must lie inside the window {window:?}"
    );
}

// ---- balance sheet: vertical placement -------------------------------------------------------

#[test]
fn the_balance_sheet_places_each_figure_in_its_section() {
    let v = vertical_balance(&balance_sheet()).unwrap();
    // Rows 0 to 2 are liabilities, row 3 is an asset; the order of the report is kept.
    assert_eq!(v.liabilities, vec![0, 1, 2]);
    assert_eq!(v.assets, vec![3]);
    assert_eq!(v.liabilities_total, cr(65_200_000));
    assert_eq!(v.assets_total, dr(65_200_000));
}

#[test]
fn a_row_with_figures_on_both_sides_or_none_is_not_placed_vertically() {
    // Both sides filled: the figure would be in two sections, so the caller draws the table.
    let mut both = balance_sheet();
    both.rows[0].cells[1] = dr(1);
    assert_eq!(vertical_balance(&both), None);
    // No figure at all: the row would be in neither section, so it is not placed either.
    let mut none = balance_sheet();
    none.rows[1].cells[0] = ViewCell::Empty;
    assert_eq!(vertical_balance(&none), None);
}

#[test]
fn a_report_that_is_not_a_two_column_statement_is_not_placed() {
    let mut one_column = balance_sheet();
    one_column.columns.truncate(1);
    assert_eq!(vertical_balance(&one_column), None);
    let mut no_totals = balance_sheet();
    no_totals.totals.clear();
    assert_eq!(vertical_balance(&no_totals), None);
}

#[test]
fn the_two_totals_agree_when_the_amounts_match_and_the_sides_are_opposite() {
    let v = vertical_balance(&balance_sheet()).unwrap();
    assert_eq!(sides_agree(&v), Some(true));
    // One paisa more on the assets side: the sides differ.
    let differ = VerticalBalance {
        assets_total: dr(65_200_001),
        ..v.clone()
    };
    assert_eq!(sides_agree(&differ), Some(false));
    // Same side on both: the amounts match but the statement does not balance.
    let same_side = VerticalBalance {
        assets_total: cr(65_200_000),
        ..v
    };
    assert_eq!(sides_agree(&same_side), Some(false));
}

#[test]
fn a_total_that_is_not_an_amount_has_nothing_to_compare() {
    let v = VerticalBalance {
        liabilities: Vec::new(),
        assets: Vec::new(),
        liabilities_total: ViewCell::Unsupported(ledgerly_core::Unsupported {
            what: "closing stock".to_owned(),
            reason: "not valued".to_owned(),
        }),
        assets_total: dr(0),
    };
    assert_eq!(sides_agree(&v), None);
}

// ---- day book --------------------------------------------------------------------------------

fn november() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 21)).unwrap()
}

#[test]
fn the_day_book_finds_its_columns_by_heading_in_the_engine_order() {
    // The headings the engine's day book writes (ledgerly-reports `list_report`).
    let headings: Vec<String> = ["Date", "Voucher type", "Voucher no.", "Amount", "Status"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(
        day_columns(&headings),
        Some(DayColumns {
            date: 0,
            kind: 1,
            number: 2,
            amount: 3,
            status: 4,
        })
    );
}

#[test]
fn the_day_book_is_not_mapped_when_a_column_is_missing() {
    // Without a Status heading the report is drawn as it is, so no figure lands under the wrong
    // heading.
    let headings: Vec<String> = ["Date", "Voucher type", "Voucher no.", "Amount"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(day_columns(&headings), None);
}

#[test]
fn the_day_tab_shows_the_last_day_and_the_month_tab_shows_the_month_to_that_day() {
    // Period 1 to 21 November 2026 (mock day-book.month, "1 to 21 Nov 2026").
    // The Day tab is the single day that ends the period: 21 November. The Month tab starts on
    // the first of that month and ends on the same day: 1 to 21 November.
    // Spec change 2026-11-23 (mock index.html:1645): `tab_of` answers `None` for a period that
    // is neither tab, so it now returns an `Option`; the two periods here keep their tabs.
    assert_eq!(tab_of(november()), Some(MONTH_TAB));
    let day = Period::new(date!(2026 - 11 - 21), date!(2026 - 11 - 21)).unwrap();
    assert_eq!(tab_of(day), Some(DAY_TAB));
    assert_eq!(period_of_tab(november(), DAY_TAB), Some(day));
    assert_eq!(period_of_tab(november(), MONTH_TAB), Some(november()));
}

#[test]
fn the_day_book_subtitle_reads_like_the_mock() {
    // Mock day-book render, index.html line 1644: "20 Nov 2026 · F2 another date" for a day,
    // "1 to 21 Nov 2026" for a month. The day here is 21 November, so the text follows the same
    // pattern with the other date.
    //
    // Spec change 2026-11-23: the day text now carries the mock's whole line, "· Alt+F2 a
    // period" included (index.html:1644, and TallyHelp "Viewing Day Book": Alt+F2 asks for a
    // period). The month text is the mock's "1 to 21 Nov 2026 · every voucher, in order"
    // (index.html:1644), not our earlier "F2 another period".
    let day = Period::new(date!(2026 - 11 - 21), date!(2026 - 11 - 21)).unwrap();
    assert_eq!(
        subtitle(day),
        "21 Nov 2026 · F2 another date · Alt+F2 a period"
    );
    assert_eq!(
        subtitle(november()),
        "1 to 21 Nov 2026 · every voucher, in order"
    );
}

#[test]
fn the_empty_day_book_names_the_day_as_the_mock_does() {
    // Mock day-book empty state, index.html line 1646: "No vouchers on 21 Nov 2026 yet".
    let day = Period::new(date!(2026 - 11 - 21), date!(2026 - 11 - 21)).unwrap();
    assert_eq!(
        ledgerly_app::screens::report_view::day_book::empty_title(day),
        "No vouchers on 21 Nov 2026 yet"
    );
}
