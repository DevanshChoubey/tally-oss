//! Card U1: theme tokens. Expected values are the mock's tokens (derived) and the WCAG 2.1
//! contrast thresholds (derived); nothing here is copied from this crate's own output.
#![allow(
    clippy::float_arithmetic,
    reason = "WCAG contrast is defined on real numbers; no money is involved"
)]

use egui::Color32;
use ledgerly_app::theme::{self, ThemeKind, Tokens};

/// WCAG 2.1 relative luminance (definition in the "relative luminance" entry of the glossary):
/// each sRGB channel c in 0..=1 maps to c/12.92 if c <= 0.03928, else ((c + 0.055) / 1.055)^2.4;
/// L = 0.2126 R + 0.7152 G + 0.0722 B.
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

fn hex(c: Color32) -> u32 {
    (u32::from(c.r()) << 16) | (u32::from(c.g()) << 8) | u32::from(c.b())
}

#[test]
fn wcag_helper_matches_the_published_extremes() {
    // derived: WCAG 2.1 says black on white is 21:1 and identical colours are 1:1.
    // Hand check: L(white) = 1, L(black) = 0, so (1 + 0.05) / (0 + 0.05) = 21.
    let white = Color32::from_rgb(255, 255, 255);
    let black = Color32::from_rgb(0, 0, 0);
    assert!((contrast(white, black) - 21.0).abs() < 1e-9);
    assert!((contrast(white, white) - 1.0).abs() < 1e-9);
}

#[test]
fn light_tokens_are_the_mock_tokens() {
    // derived: design/mock/index.html lines 13-28 (:root), spec 7 names the mock as the visual
    // reference. ink_3 is a documented deviation, see below.
    let t = theme::tokens(ThemeKind::Light);
    let expected: [(&str, Color32, u32); 13] = [
        ("paper", t.paper, 0xF2F4F7),
        ("sheet", t.sheet, 0xFFFFFF),
        ("ink", t.ink, 0x172033),
        ("ink_2", t.ink_2, 0x47536A),
        ("line", t.line, 0xDCE1E9),
        ("ruling", t.ruling, 0xCBD6EC),
        ("margin", t.margin, 0xC23B35),
        ("accent", t.accent, 0x3541A8),
        ("accent_ink", t.accent_ink, 0xFFFFFF),
        ("accent_soft", t.accent_soft, 0xE5E8FA),
        ("haldi_soft", t.haldi_soft, 0xFBF0D8),
        ("leaf_soft", t.leaf_soft, 0xDDF0E4),
        ("focus_band", t.focus_band, 0xFFF7D6),
    ];
    for (name, got, want) in expected {
        assert_eq!(hex(got), want, "light token {name}");
    }
}

#[test]
fn dark_tokens_are_the_mock_tokens() {
    // derived: design/mock/index.html lines 38-41 and 45-48 (dark overrides).
    let t = theme::tokens(ThemeKind::Dark);
    let expected: [(&str, Color32, u32); 16] = [
        ("paper", t.paper, 0x0E1320),
        ("sheet", t.sheet, 0x151B2A),
        ("ink", t.ink, 0xE5E9F2),
        ("ink_2", t.ink_2, 0xA9B3C6),
        ("ink_3", t.ink_3, 0x7D889D),
        ("line", t.line, 0x263049),
        ("ruling", t.ruling, 0x25345A),
        ("margin", t.margin, 0xE46A5F),
        ("accent", t.accent, 0x97A0F7),
        ("accent_ink", t.accent_ink, 0x0E1320),
        ("accent_soft", t.accent_soft, 0x232A5C),
        ("haldi", t.haldi, 0xE6AE4E),
        ("haldi_soft", t.haldi_soft, 0x3A2C12),
        ("leaf", t.leaf, 0x62C08A),
        ("leaf_soft", t.leaf_soft, 0x173726),
        ("focus_band", t.focus_band, 0x2C2A17),
    ];
    for (name, got, want) in expected {
        assert_eq!(hex(got), want, "dark token {name}");
    }
}

