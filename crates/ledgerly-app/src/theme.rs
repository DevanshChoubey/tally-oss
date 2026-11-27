//! Theme tokens, scales and fonts from the Ledgerly UI mock v2 (`design/mock-v2/index.html`).
//!
//! The colour values are the mock's `:root` and `:root[data-theme=...]` custom properties, which
//! did not change between commits `2178293` and `9199157`. The rules that map them to egui
//! visuals, the spacing, radius and type scales, and the font choice are in
//! `design/mock-v2/DESIGN.md` sections 3 and 7.
//!
//! Frozen interface:
//!
//! ```text
//! pub enum ThemeKind { Light, Dark, HighContrast }
//! pub fn apply(ctx: &egui::Context, kind: ThemeKind);
//! pub fn current(ctx: &egui::Context) -> ThemeKind;
//! pub fn tokens(kind: ThemeKind) -> Tokens;
//! pub fn visuals(kind: ThemeKind) -> egui::Visuals;
//! ```
//!
//! Deviations from the mock, each with a reason:
//!
//! * The high-contrast focus ring is 3 px. The mock uses 2 px in every theme. High contrast keeps
//!   the thicker ring that the v1 tests asserted, because spec 7 asks for visible focus.
//! * The mock turns on Inter's tabular figures (`font-feature-settings:"tnum" 1`). egui cannot
//!   select OpenType features, so figures use Inter's default proportional digits; amounts are
//!   still right-aligned in their columns.
//! * egui's `Shadow::spread` is an unsigned byte, so the mock's `-12px` spread of shadow-2 cannot
//!   be drawn. [`shadow_2`] keeps the offset and blur and drops the negative spread.
//! * The v1 tokens `ruling`, `margin` and `focus_band` have no v2 counterpart and are removed.
//!   Errors use `bad`, and the focus band is not drawn.

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Shadow,
    Stroke, Style, TextStyle, Theme, Visuals,
    style::{WidgetVisuals, Widgets},
};

/// Spacing on the 4-point grid, in points (`--s1`).
pub const SPACE_1: f32 = 4.0;
/// Spacing, `--s2`.
pub const SPACE_2: f32 = 8.0;
/// Spacing, `--s3`.
pub const SPACE_3: f32 = 12.0;
/// Spacing, `--s4`.
pub const SPACE_4: f32 = 16.0;
/// Spacing, `--s5`.
pub const SPACE_5: f32 = 20.0;
/// Spacing, `--s6`.
pub const SPACE_6: f32 = 24.0;
/// Spacing, `--s8`.
pub const SPACE_8: f32 = 32.0;

/// Radius of kbd and tags, `--r-xs`.
pub const RADIUS_XS: u8 = 4;
/// Radius of buttons and inputs, `--r-s`.
pub const RADIUS_S: u8 = 6;
/// Radius of cards, `--r-m`.
pub const RADIUS_M: u8 = 10;
/// Radius of the palette and dialogs, `--r-l`.
pub const RADIUS_L: u8 = 14;

/// Badges and tags, `--t-cap`.
pub const TEXT_CAP: f32 = 11.0;
/// Hints, captions and labels, `--t-meta`.
pub const TEXT_META: f32 = 12.0;
/// Tables and running text, `--t-body`.
pub const TEXT_BODY: f32 = 13.0;
/// Inputs and card titles, `--t-input` and `--t-h3`.
pub const TEXT_INPUT: f32 = 14.0;
/// Section titles and totals, `--t-h2`.
pub const TEXT_H2: f32 = 16.0;
/// Page title, `--t-h1`.
pub const TEXT_H1: f32 = 20.0;
/// KPI figures, `--t-fig`.
pub const TEXT_FIG: f32 = 26.0;

/// Hairline border width outside high contrast, `--bw`.
pub const BORDER_WIDTH: f32 = 1.0;
/// Border width in high contrast, `--bw`.
pub const BORDER_WIDTH_HIGH_CONTRAST: f32 = 2.0;

/// Name of the bold face family (Inter semibold, the mock's weight 600, then Noto Sans Bold and
/// Noto Sans Devanagari Bold).
pub const BOLD_FAMILY: &str = "bold";

/// Name of the caption text style (11 px).
pub const STYLE_CAPTION: &str = "Caption";
/// Name of the input text style (14 px).
pub const STYLE_INPUT: &str = "Input";
/// Name of the subheading text style (16 px).
pub const STYLE_SUBHEADING: &str = "Subheading";
/// Name of the figure text style (26 px, bold).
pub const STYLE_FIGURE: &str = "Figure";

