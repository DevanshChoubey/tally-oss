//! Card U1: theme tokens. Expected values are the mock v2's tokens (derived from
//! `design/mock-v2/index.html` `:root` at commit 2178293) and the WCAG 2.1 contrast thresholds
//! (derived); nothing here is copied from this crate's own output.
#![allow(
    clippy::float_arithmetic,
    reason = "WCAG contrast is defined on real numbers; no money is involved"
)]

use egui::{Color32, FontFamily, RawInput, TextStyle};
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

/// Two f32 values are the same to well within a pixel.
fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
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
    // derived: design/mock-v2/index.html `:root` (lines 30-36).
    let t = theme::tokens(ThemeKind::Light);
    let expected: [(&str, Color32, u32); 23] = [
        ("bg", t.bg, 0x00F6_F5F2),
        ("surface", t.surface, 0x00FF_FFFF),
        ("surface_2", t.surface_2, 0x00FB_FAF8),
        ("sunken", t.sunken, 0x00EF_EDE8),
        ("hover", t.hover, 0x00F1_EFEA),
        ("border", t.border, 0x00E2_DFD8),
        ("border_strong", t.border_strong, 0x008F_897E),
        ("text", t.text, 0x001C_1B19),
        ("text_2", t.text_2, 0x004A_4741),
        ("text_3", t.text_3, 0x0061_5D55),
        ("accent", t.accent, 0x006B_2D73),
        ("on_accent", t.on_accent, 0x00FF_FFFF),
        ("accent_soft", t.accent_soft, 0x00F3_E8F4),
        ("accent_ink", t.accent_ink, 0x005A_2161),
        ("ai", t.ai, 0x009A_4211),
        ("ai_soft", t.ai_soft, 0x00FB_EDE3),
        ("ai_ink", t.ai_ink, 0x0086_390D),
        ("ok", t.ok, 0x001E_6E3F),
        ("ok_soft", t.ok_soft, 0x00E3_F2E8),
        ("warn", t.warn, 0x007E_4C00),
        ("warn_soft", t.warn_soft, 0x00FB_F0DC),
        ("bad", t.bad, 0x00B3_261E),
        ("bad_soft", t.bad_soft, 0x00FC_E9E7),
    ];
    for (name, got, want) in expected {
        assert_eq!(hex(got), want, "light token {name}");
    }
}

#[test]
fn dark_tokens_are_the_mock_tokens() {
    // derived: design/mock-v2/index.html `:root[data-theme="dark"]` (lines 39-46).
    let t = theme::tokens(ThemeKind::Dark);
    let expected: [(&str, Color32, u32); 23] = [
        ("bg", t.bg, 0x0014_1312),
        ("surface", t.surface, 0x001C_1B19),
        ("surface_2", t.surface_2, 0x0023_221F),
        ("sunken", t.sunken, 0x000F_0E0D),
        ("hover", t.hover, 0x002A_2825),
        ("border", t.border, 0x0034_322E),
        ("border_strong", t.border_strong, 0x0077_726A),
        ("text", t.text, 0x00EE_EBE5),
        ("text_2", t.text_2, 0x00C9_C4BA),
        ("text_3", t.text_3, 0x00A8_A296),
        ("accent", t.accent, 0x00D9_A6E0),
        ("on_accent", t.on_accent, 0x002A_0F2E),
        ("accent_soft", t.accent_soft, 0x003A_2340),
        ("accent_ink", t.accent_ink, 0x00EB_CBEF),
        ("ai", t.ai, 0x00F2_A36E),
        ("ai_soft", t.ai_soft, 0x003D_2414),
        ("ai_ink", t.ai_ink, 0x00F7_BE97),
        ("ok", t.ok, 0x007F_CF9A),
        ("ok_soft", t.ok_soft, 0x0017_3323),
        ("warn", t.warn, 0x00E9_B860),
        ("warn_soft", t.warn_soft, 0x003A_2A10),
        ("bad", t.bad, 0x00F2_8B82),
        ("bad_soft", t.bad_soft, 0x003E_1C1A),
    ];
    for (name, got, want) in expected {
        assert_eq!(hex(got), want, "dark token {name}");
    }
}

