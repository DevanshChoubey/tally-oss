//! Theme tokens (light, dark, high contrast) from the mock. **Card U1** implements this module.
//!
//! Frozen interface (plan card U1):
//!
//! ```text
//! pub enum ThemeKind { Light, Dark, HighContrast }
//! pub fn apply(ctx: &egui::Context, kind: ThemeKind);
//! ```
//!
//! Light and dark values are the mock's tokens (`design/mock/index.html`, lines 13-48). Two
//! deviations, each with a reason:
//!
//! * Light `ink_3` is `#5D6A82`, not the mock's `#77839A`. The mock value measures 3.82:1 on the
//!   sheet and 3.47:1 on the paper, below the 4.5:1 that WCAG 2.1 success criterion 1.4.3 asks of
//!   text, and `ink_3` carries hints and muted text.
//! * Light `haldi` is `#8F5C0C` (mock `#9A6410`) and light `leaf` is `#286F45` (mock `#2C7A4B`).
//!   The mock values measure 4.41:1 on `haldi_soft` and 4.42:1 on `leaf_soft`, the backgrounds
//!   they label, again below 4.5:1.
//! * High contrast is not in the mock. It is white on black with a yellow accent, and every text
//!   colour meets the 7:1 of WCAG 2.1 criterion 1.4.6.
//!
//! What spec 7 asks of the themes besides colour: visible focus (the selection stroke is
//! 2 points wide, 3 in high contrast) and no colour-only signals (kept by the screens, which pair
//! each colour with a word or a shape).

use egui::{Color32, CornerRadius, Stroke, Theme, Visuals};

/// Which colour scheme is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeKind {
    /// Cool ledger paper with blue-black ink (the mock's default).
    #[default]
    Light,
    /// Dark navy paper with pale ink.
    Dark,
    /// White on black, for low vision.
    HighContrast,
}

impl ThemeKind {
    /// Every theme, in the order the switcher cycles through them.
    pub const ALL: [ThemeKind; 3] = [ThemeKind::Light, ThemeKind::Dark, ThemeKind::HighContrast];

    /// Name shown in the switcher and read by screen readers.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Light => "Light",
            ThemeKind::Dark => "Dark",
            ThemeKind::HighContrast => "High contrast",
        }
    }

    /// The theme after this one in [`ThemeKind::ALL`], wrapping around.
    #[must_use]
    pub fn next(self) -> ThemeKind {
        match self {
            ThemeKind::Light => ThemeKind::Dark,
            ThemeKind::Dark => ThemeKind::HighContrast,
            ThemeKind::HighContrast => ThemeKind::Light,
        }
    }

    /// Whether egui should use its dark base style for this theme.
    #[must_use]
    pub fn is_dark(self) -> bool {
        match self {
            ThemeKind::Light => false,
            ThemeKind::Dark | ThemeKind::HighContrast => true,
        }
    }
}

/// The colour tokens of one theme. Names follow the mock's CSS custom properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tokens {
    /// App chrome (`--paper`).
    pub paper: Color32,
    /// Working surfaces (`--sheet`).
    pub sheet: Color32,
    /// Body text (`--ink`).
    pub ink: Color32,
    /// Secondary text (`--ink-2`).
    pub ink_2: Color32,
    /// Hints and muted text (`--ink-3`).
    pub ink_3: Color32,
    /// Structural borders (`--line`).
    pub line: Color32,
    /// Ledger ruling (`--ruling`).
    pub ruling: Color32,
    /// Red margin rule, errors (`--margin`).
    pub margin: Color32,
    /// Focus, selection, primary (`--accent`).
    pub accent: Color32,
    /// Text on an accent fill (`--accent-ink`).
    pub accent_ink: Color32,
    /// Selected-row background (`--accent-soft`).
    pub accent_soft: Color32,
    /// Pending / attention (`--haldi`).
    pub haldi: Color32,
    /// Background for pending / attention (`--haldi-soft`).
    pub haldi_soft: Color32,
    /// Matched / ok (`--leaf`).
    pub leaf: Color32,
    /// Background for matched / ok (`--leaf-soft`).
    pub leaf_soft: Color32,
    /// Active field band on the voucher sheet (`--focus-band`).
    pub focus_band: Color32,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "each channel is masked to 8 bits first, so the cast cannot lose bits"
)]
const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

const LIGHT: Tokens = Tokens {
    paper: rgb(0x00F2_F4F7),
    sheet: rgb(0x00FF_FFFF),
    ink: rgb(0x0017_2033),
    ink_2: rgb(0x0047_536A),
    ink_3: rgb(0x005D_6A82),
    line: rgb(0x00DC_E1E9),
    ruling: rgb(0x00CB_D6EC),
    margin: rgb(0x00C2_3B35),
    accent: rgb(0x0035_41A8),
    accent_ink: rgb(0x00FF_FFFF),
    accent_soft: rgb(0x00E5_E8FA),
    haldi: rgb(0x008F_5C0C),
    haldi_soft: rgb(0x00FB_F0D8),
    leaf: rgb(0x0028_6F45),
    leaf_soft: rgb(0x00DD_F0E4),
    focus_band: rgb(0x00FF_F7D6),
};

