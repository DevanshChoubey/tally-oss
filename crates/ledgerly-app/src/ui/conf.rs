//! Confidence band: three bars and a word (`.conf` in the mock). The bars show how sure the
//! AI is; a rule is deterministic and shows no bars.

use egui::{CornerRadius, Rect, Response, RichText, Sense, Ui, Vec2, pos2};

use super::tokens;
use crate::i18n::Tr;
use crate::theme::TEXT_META;

/// How sure a proposal is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Three bars lit.
    High,
    /// Two bars lit.
    Medium,
    /// One bar lit.
    Low,
    /// A deterministic rule. No bars; the word says so.
    Rule,
}

/// How many of the three bars are lit for `level`.
#[must_use]
pub fn lit_bars(level: Confidence) -> usize {
    match level {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
        Confidence::Rule => 0,
    }
}

/// The words shown after the bars (mock `conf`: the band, then "confidence").
#[must_use]
pub fn word(level: Confidence) -> &'static str {
    match level {
        Confidence::High => "High confidence",
        Confidence::Medium => "Medium confidence",
        Confidence::Low => "Low confidence",
        Confidence::Rule => "Rule, deterministic",
    }
}

/// The word shown after the bars, in the words of `tr`.
#[must_use]
pub fn word_in(level: Confidence, tr: &Tr) -> String {
    tr.t(match level {
        Confidence::High => "conf.high",
        Confidence::Medium => "conf.medium",
        Confidence::Low => "conf.low",
        Confidence::Rule => "conf.rule",
    })
}

/// Width of one bar, in points.
const BAR_W: f32 = 5.0;
/// Height of one bar, in points.
const BAR_H: f32 = 12.0;
/// Space between bars.
const BAR_GAP: f32 = 2.0;
/// Left edge of each of the three bars, relative to the first.
const BAR_OFFSETS: [f32; 3] = [0.0, BAR_W + BAR_GAP, 2.0 * (BAR_W + BAR_GAP)];

/// Draws the confidence band for `level` in the words of `tr`: lit bars, then the word.
pub fn conf(ui: &mut Ui, level: Confidence, tr: &Tr) -> Response {
    let t = tokens(ui);
    let lit = lit_bars(level);
    let lit_color = t.text_2;
    let dim_color = t.text_2.gamma_multiply(0.25);
    let inner = ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(3.0 * BAR_W + 2.0 * BAR_GAP, BAR_H),
            Sense::hover(),
        );
        let painter = ui.painter();
        for (i, dx) in BAR_OFFSETS.iter().enumerate() {
            let bar =
                Rect::from_min_size(pos2(rect.min.x + *dx, rect.min.y), Vec2::new(BAR_W, BAR_H));
            let color = if i < lit { lit_color } else { dim_color };
            painter.rect_filled(bar, CornerRadius::same(1), color);
        }
        ui.label(
            RichText::new(word_in(level, tr))
                .size(TEXT_META)
                .strong()
                .color(t.text_2),
        );
    });
    inner.response
}
