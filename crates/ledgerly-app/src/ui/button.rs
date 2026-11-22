//! Button with its key inside (`.btn` in the mock): the label, then the key as a keycap, such as
//! "Save" followed by `Ctrl+A`. Every button can be used with the mouse, labelled in words.

use egui::{Align2, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::icon::{self, ICON_SIZE, Icon};
use super::{keycap, kind, tokens};
use crate::theme::{RADIUS_S, TEXT_BODY, Tokens, border_width};

/// The three button roles in the mock: the main action, a plain action and a destructive one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// The primary action of a screen: accent fill.
    Primary,
    /// Any other action: surface with a strong border.
    Secondary,
    /// A destructive action such as delete or reject: red text and border.
    Danger,
}

/// Height of a button, in points.
const HEIGHT: f32 = 32.0;
/// Horizontal padding inside a button.
const PAD_X: f32 = 12.0;
/// Space between the label and its key.
const GAP: f32 = 8.0;

/// Fill, edge and label colours of a button. Disabled buttons are sunken with muted text.
#[must_use]
pub fn colors(
    kind_of: ButtonKind,
    enabled: bool,
    hovered: bool,
    t: &Tokens,
) -> (Color32, Color32, Color32) {
    if !enabled {
        return (t.sunken, t.border, t.text_3);
    }
    match kind_of {
        ButtonKind::Primary => (t.accent, t.accent, t.on_accent),
        ButtonKind::Secondary => {
            let fill = if hovered { t.hover } else { t.surface };
            (fill, t.border_strong, t.text)
        }
        ButtonKind::Danger => (t.surface, t.bad, t.bad),
    }
}

/// Draws a button reading `label`, with `key` (such as `Ctrl+A`) as a keycap at its right end.
/// Pass `None` for a button without a key. Disabled when the surrounding `Ui` is disabled.
pub fn button(ui: &mut Ui, label: &str, key: Option<&str>, kind_of: ButtonKind) -> Response {
    button_icon(ui, None, label, key, kind_of)
}

/// Draws a button like [`button`], with `which` first when given (`.btn` with an icon: the icon,
/// 8 points, the label).
pub fn button_icon(
    ui: &mut Ui,
    which: Option<Icon>,
    label: &str,
    key: Option<&str>,
    kind_of: ButtonKind,
) -> Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let enabled = ui.is_enabled();
    let font = FontId::proportional(TEXT_BODY);
    let text_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let key_w = key.map_or(0.0, |k| GAP + keycap::width(ui, k));
    let icon_w = which.map_or(0.0, |_| ICON_SIZE + GAP);
    let width = 2.0 * PAD_X + icon_w + text_w + key_w;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::click());
    let (fill, edge, ink) = colors(kind_of, enabled, response.hovered(), &t);
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(rect, RADIUS_S, Stroke::new(bw, edge), StrokeKind::Inside);
    if let Some(i) = which {
        let r = Rect::from_min_size(
            pos2(rect.min.x + PAD_X, rect.center().y - ICON_SIZE / 2.0),
            Vec2::splat(ICON_SIZE),
        );
        icon::paint(painter, r, i, ink);
    }
    painter.text(
        pos2(rect.min.x + PAD_X + icon_w, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        ink,
    );
    if let Some(k) = key {
        let w = keycap::width(ui, k);
        let top = rect.center().y - keycap::KEYCAP_HEIGHT / 2.0;
        let key_rect = Rect::from_min_size(
            pos2(rect.max.x - PAD_X - w, top),
            Vec2::new(w, keycap::KEYCAP_HEIGHT),
        );
        keycap::paint(painter, key_rect, k, fill, ink, ink);
    }
    response
}
