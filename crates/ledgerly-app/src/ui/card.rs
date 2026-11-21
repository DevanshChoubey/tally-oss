//! Card: a surface with a hairline border, radius 10 and shadow-1 (`.card` in the mock).

use egui::{Frame, InnerResponse, Margin, Stroke, Ui};

use super::{kind, tokens};
use crate::theme::{RADIUS_M, border_width, shadow_1};

/// Horizontal padding inside a card (the mock's `.card.pad`).
const PAD_X: i8 = 16;
/// Vertical padding inside a card.
const PAD_Y: i8 = 14;

/// Draws `add_contents` inside a card. Cards are the only framed surface on a page.
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let t = tokens(ui);
    let k = kind(ui);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .shadow(shadow_1(k))
        .show(ui, add_contents)
}
