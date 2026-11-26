//! Table helpers over `egui_extras::TableBuilder` (`table.t` in the mock): header style, row
//! height, right-aligned numeric cells and the selected-row tint.
//!
//! egui has no tabular figures switch, so numbers are right-aligned in a proportional face.
//! Columns that hold amounts must be right-aligned with [`numeric`] so the decimal points line up
//! in the usual way.
//!
//! Typical use:
//!
//! ```text
//! table::builder(ui)
//!     .column(Column::remainder())
//!     .column(Column::auto())
//!     .header(table::HEADER_HEIGHT, |mut row| {
//!         table::header(&mut row, "Ledger", false);
//!         table::header(&mut row, "Amount", true);
//!     })
//!     .body(|mut body| {
//!         body.rows(table::ROW_HEIGHT, n, |mut row| {
//!             row.set_selected(is_selected(row.index()));
//!             row.col(|ui| table::text(ui, name));
//!             row.col(|ui| table::numeric(ui, amount));
//!         });
//!     });
//! ```

use egui::{Align, Color32, FontId, Layout, Rect, RichText, Stroke, Ui, pos2, vec2};
use egui_extras::{TableBuilder, TableRow};

use super::{kind, tokens};
use crate::theme::{TEXT_BODY, TEXT_META, Tokens, border_width};

/// Height of the header row, in points.
pub const HEADER_HEIGHT: f32 = 28.0;
/// Height of a body row, in points.
pub const ROW_HEIGHT: f32 = 30.0;

/// A table builder with the cell layout the kit uses: vertically centred, left to right.
/// Pass the result to `.column(...)`, then `.header(...)` and `.body(...)`.
pub fn builder(ui: &mut Ui) -> TableBuilder<'_> {
    TableBuilder::new(ui).cell_layout(Layout::left_to_right(Align::Center))
}

/// A header cell: the label in the caption style, in the secondary text colour. Numeric columns
/// are right-aligned, like their cells.
pub fn header(row: &mut TableRow<'_, '_>, label: &str, numeric_column: bool) {
    row.col(|ui| {
        let t = tokens(ui);
        let text = RichText::new(label)
            .size(TEXT_META)
            .strong()
            .color(t.text_3);
        if numeric_column {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(text);
            });
        } else {
            ui.label(text);
        }
    });
}

/// A text cell in the body colour.
pub fn text(ui: &mut Ui, value: &str) {
    let t = tokens(ui);
    ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
}

/// A numeric cell: right-aligned, body colour. Use it for every amount, count and rate.
pub fn numeric(ui: &mut Ui, value: &str) {
    let t = tokens(ui);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
    });
}

// ---- the mock's `table.t` lines, drawn by hand -------------------------------------------------
//
// `TableBuilder` cannot fill a whole row, draw a rule under each row or a bar at the left of the
// selected one, so a screen that draws to `table.t` (index.html lines 148-158) lays its lines out
// itself with these helpers. The other tables keep [`ROW_HEIGHT`] until their own parity round.

/// Horizontal padding of a `table.t` cell (`padding:7px 10px`).
pub const CELL_PAD_X: f32 = 10.0;
/// Extra left padding per nesting level (`td.ind1` 26, `td.ind2` 42: 16 per level).
pub const INDENT_STEP: f32 = 16.0;
/// A heading line: 6 + 17 (12 px text at line-height 1.45, whole points) + 6 + a 1 px rule.
pub const T_HEAD_HEIGHT: f32 = 30.0;
/// Extra space above a heading that opens a second section (`padding-top:18px` against 6).
pub const T_HEAD_GAP: f32 = 12.0;
/// A body line: 7 + 19 (13 px text at line-height 1.45, whole points) + 7 + a 1 px rule.
pub const T_ROW_HEIGHT: f32 = 34.0;
/// A total line: 7 + 19 + 7 and the 3 px double rule under it; its top rule is the rule of the
/// line above (`border-collapse`).
pub const T_TOTAL_HEIGHT: f32 = 36.0;
/// Width of the selected line's left bar (`box-shadow:inset 3px 0 0 var(--accent)`).
pub const SELECTED_BAR: f32 = 3.0;

/// The left padding of a cell at `depth` levels of nesting: 10, 26, 42, then 16 more per level.
#[must_use]
pub fn indent(depth: u8) -> f32 {
    CELL_PAD_X + INDENT_STEP * f32::from(depth)
}

/// What a body line is, for its fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowState {
    /// The line is the selected one (`tr.sel`).
    pub selected: bool,
    /// The pointer is over the line.
    pub hovered: bool,
    /// The line is a top-level line (`tr.sub`: semibold on `surface-2`).
    pub sub: bool,
}

