//! Keycap: a key name in a small raised box (`kbd` in the mock). Every action shows its key.

use egui::{
    Align2, Color32, Context, FontFamily, FontId, Painter, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2, pos2,
};

use super::tokens;
use crate::theme::{self, RADIUS_XS, TEXT_CAP};

/// Height of a keycap, in points: the mock's 16 px line, a 1 px top border and a 2 px bottom
/// border (`kbd{font:600 11px/16px; border-bottom-width:2px}`).
pub const KEYCAP_HEIGHT: f32 = 19.0;
/// Width of the bottom border, in points (`border-bottom-width:2px`).
const BOTTOM_EDGE: f32 = 2.0;
/// Horizontal padding on each side of the label.
const PAD_X: f32 = 5.0;
/// Narrowest keycap, so a one-letter key still reads as a box.
const MIN_WIDTH: f32 = 18.0;

/// Width a keycap labelled `label` needs.
#[must_use]
pub fn width(ui: &Ui, label: &str) -> f32 {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), cap_font(ui.ctx()), Color32::WHITE);
    (galley.size().x + 2.0 * PAD_X).max(MIN_WIDTH)
}

/// The keycap font: the bold face at the caption size (the mock's weight 600), or the regular
/// face where the bold one is not installed (a bare context in tests).
fn cap_font(ctx: &Context) -> FontId {
    let bold = theme::bold_family();
    let bound = ctx.fonts(|f| f.definitions().families.contains_key(&bold));
    let family = if bound {
        bold
    } else {
        FontFamily::Proportional
    };
    FontId::new(TEXT_CAP, family)
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
    // The theme's border width (`border:var(--bw)`: 1 px, or 2 px in high contrast), and one
    // more point of edge along the bottom (`border-bottom-width:2px`).
    let bw = theme::border_width(theme::current(painter.ctx()));
    painter.rect_filled(rect, RADIUS_XS, fill);
    painter.rect_stroke(rect, RADIUS_XS, Stroke::new(bw, edge), StrokeKind::Inside);
    let bottom = rect.max.y - BOTTOM_EDGE + 0.5;
    painter.line_segment(
        [
            pos2(rect.min.x + 2.0, bottom),
            pos2(rect.max.x - 2.0, bottom),
        ],
        Stroke::new(1.0, edge),
    );
    let inner = Rect::from_min_max(
        rect.min + Vec2::splat(1.0),
        pos2(rect.max.x - 1.0, rect.max.y - BOTTOM_EDGE),
    );
    painter.text(
        inner.center(),
        Align2::CENTER_CENTER,
        label,
        cap_font(painter.ctx()),
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