#[test]
fn high_contrast_tokens_are_the_mock_tokens() {
    // derived: design/mock-v2/index.html `:root[data-theme="high-contrast"]` (lines 49-56).
    let t = theme::tokens(ThemeKind::HighContrast);
    let expected: [(&str, Color32, u32); 23] = [
        ("bg", t.bg, 0x0000_0000),
        ("surface", t.surface, 0x0000_0000),
        ("surface_2", t.surface_2, 0x0000_0000),
        ("sunken", t.sunken, 0x0000_0000),
        ("hover", t.hover, 0x001A_1A1A),
        ("border", t.border, 0x00FF_FFFF),
        ("border_strong", t.border_strong, 0x00FF_FFFF),
        ("text", t.text, 0x00FF_FFFF),
        ("text_2", t.text_2, 0x00FF_FFFF),
        ("text_3", t.text_3, 0x00E6_E6E6),
        ("accent", t.accent, 0x00F5_B8FF),
        ("on_accent", t.on_accent, 0x0000_0000),
        ("accent_soft", t.accent_soft, 0x002B_0A30),
        ("accent_ink", t.accent_ink, 0x00F5_B8FF),
        ("ai", t.ai, 0x00FF_B37A),
        ("ai_soft", t.ai_soft, 0x002E_1606),
        ("ai_ink", t.ai_ink, 0x00FF_C79E),
        ("ok", t.ok, 0x008C_F5A9),
        ("ok_soft", t.ok_soft, 0x0006_2612),
        ("warn", t.warn, 0x00FF_D27A),
        ("warn_soft", t.warn_soft, 0x002E_2000),
        ("bad", t.bad, 0x00FF_9C94),
        ("bad_soft", t.bad_soft, 0x0033_0B08),
    ];
    for (name, got, want) in expected {
        assert_eq!(hex(got), want, "high contrast token {name}");
    }
}

#[test]
fn v1_text_3_failed_aa_and_the_v2_value_passes() {
    // derived: the v1 mock's #77839A on the surface #FFFFFF. Linear channels:
    //   R 0x77 = 119/255 = 0.4667 -> ((0.4667+0.055)/1.055)^2.4 = 0.1845
    //   G 0x83 = 131/255 = 0.5137 -> 0.2270
    //   B 0x9A = 154/255 = 0.6039 -> 0.3231
    //   L = 0.2126*0.1845 + 0.7152*0.2270 + 0.0722*0.3231 = 0.2248
    //   ratio = (1 + 0.05) / (0.2248 + 0.05) = 3.82, below the 4.5 of WCAG 2.1 SC 1.4.3.
    // The v2 mock changed the value to #615D55 (6.55:1 on the surface), which is what we ship.
    let v1 = Color32::from_rgb(0x77, 0x83, 0x9A);
    let surface = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    assert!(contrast(v1, surface) < 4.5);
    let t = theme::tokens(ThemeKind::Light);
    assert_eq!(hex(t.text_3), 0x0061_5D55);
    assert!(contrast(t.text_3, t.surface) >= 4.5);
}

#[test]
fn v1_haldi_and_leaf_failed_aa_and_the_v2_warn_and_ok_pass() {
    // derived: the v1 mock's haldi #9A6410 on its soft background #FBF0D8 and leaf #2C7A4B on
    // #DDF0E4 (hand-checkable with the helper above; WCAG 2.1 SC 1.4.3 wants 4.5:1) give
    // 4.41:1 and 4.42:1. The v2 tokens are warn #7E4C00 and ok #1E6E3F.
    let haldi_soft = Color32::from_rgb(0xFB, 0xF0, 0xD8);
    let leaf_soft = Color32::from_rgb(0xDD, 0xF0, 0xE4);
    assert!(contrast(Color32::from_rgb(0x9A, 0x64, 0x10), haldi_soft) < 4.5);
    assert!(contrast(Color32::from_rgb(0x2C, 0x7A, 0x4B), leaf_soft) < 4.5);
    let t = theme::tokens(ThemeKind::Light);
    assert_eq!(hex(t.warn), 0x007E_4C00);
    assert_eq!(hex(t.ok), 0x001E_6E3F);
    assert!(contrast(t.warn, t.warn_soft) >= 4.5);
    assert!(contrast(t.ok, t.ok_soft) >= 4.5);
}

