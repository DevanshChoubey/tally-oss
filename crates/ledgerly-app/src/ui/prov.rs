//! Provenance tag: says where a value came from, such as "engine" or "AI" (`.prov` in the mock).
//! Every value on a proposal carries one. The tag has a tooltip with the longer description.

use egui::{Color32, Frame, Margin, Response, RichText, Ui};

use super::tokens;
use crate::theme::{RADIUS_XS, TEXT_CAP, Tokens};

/// Where a value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvKind {
    /// Read from a document, such as a bill image. Shown dashed until a person touches it.
    FromBill,
    /// Proposed by the AI model.
    Ai,
    /// Typed by the person in their own words.
    YourWords,
    /// Calculated by the engine from the books.
    Engine,
    /// Read from the books (a balance or a posted voucher).
    Books,
    /// Produced by a deterministic rule, not by the AI.
    Rule,
}

/// The short label of a provenance tag.
#[must_use]
pub fn label(kind_of: ProvKind) -> &'static str {
    match kind_of {
        ProvKind::FromBill => "from bill",
        ProvKind::Ai => "AI",
        ProvKind::YourWords => "your words",
        ProvKind::Engine => "engine",
        ProvKind::Books => "books",
        ProvKind::Rule => "rule",
    }
}

/// The longer description shown in the tooltip.
#[must_use]
pub fn description(kind_of: ProvKind) -> &'static str {
    match kind_of {
        ProvKind::FromBill => "Read from the document image. Check it before you accept.",
        ProvKind::Ai => "Suggested by the AI model. The engine checks every figure.",
        ProvKind::YourWords => "Typed by you.",
        ProvKind::Engine => "Calculated by the engine from the books.",
        ProvKind::Books => "Taken from your books.",
        ProvKind::Rule => "Produced by a fixed rule, not by the AI.",
    }
}

/// Background and text colour of a tag of `kind`.
#[must_use]
pub fn colors(kind_of: ProvKind, t: &Tokens) -> (Color32, Color32) {
    match kind_of {
        ProvKind::FromBill | ProvKind::Ai => (t.ai_soft, t.ai_ink),
        ProvKind::YourWords | ProvKind::Rule => (t.sunken, t.text_2),
        ProvKind::Engine => (t.ok_soft, t.ok),
        ProvKind::Books => (t.accent_soft, t.accent_ink),
    }
}

/// Horizontal padding inside a tag.
const PAD_X: i8 = 5;

/// Draws a provenance tag for `kind_of`.
pub fn prov(ui: &mut Ui, kind_of: ProvKind) -> Response {
    let t = tokens(ui);
    let (bg, fg) = colors(kind_of, &t);
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_XS)
        .inner_margin(Margin::symmetric(PAD_X, 0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label(kind_of))
                    .size(TEXT_CAP)
                    .strong()
                    .color(fg),
            );
        })
        .response
        .on_hover_text(description(kind_of))
}