/// Font data names, as keys of [`FontDefinitions::font_data`]: Inter, the mock's face.
const INTER_REGULAR: &str = "Inter-Regular";
/// Inter semibold, the mock's bold weight (600).
const INTER_SEMIBOLD: &str = "Inter-SemiBold";
/// Noto Sans, for glyphs Inter lacks.
const NOTO_REGULAR: &str = "NotoSans-Regular";
/// Bold Noto Sans.
const NOTO_BOLD: &str = "NotoSans-Bold";
/// Devanagari fallback, regular.
const NOTO_DEVANAGARI: &str = "NotoSansDevanagari-Regular";
/// Devanagari fallback, bold.
const NOTO_DEVANAGARI_BOLD: &str = "NotoSansDevanagari-Bold";
/// Telugu fallback, regular: for ledger names in Telugu script, such as those imported from a
/// file (S22).
const NOTO_TELUGU: &str = "NotoSansTelugu-Regular";
/// Telugu fallback, bold.
const NOTO_TELUGU_BOLD: &str = "NotoSansTelugu-Bold";

/// Which colour scheme is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeKind {
    /// Warm stone paper with ink text and a plum accent (the mock's default).
    #[default]
    Light,
    /// Warm near-black paper with pale ink and a lilac accent.
    Dark,
    /// Black and white with brighter accents, for low vision.
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

/// The colour tokens of one theme. Field names are the mock's CSS custom properties in
/// `snake_case` (`--text-2` is `text_2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tokens {
    /// Page background (`--bg`).
    pub bg: Color32,
    /// Cards, tables and the top bar (`--surface`).
    pub surface: Color32,
    /// Second surface, for footers and notes (`--surface-2`).
    pub surface_2: Color32,
    /// Wells and skeletons (`--sunken`).
    pub sunken: Color32,
    /// Hover fill (`--hover`).
    pub hover: Color32,
    /// Hairline borders (`--border`).
    pub border: Color32,
    /// Input and button borders (`--border-strong`).
    pub border_strong: Color32,
    /// Body text (`--text`).
    pub text: Color32,
    /// Secondary text (`--text-2`).
    pub text_2: Color32,
    /// Meta text and captions (`--text-3`).
    pub text_3: Color32,
    /// Selection and the primary action (`--accent`).
    pub accent: Color32,
    /// Text on an `accent` fill (`--on-accent`).
    pub on_accent: Color32,
    /// Selected row and pressed fill (`--accent-soft`).
    pub accent_soft: Color32,
    /// Text on `accent_soft` (`--accent-ink`).
    pub accent_ink: Color32,
    /// AI proposals only (`--ai`).
    pub ai: Color32,
    /// AI proposal header fill (`--ai-soft`).
    pub ai_soft: Color32,
    /// Text on `ai_soft` (`--ai-ink`).
    pub ai_ink: Color32,
    /// Matched and passed (`--ok`).
    pub ok: Color32,
    /// Background for `ok` (`--ok-soft`).
    pub ok_soft: Color32,
    /// Needs attention (`--warn`).
    pub warn: Color32,
    /// Background for `warn` (`--warn-soft`).
    pub warn_soft: Color32,
    /// Errors and differences (`--bad`).
    pub bad: Color32,
    /// Background for `bad` (`--bad-soft`).
    pub bad_soft: Color32,
}

/// Light tokens: `:root[data-theme="light"]` in the mock.
const LIGHT: Tokens = Tokens {
    bg: rgb(0x00F6_F5F2),
    surface: rgb(0x00FF_FFFF),
    surface_2: rgb(0x00FB_FAF8),
    sunken: rgb(0x00EF_EDE8),
    hover: rgb(0x00F1_EFEA),
    border: rgb(0x00E2_DFD8),
    border_strong: rgb(0x008F_897E),
    text: rgb(0x001C_1B19),
    text_2: rgb(0x004A_4741),
    text_3: rgb(0x0061_5D55),
    accent: rgb(0x006B_2D73),
    on_accent: rgb(0x00FF_FFFF),
    accent_soft: rgb(0x00F3_E8F4),
    accent_ink: rgb(0x005A_2161),
    ai: rgb(0x009A_4211),
    ai_soft: rgb(0x00FB_EDE3),
    ai_ink: rgb(0x0086_390D),
    ok: rgb(0x001E_6E3F),
    ok_soft: rgb(0x00E3_F2E8),
    warn: rgb(0x007E_4C00),
    warn_soft: rgb(0x00FB_F0DC),
    bad: rgb(0x00B3_261E),
    bad_soft: rgb(0x00FC_E9E7),
};