/// Text pairs that screens draw: (name, foreground, background).
fn text_pairs(t: &Tokens) -> Vec<(&'static str, Color32, Color32)> {
    vec![
        ("text on bg", t.text, t.bg),
        ("text on surface", t.text, t.surface),
        ("text on surface_2", t.text, t.surface_2),
        ("text on accent_soft", t.text, t.accent_soft),
        ("text_2 on bg", t.text_2, t.bg),
        ("text_2 on surface", t.text_2, t.surface),
        ("text_2 on sunken", t.text_2, t.sunken),
        ("text_3 on bg", t.text_3, t.bg),
        ("text_3 on surface", t.text_3, t.surface),
        ("text_3 on surface_2", t.text_3, t.surface_2),
        ("text_3 on sunken", t.text_3, t.sunken),
        ("accent on bg", t.accent, t.bg),
        ("accent on surface", t.accent, t.surface),
        ("accent_ink on accent_soft", t.accent_ink, t.accent_soft),
        ("on_accent on accent", t.on_accent, t.accent),
        ("ai on bg", t.ai, t.bg),
        ("ai on surface", t.ai, t.surface),
        ("ai_ink on ai_soft", t.ai_ink, t.ai_soft),
        ("ok on surface", t.ok, t.surface),
        ("ok on ok_soft", t.ok, t.ok_soft),
        ("warn on surface", t.warn, t.surface),
        ("warn on warn_soft", t.warn, t.warn_soft),
        ("bad on bg", t.bad, t.bg),
        ("bad on surface", t.bad, t.surface),
        ("bad on bad_soft", t.bad, t.bad_soft),
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
    assert!(contrast(t.border, t.surface) >= 3.0);
    assert!(contrast(t.accent, t.surface) >= 3.0);
}

#[test]
fn accent_focus_indicator_meets_the_non_text_minimum_in_light_and_dark() {
    // derived: WCAG 2.1 SC 1.4.11, 3:1 against the surface it sits on.
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        let t = theme::tokens(kind);
        assert!(contrast(t.accent, t.surface) >= 3.0, "{kind:?} on surface");
        assert!(contrast(t.accent, t.bg) >= 3.0, "{kind:?} on bg");
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
    // derived: the mock paints the page with --bg and surfaces with --surface.
    for kind in ThemeKind::ALL {
        let ctx = egui::Context::default();
        theme::apply(&ctx, kind);
        let t = theme::tokens(kind);
        let style = ctx.global_style();
        assert_eq!(style.visuals.panel_fill, t.bg, "{kind:?} panel");
        assert_eq!(style.visuals.window_fill, t.surface, "{kind:?} window");
        assert_eq!(style.visuals.dark_mode, kind.is_dark(), "{kind:?} base");
    }
}

#[test]
fn apply_twice_switches_theme_in_place() {
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Dark);
    theme::apply(&ctx, ThemeKind::Light);
    let t = theme::tokens(ThemeKind::Light);
    assert_eq!(ctx.global_style().visuals.panel_fill, t.bg);
}

#[test]
fn fonts_and_text_styles_are_installed_once_on_apply() {
    // derived: the mock's UI face is Inter (design/mock-v2/index.html `font-family`), regular and
    // 600 for bold. Inter 4.1 (OFL, github.com/rsms/inter) is bundled, downloaded with the user's
    // permission 2026-11-22; it replaces the Noto Sans stand-in of wave 1. Noto Sans follows it
    // for any glyph Inter lacks, Noto Sans Devanagari is the fallback for Hindi names, Noto Sans Telugu for Telugu
    // names (S22, the owner's masters file), and the
    // bold face is its own family led by Inter SemiBold.
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    theme::apply(&ctx, ThemeKind::Dark);
    ctx.run_ui(
        RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(800.0, 600.0),
            )),
            ..RawInput::default()
        },
        |_ui| {},
    )
    .drop_without_applying_deltas();
    let families = ctx.fonts(|f| f.definitions().families.clone());
    let chain = families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        chain.iter().take(3).map(String::as_str).collect::<Vec<_>>(),
        vec![
            "Inter-Regular",
            "NotoSans-Regular",
            "NotoSansDevanagari-Regular"
        ]
    );
    assert!(
        chain
            .iter()
            .any(|name| name == "NotoSansDevanagari-Regular")
    );
    let bold = families
        .get(&theme::bold_family())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        bold,
        vec![
            "Inter-SemiBold".to_owned(),
            "NotoSans-Bold".to_owned(),
            "NotoSansDevanagari-Bold".to_owned(),
            "NotoSansTelugu-Bold".to_owned()
        ]
    );
}

