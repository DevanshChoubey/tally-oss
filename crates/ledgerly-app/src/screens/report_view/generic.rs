//! Every report without a look of its own, drawn to the mock's `table.t` lines
//! (`design/mock-v2/index.html` lines 148-158; the ledger drill state at about line 1600 draws a
//! register this way): a heading line, one line per row and a total line, in a card.
//!
//! The rows scroll between the heading and the total, and only the rows in view are laid out, so
//! a long register costs only what is on screen (spec 8). The screen moves figures into cells and
//! never computes one.

use egui::{Frame, Rect, Role, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use ledgerly_core::DrCr;

use super::text::{amount_on_side, cell_text};
use super::{TableEvents, ViewCell, ViewReport, ViewRow, ViewRowKind};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge_width, paint_badge};
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT, T_TOTAL_HEIGHT};
use crate::ui::{a11y, bold_font, kind, tokens};

/// Heading of the label column.
const LABEL_HEADING: &str = "Particulars";
/// Heading of the label column of the group ledgers report (index.html:1600, "Ledger or group").
const GROUP_LABEL_HEADING: &str = "Ledger or group";
/// Width a figure column takes when there is room (the table's earlier width).
const FIGURE_WIDTH: f32 = 170.0;
/// The narrowest a figure column gets on a narrow window.
const FIGURE_MIN: f32 = 90.0;
/// The narrowest the label and each text column get before the figure columns give way.
const FILL_MIN: f32 = 120.0;

/// How a column's cells sit in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Text and dates: left-aligned, sharing the width left over with the label (`td`).
    Text,
    /// Amounts and quantities: right-aligned in a fixed width (`td.num`).
    Figure,
}

/// The heading of the label column: the group ledgers report (its first column is the GSTIN)
/// lists ledgers and groups, every other report lists particulars.
#[must_use]
pub fn label_heading(report: &ViewReport) -> &'static str {
    let columns = &report.columns;
    if columns.first().is_some_and(|c| c == "GSTIN") {
        GROUP_LABEL_HEADING
    } else if columns.first().is_some_and(|c| c == super::mock::TB_GROUP) {
        // index.html `trial-balance.default`: Ledger, Group, Debit, Credit.
        "Ledger"
    } else if columns.first().is_some_and(|c| c == "Date") && columns.iter().any(|c| c == "User") {
        // `audit.default`: Seq, Date, Time, User, Action, Object (see `mock::audit_log`).
        "Seq"
    } else if columns.iter().any(|c| c == "Open Dr") {
        // `outstanding.receivables`: Customer, Bill, Bill date, Due, Open, Overdue.
        "Customer"
    } else if columns.iter().any(|c| c == "Open Cr") {
        "Supplier"
    } else if columns.iter().any(|c| c == super::mock::VALUE_HEADING) {
        // `stock-summary.default`: Item, HSN, Quantity, Unit, Value.
        "Item"
    } else {
        LABEL_HEADING
    }
}

/// The kind of each column, read from its first cell that is neither empty nor unsupported (the
/// rows first, then the totals). A column with no such cell is a figure column.
#[must_use]
pub fn column_kinds(report: &ViewReport) -> Vec<ColumnKind> {
    (0..report.columns.len())
        .map(|i| {
            // The overdue column holds words ("31-60 days") but sits at the right like a figure
            // (index.html `outstanding.receivables`).
            if report.columns.get(i).is_some_and(|c| c == "Overdue") {
                return ColumnKind::Figure;
            }
            report
                .rows
                .iter()
                .filter_map(|row| row.cells.get(i))
                .chain(report.totals.get(i))
                .find_map(kind_of)
                .unwrap_or(ColumnKind::Figure)
        })
        .collect()
}

/// The kind a cell says its column is, if it says.
fn kind_of(cell: &ViewCell) -> Option<ColumnKind> {
    match cell {
        ViewCell::Amount(_)
        | ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Provisional { .. } => Some(ColumnKind::Figure),
        ViewCell::Date(_) => Some(ColumnKind::Text),
        ViewCell::Text(text) => (!text.trim().is_empty()).then_some(ColumnKind::Text),
        ViewCell::Empty | ViewCell::Unsupported(_) => None,
    }
}

/// The left and right of the label column and then of each column, in points from the line's
/// left, for a line `width` wide. Figure columns are 170 points wide, narrowing to 90 so the
/// label and each text column keep 120; the label and the text columns share the rest equally.
#[must_use]
pub fn column_spans(width: f32, kinds: &[ColumnKind]) -> Vec<(f32, f32)> {
    let mut all = Vec::with_capacity(kinds.len() + 1);
    all.push(ColumnKind::Text);
    all.extend_from_slice(kinds);
    ordered_spans(width, &all)
}

