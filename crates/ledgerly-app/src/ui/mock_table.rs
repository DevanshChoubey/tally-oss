//! A `table.t` drawn the way the mock lays it out (index.html:148-150): columns of fixed or
//! remaining width, a bold heading line over a rule, then body lines whose height follows their
//! tallest cell (text wraps, as in a browser), each with a rule under it.
//!
//! `TableBuilder` gives every row one height and clips long text, so it cannot draw the mock's
//! wrapping rows. Here each cell is a closure drawn into its own column, and the line takes the
//! height of the tallest. The table fills the width it is given; put it in [`super::card::card_flush`]
//! for a table that runs edge to edge, or in a padded card for one inside a panel.

#![allow(clippy::float_arithmetic, reason = "layout in points")]

use egui::{Align, Label, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};

use super::switch::switch;
use super::table::{CELL_PAD_X, T_HEAD_HEIGHT, T_ROW_HEIGHT, paint_cell_rule};
use super::{bold_font, para_text, tokens};
use crate::theme::{TEXT_BODY, TEXT_META};

/// Vertical padding of a body cell (`td{padding:7px 10px}`).
const PAD_Y: f32 = 7.0;

/// The width of one column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Width {
    /// Exactly this many points, padding included.
    Fixed(f32),
    /// An equal share of what the fixed columns leave.
    Rest,
}

/// One cell: draws itself into a top-down `Ui` as wide as its column less the padding.
pub type Cell<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// A cell of body text.
pub fn text(value: &str) -> Cell<'_> {
    Box::new(move |ui| {
        let t = tokens(ui);
        ui.add(Label::new(para_text(value, TEXT_BODY, t.text)).wrap());
    })
}

/// A cell of secondary text (`.t2`).
pub fn muted(value: &str) -> Cell<'_> {
    Box::new(move |ui| {
        let t = tokens(ui);
        ui.add(Label::new(para_text(value, TEXT_BODY, t.text_2)).wrap());
    })
}

/// A cell holding a read-only switch named `name`.
pub fn switch_cell(on: bool, name: &str) -> Cell<'_> {
    Box::new(move |ui| {
        switch(ui, on, Sense::hover(), name);
    })
}

/// Draws the table at the cursor and returns where it was drawn.
pub fn table(
    ui: &mut Ui,
    widths: &[Width],
    head: Option<&[&str]>,
    rows: Vec<Vec<Cell<'_>>>,
) -> Rect {
    let t = tokens(ui);
    let total = ui.available_width();
    let fixed: f32 = widths
        .iter()
        .map(|w| if let Width::Fixed(x) = w { *x } else { 0.0 })
        .sum();
    let flexible = widths.iter().filter(|w| **w == Width::Rest).count();
    let share = if flexible == 0 {
        0.0
    } else {
        ((total - fixed) / f32::from(u8::try_from(flexible).unwrap_or(u8::MAX))).max(0.0)
    };
    let cols: Vec<f32> = widths
        .iter()
        .map(|w| match w {
            Width::Fixed(x) => *x,
            Width::Rest => share,
        })
        .collect();
    let left = ui.cursor().min.x;
    let top = ui.cursor().min.y;
    let mut y = top;
    if let Some(head) = head {
        let mut x = left;
        for (label, w) in head.iter().zip(&cols) {
            ui.painter().text(
                pos2(x + CELL_PAD_X, y + T_HEAD_HEIGHT / 2.0 - 0.5),
                egui::Align2::LEFT_CENTER,
                label,
                bold_font(ui, TEXT_META),
                t.text_3,
            );
            x += w;
        }
        paint_cell_rule(
            ui,
            Rect::from_min_size(pos2(left, y), vec2(total, T_HEAD_HEIGHT)),
        );
        y += T_HEAD_HEIGHT;
    }
    for row in rows {
        let mut content = 0.0_f32;
        let mut x = left;
        for (cell, w) in row.into_iter().zip(&cols) {
            let area = Rect::from_min_size(
                pos2(x + CELL_PAD_X, y + PAD_Y),
                vec2((w - 2.0 * CELL_PAD_X).max(1.0), 1000.0),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(area)
                    .layout(Layout::top_down(Align::Min)),
            );
            cell(&mut child);
            content = content.max(child.min_rect().height());
            x += w;
        }
        let height = (content + 2.0 * PAD_Y + 1.0).max(T_ROW_HEIGHT);
        paint_cell_rule(ui, Rect::from_min_size(pos2(left, y), vec2(total, height)));
        y += height;
    }
    let (_, rect) = ui.allocate_space(vec2(total, y - top));
    rect
}

/// A heading for a panel (`h2` in a `.card.pad`): the bold 16 point title, with an optional icon.
pub fn panel_title(ui: &mut Ui, glyph: Option<super::icon::Icon>, title: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        if let Some(glyph) = glyph {
            super::icon::icon(ui, glyph, t.text_2);
            ui.add_space(6.0);
        }
        ui.label(
            RichText::new(title)
                .font(bold_font(ui, crate::theme::TEXT_H2))
                .color(t.text),
        );
    });
    ui.add_space(6.0);
}
