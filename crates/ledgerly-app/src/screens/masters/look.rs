//! The masters list drawn to the mock (`design/mock-v2/index.html`, `SCREENS.masters` at lines
//! 1791-1800): the filter box (`.qe`, lines 190-192) and the list in a card drawn to the
//! `table.t` rules (lines 148-158) with [`crate::ui::table`]'s helpers.
//!
//! Drawing only. The rows are plain values the caller made from the masters; nothing here reads
//! the books or works out a figure (rule C4).

use egui::{
    Align, Align2, FontId, Frame, Id, Layout, Margin, Rect, RichText, ScrollArea, Sense, Stroke,
    Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::theme::{RADIUS_M, RADIUS_XS, TEXT_BODY, TEXT_CAP, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::icon::{self, ICON_SIZE, Icon};
use crate::ui::prov::{self, ProvKind};
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT, T_TOTAL_HEIGHT};
use crate::ui::{bold_font, kind, tokens};

/// Height of the filter's text line (`.qe input{height:30px}`).
const FILTER_LINE: f32 = 30.0;
/// The gap between the filter box and the card under it (`margin-bottom:12px`).
pub(super) const FILTER_GAP: f32 = 12.0;
/// The least width of a column that takes what is left.
const FILL_MIN: f32 = 160.0;
/// The gap between a name and its alias in a name cell (the space before `.meta`).
const ALIAS_GAP: f32 = 4.0;
/// The gap between a heading and its provenance tag.
const TAG_GAP: f32 = 6.0;
/// The padding at each side of a provenance tag (`.prov{padding:0 5px}`).
const TAG_PAD_X: f32 = 5.0;
/// The height of a provenance tag (`.prov{line-height:16px}`).
const TAG_HEIGHT: f32 = 16.0;

/// The id of the list's filter box, so a test can give it the keyboard.
#[must_use]
pub fn filter_id() -> Id {
    Id::unique("masters-filter")
}

/// The filter box: a search glyph and a frameless one-line field in a bordered box (`.qe`).
/// Returns whether the text changed.
pub(super) fn filter_box(ui: &mut Ui, text: &mut String, hint: &str) -> bool {
    let t = tokens(ui);
    let k = kind(ui);
    let focused = ui.memory(|m| m.has_focus(filter_id()));
    let outline = if focused {
        Stroke::new(2.0, t.accent)
    } else {
        Stroke::new(border_width(k), t.border_strong)
    };
    Frame::new()
        .fill(t.surface)
        .stroke(outline)
        .corner_radius(RADIUS_M)
        .inner_margin(Margin {
            left: 12,
            right: 8,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (spot, _) =
                    ui.allocate_exact_size(vec2(ICON_SIZE, FILTER_LINE), Sense::hover());
                let glyph = Rect::from_center_size(spot.center(), Vec2::splat(ICON_SIZE));
                icon::paint(ui.painter(), glyph, Icon::Search, t.text_2);
                ui.add(
                    egui::TextEdit::singleline(text)
                        .id(filter_id())
                        .hint_text(RichText::new(hint).size(TEXT_BODY).color(t.text_3))
                        .frame(Frame::NONE)
                        .margin(Margin::ZERO)
                        .font(FontId::proportional(TEXT_BODY))
                        .text_color(t.text)
                        .align(Align2::LEFT_CENTER)
                        .min_size(vec2(0.0, FILTER_LINE))
                        .desired_width(ui.available_width()),
                )
                .changed()
            })
            .inner
        })
        .inner
}

/// One column of a list.
#[derive(Debug, Clone, Copy)]
pub(super) struct Col {
    /// The heading.
    pub label: &'static str,
    /// The width in points, or `None` for a share of what the fixed columns leave.
    pub width: Option<f32>,
    /// Figures: right-aligned heading and cells (`th.num`, `td.num`).
    pub numeric: bool,
    /// Shown when the pointer is over the heading, for a column whose figures are not here yet.
    pub hint: Option<&'static str>,
    /// The figures came from the engine: the heading carries the engine provenance tag with this
    /// text (`prov('eng','engine')`, index.html:1796; `prov('eng','stock engine')`, line 1798).
    pub tag: Option<&'static str>,
}

