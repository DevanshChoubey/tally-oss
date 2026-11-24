//! Card S7 (accessibility): WCAG contrast over the theme token pairs that screens draw.
//!
//! Thresholds are derived from WCAG 2.1: SC 1.4.3 (4.5:1 for text in light and dark),
//! SC 1.4.6 (7:1 for text in high contrast) and SC 1.4.11 (3:1 for non-text UI). Ratios are
//! measured with the WCAG relative-luminance formula below, which is written out here rather
//! than taken from `theme.rs`. No token value is changed by this card. The one pair family that
//! fails 3:1 (the divider `border` token) is pinned as a known gap; see the "Accessibility"
//! section of `design/PARITY-GAPS.md`.
#![allow(
    clippy::float_arithmetic,
    reason = "WCAG contrast is defined on real numbers; no money is involved"
)]

use egui::Color32;
use ledgerly_app::theme::{self, ThemeKind, Tokens};

/// WCAG 2.1 relative luminance: each sRGB channel c in 0..=1 maps to c/12.92 if c <= 0.03928,
/// else ((c + 0.055) / 1.055)^2.4; L = 0.2126 R + 0.7152 G + 0.0722 B.
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

/// WCAG 2.1 contrast ratio: (L1 + 0.05) / (L2 + 0.05), L1 the lighter.
fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Text pairs (foreground on background) that the screens draw, per theme.
fn text_pairs(t: &Tokens) -> Vec<(&'static str, Color32, Color32)> {
    vec![
        ("text on bg", t.text, t.bg),
        ("text on hover", t.text, t.hover),
        ("text_2 on hover", t.text_2, t.hover),
        ("text_2 on accent_soft", t.text_2, t.accent_soft),
        ("text_3 on hover", t.text_3, t.hover),
        ("text_3 on accent_soft", t.text_3, t.accent_soft),
        ("text_3 on ai_soft", t.text_3, t.ai_soft),
        ("text_3 on bad_soft", t.text_3, t.bad_soft),
        ("accent on accent_soft", t.accent, t.accent_soft),
        ("ai on ai_soft", t.ai, t.ai_soft),
        ("ok on ok_soft", t.ok, t.ok_soft),
        ("warn on warn_soft", t.warn, t.warn_soft),
        ("bad on bad_soft", t.bad, t.bad_soft),
        ("on_accent on accent", t.on_accent, t.accent),
    ]
}

/// Non-text UI pairs that must reach 3:1: the strong border that outlines controls, and the
/// accent used for focus, against the surfaces they sit on.
fn non_text_pairs(t: &Tokens) -> Vec<(&'static str, Color32, Color32)> {
    vec![
        ("border_strong on bg", t.border_strong, t.bg),
        ("border_strong on surface", t.border_strong, t.surface),
        ("accent on surface", t.accent, t.surface),
        ("accent on bg", t.accent, t.bg),
    ]
}

#[test]
fn text_on_its_fills_meets_the_threshold_of_each_theme() {
    // derived: SC 1.4.3 needs 4.5:1 for text in light and dark; SC 1.4.6 needs 7:1 in high
    // contrast. Passing pairs are listed in text_pairs; none needed a token change.
    for kind in ThemeKind::ALL {
        let t = theme::tokens(kind);
        let min = if kind == ThemeKind::HighContrast {
            7.0
        } else {
            4.5
        };
        for (name, fg, bg) in text_pairs(&t) {
            let ratio = contrast(fg, bg);
            assert!(
                ratio >= min,
                "{kind:?}: {name} is {ratio:.2}:1, need {min}:1"
            );
        }
    }
}

#[test]
fn controls_and_focus_meet_the_non_text_minimum() {
    // derived: SC 1.4.11 needs 3:1 for the visual boundary of a control and for focus.
    for kind in ThemeKind::ALL {
        let t = theme::tokens(kind);
        for (name, fg, bg) in non_text_pairs(&t) {
            let ratio = contrast(fg, bg);
            assert!(ratio >= 3.0, "{kind:?}: {name} is {ratio:.2}:1, need 3:1");
        }
    }
}

#[test]
fn divider_border_is_a_known_gap_below_the_non_text_minimum() {
    // Hand measurement, written out: light border #E2DFD8 on surface #FFFFFF is 1.33:1 and dark
    // border #34322E on surface #1C1B19 is 1.35:1; both sit below SC 1.4.11's 3:1. The token is
    // used for hairline dividers, not for control outlines, so the gap is recorded rather than
    // changed (tokens are frozen by this card). Pinned here so a token change shows up.
    // provenance: measured with the formula above; recorded in design/PARITY-GAPS.md "Accessibility".
    for (kind, bg) in [
        (ThemeKind::Light, Color32::from_rgb(0xFF, 0xFF, 0xFF)),
        (ThemeKind::Dark, Color32::from_rgb(0x1C, 0x1B, 0x19)),
    ] {
        let t = theme::tokens(kind);
        let ratio = contrast(t.border, bg);
        assert!(
            ratio < 3.0,
            "{kind:?}: divider border is {ratio:.2}:1; the known gap is below 3:1"
        );
    }
}
