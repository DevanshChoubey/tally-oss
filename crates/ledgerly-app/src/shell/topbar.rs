//! The top bar (48 px, `.top` in `design/mock-v2/index.html`): the brand, the company chip (F3),
//! the date chip (F2), the period-lock chip, the search chip (Ctrl+K), the AI chip, the theme
//! switch and the keyboard map.
//!
//! The rule: a chip shows only what is known. The words come from the pure functions below, so
//! tests can read them without drawing:
//!
//! * [`company_chip`] says "No company open" when no books are loaded, and never a name it does not
//!   have.
//! * [`lock_chip`] returns `None` (the chip is hidden) while no lock date is known.
//! * [`ai_chip`] says "AI: not set up" while no AI tier is stored. It names a tier only when one is
//!   stored, so it never claims a privacy tier that nothing enforces (parity report, section 6).
//! * [`date_chip`] shows the date it is given; the shell passes today's date.
//!
//! F2 (change date) and F3 (switch company) have no screen yet. Their chips say so when clicked
//! (through the key bar's notice) rather than doing nothing.

use egui::{Align, Layout, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use ledgerly_core::{Date, FinancialYear};
use time::Weekday;

use super::chrome::ChromeCmd;
use super::navigate::{AI_SETTINGS_KEY, KEYS_KEY};
use crate::AppCtx;
use crate::palette::PaletteTarget;
use crate::screen::AiTier;
use crate::theme::{ThemeKind, Tokens};
use crate::ui::chip::{self, Chip, SEARCH_MAX_WIDTH};
use crate::ui::icon::Icon;
use crate::ui::{bold_font, format};

/// The search chip's words (`design/mock-v2/index.html`, `renderShell`).
const SEARCH_TEXT: &str = "Search, jump, ask or type an entry";
/// Side of the brand mark, in points (`.brand svg{width:24px}`).
const BRAND_MARK: f32 = 24.0;
/// The brand's size, in points (`.brand{font-size:15px}`).
const BRAND_SIZE: f32 = 15.0;
/// Gap between top bar items, in points (`.top{gap:8px}`).
const GAP: f32 = 8.0;

/// The company chip's words: the open company's name and financial year (`Name FY 2026-27`), or
/// "No company open" while no books are loaded.
#[must_use]
pub fn company_chip(ctx: &AppCtx) -> String {
    match company_parts(ctx) {
        Some((name, fy)) => format!("{name} {fy}"),
        None => "No company open".to_owned(),
    }
}

/// The company chip's two parts: the name (bold) and `FY 2026-27`, or `None` with no books.
fn company_parts(ctx: &AppCtx) -> Option<(String, String)> {
    ctx.books.as_ref().map(|books| {
        (
            books.company.name.clone(),
            format!(
                "FY {}",
                FinancialYear {
                    start: books.company.fy_start
                }
                .label()
            ),
        )
    })
}

/// The date chip's words, such as `Sat, 21 Nov 2026` (the mock's top bar).
#[must_use]
pub fn date_chip(today: Date) -> String {
    format!(
        "{}, {}",
        weekday_abbrev(today.weekday()),
        format::date(today)
    )
}

/// The three-letter English abbreviation of a weekday.
fn weekday_abbrev(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

/// The lock chip's words, `Locked to 31 Mar 2026`, or `None` while no lock date is known (the
/// chip is then not drawn).
#[must_use]
pub fn lock_chip(lock: Option<Date>) -> Option<String> {
    lock.map(|date| format!("Locked to {}", format::date(date)))
}

/// The AI chip's words: the stored tier, or "AI: not set up" while none is stored.
#[must_use]
pub fn ai_chip(tier: Option<AiTier>) -> String {
    match tier {
        None => "AI: not set up".to_owned(),
        Some(AiTier::Off) => "AI off".to_owned(),
        Some(AiTier::Local) => "AI on this computer".to_owned(),
        Some(AiTier::Cloud) => "AI cloud, your key".to_owned(),
    }
}

/// The AI chip's icon and tint (`.aichip[data-tier]`): local in `ok`, cloud in `ai`, off and not
/// set up in `text-3`.
fn ai_look(tier: Option<AiTier>, t: &Tokens) -> (Icon, egui::Color32) {
    match tier {
        Some(AiTier::Local) => (Icon::Cpu, t.ok),
        Some(AiTier::Cloud) => (Icon::Cloud, t.ai),
        Some(AiTier::Off) | None => (Icon::Off, t.text_3),
    }
}

/// Paints the brand mark (`.brand svg` in the mock): an accent square with rounded corners, an L
/// stroke and two shorter lines in the on-accent colour.
#[allow(clippy::float_arithmetic, reason = "scaling the 24-unit mark")]
fn brand_mark(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(BRAND_MARK), Sense::hover());
    let painter = ui.painter();
    let unit = rect.width() / 24.0;
    let at = |x: f32, y: f32| -> Pos2 { rect.min + vec2(x * unit, y * unit) };
    painter.rect_filled(rect, 6.0 * unit, t.accent);
    let main = Stroke::new(2.4 * unit, t.on_accent);
    painter.add(egui::Shape::line(
        vec![at(8.0, 6.0), at(8.0, 17.0), at(17.0, 17.0)],
        main,
    ));
    for end in [at(8.0, 6.0), at(17.0, 17.0)] {
        painter.circle_filled(end, main.width / 2.0, t.on_accent);
    }
    let soft = t.on_accent.gamma_multiply(0.75);
    for y in [6.5, 11.0] {
        painter.line_segment([at(12.0, y), at(17.0, y)], Stroke::new(2.0 * unit, soft));
        for x in [12.0, 17.0] {
            painter.circle_filled(at(x, y), unit, soft);
        }
    }
}

/// Draws the top bar and returns what the user asked for in it.
#[allow(clippy::float_arithmetic, reason = "centring the search chip")]
pub(super) fn top_bar(
    ui: &mut Ui,
    ctx: &AppCtx,
    today: Date,
    theme: ThemeKind,
    t: &Tokens,
) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.spacing_mut().item_spacing.x = GAP;
    let full = ui.max_rect();
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        brand_mark(ui, t);
        ui.label(
            egui::RichText::new("Ledgerly")
                .font(bold_font(ui, BRAND_SIZE))
                .color(t.text),
        );
        ui.add_space(6.0);
        cmds.extend(left_chips(ui, ctx, today));
        let left_end = ui.cursor().min.x;
        // The right side first, from the right edge, so the search chip can take what is left.
        let right = right_items(ui, ctx, theme, t, &mut cmds, full);
        let room = (right - left_end - 2.0 * GAP).max(0.0);
        let width = room.min(SEARCH_MAX_WIDTH);
        // `margin:0 auto`: centred in the free space between the two sides.
        let x = left_end + GAP + (room - width) / 2.0;
        let search = Rect::from_min_size(
            pos2(x, full.center().y - chip::SEARCH_HEIGHT / 2.0),
            vec2(width, chip::SEARCH_HEIGHT),
        );
        let response = ui
            .scope_builder(egui::UiBuilder::new().max_rect(search), |ui| {
                chip::search_chip(ui, width, SEARCH_TEXT, "Ctrl+K")
            })
            .inner;
        if response.on_hover_text("Open the command palette").clicked() {
            cmds.push(ChromeCmd::OpenPalette);
        }
    });
    cmds
}

