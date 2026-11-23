//! Switch: an on/off pill with a round knob (`.switch` in the mock). The accent when on, the
//! border colour when off.

use egui::{Response, Sense, Ui, pos2, vec2};

use super::tokens;

/// A switch's width, in points (`.switch` in the mock).
pub const SWITCH_WIDTH: f32 = 34.0;
/// A switch's height, in points.
pub const SWITCH_HEIGHT: f32 = 18.0;
/// How far the switch's knob sits in from its end, in points.
const KNOB_INSET: f32 = 9.0;
/// The knob's radius, in points.
const KNOB_RADIUS: f32 = 7.0;

/// Draws a switch showing `on`. With `sense` of [`Sense::click`] the caller flips its value when
/// the response is clicked; with [`Sense::hover`] the switch only shows a state.
pub fn switch(ui: &mut Ui, on: bool, sense: Sense) -> Response {
    let t = tokens(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(SWITCH_WIDTH, SWITCH_HEIGHT), sense);
    let fill = if on { t.accent } else { t.border };
    #[expect(
        clippy::float_arithmetic,
        reason = "the pill's radius is half its height"
    )]
    let radius = SWITCH_HEIGHT / 2.0;
    ui.painter().rect_filled(rect, radius, fill);
    #[expect(
        clippy::float_arithmetic,
        reason = "the knob's centre from the switch's end"
    )]
    let knob_x = if on {
        rect.right() - KNOB_INSET
    } else {
        rect.left() + KNOB_INSET
    };
    ui.painter()
        .circle_filled(pos2(knob_x, rect.center().y), KNOB_RADIUS, t.surface);
    response
}