/// Dark tokens: `:root[data-theme="dark"]` in the mock.
const DARK: Tokens = Tokens {
    bg: rgb(0x0014_1312),
    surface: rgb(0x001C_1B19),
    surface_2: rgb(0x0023_221F),
    sunken: rgb(0x000F_0E0D),
    hover: rgb(0x002A_2825),
    border: rgb(0x0034_322E),
    border_strong: rgb(0x0077_726A),
    text: rgb(0x00EE_EBE5),
    text_2: rgb(0x00C9_C4BA),
    text_3: rgb(0x00A8_A296),
    accent: rgb(0x00D9_A6E0),
    on_accent: rgb(0x002A_0F2E),
    accent_soft: rgb(0x003A_2340),
    accent_ink: rgb(0x00EB_CBEF),
    ai: rgb(0x00F2_A36E),
    ai_soft: rgb(0x003D_2414),
    ai_ink: rgb(0x00F7_BE97),
    ok: rgb(0x007F_CF9A),
    ok_soft: rgb(0x0017_3323),
    warn: rgb(0x00E9_B860),
    warn_soft: rgb(0x003A_2A10),
    bad: rgb(0x00F2_8B82),
    bad_soft: rgb(0x003E_1C1A),
};

/// High-contrast tokens: `:root[data-theme="hc"]` in the mock.
const HIGH_CONTRAST: Tokens = Tokens {
    bg: rgb(0x0000_0000),
    surface: rgb(0x0000_0000),
    surface_2: rgb(0x0000_0000),
    sunken: rgb(0x0000_0000),
    hover: rgb(0x001A_1A1A),
    border: rgb(0x00FF_FFFF),
    border_strong: rgb(0x00FF_FFFF),
    text: rgb(0x00FF_FFFF),
    text_2: rgb(0x00FF_FFFF),
    text_3: rgb(0x00E6_E6E6),
    accent: rgb(0x00F5_B8FF),
    on_accent: rgb(0x0000_0000),
    accent_soft: rgb(0x002B_0A30),
    accent_ink: rgb(0x00F5_B8FF),
    ai: rgb(0x00FF_B37A),
    ai_soft: rgb(0x002E_1606),
    ai_ink: rgb(0x00FF_C79E),
    ok: rgb(0x008C_F5A9),
    ok_soft: rgb(0x0006_2612),
    warn: rgb(0x00FF_D27A),
    warn_soft: rgb(0x002E_2000),
    bad: rgb(0x00FF_9C94),
    bad_soft: rgb(0x0033_0B08),
};

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

/// The tokens of `kind`.
#[must_use]
pub fn tokens(kind: ThemeKind) -> Tokens {
    match kind {
        ThemeKind::Light => LIGHT,
        ThemeKind::Dark => DARK,
        ThemeKind::HighContrast => HIGH_CONTRAST,
    }
}

/// Width of hairline borders in `kind` (`--bw`): 2 px in high contrast, else 1 px.
#[must_use]
pub fn border_width(kind: ThemeKind) -> f32 {
    match kind {
        ThemeKind::Light | ThemeKind::Dark => BORDER_WIDTH,
        ThemeKind::HighContrast => BORDER_WIDTH_HIGH_CONTRAST,
    }
}

/// Width of the focus ring and of the selection stroke in `kind`: 2 px, or 3 px in high contrast.
#[must_use]
pub fn focus_width(kind: ThemeKind) -> f32 {
    match kind {
        ThemeKind::Light | ThemeKind::Dark => 2.0,
        ThemeKind::HighContrast => 3.0,
    }
}

/// The scrim behind overlays (`--scrim`).
#[must_use]
pub fn scrim(kind: ThemeKind) -> Color32 {
    match kind {
        ThemeKind::Light => Color32::from_rgba_unmultiplied(28, 27, 25, 97),
        ThemeKind::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 140),
        ThemeKind::HighContrast => Color32::from_rgba_unmultiplied(0, 0, 0, 204),
    }
}

/// Shadow-1 (`--shadow-1`), used by cards: a 1 px lift with a 2 px blur. None in high contrast.
#[must_use]
pub fn shadow_1(kind: ThemeKind) -> Shadow {
    match kind {
        ThemeKind::Light => Shadow {
            offset: [0, 1],
            blur: 2,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(28, 27, 25, 15),
        },
        ThemeKind::Dark => Shadow {
            offset: [0, 1],
            blur: 2,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 102),
        },
        ThemeKind::HighContrast => Shadow::NONE,
    }
}

