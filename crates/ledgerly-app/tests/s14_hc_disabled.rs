//! Sprint 14: in high contrast, egui's own disabled widgets must stay legible.
//!
//! egui draws a disabled widget by multiplying its colours by `Visuals::disabled_alpha`
//! (default 0.5), which over the page colour drops high-contrast text below 7:1.
//! provenance: derived. WCAG 2.1 SC 1.4.6 asks 7:1 for text in a high-contrast theme; the
//! blend and the relative-luminance formula below are WCAG 2.1's own definitions.
#![allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "WCAG contrast is defined on real numbers; the blend is clamped to 0..=255; no money"
)]

use egui::Color32;
use ledgerly_app::theme::{self, ThemeKind};

fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// `fg` drawn at `alpha` over `bg`, per channel.
fn blend(fg: Color32, bg: Color32, alpha: f32) -> Color32 {
    let mix = |f: u8, b: u8| {
        let v = f32::from(f) * alpha + f32::from(b) * (1.0 - alpha);
        v.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(
        mix(fg.r(), bg.r()),
        mix(fg.g(), bg.g()),
        mix(fg.b(), bg.b()),
    )
}

#[test]
fn disabled_text_in_high_contrast_keeps_seven_to_one() {
    let t = theme::tokens(ThemeKind::HighContrast);
    let alpha = theme::visuals(ThemeKind::HighContrast).disabled_alpha;
    let seen = blend(t.text, t.surface, alpha);
    let ratio = contrast(seen, t.surface);
    assert!(
        ratio >= 7.0,
        "disabled text in high contrast measures {ratio:.2}:1 at alpha {alpha}"
    );
}
