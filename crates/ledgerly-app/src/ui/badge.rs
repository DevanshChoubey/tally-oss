//! Status badge: an icon and a word on a soft tint (`.badge` and `.b-*` in the mock).

use egui::{Color32, Frame, Margin, Painter, Pos2, Response, RichText, Sense, Stroke, Ui, Vec2};

use super::{kind, tokens};
use crate::theme::{RADIUS_XS, TEXT_CAP, ThemeKind, Tokens, border_width};

/// What a badge says. Each kind has its own icon as well as its colour, so colour is never the
/// only signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    /// Matched, passed or done. Check mark.
    Ok,
    /// Needs attention. Triangle.
    Warn,
    /// An error or a difference. Cross.
    Bad,
    /// The current or selected item. Dot.
    Accent,
    /// An AI proposal. Sparkle.
    Ai,
    /// A neutral status, such as "Locked". Ring.
    Neutral,
}

/// Background and text colour of a badge of `kind` (the mock's `.b-ok` to `.b-n`).
#[must_use]
pub fn colors(kind: BadgeKind, t: &Tokens) -> (Color32, Color32) {
    match kind {
        BadgeKind::Ok => (t.ok_soft, t.ok),
        BadgeKind::Warn => (t.warn_soft, t.warn),
        BadgeKind::Bad => (t.bad_soft, t.bad),
        BadgeKind::Accent => (t.accent_soft, t.accent_ink),
        BadgeKind::Ai => (t.ai_soft, t.ai_ink),
        BadgeKind::Neutral => (t.sunken, t.text_2),
    }
}

/// Side of the icon box, in points.
const ICON: f32 = 10.0;
/// Horizontal padding inside the badge (the mock's 7 px).
const PAD_X: i8 = 7;
/// Vertical padding inside the badge.
const PAD_Y: i8 = 1;

/// Draws a badge of `kind` reading `text`. In high contrast it is outlined in its text colour.
pub fn badge(ui: &mut Ui, kind_of: BadgeKind, text: &str) -> Response {
    let t = tokens(ui);
    let (bg, fg) = colors(kind_of, &t);
    let outline = if kind(ui) == ThemeKind::HighContrast {
        Stroke::new(border_width(ThemeKind::HighContrast), fg)
    } else {
        Stroke::NONE
    };
    Frame::new()
        .fill(bg)
        .stroke(outline)
        .corner_radius(RADIUS_XS)
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(ICON), Sense::hover());
                paint_icon(ui.painter(), rect.center(), kind_of, fg);
                ui.label(
                    RichText::new(text)
                        .font(super::bold_font(ui, TEXT_CAP))
                        .color(fg),
                );
            });
        })
        .response
}

/// Paints the icon of `kind` centred on `c`, in `color`.
fn paint_icon(painter: &Painter, c: Pos2, kind_of: BadgeKind, color: Color32) {
    let s = Stroke::new(1.5, color);
    match kind_of {
        BadgeKind::Ok => {
            painter.line_segment([c + Vec2::new(-3.5, 0.0), c + Vec2::new(-1.0, 2.5)], s);
            painter.line_segment([c + Vec2::new(-1.0, 2.5), c + Vec2::new(3.5, -2.5)], s);
        }
        BadgeKind::Warn => {
            let top = c + Vec2::new(0.0, -4.0);
            let left = c + Vec2::new(-4.0, 3.5);
            let right = c + Vec2::new(4.0, 3.5);
            painter.line_segment([top, left], s);
            painter.line_segment([left, right], s);
            painter.line_segment([right, top], s);
        }
        BadgeKind::Bad => {
            painter.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], s);
            painter.line_segment([c + Vec2::new(3.5, -3.5), c + Vec2::new(-3.5, 3.5)], s);
        }
        BadgeKind::Accent => {
            painter.circle_filled(c, 3.5, color);
        }
        BadgeKind::Ai => {
            painter.line_segment([c + Vec2::new(0.0, -4.0), c + Vec2::new(0.0, 4.0)], s);
            painter.line_segment([c + Vec2::new(-4.0, 0.0), c + Vec2::new(4.0, 0.0)], s);
        }
        BadgeKind::Neutral => {
            painter.circle_stroke(c, 3.5, s);
        }
    }
}