#[test]
fn light_ink_3_deviates_from_the_mock_because_the_mock_fails_wcag() {
    // derived: the mock's #77839A on the sheet #FFFFFF. Linear channels:
    //   R 0x77 = 119/255 = 0.4667 -> ((0.4667+0.055)/1.055)^2.4 = 0.1845
    //   G 0x83 = 131/255 = 0.5137 -> 0.2270
    //   B 0x9A = 154/255 = 0.6039 -> 0.3231
    //   L = 0.2126*0.1845 + 0.7152*0.2270 + 0.0722*0.3231 = 0.2248
    //   ratio = (1 + 0.05) / (0.2248 + 0.05) = 3.82, below the 4.5 of WCAG 2.1 SC 1.4.3.
    let mock = Color32::from_rgb(0x77, 0x83, 0x9A);
    let sheet = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    assert!(contrast(mock, sheet) < 4.5);
    assert_ne!(hex(theme::tokens(ThemeKind::Light).ink_3), 0x77839A);
}

#[test]
fn light_haldi_and_leaf_deviate_from_the_mock_because_the_mock_fails_wcag() {
    // derived: the mock's haldi #9A6410 on its own soft background #FBF0D8 and leaf #2C7A4B on
    // #DDF0E4 (hand-checkable with the helper above; WCAG 2.1 SC 1.4.3 wants 4.5:1) give
    // 4.41:1 and 4.42:1. The shipped tokens are #8F5C0C and #286F45.
    let haldi_soft = Color32::from_rgb(0xFB, 0xF0, 0xD8);
    let leaf_soft = Color32::from_rgb(0xDD, 0xF0, 0xE4);
    assert!(contrast(Color32::from_rgb(0x9A, 0x64, 0x10), haldi_soft) < 4.5);
    assert!(contrast(Color32::from_rgb(0x2C, 0x7A, 0x4B), leaf_soft) < 4.5);
    let t = theme::tokens(ThemeKind::Light);
    assert_eq!(hex(t.haldi), 0x8F5C0C);
    assert_eq!(hex(t.leaf), 0x286F45);
}

/// Text pairs that screens draw: (name, foreground, background).
fn text_pairs(t: &Tokens) -> Vec<(&'static str, Color32, Color32)> {
    vec![
        ("ink on sheet", t.ink, t.sheet),
        ("ink on paper", t.ink, t.paper),
        ("ink_2 on sheet", t.ink_2, t.sheet),
        ("ink_2 on paper", t.ink_2, t.paper),
        ("ink_3 on sheet", t.ink_3, t.sheet),
        ("ink_3 on paper", t.ink_3, t.paper),
        ("accent on sheet", t.accent, t.sheet),
        ("accent on paper", t.accent, t.paper),
        ("margin on sheet", t.margin, t.sheet),
        ("margin on paper", t.margin, t.paper),
        ("haldi on sheet", t.haldi, t.sheet),
        ("haldi on paper", t.haldi, t.paper),
        ("leaf on sheet", t.leaf, t.sheet),
        ("leaf on paper", t.leaf, t.paper),
        ("accent_ink on accent", t.accent_ink, t.accent),
        ("ink on accent_soft", t.ink, t.accent_soft),
        ("accent on accent_soft", t.accent, t.accent_soft),
        ("haldi on haldi_soft", t.haldi, t.haldi_soft),
        ("leaf on leaf_soft", t.leaf, t.leaf_soft),
        ("ink on focus_band", t.ink, t.focus_band),
    ]
}

#[test]
fn light_and_dark_text_meets_wcag_aa() {
    // derived: WCAG 2.1 SC 1.4.3 requires 4.5:1 for normal text.
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        let t = theme::tokens(kind);
        for (name, fg, bg) in text_pairs(&t) {
            let ratio = contrast(fg, bg);
            assert!(ratio >= 4.5, "{kind:?}: {name} is {ratio:.2}:1");
        }
    }
}