/// The left and right of each place in a line `width` wide, in the order given (the label's
/// place is a text place wherever it sits). Sized as [`column_spans`] says.
fn ordered_spans(width: f32, kinds: &[ColumnKind]) -> Vec<(f32, f32)> {
    let figures = kinds.iter().filter(|k| **k == ColumnKind::Figure).count();
    let fills = kinds.len() - figures;
    let figure = if figures == 0 {
        0.0
    } else {
        ((width - FILL_MIN * f32_of(fills)) / f32_of(figures)).clamp(FIGURE_MIN, FIGURE_WIDTH)
    };
    let fill = if fills == 0 {
        0.0
    } else {
        ((width - figure * f32_of(figures)) / f32_of(fills)).max(0.0)
    };
    let mut x = 0.0;
    kinds
        .iter()
        .map(|kind| {
            let w = match kind {
                ColumnKind::Text => fill,
                ColumnKind::Figure => figure,
            };
            let s = (x, x + w);
            x += w;
            s
        })
        .collect()
}

/// Widths of the ledger vouchers report's Date, Voucher and Type columns: as wide as their
/// text, so the particulars and the figures keep the rest of a narrow card.
const LEDGER_FIXED: [f32; 3] = [98.0, 68.0, 58.0];
/// Widths of its Debit, Credit and Balance columns (the balance carries its Dr or Cr word).
const LEDGER_FIGURES: [f32; 3] = [90.0, 90.0, 108.0];

/// The spans of the ledger vouchers report's `places` places (Date, Voucher, Type, Particulars,
/// then three figures): fixed text and figure columns, and the particulars taking what is left.
fn ledger_spans(width: f32, places: usize) -> Vec<(f32, f32)> {
    let taken: f32 = LEDGER_FIXED.iter().chain(&LEDGER_FIGURES).sum();
    let label = (width - taken).max(0.0);
    let mut widths: Vec<f32> = LEDGER_FIXED.to_vec();
    widths.push(label);
    widths.extend(LEDGER_FIGURES);
    let mut x = 0.0;
    widths
        .into_iter()
        .take(places)
        .map(|w| {
            let s = (x, x + w);
            x += w;
            s
        })
        .collect()
}

/// One place in a line: the label, or a report column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Label,
    Col(usize),
}

/// The ledger vouchers report's columns, as the engine gives them.
const LEDGER_COLUMNS: [&str; 7] = [
    "Date",
    "Particulars",
    "Voucher Type",
    "Voucher No",
    "Debit",
    "Credit",
    "Balance",
];

/// Whether `report` is the ledger vouchers report (its columns are exactly the engine's).
fn is_ledger_report(report: &ViewReport) -> bool {
    report.columns.iter().map(String::as_str).eq(LEDGER_COLUMNS)
}

/// The order the places of a line are drawn in. Every report draws the label first, then its
/// columns in order, except the ledger vouchers report, which follows the mock's ledger table
/// (index.html:1600): Date, Voucher, Type, then the particulars, then Debit, Credit, Balance.
fn place_order(report: &ViewReport) -> Vec<Place> {
    if is_ledger_report(report) {
        return vec![
            Place::Col(0),
            Place::Col(3),
            Place::Col(2),
            Place::Label,
            Place::Col(1),
            Place::Col(4),
            Place::Col(5),
            Place::Col(6),
        ];
    }
    let mut out = vec![Place::Label];
    out.extend((0..report.columns.len()).map(Place::Col));
    out
}

/// The heading drawn over column `i`: the engine's, except where the mock names it shorter.
fn column_heading<'a>(report: &ViewReport, name: &'a str) -> &'a str {
    if is_ledger_report(report) {
        match name {
            "Voucher No" => "Voucher",
            "Voucher Type" => "Type",
            other => other,
        }
    } else {
        match name {
            // The engine-facing headings name the side (see `mock.rs`); the mock prints the
            // plain word.
            "Open Dr" | "Open Cr" => "Open",
            "Value Dr" => "Value",
            "Cost Dr" => "Cost",
            other => other,
        }
    }
}

/// The height of each line, top to bottom: the heading, one per row, then the total when the
/// report has totals.
#[must_use]
pub fn line_heights(report: &ViewReport) -> Vec<f32> {
    let mut out = Vec::with_capacity(report.rows.len() + 2);
    out.push(T_HEAD_HEIGHT);
    out.extend(report.rows.iter().map(|_| T_ROW_HEIGHT));
    if !report.totals.is_empty() {
        out.push(T_TOTAL_HEIGHT);
    }
    out
}

/// Whether column `i` only repeats the label column: it is headed like the label column and every
/// cell in it is empty or the row's own label. Such a column is not drawn a second time (the
/// ledger vouchers report carries its particulars both as the row label and as a column).
#[must_use]
pub fn repeats_label(report: &ViewReport, i: usize) -> bool {
    report
        .columns
        .get(i)
        .is_some_and(|c| c == LABEL_HEADING || c == "Item")
        && report.rows.iter().all(|row| match row.cells.get(i) {
            Some(ViewCell::Empty) | None => true,
            Some(ViewCell::Text(text)) => *text == row.label,
            Some(_) => false,
        })
        && report
            .totals
            .get(i)
            .is_none_or(|cell| matches!(cell, ViewCell::Empty))
}

