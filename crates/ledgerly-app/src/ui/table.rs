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

use egui::{Align, Layout, RichText, Ui};
use egui_extras::{TableBuilder, TableRow};

use super::tokens;
use crate::theme::{TEXT_BODY, TEXT_META};

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