#[test]
fn text_styles_follow_the_type_scale() {
    // derived: design/mock-v2/index.html `--t-*` (lines 24): cap 11, meta 12, body 13, input 14,
    // h2 16, h1 20, fig 26.
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    let styles = ctx.global_style().text_styles.clone();
    let size = |style: TextStyle| styles.get(&style).map_or(0.0, |f| f.size);
    assert!(same(size(TextStyle::Small), 12.0));
    assert!(same(size(TextStyle::Body), 13.0));
    assert!(same(size(TextStyle::Button), 13.0));
    assert!(same(size(TextStyle::Heading), 20.0));
    assert!(same(size(TextStyle::Monospace), 13.0));
    let named = |name: &str| size(TextStyle::Name(name.into()));
    assert!(same(named(theme::STYLE_CAPTION), 11.0));
    assert!(same(named(theme::STYLE_INPUT), 14.0));
    assert!(same(named(theme::STYLE_SUBHEADING), 16.0));
    assert!(same(named(theme::STYLE_FIGURE), 26.0));
}

#[test]
fn spacing_and_radius_scales_follow_the_mock() {
    // derived: design/mock-v2/index.html `--s1..--s8` and `--r-xs..--r-l` (lines 20-22).
    let spaces = [
        theme::SPACE_1,
        theme::SPACE_2,
        theme::SPACE_3,
        theme::SPACE_4,
        theme::SPACE_5,
        theme::SPACE_6,
        theme::SPACE_8,
    ];
    let want_spaces = [4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0];
    for (got, want) in spaces.into_iter().zip(want_spaces) {
        assert!(same(got, want), "space {got} != {want}");
    }
    let radii = [
        theme::RADIUS_XS,
        theme::RADIUS_S,
        theme::RADIUS_M,
        theme::RADIUS_L,
    ];
    assert_eq!(radii, [4, 6, 10, 14]);
}

#[test]
fn focus_stroke_is_visible_and_thicker_in_high_contrast() {
    // derived: spec 7 asks for visible focus. The mock's focus ring is 2px
    // (`:focus-visible{outline:2px solid var(--accent)}`); high contrast uses 3.
    let focus = |k| theme::visuals(k).selection.stroke.width;
    assert!((focus(ThemeKind::Light) - 2.0).abs() < f32::EPSILON);
    assert!((focus(ThemeKind::Dark) - 2.0).abs() < f32::EPSILON);
    assert!((focus(ThemeKind::HighContrast) - 3.0).abs() < f32::EPSILON);
}

#[test]
fn strong_text_is_readable_on_the_surface_and_the_page() {
    // derived: WCAG 2.1 SC 1.4.3, 4.5:1 for normal text (7:1, SC 1.4.6, in high contrast).
    // egui draws `RichText::strong()` in `Visuals::strong_text_color()`, which is the pressed
    // widget's text colour. KPI figures, report headers and totals use it. The UI comparison
    // with the mock (2026-11-21) found it white on white in light and near-black on navy in dark.
    for kind in [ThemeKind::Light, ThemeKind::Dark, ThemeKind::HighContrast] {
        let t = theme::tokens(kind);
        let strong = theme::visuals(kind).strong_text_color();
        let floor = if kind == ThemeKind::HighContrast {
            7.0
        } else {
            4.5
        };
        for (place, bg) in [("surface", t.surface), ("bg", t.bg)] {
            let ratio = contrast(strong, bg);
            assert!(
                ratio >= floor,
                "{kind:?}: strong text on {place} is {ratio:.2}:1"
            );
        }
    }
}

#[test]
fn pressed_widget_text_is_readable_on_its_own_fill() {
    // derived: WCAG 2.1 SC 1.4.3 (SC 1.4.6 in high contrast), the label of a pressed button
    // against the pressed fill.
    for kind in [ThemeKind::Light, ThemeKind::Dark, ThemeKind::HighContrast] {
        let active = theme::visuals(kind).widgets.active;
        let floor = if kind == ThemeKind::HighContrast {
            7.0
        } else {
            4.5
        };
        let ratio = contrast(active.fg_stroke.color, active.bg_fill);
        assert!(ratio >= floor, "{kind:?}: pressed text is {ratio:.2}:1");
    }
}
