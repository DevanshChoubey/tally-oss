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

use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::icon::{self, ICON_SIZE, Icon};
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT};
use crate::ui::{bold_font, kind, tokens};

/// Height of the filter's text line (`.qe input{height:30px}`).
const FILTER_LINE: f32 = 30.0;
/// The gap between the filter box and the card under it (`margin-bottom:12px`).
pub(super) const FILTER_GAP: f32 = 12.0;
/// The least width of a column that takes what is left.
const FILL_MIN: f32 = 160.0;
/// The gap between a name and its alias in a name cell (the space before `.meta`).
const ALIAS_GAP: f32 = 4.0;

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
        table::paint_cell_text(ui, r, 1.0, col.label, font.clone(), t.text_3, place);
        if let Some(hint) = col.hint
            && response.hover_pos().is_some_and(|p| r.contains(p))
        {
            response.clone().on_hover_text(hint);
        }
    }
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
