//! The generic report table drawn to the mock's `table.t` lines (`design/mock-v2/index.html`
//! lines 148-158, as the ledger drill state draws it at about line 1600): the line heights, the
//! drawn row and the scroll to a selected line in a long, virtualised register (spec 8).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap; layout is measured in points (rule A3 allows floats for app layout)"
)]

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::report_view::generic::{self, ColumnKind};
use ledgerly_app::screens::report_view::{
    ReportSource, ReportToken, ReportViewScreen, ViewCell, ViewDetail, ViewError, ViewKey,
    ViewParams, ViewReport, ViewRow, ViewRowKind,
};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Outcome, Paise, Period, SignedAmount};
use time::macros::date;

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

/// A one-column report of `n` ledger rows, with a total line when `totals` is set.
fn report(n: usize, totals: bool) -> ViewReport {
    let rows = (0..n)
        .map(|i| ViewRow {
            key: None,
            label: format!("Row {i}"),
            depth: 0,
            kind: ViewRowKind::Ledger,
            cells: vec![ViewCell::Text(i.to_string())],
        })
        .collect();
    ViewReport {
        token: ReportToken("r".to_owned()),
        title: "R".to_owned(),
        params: ViewParams {
            period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 21)).unwrap(),
            detail: ViewDetail::Condensed,
        },
        columns: vec!["Amount".to_owned()],
        rows,
        totals: if totals {
            vec![ViewCell::Text("total".to_owned())]
        } else {
            Vec::new()
        },
        notes: Vec::new(),
    }
}

/// One frame's input at `time` seconds; egui animates a scroll, so frames a second apart let it
/// finish.
fn input(key: Option<Key>, time: f64) -> RawInput {
    RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events: key
            .map(|key| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .into_iter()
            .collect(),
        ..RawInput::default()
    }
}

fn frame(ctx: &egui::Context, screen: &mut ReportViewScreen, key: Option<Key>) {
    let mut app = AppCtx::default();
    let time = ctx.input(|i| i.time) + 1.0;
    let out = ctx.run_ui(input(key, time), |ui| {
        let _stay = screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn each_line_of_the_generic_table_has_the_mocks_height() {
    // provenance: derived from index.html:149-150 and 156 with 13 px body text at line-height
    // 1.45 drawn on whole points (19) and 12 px headings (17), the same derivation as the balance
    // sheet's (`each_line_of_the_statement_has_the_mocks_height`):
    // * heading `th`: 6 + 17 + 6 + 1 px rule = 30;
    // * a row `td`: 7 + 19 + 7 + 1 px rule = 34;
    // * the total: 7 + 19 + 7 and the 3 px double rule = 36.
    assert_eq!(
        generic::line_heights(&report(3, true)),
        [30.0, 34.0, 34.0, 34.0, 36.0]
    );
    // No totals, no total line.
    assert_eq!(generic::line_heights(&report(2, false)), [30.0, 34.0, 34.0]);
}

#[test]
fn a_drawn_line_of_the_generic_table_is_34_points_tall() {
    // provenance: derived above: a `table.t` body line is 34 points. Catches the table still
    // drawn at its older 22-point rows.
    let ctx = egui::Context::default();
    let mut screen = ReportViewScreen::new(report(5, true), host_source()).unwrap();
    frame(&ctx, &mut screen, None);
    frame(&ctx, &mut screen, None);
    let line = screen
        .selected_line_rect()
        .expect("the selected line was drawn");
    assert!((line.height() - 34.0).abs() < 0.01, "{line:?}");
}

#[test]
fn end_brings_the_last_of_200_lines_into_view_above_the_footnote() {
    // provenance: spec 7 "J / K" and End move the selection, and spec 8 keeps a long register
    // virtualised. Derived: 200 lines of 34 points (6,800) do not fit an 800-point window, so
    // the last line can only be in view if the table scrolled to it; in view means between the
    // head and the footnote area pinned under the table.
    let ctx = egui::Context::default();
    let mut screen = ReportViewScreen::new(report(200, true), host_source()).unwrap();
    frame(&ctx, &mut screen, None);
    frame(&ctx, &mut screen, Some(Key::End));
    frame(&ctx, &mut screen, None);
    frame(&ctx, &mut screen, None);
    assert_eq!(screen.selected(), 199);
    let line = screen
        .selected_line_rect()
        .expect("the last line was drawn");
    let foot = screen.footer_rect().expect("the footnote area was drawn");
    assert!(line.bottom() <= foot.top() + 0.5, "{line:?} under {foot:?}");
    assert!(line.top() >= 0.0, "{line:?}");
}

// ---- text columns and figure columns ------------------------------------------------------------

fn ledger_vouchers() -> ViewReport {
    // The shape of the engine's ledger vouchers report: a date, text and figures; the first row
    // (the opening balance) has no date.
    let row = |date: ViewCell, kind: &str, debit: ViewCell| ViewRow {
        key: None,
        label: "Line".to_owned(),
        depth: 0,
        kind: ViewRowKind::Voucher,
        cells: vec![date, ViewCell::Text(kind.to_owned()), debit],
    };
    let mut r = report(0, false);
    r.columns = vec![
        "Date".to_owned(),
        "Voucher Type".to_owned(),
        "Debit".to_owned(),
    ];
    r.rows = vec![
        row(ViewCell::Empty, "", ViewCell::Empty),
        row(
            ViewCell::Date(date!(2026 - 11 - 10)),
            "Sales",
            ViewCell::Amount(SignedAmount::dr(Paise::new(11_800_000))),
        ),
    ];
    r
}

#[test]
fn dates_and_text_are_text_columns_and_amounts_are_figure_columns() {
    // provenance: index.html:1600: the ledger drill's Date, Voucher, Type and Narration are plain
    // `th`/`td` (left-aligned) and Debit, Credit and Balance are `th.num` (right-aligned). The
    // kind is read from the first cell that is not empty, so the empty opening row decides
    // nothing.
    assert_eq!(
        generic::column_kinds(&ledger_vouchers()),
        [ColumnKind::Text, ColumnKind::Text, ColumnKind::Figure]
    );
}

#[test]
fn text_columns_share_the_width_left_with_the_label() {
    // provenance: derived. A figure column keeps 170 points (the table's width for figures
    // before this round); the label and the text columns share what is left equally. At 1000
    // points with one text and two figure columns: 1000 - 2 x 170 = 660, shared by 2 = 330.
    // Label 0-330, text 330-660, figures 660-830 and 830-1000.
    assert_eq!(
        generic::column_spans(
            1000.0,
            &[ColumnKind::Text, ColumnKind::Figure, ColumnKind::Figure]
        ),
        [
            (0.0, 330.0),
            (330.0, 660.0),
            (660.0, 830.0),
            (830.0, 1000.0)
        ]
    );
}