/// The fill of a body line, by the mock's cascade (index.html lines 151-157): a selected line
/// under the pointer is `accent-soft`; else the pointer gives `hover`; else a top-level line is
/// `surface-2` (its rule comes after the selected rule, with the same weight, so it wins); else a
/// selected line is `accent-soft`. `None` leaves the card's surface.
#[must_use]
pub fn row_fill(t: &Tokens, state: RowState) -> Option<Color32> {
    match (state.selected, state.hovered, state.sub) {
        (true, true, _) | (true, false, false) => Some(t.accent_soft),
        (false, true, _) => Some(t.hover),
        (_, false, true) => Some(t.surface_2),
        (false, false, false) => None,
    }
}

/// The rule of a `table.t` line: the theme's border width (`var(--bw)`: 1 px, or 2 px in high
/// contrast) in `color`, and the y of its centre when it lies inside `rect` along its bottom.
#[allow(clippy::float_arithmetic, reason = "half a stroke width")]
fn bottom_rule(ui: &Ui, rect: Rect, color: Color32) -> (f32, Stroke) {
    let bw = border_width(kind(ui));
    (rect.bottom() - bw / 2.0, Stroke::new(bw, color))
}

/// Paints a body line's fill, its rule and, when selected, the accent bar at its left.
pub fn paint_row(ui: &Ui, rect: Rect, state: RowState) {
    let t = tokens(ui);
    let painter = ui.painter();
    if let Some(fill) = row_fill(&t, state) {
        painter.rect_filled(rect, 0.0, fill);
    }
    let (y, rule) = bottom_rule(ui, rect, t.border);
    painter.hline(rect.x_range(), y, rule);
    if state.selected {
        let bar = Rect::from_min_size(rect.min, vec2(SELECTED_BAR, rect.height()));
        painter.rect_filled(bar, 0.0, t.accent);
    }
}

/// Paints a total line's rules: one in `text-2` along its top, the theme's border width
/// (`border-top:var(--bw)`), and a 3 px double one along its bottom (`3px double`, the same in
/// every theme).
#[allow(clippy::float_arithmetic, reason = "half a stroke width")]
pub fn paint_total_rules(ui: &Ui, rect: Rect) {
    let t = tokens(ui);
    let painter = ui.painter();
    let bw = border_width(kind(ui));
    painter.hline(
        rect.x_range(),
        rect.top() + bw / 2.0,
        Stroke::new(bw, t.text_2),
    );
    let stroke = Stroke::new(1.0, t.text_2);
    painter.hline(rect.x_range(), rect.bottom() - 2.5, stroke);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, stroke);
}

/// Paints a heading line's rule along its bottom.
pub fn paint_head_rule(ui: &Ui, rect: Rect) {
    paint_cell_rule(ui, rect);
}

/// Paints the `table.t` rule along the bottom of `rect` (a line or one cell of it): the theme's
/// border width in `border`, inside `rect`.
pub fn paint_cell_rule(ui: &Ui, rect: Rect) {
    let t = tokens(ui);
    let (y, rule) = bottom_rule(ui, rect, t.border);
    ui.painter().hline(rect.x_range(), y, rule);
}

/// Paints `text` in `font` and `color`, vertically centred in `rect` (less its bottom rule of
/// `rule` points), placed by `place`. The text is clipped to `rect`.
pub fn paint_cell_text(
    ui: &Ui,
    rect: Rect,
    rule: f32,
    text: &str,
    font: FontId,
    color: Color32,
    place: CellPlace,
) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let y = rect.top() + (rect.height() - rule - galley.size().y) / 2.0;
    let x = match place {
        CellPlace::Left(left) => rect.left() + left,
        CellPlace::Right => rect.right() - CELL_PAD_X - galley.size().x,
    };
    ui.painter()
        .with_clip_rect(rect)
        .galley(pos2(x, y), galley, color);
}

/// Where [`paint_cell_text`] puts its text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellPlace {
    /// From this many points right of the line's left edge.
    Left(f32),
    /// Ending at the line's right padding.
    Right,
}

/// The small "engine" tag after a label (`prov('eng','engine')`).
pub fn paint_engine_tag(ui: &Ui, rect: Rect, label: &str, font: &FontId) {
    let t = tokens(ui);
    let label_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), t.text)
        .size()
        .x;
    let tag_font = FontId::proportional(crate::theme::TEXT_META - 2.0);
    let galley = ui
        .painter()
        .layout_no_wrap(crate::i18n::lit("engine"), tag_font, t.ok);
    let pad = vec2(6.0, 2.0);
    let size = galley.size() + pad * 2.0;
    let left = rect.left() + CELL_PAD_X + label_width + 8.0;
    let tag = Rect::from_min_size(pos2(left, rect.center().y - size.y / 2.0), size);
    ui.painter().rect_filled(tag, 3.0, t.ok_soft);
    ui.painter().galley(tag.min + pad, galley, t.ok);
}