const DARK: Tokens = Tokens {
    paper: rgb(0x000E_1320),
    sheet: rgb(0x0015_1B2A),
    ink: rgb(0x00E5_E9F2),
    ink_2: rgb(0x00A9_B3C6),
    ink_3: rgb(0x007D_889D),
    line: rgb(0x0026_3049),
    ruling: rgb(0x0025_345A),
    margin: rgb(0x00E4_6A5F),
    accent: rgb(0x0097_A0F7),
    accent_ink: rgb(0x000E_1320),
    accent_soft: rgb(0x0023_2A5C),
    haldi: rgb(0x00E6_AE4E),
    haldi_soft: rgb(0x003A_2C12),
    leaf: rgb(0x0062_C08A),
    leaf_soft: rgb(0x0017_3726),
    focus_band: rgb(0x002C_2A17),
};

const HIGH_CONTRAST: Tokens = Tokens {
    paper: rgb(0x0000_0000),
    sheet: rgb(0x0000_0000),
    ink: rgb(0x00FF_FFFF),
    ink_2: rgb(0x00F2_F2F2),
    ink_3: rgb(0x00D9_D9D9),
    line: rgb(0x00FF_FFFF),
    ruling: rgb(0x0080_80FF),
    margin: rgb(0x00FF_8A80),
    accent: rgb(0x00FF_D400),
    accent_ink: rgb(0x0000_0000),
    accent_soft: rgb(0x0000_1F8F),
    haldi: rgb(0x00FF_B000),
    haldi_soft: rgb(0x0033_2200),
    leaf: rgb(0x005C_FF9D),
    leaf_soft: rgb(0x0000_3319),
    focus_band: rgb(0x0033_3300),
};

/// The tokens of `kind`.
#[must_use]
pub fn tokens(kind: ThemeKind) -> Tokens {
    match kind {
        ThemeKind::Light => LIGHT,
        ThemeKind::Dark => DARK,
        ThemeKind::HighContrast => HIGH_CONTRAST,
    }
}

/// Builds the egui visuals for `kind`.
#[must_use]
pub fn visuals(kind: ThemeKind) -> Visuals {
    let t = tokens(kind);
    let mut v = if kind.is_dark() {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(3);
    let focus_width = match kind {
        ThemeKind::HighContrast => 3.0,
        ThemeKind::Light | ThemeKind::Dark => 2.0,
    };
    v.panel_fill = t.paper;
    v.window_fill = t.sheet;
    v.window_stroke = Stroke::new(1.0, t.line);
    v.extreme_bg_color = t.sheet;
    v.text_edit_bg_color = Some(t.sheet);
    v.faint_bg_color = t.paper;
    v.code_bg_color = t.paper;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = t.haldi;
    v.error_fg_color = t.margin;
    v.weak_text_color = Some(t.ink_3);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(focus_width, t.accent);
    v.widgets.noninteractive.bg_fill = t.sheet;
    v.widgets.noninteractive.weak_bg_fill = t.sheet;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.ink);
    v.widgets.noninteractive.corner_radius = radius;
    v.widgets.inactive.bg_fill = t.sheet;
    v.widgets.inactive.weak_bg_fill = t.sheet;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.ink_2);
    v.widgets.inactive.corner_radius = radius;
    v.widgets.hovered.bg_fill = t.accent_soft;
    v.widgets.hovered.weak_bg_fill = t.accent_soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.accent);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, t.ink);
    v.widgets.hovered.corner_radius = radius;
    v.widgets.active.bg_fill = t.accent;
    v.widgets.active.weak_bg_fill = t.accent;
    v.widgets.active.bg_stroke = Stroke::new(focus_width, t.accent);
    v.widgets.active.fg_stroke = Stroke::new(2.0, t.accent_ink);
    v.widgets.active.corner_radius = radius;
    v.widgets.open.bg_fill = t.accent_soft;
    v.widgets.open.weak_bg_fill = t.accent_soft;
    v.widgets.open.bg_stroke = Stroke::new(1.0, t.accent);
    v.widgets.open.fg_stroke = Stroke::new(1.0, t.ink);
    v.widgets.open.corner_radius = radius;
    v
}

/// Makes `kind` the active theme of `ctx`. Takes effect for every frame drawn afterwards.
pub fn apply(ctx: &egui::Context, kind: ThemeKind) {
    let base = if kind.is_dark() {
        Theme::Dark
    } else {
        Theme::Light
    };
    ctx.set_theme(base);
    ctx.set_visuals_of(base, visuals(kind));
}
