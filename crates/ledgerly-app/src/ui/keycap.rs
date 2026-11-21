//! Keycap: a key name in a small raised box (`kbd` in the mock). Every action shows its key.

use egui::{Align2, Color32, FontId, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::tokens;
use crate::theme::{RADIUS_XS, TEXT_CAP};

/// Height of a keycap, in points.
pub const KEYCAP_HEIGHT: f32 = 16.0;
/// Horizontal padding on each side of the label.
const PAD_X: f32 = 5.0;
/// Narrowest keycap, so a one-letter key still reads as a box.
const MIN_WIDTH: f32 = 18.0;

/// Width a keycap labelled `label` needs.
#[must_use]
pub fn width(ui: &Ui, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(TEXT_CAP),
        Color32::WHITE,
    );
    (galley.size().x + 2.0 * PAD_X).max(MIN_WIDTH)
}

/// Paints a keycap into `rect`: `fill` behind the label, `edge` as the outline, `ink` for the
/// label. Used by [`keycap`] and by buttons that carry a key.
pub fn paint(
    painter: &Painter,
    rect: Rect,
    label: &str,
    fill: Color32,
    edge: Color32,
    ink: Color32,
) {
    painter.rect_filled(rect, RADIUS_XS, fill);
    painter.rect_stroke(rect, RADIUS_XS, Stroke::new(1.0, edge), StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(TEXT_CAP),
        ink,
    );
}

/// A keycap showing `label`, such as `Ctrl+A`, drawn in the current theme.
pub fn keycap(ui: &mut Ui, label: &str) -> Response {
    let t = tokens(ui);
    let w = width(ui, label);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, KEYCAP_HEIGHT), Sense::hover());
    paint(ui.painter(), rect, label, t.surface_2, t.border, t.text_2);
    response
}
