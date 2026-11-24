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
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT, T_TOTAL_HEIGHT};
use crate::ui::{a11y, bold_font, kind, tokens};

/// Heading of the label column.
const LABEL_HEADING: &str = "Particulars";
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

/// The kind of each column, read from its first cell that is neither empty nor unsupported (the
/// rows first, then the totals). A column with no such cell is a figure column.
#[must_use]
pub fn column_kinds(report: &ViewReport) -> Vec<ColumnKind> {
    (0..report.columns.len())
        .map(|i| {
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
    let figures = kinds.iter().filter(|k| **k == ColumnKind::Figure).count();
    let fills = 1 + kinds.iter().filter(|k| **k == ColumnKind::Text).count();
    let figure = if figures == 0 {
        0.0
    } else {
        ((width - FILL_MIN * f32_of(fills)) / f32_of(figures)).clamp(FIGURE_MIN, FIGURE_WIDTH)
    };
    let fill = ((width - figure * f32_of(figures)) / f32_of(fills)).max(0.0);
    let mut x = 0.0;
    let mut span = |w: f32| {
        let s = (x, x + w);
        x += w;
        s
    };
    let mut out = Vec::with_capacity(kinds.len() + 1);
    out.push(span(fill));
    for kind in kinds {
        out.push(span(match kind {
            ColumnKind::Text => fill,
            ColumnKind::Figure => figure,
        }));
    }
    out
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
    report.columns.get(i).is_some_and(|c| c == LABEL_HEADING)
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
        "Debit" => amount_on_side(cell, DrCr::Dr),
        "Credit" => amount_on_side(cell, DrCr::Cr),
        _ => cell_text(cell),
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
            paint_heading(ui, head, &cols, &report.columns);
            drawn = draw_rows(ui, report, &cols, selected, scroll_to_selected, room);
            if !report.totals.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(width, T_TOTAL_HEIGHT), Sense::hover());
                paint_total(ui, rect, &cols, &report.columns, &report.totals);
            }
        });
    drawn
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
                    sub: emphasised(row.kind),
                };
                a11y::name(ui, &response, Role::Row, &line_name(row));
                a11y::focus_ring(ui, &response, rect);
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

/// Where the label and each column sit across a line, and how their cells sit in them.
struct Columns {
    /// [`column_spans`] of the drawn columns: the label's, then each drawn column's.
    spans: Vec<(f32, f32)>,
    /// [`column_kinds`] of the drawn columns.
    kinds: Vec<ColumnKind>,
    /// For each report column, its place among the drawn columns; `None` when it is not drawn
    /// (see [`repeats_label`]).
    slot: Vec<Option<usize>>,
}

impl Columns {
    fn new(report: &ViewReport, width: f32) -> Self {
        let all = column_kinds(report);
        let mut slot = Vec::with_capacity(all.len());
        let mut kinds = Vec::with_capacity(all.len());
        for (i, kind) in all.iter().enumerate() {
            if repeats_label(report, i) {
                slot.push(None);
            } else {
                slot.push(Some(kinds.len()));
                kinds.push(*kind);
            }
        }
        Self {
            spans: column_spans(width, &kinds),
            kinds,
            slot,
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
        self.spans
            .first()
            .map_or(line, |span| Self::span(line, *span))
    }

    /// Report column `i` of `line` and where its text goes; `None` past the last column or for a
    /// column that is not drawn.
    fn cell(&self, line: Rect, i: usize) -> Option<(Rect, CellPlace)> {
        let drawn = (*self.slot.get(i)?)?;
        let span = *self.spans.get(drawn + 1)?;
        let place = match self.kinds.get(drawn)? {
            ColumnKind::Text => CellPlace::Left(table::CELL_PAD_X),
            ColumnKind::Figure => CellPlace::Right,
        };
        Some((Self::span(line, span), place))
    }
}

/// The heading line in the `th` style: the label column's heading, then each column's at its
/// right.
fn paint_heading(ui: &Ui, rect: Rect, cols: &Columns, columns: &[String]) {
    let t = tokens(ui);
    table::paint_head_rule(ui, rect);
    let font = bold_font(ui, TEXT_META);
    table::paint_cell_text(
        ui,
        cols.label(rect),
        1.0,
        LABEL_HEADING,
        font.clone(),
        t.text_3,
        CellPlace::Left(table::CELL_PAD_X),
    );
    for (i, name) in columns.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
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
        cols.label(rect),
        1.0,
        &row.label,
        font.clone(),
        t.text,
        CellPlace::Left(table::indent(row.depth)),
    );
    for (i, cell) in row.cells.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
            table::paint_cell_text(
                ui,
                cell_rect,
                1.0,
                &column_cell_text(headings.get(i).map_or("", String::as_str), cell),
                font.clone(),
                t.text,
                place,
            );
        }
    }
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
    table::paint_cell_text(
        ui,
        cols.label(rect),
        3.0,
        "Total",
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    for (i, cell) in totals.iter().enumerate() {
        if let Some((cell_rect, place)) = cols.cell(rect, i) {
            table::paint_cell_text(
                ui,
                cell_rect,
                3.0,
                &column_cell_text(headings.get(i).map_or("", String::as_str), cell),
                font.clone(),
                t.text,
                place,
            );
        }
    }
}