/// The text of `cell` in the column headed `heading`. A figure in the `Debit` column is a debit
/// and one in `Credit` a credit, so they drop their Dr or Cr word (the mock prints bare figures,
/// index.html:1604-1605); a figure on the other side of its column keeps its side word. Every
/// other column reads as [`cell_text`] writes it.
#[must_use]
pub fn column_cell_text(heading: &str, cell: &ViewCell) -> String {
    match heading {
        "Debit" | "Open Dr" | "Value Dr" | "Cost Dr" => amount_on_side(cell, DrCr::Dr),
        "Credit" | "Open Cr" => amount_on_side(cell, DrCr::Cr),
        _ => cell_text(cell),
    }
}

/// The text drawn for `cell` in the column headed `heading`: [`column_cell_text`], except that a
/// provisional figure is the bare figure. Its reason is the banner's, and the screen reader's
/// (see [`line_name`]); a long reason in the cell was cut off where the figure belongs.
fn drawn_cell_text(heading: &str, cell: &ViewCell) -> String {
    match cell {
        // The mock writes a date as "15 Sep 2026" (`fmtDate`), not as the export's 15-Sep-2026.
        ViewCell::Date(d) => crate::ui::format::date(*d),
        ViewCell::Provisional { amount, .. } => {
            column_cell_text(heading, &ViewCell::Amount(*amount))
        }
        ViewCell::Amount(_)
        | ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => column_cell_text(heading, cell),
    }
}

/// The word on the badge that marks a provisional figure (index.html `badge('Provisional')`).
const PROVISIONAL: &str = "Provisional";
/// Gap between a label and the badge after it, and between the badge and a figure.
const BADGE_GAP: f32 = 6.0;

/// Whether `headings` are the two-sided profit and loss's, where a badge goes beside the label of
/// the side the figure is on (index.html `plScreen`) and not beside the figure.
fn is_two_sided(headings: &[String]) -> bool {
    headings.iter().map(String::as_str).eq([
        super::mock::PL_DEBIT,
        "Particulars",
        super::mock::PL_CREDIT,
    ])
}

/// What a line needs to place the "Provisional" badges of its cells.
struct LineMarks<'a> {
    /// The line.
    rect: Rect,
    /// Where its columns are.
    cols: &'a Columns,
    /// The report's headings.
    headings: &'a [String],
    /// The font its text is drawn in.
    font: &'a egui::FontId,
    /// The right end of the row's label, from the label column's left (the debit side).
    label_end: f32,
    /// The text of the credit side's label, on the two-sided account.
    particulars: Option<&'a str>,
}

impl LineMarks<'_> {
    /// Paints the badge for the provisional figure `figure` in column `i`: right before the
    /// figure, or on the two-sided account after the label of the figure's side.
    fn paint(&self, ui: &Ui, i: usize, figure: &str) {
        let t = tokens(ui);
        let y = self.rect.center().y;
        let text_width = |text: &str| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), self.font.clone(), t.text)
                .size()
                .x
        };
        if is_two_sided(self.headings) {
            let left = if i == 0 {
                self.cols.label(self.rect).left() + self.label_end
            } else {
                let start = self
                    .cols
                    .cell(self.rect, 1)
                    .map_or(self.rect.left(), |(r, _)| r.left() + table::CELL_PAD_X);
                start + self.particulars.map_or(0.0, text_width)
            };
            paint_badge(ui, left + BADGE_GAP, y, BadgeKind::Warn, PROVISIONAL);
        } else if let Some((cell_rect, _)) = self.cols.cell(self.rect, i) {
            let left = cell_rect.right()
                - table::CELL_PAD_X
                - text_width(figure)
                - BADGE_GAP
                - badge_width(ui, PROVISIONAL);
            paint_badge(ui, left, y, BadgeKind::Warn, PROVISIONAL);
        }
    }
}

/// What [`draw`] reports back besides the clicks.
pub(super) struct Drawn {
    /// Clicks and double-clicks on lines.
    pub events: TableEvents,
    /// The selected line's rectangle, when it was laid out this frame.
    pub selected: Option<Rect>,
}

