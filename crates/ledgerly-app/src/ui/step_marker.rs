//! A step's round marker (`.steps button i`, `design/mock-v2/index.html:314`): a green check for
//! a step passed, the number on the accent for the step on screen, the number in a ring for one
//! still ahead. The setup wizard's rail and the import screen's step line both draw it.

use egui::{Align2, Pos2, Rect, Stroke, Ui, Vec2};

use super::icon::{self, Icon};
use super::{bold_font, kind, tokens};
use crate::theme::{TEXT_CAP, border_width};

/// A marker's diameter.
pub const MARKER_SIZE: f32 = 22.0;
/// The check inside a passed step's marker.
const CHECK_SIZE: f32 = 14.0;

/// How a marker is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMarker {
    /// A step already passed: a green check.
    Passed,
    /// The step on screen: its number on the accent.
    Current,
    /// A step still ahead: its number in a ring.
    Ahead,
}

/// Draws the marker for step `number` (the text inside it) centred on `center`.
#[expect(
    clippy::float_arithmetic,
    reason = "the ring sits half a stroke inside the circle"
)]
pub fn paint(ui: &Ui, center: Pos2, marker: StepMarker, number: &str) {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let painter = ui.painter();
    let radius = MARKER_SIZE / 2.0;
    let cap = bold_font(ui, TEXT_CAP);
    match marker {
        StepMarker::Passed => {
            painter.circle_filled(center, radius, t.ok);
            let check = Rect::from_center_size(center, Vec2::splat(CHECK_SIZE));
            icon::paint(painter, check, Icon::Check, t.surface);
        }
        StepMarker::Current => {
            painter.circle_filled(center, radius, t.accent);
            painter.text(center, Align2::CENTER_CENTER, number, cap, t.on_accent);
        }
        StepMarker::Ahead => {
            painter.circle_stroke(center, radius - bw / 2.0, Stroke::new(bw, t.border_strong));
            painter.text(center, Align2::CENTER_CENTER, number, cap, t.text_2);
        }
    }
}
