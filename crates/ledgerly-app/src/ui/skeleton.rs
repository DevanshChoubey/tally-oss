//! Skeleton: static placeholder bars shown while a result loads (`.skel` in the mock). The mock
//! shimmers them; the app keeps them still, so a screenshot and a slow machine look the same.
//! A stale result is never drawn as if it were current.

use egui::{CornerRadius, Sense, Ui, Vec2};

use super::tokens;
use crate::theme::{RADIUS_XS, SPACE_2};

/// Height of one placeholder bar, in points.
pub const BAR_HEIGHT: f32 = 12.0;

/// Draws one placeholder bar `width` points wide.
pub fn skeleton_bar(ui: &mut Ui, width: f32) {
    let t = tokens(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, BAR_HEIGHT), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_XS), t.sunken);
}

/// Draws `rows` placeholder bars stacked with the usual gap, each `width` points wide.
pub fn skeleton_rows(ui: &mut Ui, rows: usize, width: f32) {
    ui.vertical(|ui| {
        for _ in 0..rows {
            skeleton_bar(ui, width);
            ui.add_space(SPACE_2);
        }
    });
}