/// Draws the report in a card as tall as the space left. Clicking a line selects it and
/// double-clicking opens it; with `scroll_to_selected` the selected line is brought into view.
pub(super) fn draw(
    ui: &mut Ui,
    report: &ViewReport,
    selected: usize,
    scroll_to_selected: bool,
) -> Drawn {
    let t = tokens(ui);
    let k = kind(ui);
    let mut drawn = Drawn {
        events: TableEvents::default(),
        selected: None,
    };
    let total_height = if report.totals.is_empty() {
        0.0
    } else {
        T_TOTAL_HEIGHT
    };
    let room = (ui.available_height() - T_HEAD_HEIGHT - total_height - 2.0).max(T_ROW_HEIGHT * 3.0);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            let cols = Columns::new(report, width);
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            paint_heading(ui, head, &cols, label_heading(report), report);
            drawn = draw_rows(ui, report, &cols, selected, scroll_to_selected, room);
            if !report.totals.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(width, T_TOTAL_HEIGHT), Sense::hover());
                paint_total(ui, rect, &cols, &report.columns, &report.totals);
            }
        });
    drawn
}

/// The integrity certificate's facts as (what, value) lines, the engine's cells as written: each
/// row's label and the text of its first cell. Nothing is worked out here.
#[must_use]
pub(super) fn certificate_lines(report: &ViewReport) -> Vec<(String, String)> {
    report
        .rows
        .iter()
        .map(|row| {
            // The engine's first cell repeats the label; its last is the value.
            let value = row.cells.last().map_or_else(String::new, cell_text);
            (row.label.clone(), value)
        })
        .collect()
}

/// The integrity certificate as the mock draws it (index.html `audit.integrity`): a card titled
/// "Integrity certificate" with one fact per line, the label at the left and the value at the
/// right, no column headings. A long value (the head hash) wraps inside its column.
pub(super) fn draw_certificate(ui: &mut Ui, report: &ViewReport) -> Drawn {
    let t = tokens(ui);
    let lines = certificate_lines(report);
    crate::ui::card::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        crate::ui::mock_table::panel_title(ui, None, "Integrity certificate");
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let width = ui.available_width();
        let value_left = (width * 0.5).floor();
        for (what, value) in &lines {
            let mono = value.len() > 40;
            let font = if mono {
                egui::FontId::monospace(crate::theme::TEXT_META)
            } else {
                egui::FontId::proportional(TEXT_BODY)
            };
            let galley = ui.painter().layout(
                value.clone(),
                font,
                t.text,
                (width - value_left - table::CELL_PAD_X).max(40.0),
            );
            let height = T_ROW_HEIGHT.max(galley.size().y + 14.0 + 1.0);
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
            a11y::name(
                ui,
                &response,
                Role::Row,
                &a11y::join(&[what.as_str(), value.as_str()]),
            );
            table::paint_cell_rule(ui, rect);
            table::paint_cell_text(
                ui,
                rect,
                1.0,
                what,
                egui::FontId::proportional(TEXT_BODY),
                t.text,
                CellPlace::Left(table::CELL_PAD_X),
            );
            let y = rect.top() + (rect.height() - 1.0 - galley.size().y) / 2.0;
            ui.painter()
                .galley(pos2(rect.left() + value_left, y), galley, t.text);
        }
    });
    Drawn {
        events: TableEvents::default(),
        selected: None,
    }
}

/// The rows, in a scroll area at most `max_height` tall that lays out only the rows in view.
fn draw_rows(
    ui: &mut Ui,
    report: &ViewReport,
    cols: &Columns,
    selected: usize,
    scroll_to_selected: bool,
    max_height: f32,
) -> Drawn {
    let mut drawn = Drawn {
        events: TableEvents::default(),
        selected: None,
    };
    let count = report.rows.len();
    ScrollArea::vertical()
        .id_salt(("u7-report-lines", report.token.0.as_str()))
        .max_height(max_height)
        .auto_shrink([false, true])
        .show_viewport(ui, |ui, viewport| {
            let top = ui.max_rect().top();
            let left = ui.max_rect().left();
            let width = ui.max_rect().width();
            ui.set_height(T_ROW_HEIGHT * f32_of(count));
            let line = |i: usize| {
                Rect::from_min_size(
                    pos2(left, top + T_ROW_HEIGHT * f32_of(i)),
                    vec2(width, T_ROW_HEIGHT),
                )
            };
            if scroll_to_selected && selected < count {
                ui.scroll_to_rect(line(selected), None);
            }
            let first = row_at(viewport.min.y);
            let end = row_at(viewport.max.y).saturating_add(1).min(count);
            for (i, row) in report.rows.iter().enumerate().take(end).skip(first) {
                let rect = line(i);
                let response = ui.interact(rect, ui.scope_id().with(("line", i)), Sense::click());
                let state = RowState {
                    selected: i == selected,
                    hovered: response.hovered(),
                    sub: line_is_emphasised(&report.columns, row.kind),
                };
                a11y::name(ui, &response, Role::Row, &line_name(row));
                a11y::focus_ring(ui, &response, rect);
                if is_band(&report.columns, row) {
                    // `band()`: the whole row on `accent-soft`, selected or not.
                    ui.painter().rect_filled(rect, 0.0, tokens(ui).accent_soft);
                }
                paint_line(ui, rect, cols, &report.columns, row, state);
                if state.selected {
                    drawn.selected = Some(rect);
                }
                if response.clicked() {
                    drawn.events.clicked = Some(i);
                }
                if response.double_clicked() {
                    drawn.events.opened = Some(i);
                }
            }
        });
    drawn
}

