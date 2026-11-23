//! The balance sheet drawn to the mock (`design/mock-v2/index.html`, `SCREENS['balance-sheet']`
//! at lines 1569-1634 and the `table.t` rules at lines 148-158): the statement's row geometry and
//! fills, the amount text per section, the keys, the subtitle and the breadcrumb.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use ledgerly_app::screens::report_view::balance::{self, Section, VerticalBalance};
use ledgerly_app::screens::report_view::{
    ReportSource, ReportToken, ReportViewScreen, ViewCell, ViewDetail, ViewError, ViewKey,
    ViewParams, ViewReport, ViewRow, ViewRowKind,
};
use ledgerly_app::shell::KeyHint;
use ledgerly_app::shell::chrome::key_hints;
use ledgerly_app::theme::{ThemeKind, tokens};
use ledgerly_app::ui::table::{self, RowState};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Outcome, Paise, Period, SignedAmount};
use ledgerly_model::Scenario;
use time::macros::date;

fn dr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

// ---- the amount text of a row, per section ------------------------------------------------------

#[test]
fn an_amount_on_its_sections_own_side_drops_dr_or_cr() {
    // provenance: index.html:1585, the statement prints `inr(n.amt)`, a figure with no Dr or Cr;
    // the section says the side (liabilities are credit, assets debit). Derived: a figure on the
    // other side of its section (a loss under liabilities, an overdraft under assets) keeps its
    // Dr or Cr, so it is never read as the usual side. Zero has no side. Catches "5,00,000.00 Cr"
    // under Liabilities, and a loss shown as if it were a profit.
    let lia = Section::Liabilities;
    let ast = Section::Assets;
    assert_eq!(balance::amount_in(&cr(50_000_000), lia), "5,00,000.00");
    assert_eq!(balance::amount_in(&dr(100_000), lia), "1,000.00 Dr");
    assert_eq!(balance::amount_in(&dr(65_200_000), ast), "6,52,000.00");
    assert_eq!(balance::amount_in(&cr(250_000), ast), "2,500.00 Cr");
    assert_eq!(balance::amount_in(&dr(0), lia), "0.00");
    assert_eq!(balance::amount_in(&cr(0), ast), "0.00");
    let provisional = ViewCell::Provisional {
        amount: SignedAmount::dr(Paise::new(142_553_900)),
        reason: "negative stock".to_owned(),
    };
    assert_eq!(
        balance::amount_in(&provisional, ast),
        "14,25,539.00 (provisional: negative stock)"
    );
    assert_eq!(
        balance::amount_in(&ViewCell::Text("n/a".to_owned()), lia),
        "n/a"
    );
}

// ---- the statement's geometry ----------------------------------------------------------------

#[test]
fn rows_indent_like_the_mocks_ind1_and_ind2() {
    // provenance: index.html:150 and 158: a cell has 10 px of padding; `ind1` is 26 and `ind2` 42,
    // so each level adds 16. Derived: the mock stops at two levels only because its sample is two
    // deep; a third level continues the step (58) so it stays apart from the second.
    assert!((table::indent(0) - 10.0).abs() < f32::EPSILON);
    assert!((table::indent(1) - 26.0).abs() < f32::EPSILON);
    assert!((table::indent(2) - 42.0).abs() < f32::EPSILON);
    assert!((table::indent(3) - 58.0).abs() < f32::EPSILON);
}

fn four_rows() -> VerticalBalance {
    VerticalBalance {
        liabilities: vec![0, 1, 2],
        assets: vec![3],
        liabilities_total: cr(65_200_000),
        assets_total: dr(65_200_000),
    }
}

#[test]
fn each_line_of_the_statement_has_the_mocks_height() {
    // provenance: derived from index.html:149-150, 156 and 1593-1594, with 13 px body text at
    // line-height 1.45 drawn on whole points (19, `running_text_uses_the_mocks_line_height`) and
    // 12 px headings (17):
    // * heading `th`: 6 + 17 + 6 + 1 px rule = 30;
    // * the Assets heading has `padding-top:18px`: 18 + 17 + 6 + 1 = 42;
    // * a row `td`: 7 + 19 + 7 + 1 px rule = 34;
    // * the total: 7 + 19 + 7, its top rule shared with the row above (border-collapse), and the
    //   3 px double rule under it: 36.
    // Three liabilities and one asset: heading, 3 rows, total, heading, 1 row, total.
    assert_eq!(
        balance::line_heights(&four_rows()),
        [30.0, 34.0, 34.0, 34.0, 36.0, 42.0, 34.0, 36.0]
    );
}

#[test]
fn a_row_is_filled_by_the_mocks_cascade() {
    // provenance: index.html:151-157, in source order: `sel` is accent-soft, hover is `--hover`,
    // a selected row under the pointer stays accent-soft, and `sub` (a top-level row) is
    // surface-2. `tr.sub td` comes after `tr.sel td` with the same specificity, so a selected
    // top-level row is surface-2 (the capture of 2026-11-23 shows Capital account grey with the
    // accent bar); the hover rules are more specific and win over both.
    let t = tokens(ThemeKind::Light);
    let fill = |selected, hovered, sub| {
        table::row_fill(
            &t,
            RowState {
                selected,
                hovered,
                sub,
            },
        )
    };
    assert_eq!(fill(false, false, false), None);
    assert_eq!(fill(true, false, false), Some(t.accent_soft));
    assert_eq!(fill(false, true, false), Some(t.hover));
    assert_eq!(fill(true, true, false), Some(t.accent_soft));
    assert_eq!(fill(false, false, true), Some(t.surface_2));
    assert_eq!(fill(true, false, true), Some(t.surface_2));
    assert_eq!(fill(false, true, true), Some(t.hover));
    assert_eq!(fill(true, true, true), Some(t.accent_soft));
}

