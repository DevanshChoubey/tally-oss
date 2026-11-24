//! Accessibility for the kit's own widgets. Each interactive widget names itself for screen
//! readers through egui's [`WidgetInfo`], which egui exposes to the platform through AccessKit,
//! and each one shows a focus ring while it has keyboard focus.
//!
//! The kit paints most of its widgets by hand, so egui cannot name them from their text: a
//! painted label creates no accessibility node. Each kit function therefore calls [`name`] or
//! [`name_selected`] with the same text it paints, so the spoken name and the drawn text cannot
//! drift apart.

use egui::{Rect, Response, Role, Stroke, StrokeKind, Ui, WidgetInfo};

use super::{kind, tokens};
use crate::theme;

/// Gap between a widget's edge and its focus ring, in points.
const RING_GAP: f32 = 2.0;

/// Names `response` for screen readers with `role` and the accessible name `label`. The widget is
/// reported disabled when `ui` is.
pub fn name(ui: &Ui, response: &Response, role: Role, label: &str) {
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(role, enabled, label));
}

/// Like [`name`], and also reports whether the widget is selected (a tab, a chosen option card,
/// a toggled switch).
pub fn name_selected(ui: &Ui, response: &Response, role: Role, selected: bool, label: &str) {
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(role, enabled, selected, label));
}

/// Joins the non-empty `parts` of an accessible name with ", ", as a screen reader reads a row
/// of text: `"Cash, debit, 12,000.00"`. Parts are taken as they are drawn; nothing is reformatted.
#[must_use]
pub fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Draws the keyboard focus ring around `rect` when `response` has keyboard focus. The ring is
/// the focus width of the active theme in the accent colour, outside the widget.
pub fn focus_ring(ui: &Ui, response: &Response, rect: Rect) {
    if !response.has_focus() {
        return;
    }
    let t = tokens(ui);
    let width = theme::focus_width(kind(ui));
    ui.painter().rect_stroke(
        rect.expand(RING_GAP + width / 2.0),
        theme::RADIUS_S,
        Stroke::new(width, t.accent),
        StrokeKind::Middle,
    );
}