/// The row whose line holds `y` points below the first line's top.
fn row_at(y: f32) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is floored and at least 0; Rust's float-to-int cast saturates"
    )]
    let row = (y.max(0.0) / T_ROW_HEIGHT).floor() as usize;
    row
}

/// A count as points.
fn f32_of(n: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a count past 2^24 lines only loses a fraction of a point"
    )]
    let x = n as f32;
    x
}

/// Group, subtotal and difference rows are semibold on `surface-2` (`tr.sub`); the rest are plain.
fn emphasised(kind: ViewRowKind) -> bool {
    match kind {
        ViewRowKind::Group | ViewRowKind::Subtotal | ViewRowKind::Difference => true,
        ViewRowKind::Ledger
        | ViewRowKind::Voucher
        | ViewRowKind::Item
        | ViewRowKind::Bill
        | ViewRowKind::Text => false,
    }
}

/// Whether a line of `kind` is drawn semibold on `surface-2` in a table with these headings.
///
/// The two-sided account draws its group lines plain (index.html `plScreen`: only the Total line
/// is bold); every other table keeps `tr.sub` for them.
fn line_is_emphasised(headings: &[String], kind: ViewRowKind) -> bool {
    emphasised(kind) && !(kind == ViewRowKind::Group && is_two_sided(headings))
}

/// Whether `row` is a band of the two-sided account ("Trading account"), drawn on `accent-soft`.
fn is_band(headings: &[String], row: &ViewRow) -> bool {
    row.kind == ViewRowKind::Text && is_two_sided(headings)
}

/// Where the label and each column sit across a line, and how their cells sit in them.
struct Columns {
    /// The label's span.
    label: (f32, f32),
    /// The span a line with no drill key (the ledger report's opening and closing balances) draws
    /// its label across, so the words are not clipped: from the Voucher column to the label's end.
    wide_label: Option<(f32, f32)>,
    /// For each report column, its span and how its cells sit; `None` when it is not drawn (see
    /// [`repeats_label`]).
    cols: Vec<Option<((f32, f32), CellPlace)>>,
}

impl Columns {
    fn new(report: &ViewReport, width: f32) -> Self {
        let kinds = column_kinds(report);
        let order: Vec<Place> = place_order(report)
            .into_iter()
            .filter(|p| match p {
                Place::Label => true,
                Place::Col(i) => !repeats_label(report, *i),
            })
            .collect();
        let order_kinds: Vec<ColumnKind> = order
            .iter()
            .map(|p| match p {
                Place::Label => ColumnKind::Text,
                Place::Col(i) => kinds.get(*i).copied().unwrap_or(ColumnKind::Figure),
            })
            .collect();
        let spans = if is_ledger_report(report) {
            ledger_spans(width, order.len())
        } else {
            ordered_spans(width, &order_kinds)
        };
        let mut label = (0.0, width);
        let mut cols = vec![None; kinds.len()];
        for ((place, span), kind) in order.iter().zip(spans).zip(order_kinds) {
            match place {
                Place::Label => label = span,
                Place::Col(i) => {
                    let how = match kind {
                        ColumnKind::Text => CellPlace::Left(table::CELL_PAD_X),
                        ColumnKind::Figure => CellPlace::Right,
                    };
                    if let Some(slot) = cols.get_mut(*i) {
                        *slot = Some((span, how));
                    }
                }
            }
        }
        let wide_label = is_ledger_report(report).then_some((LEDGER_FIXED[0], label.1));
        Self {
            label,
            wide_label,
            cols,
        }
    }

    fn span(line: Rect, (left, right): (f32, f32)) -> Rect {
        Rect::from_min_max(
            pos2(line.left() + left, line.top()),
            pos2(line.left() + right, line.bottom()),
        )
    }

    /// The label part of `line`.
    fn label(&self, line: Rect) -> Rect {
        Self::span(line, self.label)
    }

    /// The label part of a `row`'s line: wider for a line with no drill key in the ledger report.
    fn label_of(&self, line: Rect, row: &ViewRow) -> Rect {
        match (self.wide_label, &row.key) {
            (Some(span), None) => Self::span(line, span),
            _ => self.label(line),
        }
    }

    /// Report column `i` of `line` and where its text goes; `None` past the last column or for a
    /// column that is not drawn.
    fn cell(&self, line: Rect, i: usize) -> Option<(Rect, CellPlace)> {
        let (span, place) = (*self.cols.get(i)?)?;
        Some((Self::span(line, span), place))
    }
}