// ---- the screen's words ----------------------------------------------------------------------

fn balance_screen(detail: Detail) -> ReportViewScreen {
    let period = Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 21)).unwrap();
    let params = ReportParams {
        kind: ReportKind::BalanceSheet,
        period,
        as_of: date!(2026 - 11 - 21),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    ReportViewScreen::worker(ReportSource::worker(params)).unwrap()
}

#[test]
fn the_subtitle_names_the_financial_year_as_the_mock_does() {
    // provenance: index.html:1577: "As at 21 Nov 2026 · FY 2026-27 · Enter opens a row, Esc goes
    // back up". 21 Nov 2026 falls in the year that starts 1 April 2026, labelled 2026-27.
    // Catches the subtitle without the year.
    let screen = balance_screen(Detail::Condensed);
    assert_eq!(
        screen.subtitle().as_deref(),
        Some("As at 21 Nov 2026 \u{00B7} FY 2026-27 \u{00B7} Enter opens a row, Esc goes back up")
    );
}

#[test]
fn the_breadcrumb_names_the_balance_sheet_as_the_page_does() {
    // provenance: index.html:1572: the crumb is "Balance sheet", the page title's wording.
    // Catches the engine's "Balance Sheet" in the breadcrumb under a "Balance sheet" heading.
    let screen = balance_screen(Detail::Condensed);
    assert_eq!(screen.title(), "Balance sheet");
    assert_eq!(screen.page_title(), "Balance sheet");
}

fn pairs(hints: &[KeyHint]) -> Vec<(String, String)> {
    hints
        .iter()
        .map(|h| (h.keys.clone(), h.label.clone()))
        .collect()
}

fn owned(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, l)| ((*k).to_owned(), (*l).to_owned()))
        .collect()
}

#[test]
fn the_key_bar_reads_like_the_mocks() {
    // provenance: index.html:1634: "J / K" Move, Enter Drill in, Esc Back up, Alt+F1 Detailed.
    // Alt+F1 names what it switches to, as the button does (index.html:1576: "Condensed" in the
    // detailed view). F2 keeps "Period": ours opens a from-to period editor, not a single date
    // (design/PARITY-GAPS.md). Ctrl+E Export comes before F2, as in the mock. Catches "Up /
    // Down" and "Detailed or condensed".
    let condensed = balance_screen(Detail::Condensed);
    assert_eq!(
        pairs(&condensed.keys()),
        owned(&[
            ("J / K", "Move"),
            ("Enter", "Drill in"),
            ("Esc", "Back up"),
            ("Alt+F1", "Detailed"),
            ("Ctrl+E", "Export"),
            ("F2", "Period"),
        ])
    );
    let detailed = balance_screen(Detail::Detailed);
    let alt: Vec<String> = detailed
        .keys()
        .iter()
        .filter(|h| h.keys == "Alt+F1")
        .map(|h| h.label.clone())
        .collect();
    assert_eq!(alt, ["Condensed"]);
}

#[test]
fn a_screen_that_names_esc_gets_no_second_esc() {
    // provenance: index.html:910-914 (`renderKeys`) lists the screen's keys and nothing more; the
    // balance sheet's own "Esc Back up" is its Esc. Catches the 2026-11-23 capture's key bar,
    // which read "Esc Back up" and then "Esc Back".
    let screen = [KeyHint::new("Esc", "Back up")];
    assert_eq!(key_hints(&screen, true), owned(&[("Esc", "Back up")]));
}

// ---- J and K move -----------------------------------------------------------------------------

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

fn press(key: Key) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

#[test]
fn j_and_k_move_the_selection() {
    // provenance: index.html:1634 and 1656: the balance sheet and the day book both offer
    // "J / K" Move (the mock's `listKeys`). Catches a hint for keys that do nothing.
    let rows = (0..3)
        .map(|i| ViewRow {
            key: None,
            label: format!("Row {i}"),
            depth: 0,
            kind: ViewRowKind::Ledger,
            cells: vec![ViewCell::Text(i.to_string())],
        })
        .collect();
    let report = ViewReport {
        token: ReportToken("r".to_owned()),
        title: "R".to_owned(),
        params: ViewParams {
            period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 21)).unwrap(),
            detail: ViewDetail::Condensed,
        },
        columns: vec!["Amount".to_owned()],
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    };
    let mut screen = ReportViewScreen::new(report, host_source()).unwrap();
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut frame = |screen: &mut ReportViewScreen, input| {
        let out = ctx.run_ui(input, |ui| {
            screen.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    };
    frame(&mut screen, press(Key::J));
    frame(&mut screen, press(Key::J));
    assert_eq!(screen.selected(), 2);
    frame(&mut screen, press(Key::K));
    assert_eq!(screen.selected(), 1);
}
