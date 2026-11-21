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

use egui::{Layout, RichText, Ui};
use ledgerly_core::{Date, FinancialYear};

use super::chrome::ChromeCmd;
use super::navigate::{AI_SETTINGS_KEY, KEYS_KEY};
use crate::palette::PaletteTarget;
use crate::screen::AiTier;
use crate::theme::{TEXT_H2, ThemeKind, Tokens};
use crate::ui::{chip, format};
use crate::AppCtx;

/// The company chip's words: the open company's name and financial year (`Name FY 2026-27`), or
/// "No company open" while no books are loaded.
#[must_use]
pub fn company_chip(ctx: &AppCtx) -> String {
    match ctx.books.as_ref() {
        Some(books) => format!(
            "{} FY {}",
            books.company.name,
            FinancialYear {
                start: books.company.fy_start
            }
            .label()
        ),
        None => "No company open".to_owned(),
    }
}

/// The date chip's words, such as `21 Nov 2026`.
#[must_use]
pub fn date_chip(today: Date) -> String {
    format::date(today)
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

/// Draws the top bar and returns what the user asked for in it.
pub(super) fn top_bar(
    ui: &mut Ui,
    ctx: &AppCtx,
    today: Date,
    theme: ThemeKind,
    t: &Tokens,
) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(RichText::new("Ledgerly").strong().size(TEXT_H2).color(t.text));
        ui.add_space(8.0);

        if chip::chip(ui, &company_chip(ctx))
            .on_hover_text("Switch company (F3)")
            .clicked()
        {
            cmds.push(ChromeCmd::Say(
                "Switch company (F3) has no screen yet, so it cannot open from here.".to_owned(),
            ));
        }
        if chip::chip(ui, &date_chip(today))
            .on_hover_text("Change date or period (F2)")
            .clicked()
        {
            cmds.push(ChromeCmd::Say(
                "Change date or period (F2) has no screen yet, so it cannot open from here."
                    .to_owned(),
            ));
        }
        if let Some(text) = lock_chip(ctx.lock_date) {
            chip::chip(ui, &text).on_hover_text(
                "Period lock: no create, alter or delete on or before this date",
            );
        }

        if chip::chip(ui, "Search or jump \u{00B7} Ctrl+K")
            .on_hover_text("Open the command palette")
            .clicked()
        {
            cmds.push(ChromeCmd::OpenPalette);
        }

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if chip::chip(ui, "Keyboard map ?")
                .on_hover_text("Keyboard map (?)")
                .clicked()
            {
                cmds.push(ChromeCmd::OpenOnTop(PaletteTarget::Report(
                    KEYS_KEY.to_owned(),
                )));
            }
            if chip::chip(ui, &format!("Theme: {}", theme.label()))
                .on_hover_text("Switch theme")
                .clicked()
            {
                cmds.push(ChromeCmd::CycleTheme);
            }
            if chip::chip(ui, &ai_chip(ctx.ai_tier))
                .on_hover_text("AI and privacy")
                .clicked()
            {
                cmds.push(ChromeCmd::OpenOnTop(PaletteTarget::Report(
                    AI_SETTINGS_KEY.to_owned(),
                )));
            }
        });
    });
    cmds
}