/// The heading line in the `th` style: the label column's heading, then each column's at its
/// right.
fn paint_heading(ui: &Ui, rect: Rect, cols: &Columns, label: &str, report: &ViewReport) {
    let t = tokens(ui);
    table::paint_head_rule(ui, rect);
    let font = bold_font(ui, TEXT_META);
    table::paint_cell_text(
        ui,
        cols.label(rect),
        1.0,
        label,
        font.clone(),
        t.text_3,
        CellPlace::Left(table::CELL_PAD_X),
    );
    for (i, name) in report.columns.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
            let name = column_heading(report, name);
            table::paint_cell_text(ui, cell_rect, 1.0, name, font.clone(), t.text_3, place);
        }
    }
}

/// A row: fill, rule and bar by [`table::paint_row`], the label at its depth's indent and each
/// figure at its column's right.
fn paint_line(
    ui: &Ui,
    rect: Rect,
    cols: &Columns,
    headings: &[String],
    row: &ViewRow,
    state: RowState,
) {
    let t = tokens(ui);
    table::paint_row(ui, rect, state);
    let font = if state.sub {
        bold_font(ui, TEXT_BODY)
    } else {
        egui::FontId::proportional(TEXT_BODY)
    };
    table::paint_cell_text(
        ui,
        cols.label_of(rect, row),
        1.0,
        &row.label,
        font.clone(),
        t.text,
        CellPlace::Left(table::indent(row.depth)),
    );
    let label_end = table::indent(row.depth)
        + ui.painter()
            .layout_no_wrap(row.label.clone(), font.clone(), t.text)
            .size()
            .x;
    let particulars = match row.cells.get(1) {
        Some(ViewCell::Text(text)) => Some(text.as_str()),
        Some(_) | None => None,
    };
    let marks = LineMarks {
        rect,
        cols,
        headings,
        font: &font,
        label_end,
        particulars,
    };
    for (i, cell) in row.cells.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
            let heading = headings.get(i).map_or("", String::as_str);
            let text = drawn_cell_text(heading, cell);
            let ink = if is_overdue_cell(heading, cell) {
                t.bad
            } else {
                t.text
            };
            table::paint_cell_text(ui, cell_rect, 1.0, &text, font.clone(), ink, place);
            if matches!(cell, ViewCell::Provisional { .. }) {
                marks.paint(ui, i, &text);
            }
        }
    }
}

/// Whether `cell` is an Overdue cell naming an ageing bucket past the due date, which the mock
/// draws in red ink (index.html:2331, `color:var(--bad)`).
#[must_use]
pub(super) fn is_overdue_cell(heading: &str, cell: &ViewCell) -> bool {
    heading == "Overdue"
        && matches!(cell, ViewCell::Text(text) if super::mock::OVERDUE_BUCKETS.contains(&text.as_str()))
}

/// What a screen reader says for a row: its label, then each cell as the row draws it, in column
/// order (`cell_text` keeps the Dr or Cr word).
fn line_name(row: &ViewRow) -> String {
    let cells: Vec<String> = row.cells.iter().map(cell_text).collect();
    let mut parts: Vec<&str> = vec![row.label.as_str()];
    parts.extend(cells.iter().map(String::as_str));
    a11y::join(&parts)
}

/// The total line: semibold, with the rule above and the double rule below.
fn paint_total(ui: &Ui, rect: Rect, cols: &Columns, headings: &[String], totals: &[ViewCell]) {
    let t = tokens(ui);
    table::paint_total_rules(ui, rect);
    let font = bold_font(ui, TEXT_BODY);
    // index.html `stockScreen`: the stock summary's total reads "Total closing stock" with the
    // engine's tag, the figure being the stock engine's.
    let stock = headings.iter().any(|h| h == "Cost Dr");
    let label = if stock {
        "Total closing stock"
    } else {
        "Total"
    };
    table::paint_cell_text(
        ui,
        cols.label(rect),
        3.0,
        label,
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    if stock {
        table::paint_engine_tag(ui, cols.label(rect), label, &font);
    }
    let marks = LineMarks {
        rect,
        cols,
        headings,
        font: &font,
        label_end: 0.0,
        particulars: None,
    };
    for (i, cell) in totals.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
            let text = drawn_cell_text(headings.get(i).map_or("", String::as_str), cell);
            table::paint_cell_text(ui, cell_rect, 3.0, &text, font.clone(), t.text, place);
            if matches!(cell, ViewCell::Provisional { .. }) {
                marks.paint(ui, i, &text);
            }
        }
    }
}

#[cfg(test)]
mod provisional_marks {
    //! A provisional figure is drawn as the figure with a "Provisional" badge; the reason is the
    //! banner's, never the cell's (the 2026-11-25 capture of `stock-summary.provisional` drew the
    //! reason, cut off, where the value belongs).
    //!
    //! provenance: design/mock-v2/index.html `stockScreen` (line 2348: the badge sits before the
    //! bare value, line 2349: the total carries one too) and `plScreen` (line 2238: `prov` on the
    //! closing stock line, the badge beside its label).
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use crate::screens::report_view::{ReportToken, ViewParams, ViewRow};
    use egui::Shape;
    use ledgerly_core::{Paise, Period, SignedAmount};
    use time::macros::date;