/// The company, date and lock chips.
fn left_chips(ui: &mut Ui, ctx: &AppCtx, today: Date) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    let parts = company_parts(ctx);
    let company = match &parts {
        Some((name, fy)) => Chip {
            icon: Some(Icon::Building),
            bold: Some(name.as_str()),
            text: fy.as_str(),
            key: Some("F3"),
            tint: None,
        },
        None => Chip {
            icon: Some(Icon::Building),
            text: "No company open",
            key: Some("F3"),
            ..Chip::default()
        },
    };
    if chip::chip_parts(ui, company)
        .on_hover_text("Switch company (F3)")
        .clicked()
    {
        cmds.push(ChromeCmd::Say(
            "Switch company (F3) has no screen yet, so it cannot open from here.".to_owned(),
        ));
    }
    let date = date_chip(today);
    let date_parts = Chip {
        icon: Some(Icon::Cal),
        bold: Some(date.as_str()),
        key: Some("F2"),
        ..Chip::default()
    };
    if chip::chip_parts(ui, date_parts)
        .on_hover_text("Change date or period (F2)")
        .clicked()
    {
        cmds.push(ChromeCmd::Say(
            "Change date or period (F2) has no screen yet, so it cannot open from here.".to_owned(),
        ));
    }
    if let Some(text) = lock_chip(ctx.lock_date) {
        chip::chip_parts(
            ui,
            Chip {
                icon: Some(Icon::Lock),
                text: text.as_str(),
                ..Chip::default()
            },
        )
        .on_hover_text("Period lock: no create, alter or delete on or before this date");
    }
    cmds
}

/// The AI chip, the theme switch and the keyboard map, drawn from the right edge of `full`.
/// Returns the left edge of what was drawn.
fn right_items(
    ui: &mut Ui,
    ctx: &AppCtx,
    theme: ThemeKind,
    t: &Tokens,
    cmds: &mut Vec<ChromeCmd>,
    full: Rect,
) -> f32 {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(full)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if chip::icon_button(ui, Icon::Kbd, "Keyboard map (?)").clicked() {
                cmds.push(ChromeCmd::OpenOnTop(PaletteTarget::Report(
                    KEYS_KEY.to_owned(),
                )));
            }
            let theme_icon = match theme {
                ThemeKind::Light => Icon::Sun,
                ThemeKind::Dark | ThemeKind::HighContrast => Icon::Contrast,
            };
            if chip::icon_button(
                ui,
                theme_icon,
                &format!("Theme: {} (switch)", theme.label()),
            )
            .clicked()
            {
                cmds.push(ChromeCmd::CycleTheme);
            }
            let (icon, tint) = ai_look(ctx.ai_tier, t);
            let words = ai_chip(ctx.ai_tier);
            if chip::chip_parts(
                ui,
                Chip {
                    icon: Some(icon),
                    text: words.as_str(),
                    tint: Some(tint),
                    ..Chip::default()
                },
            )
            .on_hover_text("AI and privacy")
            .clicked()
            {
                cmds.push(ChromeCmd::OpenOnTop(PaletteTarget::Report(
                    AI_SETTINGS_KEY.to_owned(),
                )));
            }
            ui.min_rect().min.x
        },
    )
    .inner
}
