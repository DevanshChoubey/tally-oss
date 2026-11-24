//! Toast: a short message in a dark pill, centred above the key bar (`.toast`,
//! `design/mock-v2/index.html:290-292`; `toast()` at 920). It may carry one underlined action.

use egui::{Align2, Area, Context, Frame, Id, Margin, Order, RichText, Role, Sense, Stroke, vec2};

use super::{a11y, bold_font};
use crate::theme::{self, RADIUS_M, TEXT_BODY, border_width};

/// How far the pill sits above the window's bottom edge (`bottom:48px`).
const BOTTOM_OFFSET: f32 = 48.0;
/// The widest the pill gets before its text wraps.
const MAX_WIDTH: f32 = 560.0;

/// What the person did with the toast in one frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToastOutcome {
    /// The action button was clicked.
    pub action: bool,
}

/// Draws the toast with `text` and, when given, an action button labelled `action`.
pub fn toast(ctx: &Context, text: &str, action: Option<&str>) -> ToastOutcome {
    let kind = theme::current(ctx);
    let t = theme::tokens(kind);
    let mut outcome = ToastOutcome::default();
    Area::new(Id::unique("ledgerly-toast"))
        .order(Order::Tooltip)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -BOTTOM_OFFSET))
        .show(ctx, |ui| {
            Frame::new()
                .fill(t.text)
                .stroke(Stroke::new(border_width(kind), t.text))
                .corner_radius(RADIUS_M)
                .shadow(theme::shadow_2(kind))
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_max_width(MAX_WIDTH);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.add(
                            egui::Label::new(RichText::new(text).size(TEXT_BODY).color(t.surface))
                                .wrap(),
                        );
                        if let Some(label) = action {
                            let button = ui.add(
                                egui::Label::new(
                                    RichText::new(label)
                                        .font(bold_font(ui, TEXT_BODY))
                                        .color(t.surface)
                                        .underline(),
                                )
                                .sense(Sense::click()),
                            );
                            a11y::name(ui, &button, Role::Button, label);
                            outcome.action = button.clicked();
                        }
                    });
                });
        });
    outcome
}