    const REASON: &str =
        "negative stock: item Steel pickaxe 2.5 kg first went below zero on 2026-11-19";

    fn provisional(side: DrCr, paise: i64) -> ViewCell {
        ViewCell::Provisional {
            amount: SignedAmount {
                amount: Paise::new(paise),
                side,
            },
            reason: REASON.to_owned(),
        }
    }

    fn report(columns: &[&str], rows: Vec<ViewRow>, totals: Vec<ViewCell>) -> ViewReport {
        ViewReport {
            token: ReportToken("provisional".to_owned()),
            title: "t".to_owned(),
            params: ViewParams {
                period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
                detail: crate::screens::report_view::ViewDetail::Condensed,
            },
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            rows,
            totals,
            notes: Vec::new(),
        }
    }

    fn line(label: &str, cells: Vec<ViewCell>) -> ViewRow {
        ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind: ViewRowKind::Item,
            cells,
        }
    }

    fn texts(shape: &Shape, out: &mut Vec<String>) {
        if let Shape::Text(t) = shape {
            out.push(t.galley.text().to_owned());
        } else if let Shape::Vec(inner) = shape {
            for s in inner {
                texts(s, out);
            }
        }
    }

    fn drawn_texts(report: &ViewReport) -> Vec<String> {
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw(ui, report, 0, false);
        });
        let mut drawn = Vec::new();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut drawn);
        }
        out.drop_without_applying_deltas();
        drawn
    }

    #[test]
    fn the_stock_summary_draws_a_badge_and_the_bare_value_never_the_reason() {
        // derived: the pickaxe's 70 nos at 470.00 is 32,900.00 (index.html: ITEMS cost 470);
        // the total 13,90,759.00 is the engine's. Neither cell shows the reason; each has a badge.
        let r = report(
            &["HSN", "Quantity", "Unit", "Cost Dr", "Value Dr"],
            vec![
                line(
                    "Steel pickaxe 2.5 kg",
                    vec![
                        ViewCell::Text("none".to_owned()),
                        ViewCell::Empty,
                        ViewCell::Text("nos".to_owned()),
                        ViewCell::Amount(SignedAmount::dr(Paise::new(47_000))),
                        provisional(DrCr::Dr, 3_290_000),
                    ],
                ),
                line(
                    "Monoblock pump 1 HP",
                    vec![
                        ViewCell::Text("8413".to_owned()),
                        ViewCell::Empty,
                        ViewCell::Text("nos".to_owned()),
                        ViewCell::Empty,
                        ViewCell::Amount(SignedAmount::dr(Paise::new(8_340_000))),
                    ],
                ),
            ],
            vec![
                ViewCell::Empty,
                ViewCell::Empty,
                ViewCell::Empty,
                ViewCell::Empty,
                provisional(DrCr::Dr, 139_075_900),
            ],
        );
        let drawn = drawn_texts(&r);
        assert!(
            drawn.iter().all(|t| !t.contains("below zero")),
            "the reason is drawn in a cell: {drawn:?}"
        );
        assert!(drawn.iter().any(|t| t == "32,900.00"), "{drawn:?}");
        // provenance: index.html `stockScreen` line 2355: the total row reads "Total closing
        // stock" followed by the green "engine" tag; the Cost column shows the rate 470.00.
        assert!(
            drawn.iter().any(|t| t == "Total closing stock"),
            "{drawn:?}"
        );
        assert!(drawn.iter().any(|t| t == "engine"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "470.00"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "13,90,759.00"), "{drawn:?}");
        assert_eq!(
            drawn.iter().filter(|t| *t == "Provisional").count(),
            2,
            "one badge on the item and one on the total: {drawn:?}"
        );
    }

    #[test]
    fn the_profit_and_loss_marks_a_provisional_line_with_a_badge_not_a_label_suffix() {
        // derived: the closing stock 13,90,759.00 sits on the credit side; its line reads
        // "Closing stock" with a badge, not "Closing stock (provisional)".
        let r = report(
            &["Debit", "Particulars", "Credit"],
            vec![line(
                "Opening stock",
                vec![
                    ViewCell::Amount(SignedAmount::dr(Paise::new(91_200_000))),
                    ViewCell::Text("Closing stock".to_owned()),
                    provisional(DrCr::Cr, 139_075_900),
                ],
            )],
            Vec::new(),
        );
        let drawn = drawn_texts(&r);
        assert!(
            drawn.iter().all(|t| !t.contains("(provisional")),
            "{drawn:?}"
        );
        assert!(drawn.iter().all(|t| !t.contains("below zero")), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "13,90,759.00"), "{drawn:?}");
        assert_eq!(drawn.iter().filter(|t| *t == "Provisional").count(), 1);
    }
}