/// One cell of a list.
#[derive(Debug, Clone)]
pub(super) enum Cell {
    /// Text in the body ink.
    Text(String),
    /// Text in the secondary ink (`td.t2`).
    Quiet(String),
    /// A name with its alias after it in the `meta` style ("alias SGT").
    Named {
        /// The name.
        name: String,
        /// The aliases, joined; empty for none.
        alias: String,
    },
    /// A badge (`badge(..)` in a cell).
    Badge(BadgeKind, &'static str),
}

/// The total line under a list (`tr.total`, index.html:1800): the label across every column but
/// the last, and the figure in the last.
#[derive(Debug, Clone)]
pub(super) struct TotalLine {
    /// The label ("Closing stock").
    pub label: &'static str,
    /// The figure, as the engine gave it.
    pub value: String,
}

/// What the pointer did to the list this frame.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ListEvents {
    /// The row clicked.
    pub clicked: Option<usize>,
    /// The row double-clicked.
    pub opened: Option<usize>,
}

/// The column rectangles of a line `rect`, left to right.
fn column_rects(cols: &[Col], rect: Rect) -> Vec<Rect> {
    let fixed: f32 = cols.iter().filter_map(|c| c.width).sum();
    let fills = cols.iter().filter(|c| c.width.is_none()).count();
    let share = if fills == 0 {
        0.0
    } else {
        #[expect(clippy::cast_precision_loss, reason = "a handful of columns")]
        let n = fills as f32;
        ((rect.width() - fixed) / n).max(FILL_MIN)
    };
    let mut x = rect.left();
    cols.iter()
        .map(|c| {
            let w = c.width.unwrap_or(share);
            let r = Rect::from_min_max(pos2(x, rect.top()), pos2(x + w, rect.bottom()));
            x += w;
            r
        })
        .collect()
}

/// The list in a card that hugs it: the heading line, then the rows in a scroll area as tall as
/// the room left. Clicking a row reports it, double-clicking opens it.
pub(super) fn list_card(
    ui: &mut Ui,
    salt: &str,
    cols: &[Col],
    rows: &[Vec<Cell>],
    selected: Option<usize>,
    scroll_to_selected: bool,
    total: Option<&TotalLine>,
) -> ListEvents {
    let t = tokens(ui);
    let k = kind(ui);
    let room = (ui.available_height() - T_HEAD_HEIGHT - 8.0).max(T_ROW_HEIGHT * 3.0);
    let mut events = ListEvents::default();
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            let (head, response) =
                ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            paint_heading(ui, cols, head, &response);
            ScrollArea::vertical()
                .id_salt(("masters-list", salt))
                .max_height(room)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let width = ui.available_width();
                    for (index, row) in rows.iter().enumerate() {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::click());
                        let state = RowState {
                            selected: selected == Some(index),
                            hovered: response.hovered(),
                            sub: false,
                        };
                        table::paint_row(ui, rect, state);
                        paint_cells(ui, cols, rect, row);
                        if state.selected && scroll_to_selected {
                            response.scroll_to_me(None);
                        }
                        if response.clicked() {
                            events.clicked = Some(index);
                        }
                        if response.double_clicked() {
                            events.opened = Some(index);
                        }
                    }
                    if let Some(line) = total {
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(width, T_TOTAL_HEIGHT), Sense::hover());
                        paint_total(ui, cols, rect, line);
                    }
                });
        });
    events
}

/// The heading line in the `th` style; a column with a hint shows it under the pointer.
fn paint_heading(ui: &Ui, cols: &[Col], rect: Rect, response: &egui::Response) {
    let t = tokens(ui);
    table::paint_head_rule(ui, rect);
    let font = bold_font(ui, TEXT_META);
    for (col, r) in cols.iter().zip(column_rects(cols, rect)) {
        let place = if col.numeric {
            CellPlace::Right
        } else {
            CellPlace::Left(table::CELL_PAD_X)
        };
        let label_rect = if let Some(text) = col.tag {
            let tag = paint_engine_tag(ui, r, text);
            // `CellPlace::Right` keeps the cell's right padding, so give it back here: the label
            // then ends `TAG_GAP` before the tag.
            Rect::from_min_max(
                r.min,
                pos2(tag.left() - TAG_GAP + table::CELL_PAD_X, r.max.y),
            )
        } else {
            r
        };
        table::paint_cell_text(
            ui,
            label_rect,
            1.0,
            col.label,
            font.clone(),
            t.text_3,
            place,
        );
        if let Some(hint) = col.hint
            && response.hover_pos().is_some_and(|p| r.contains(p))
        {
            response.clone().on_hover_text(hint);
        }
    }
}

