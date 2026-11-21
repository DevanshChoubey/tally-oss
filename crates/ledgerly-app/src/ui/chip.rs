//! Chip: a bordered control in the top bar, such as the company, the date or the AI tier
//! (`.chip` in the mock).

use egui::{Align2, Color32, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::{kind, tokens};
use crate::theme::{RADIUS_S, TEXT_META, border_width};

/// Height of a chip, in points (the mock's 30 px).
pub const CHIP_HEIGHT: f32 = 30.0;
/// Horizontal padding inside a chip.
const PAD_X: f32 = 10.0;

/// Draws a chip reading `label` and returns its response. The hover fill appears on hover.
pub fn chip(ui: &mut Ui, label: &str) -> Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let font = FontId::proportional(TEXT_META);
    let text_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(2.0 * PAD_X + text_w, CHIP_HEIGHT),
        Sense::click(),
    );
    let fill = if response.hovered() { t.hover } else { t.surface };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(rect, RADIUS_S, Stroke::new(bw, t.border), StrokeKind::Inside);
    painter.text(
        pos2(rect.min.x + PAD_X, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        t.text_2,
    );
    response
}