#[cfg(test)]
mod s14_look {
    //! Round s14: the two-sided profit and loss draws plain group lines and shaded bands, and a
    //! date reads as the mock writes it.
    //!
    //! provenance: design/mock-v2/index.html `plScreen`: `band()` is a row on `accent-soft`
    //! (line 2237), group lines are plain `td`s (class `g` carries no weight, line 2236) and only
    //! the Total line is bold; the ledger report's rows write a date as "15 Sep 2026" (`fmtDate`).
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use time::macros::date;

    fn two_sided() -> Vec<String> {
        [
            super::super::mock::PL_DEBIT,
            "Particulars",
            super::super::mock::PL_CREDIT,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    fn row(kind: ViewRowKind, label: &str) -> ViewRow {
        ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind,
            cells: Vec::new(),
        }
    }

    #[test]
    fn a_group_line_of_the_two_sided_account_is_plain_but_elsewhere_it_is_bold() {
        let plain = vec!["Group".to_owned(), "Debit".to_owned()];
        assert!(line_is_emphasised(&plain, ViewRowKind::Group));
        assert!(!line_is_emphasised(&two_sided(), ViewRowKind::Group));
        assert!(line_is_emphasised(&two_sided(), ViewRowKind::Subtotal));
    }

    #[test]
    fn a_band_is_a_text_line_of_the_two_sided_account() {
        let band = row(ViewRowKind::Text, "Trading account");
        assert!(is_band(&two_sided(), &band));
        assert!(!is_band(&["Debit".to_owned()], &band));
        assert!(!is_band(
            &two_sided(),
            &row(ViewRowKind::Group, "Purchase Accounts")
        ));
    }

    #[test]
    fn a_date_cell_is_drawn_as_day_month_name_year() {
        assert_eq!(
            drawn_cell_text("Date", &ViewCell::Date(date!(2026 - 09 - 15))),
            "15 Sep 2026"
        );
    }
}

#[cfg(test)]
mod s14_overdue_ink {
    //! The Overdue cell of a bill past its due date is red ink, as the mock draws it.
    //!
    //! provenance: design/mock-v2/index.html:2331: `b.overdue>0 ? <span style="color:var(--bad)">`
    //! and otherwise plain text ("not due", "on demand"). The engine's buckets for a bill past its
    //! due date are `0-30 days` to `Over 90 days`; `Not yet due`, `No due date` and the "Due
    //! today" of a bill 0 days overdue are not overdue and keep plain ink.

    use super::*;

    #[test]
    fn only_a_bucket_past_the_due_date_in_the_overdue_column_is_red() {
        for bucket in ["0-30 days", "31-60 days", "61-90 days", "Over 90 days"] {
            assert!(is_overdue_cell(
                "Overdue",
                &ViewCell::Text(bucket.to_owned())
            ));
        }
        for plain in ["Not yet due", "No due date", "Due today", ""] {
            assert!(!is_overdue_cell(
                "Overdue",
                &ViewCell::Text(plain.to_owned())
            ));
        }
        // The same words in another column are not an overdue mark.
        assert!(!is_overdue_cell(
            "Bill",
            &ViewCell::Text("31-60 days".to_owned())
        ));
    }
}

#[cfg(test)]
mod s14_certificate {
    //! The integrity certificate reads as facts, one per line.
    //!
    //! provenance: design/mock-v2/index.html `audit.integrity`: label and value per line under the
    //! title "Integrity certificate". The values are the engine's cells, passed through.

    #![allow(
        clippy::unwrap_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use crate::screens::report_view::{ReportToken, ViewDetail, ViewParams};

    #[test]
    fn each_engine_row_is_one_line_with_its_value_as_written() {
        let row = |label: &str, text: &str| ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind: ViewRowKind::Text,
            // The engine's rows carry the label as their first cell and the value as their second
            // (`ledgerly-reports` `audit.rs` `text_row`).
            cells: vec![
                ViewCell::Text(label.to_owned()),
                ViewCell::Text(text.to_owned()),
            ],
        };
        let report = ViewReport {
            token: ReportToken("t".to_owned()),
            title: "Integrity certificate".to_owned(),
            params: ViewParams {
                period: ledgerly_core::Period::new(
                    time::macros::date!(2026 - 04 - 01),
                    time::macros::date!(2026 - 11 - 20),
                )
                .unwrap(),
                detail: ViewDetail::Detailed,
            },
            columns: vec!["Item".to_owned(), "Value".to_owned()],
            rows: vec![row("Result", "PASS"), row("Events checked", "156")],
            totals: Vec::new(),
            notes: Vec::new(),
        };
        assert_eq!(
            certificate_lines(&report),
            vec![
                ("Result".to_owned(), "PASS".to_owned()),
                ("Events checked".to_owned(), "156".to_owned()),
            ]
        );
    }
}