#[test]
fn high_contrast_text_meets_wcag_aaa() {
    // derived: WCAG 2.1 SC 1.4.6 requires 7:1 for normal text.
    let t = theme::tokens(ThemeKind::HighContrast);
    for (name, fg, bg) in text_pairs(&t) {
        let ratio = contrast(fg, bg);
        assert!(ratio >= 7.0, "high contrast: {name} is {ratio:.2}:1");
    }
}

#[test]
fn borders_and_focus_meet_the_non_text_minimum_in_high_contrast() {
    // derived: WCAG 2.1 SC 1.4.11 requires 3:1 for interface components and focus indicators.
    let t = theme::tokens(ThemeKind::HighContrast);
    assert!(contrast(t.line, t.sheet) >= 3.0);
    assert!(contrast(t.accent, t.sheet) >= 3.0);
}

#[test]
fn accent_focus_indicator_meets_the_non_text_minimum_in_light_and_dark() {
    // derived: WCAG 2.1 SC 1.4.11, 3:1 against the surface it sits on.
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        let t = theme::tokens(kind);
        assert!(contrast(t.accent, t.sheet) >= 3.0, "{kind:?} on sheet");
        assert!(contrast(t.accent, t.paper) >= 3.0, "{kind:?} on paper");
    }
}

#[test]
fn the_three_themes_are_distinct() {
    let l = theme::tokens(ThemeKind::Light);
    let d = theme::tokens(ThemeKind::Dark);
    let h = theme::tokens(ThemeKind::HighContrast);
    assert_ne!(l, d);
    assert_ne!(d, h);
    assert_ne!(l, h);
}

#[test]
fn switcher_cycles_through_all_three_and_wraps() {
    // derived: spec 7 lists three themes (light, dark, high contrast); a switcher that cycles
    // returns to its start after one lap.
    assert_eq!(ThemeKind::ALL.len(), 3);
    let mut seen = vec![ThemeKind::Light];
    let mut k = ThemeKind::Light.next();
    while k != ThemeKind::Light {
        seen.push(k);
        k = k.next();
        assert!(seen.len() <= 3, "cycle longer than three themes");
    }
    assert_eq!(seen, ThemeKind::ALL.to_vec());
}

#[test]
fn labels_are_nonempty_and_unique() {
    let labels: Vec<_> = ThemeKind::ALL.iter().map(|k| k.label()).collect();
    assert!(labels.iter().all(|l| !l.is_empty()));
    let mut sorted = labels.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), labels.len());
}

#[test]
fn apply_sets_the_context_visuals() {
    // derived: the mock paints chrome with --paper and surfaces with --sheet.
    for kind in ThemeKind::ALL {
        let ctx = egui::Context::default();
        theme::apply(&ctx, kind);
        let t = theme::tokens(kind);
        let style = ctx.global_style();
        assert_eq!(style.visuals.panel_fill, t.paper, "{kind:?} panel");
        assert_eq!(style.visuals.window_fill, t.sheet, "{kind:?} window");
        assert_eq!(style.visuals.dark_mode, kind.is_dark(), "{kind:?} base");
    }
}

#[test]
fn apply_twice_switches_theme_in_place() {
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Dark);
    theme::apply(&ctx, ThemeKind::Light);
    let t = theme::tokens(ThemeKind::Light);
    assert_eq!(ctx.global_style().visuals.panel_fill, t.paper);
}

#[test]
fn focus_stroke_is_visible_and_thicker_in_high_contrast() {
    // derived: spec 7 asks for visible focus. The mock's focus ring is 2px
    // (`:focus-visible{outline:2px solid var(--accent)}`, line 22); high contrast uses 3.
    let focus = |k| theme::visuals(k).selection.stroke.width;
    assert!((focus(ThemeKind::Light) - 2.0).abs() < f32::EPSILON);
    assert!((focus(ThemeKind::Dark) - 2.0).abs() < f32::EPSILON);
    assert!((focus(ThemeKind::HighContrast) - 3.0).abs() < f32::EPSILON);
}