/// Paints an engine provenance tag reading `text` at the right of a heading cell `r` (`.prov`:
/// the engine colours, caption size, bold) and returns its rectangle.
fn paint_engine_tag(ui: &Ui, r: Rect, text: &str) -> Rect {
    let t = tokens(ui);
    let (bg, fg) = prov::colors(ProvKind::Engine, &t);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), bold_font(ui, TEXT_CAP), fg);
    let text = galley.size();
    let size = vec2(text.x + 2.0 * TAG_PAD_X, TAG_HEIGHT);
    let right = r.right() - table::CELL_PAD_X;
    // The heading's rule is the cell's last point, so the middle is half a point higher.
    let tag = Rect::from_center_size(pos2(right - size.x / 2.0, r.center().y - 0.5), size);
    ui.painter().rect_filled(tag, RADIUS_XS, bg);
    ui.painter().galley(
        pos2(tag.left() + TAG_PAD_X, tag.center().y - text.y / 2.0),
        galley,
        fg,
    );
    tag
}

/// The total line: its rules, the label in the first column and the figure in the last, in the
/// total weight (`font-weight:650`).
fn paint_total(ui: &Ui, cols: &[Col], rect: Rect, line: &TotalLine) {
    let t = tokens(ui);
    table::paint_total_rules(ui, rect);
    let font = bold_font(ui, TEXT_BODY);
    let rects = column_rects(cols, rect);
    let (Some(first), Some(last)) = (rects.first(), rects.last()) else {
        return;
    };
    let label_rect = Rect::from_min_max(first.min, pos2(last.left(), rect.bottom()));
    table::paint_cell_text(
        ui,
        label_rect,
        3.0,
        line.label,
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    table::paint_cell_text(ui, *last, 3.0, &line.value, font, t.text, CellPlace::Right);
}

/// The cells of one row.
fn paint_cells(ui: &mut Ui, cols: &[Col], rect: Rect, row: &[Cell]) {
    let t = tokens(ui);
    let font = FontId::proportional(TEXT_BODY);
    for ((col, r), cell) in cols.iter().zip(column_rects(cols, rect)).zip(row) {
        let place = if col.numeric {
            CellPlace::Right
        } else {
            CellPlace::Left(table::CELL_PAD_X)
        };
        match cell {
            Cell::Text(text) => {
                table::paint_cell_text(ui, r, 1.0, text, font.clone(), t.text, place);
            }
            Cell::Quiet(text) => {
                table::paint_cell_text(ui, r, 1.0, text, font.clone(), t.text_2, place);
            }
            Cell::Named { name, alias } => {
                table::paint_cell_text(ui, r, 1.0, name, font.clone(), t.text, place);
                if !alias.is_empty() {
                    let name_w = ui
                        .painter()
                        .layout_no_wrap(name.clone(), font.clone(), t.text)
                        .size()
                        .x;
                    let left = table::CELL_PAD_X + name_w + ALIAS_GAP;
                    table::paint_cell_text(
                        ui,
                        r,
                        1.0,
                        &format!("alias {alias}"),
                        FontId::proportional(TEXT_META),
                        t.text_3,
                        CellPlace::Left(left),
                    );
                }
            }
            Cell::Badge(kind_of, text) => {
                let inner = Rect::from_min_max(
                    pos2(r.left() + table::CELL_PAD_X, r.top()),
                    pos2(r.right() - table::CELL_PAD_X, r.bottom() - 1.0),
                );
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                badge(&mut child, *kind_of, text);
            }
        }
    }
}