/// Shadow-2 (`--shadow-2`), used by overlays: palette, dialogs, typeahead and toasts.
///
/// The mock's spread of -12 px cannot be drawn (`Shadow::spread` is unsigned), so this keeps
/// the offset and blur and uses a spread of 0. None in high contrast.
#[must_use]
pub fn shadow_2(kind: ThemeKind) -> Shadow {
    match kind {
        ThemeKind::Light => Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(28, 27, 25, 77),
        },
        ThemeKind::Dark => Shadow {
            offset: [0, 16],
            blur: 40,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 179),
        },
        ThemeKind::HighContrast => Shadow::NONE,
    }
}

/// Builds the egui visuals for `kind`, mapped from the tokens as DESIGN.md section 7 asks.
#[must_use]
pub fn visuals(kind: ThemeKind) -> Visuals {
    let t = tokens(kind);
    let mut v = if kind.is_dark() {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = t.bg;
    v.window_fill = t.surface;
    v.window_stroke = Stroke::new(border_width(kind), t.border);
    v.window_corner_radius = CornerRadius::same(RADIUS_L);
    v.window_shadow = shadow_2(kind);
    v.menu_corner_radius = CornerRadius::same(RADIUS_M);
    v.popup_shadow = shadow_2(kind);
    v.extreme_bg_color = t.surface;
    v.text_edit_bg_color = Some(t.surface);
    v.faint_bg_color = t.surface_2;
    v.code_bg_color = t.sunken;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.bad;
    v.weak_text_color = Some(t.text_3);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(focus_width(kind), t.accent);
    set_widget_states(&mut v.widgets, &t, kind);
    if kind == ThemeKind::HighContrast {
        // egui fades a disabled widget to this share of its colours (default 0.5, which
        // measures 5.3:1 for text here); high contrast keeps text at 7:1 or better.
        v.disabled_alpha = 0.8;
    }
    v
}

/// Fills the widget states (buttons, inputs, checkboxes and so on) from the tokens.
fn set_widget_states(w: &mut Widgets, t: &Tokens, kind: ThemeKind) {
    let bw = border_width(kind);
    let focus = focus_width(kind);
    let radius = CornerRadius::same(RADIUS_S);
    set_widget(
        &mut w.noninteractive,
        t.surface,
        Stroke::new(bw, t.border),
        Stroke::new(1.0, t.text),
        radius,
    );
    set_widget(
        &mut w.inactive,
        t.surface,
        Stroke::new(bw, t.border_strong),
        Stroke::new(1.0, t.text),
        radius,
    );
    set_widget(
        &mut w.hovered,
        t.hover,
        Stroke::new(bw, t.border_strong),
        Stroke::new(1.5, t.text),
        radius,
    );
    // egui draws `RichText::strong()` in the pressed widget's text colour, so that colour must
    // read on the surface and the page as well as on the pressed fill: text on the soft accent,
    // with the accent kept on the border.
    set_widget(
        &mut w.active,
        t.accent_soft,
        Stroke::new(focus, t.accent),
        Stroke::new(2.0, t.text),
        radius,
    );
    set_widget(
        &mut w.open,
        t.accent_soft,
        Stroke::new(bw, t.accent),
        Stroke::new(1.0, t.text),
        radius,
    );
}

/// Sets one widget state: fill (strong and weak), edge, text stroke and corner radius.
fn set_widget(
    w: &mut WidgetVisuals,
    fill: Color32,
    edge: Stroke,
    text: Stroke,
    radius: CornerRadius,
) {
    w.bg_fill = fill;
    w.weak_bg_fill = fill;
    w.bg_stroke = edge;
    w.fg_stroke = text;
    w.corner_radius = radius;
}

/// The theme `ctx` was given by [`apply`]. Returns [`ThemeKind::Light`] before `apply` has run.
#[must_use]
pub fn current(ctx: &Context) -> ThemeKind {
    ctx.data(|d| d.get_temp::<ThemeKind>(kind_id()))
        .unwrap_or_default()
}

/// Makes `kind` the active theme of `ctx`. Takes effect for every frame drawn afterwards.
///
/// Cheap to call every frame: the fonts and text styles are installed once per context.
pub fn apply(ctx: &Context, kind: ThemeKind) {
    let base = if kind.is_dark() {
        Theme::Dark
    } else {
        Theme::Light
    };
    ctx.set_theme(base);
    ctx.set_visuals_of(base, visuals(kind));
    ctx.data_mut(|d| d.insert_temp(kind_id(), kind));
    install_fonts_once(ctx);
}

/// The fonts, and the text styles that use them, are set once per context. Setting fonts again
/// would rebuild the glyph atlas.
fn install_fonts_once(ctx: &Context) {
    if ctx.data(|d| d.get_temp::<bool>(fonts_id())) == Some(true) {
        return;
    }
    ctx.set_fonts(font_definitions());
    ctx.all_styles_mut(set_text_styles);
    ctx.data_mut(|d| d.insert_temp(fonts_id(), true));
}

/// Font definitions: Inter first (the mock's face), Noto Sans for glyphs Inter lacks, Noto Sans
/// Devanagari for Hindi names, then egui's own fallbacks. The bold face is the [`BOLD_FAMILY`]
/// family: Inter semibold, then the Noto bold faces. Monospace is egui's own.
#[must_use]
pub fn font_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    insert_face(
        &mut defs,
        INTER_REGULAR,
        include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    );
    insert_face(
        &mut defs,
        INTER_SEMIBOLD,
        include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_REGULAR,
        include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_BOLD,
        include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_DEVANAGARI,
        include_bytes!("../assets/fonts/NotoSansDevanagari-Regular.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_DEVANAGARI_BOLD,
        include_bytes!("../assets/fonts/NotoSansDevanagari-Bold.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_TELUGU,
        include_bytes!("../assets/fonts/NotoSansTelugu-Regular.ttf"),
    );
    insert_face(
        &mut defs,
        NOTO_TELUGU_BOLD,
        include_bytes!("../assets/fonts/NotoSansTelugu-Bold.ttf"),
    );
    let fallback = defs
        .families
        .remove(&FontFamily::Proportional)
        .unwrap_or_default();
    let mut chain = vec![
        INTER_REGULAR.to_owned(),
        NOTO_REGULAR.to_owned(),
        NOTO_DEVANAGARI.to_owned(),
        NOTO_TELUGU.to_owned(),
    ];
    chain.extend(fallback);
    defs.families.insert(FontFamily::Proportional, chain);
    defs.families.insert(
        bold_family(),
        vec![
            INTER_SEMIBOLD.to_owned(),
            NOTO_BOLD.to_owned(),
            NOTO_DEVANAGARI_BOLD.to_owned(),
            NOTO_TELUGU_BOLD.to_owned(),
        ],
    );
    defs
}

/// Adds one font face under `name`.
fn insert_face(defs: &mut FontDefinitions, name: &str, bytes: &'static [u8]) {
    defs.font_data
        .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
}

/// The bold face family, as an egui [`FontFamily`].
#[must_use]
pub fn bold_family() -> FontFamily {
    FontFamily::Name(Arc::from(BOLD_FAMILY))
}

/// Sets the egui text styles from the type scale: Small at 12 px, Body and Button at 13 px,
/// Heading at 20 px, Monospace at 13 px, and named styles for the caption, input, subheading
/// and figure sizes.
fn set_text_styles(style: &mut Style) {
    let s = &mut style.text_styles;
    s.insert(TextStyle::Small, FontId::proportional(TEXT_META));
    s.insert(TextStyle::Body, FontId::proportional(TEXT_BODY));
    s.insert(TextStyle::Button, FontId::proportional(TEXT_BODY));
    s.insert(TextStyle::Heading, FontId::proportional(TEXT_H1));
    s.insert(TextStyle::Monospace, FontId::monospace(TEXT_BODY));
    let named = [
        (STYLE_CAPTION, TEXT_CAP, FontFamily::Proportional),
        (STYLE_INPUT, TEXT_INPUT, FontFamily::Proportional),
        (STYLE_SUBHEADING, TEXT_H2, FontFamily::Proportional),
        (STYLE_FIGURE, TEXT_FIG, bold_family()),
    ];
    for (name, size, family) in named {
        s.insert(TextStyle::Name(Arc::from(name)), FontId::new(size, family));
    }
}

/// The egui id under which [`apply`] stores the active [`ThemeKind`].
fn kind_id() -> Id {
    Id::unique("ledgerly.theme.kind")
}

/// The egui id under which [`apply`] records that the fonts are installed.
fn fonts_id() -> Id {
    Id::unique("ledgerly.theme.fonts_installed")
}
