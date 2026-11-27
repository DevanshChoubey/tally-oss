//! Ask your books (mock `SCREENS.ask`, index.html:1164-1200; state `ask.offline-ai`).
//!
//! Spec 6.8: a question becomes a `ReportQuery`, the engine runs it, and the answer shows the
//! query. Nothing in this build turns words into a query: no local model client exists and the
//! cloud tier is refused (see [`crate::screens::ai_gate`]). So the question box is drawn
//! disabled, the note says why for the company's stored tier, and the screen offers what the
//! mock's AI-off state offers (index.html:1169-1170): the report list. Each button opens the real
//! report or screen, whose figures the engine computes. The screen itself shows no figure.
//!
//! With the assistant in the build and a provider the gate reads Ready, the screen delegates to
//! `ask_ai`, which asks the books through `Api::ask_books` (mock states `ai.ask.*`). Until the gate
//! reads Ready, or when it refuses, the report list stays.

use crate::i18n::Tr;
use egui::{RichText, Ui};

use crate::palette::PaletteTarget;
use crate::screen::KeyHint;
use crate::screens::ai_gate::{self, Feature, Gate, TierGate};
use crate::screens::gst::GstTab;
use crate::shell::navigate::{self, NavError};
use crate::theme::TEXT_H2;
use crate::ui::card::card;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::{bold_font, tokens};
use crate::{AppCtx, Screen, ScreenAction};

/// The reports the screen offers in place of an answer (index.html:1170).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskReport {
    /// Outstanding receivables.
    Receivables,
    /// The balance sheet.
    BalanceSheet,
    /// The day book.
    DayBook,
    /// The GSTR-3B tab of the GST screen.
    Gstr3b,
    /// The bank reconciliation.
    BankReconciliation,
    /// The masters screen, showing ledgers.
    Ledgers,
}

impl AskReport {
    /// Every report, in the mock's order.
    pub const ALL: [AskReport; 6] = [
        AskReport::Receivables,
        AskReport::BalanceSheet,
        AskReport::DayBook,
        AskReport::Gstr3b,
        AskReport::BankReconciliation,
        AskReport::Ledgers,
    ];

    /// The words on the report's button in English (index.html:1170): [`AskReport::label_in`]
    /// with the English file, kept for the tests that read the English list.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            AskReport::Receivables => "Outstanding receivables",
            AskReport::BalanceSheet => "Balance sheet",
            AskReport::DayBook => "Day book",
            AskReport::Gstr3b => "GSTR-3B working",
            AskReport::BankReconciliation => "Bank reconciliation",
            AskReport::Ledgers => "Ledgers",
        }
    }

    /// The words on the report's button in the words of `tr` (index.html:1170).
    #[must_use]
    pub fn label_in(self, tr: &Tr) -> String {
        tr.t(match self {
            AskReport::Receivables => "ask.report.receivables",
            AskReport::BalanceSheet => "ask.report.balance_sheet",
            AskReport::DayBook => "ask.report.day_book",
            AskReport::Gstr3b => "ask.report.gstr3b",
            AskReport::BankReconciliation => "ask.report.bank_recon",
            AskReport::Ledgers => "ask.report.ledgers",
        })
    }

    /// The icon on the report's button, the mock's own (index.html:1242).
    fn icon(self) -> Icon {
        match self {
            AskReport::Receivables => Icon::Clock,
            AskReport::BalanceSheet => Icon::Scale,
            AskReport::DayBook => Icon::Book,
            AskReport::Gstr3b => Icon::Gst,
            AskReport::BankReconciliation => Icon::Bank,
            AskReport::Ledgers => Icon::Users,
        }
    }

    /// The screen the report's button opens, as the palette opens it. GSTR-3B opens the GST
    /// screen on its GSTR-3B tab.
    ///
    /// # Errors
    /// As [`navigate::open_target`], or [`navigate::gst_screen`] for GSTR-3B; for example
    /// [`NavError::Unsupported`] when the company has no GST registration or no bank ledger.
    pub fn open(self, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
        let key = match self {
            AskReport::Receivables => "receivables",
            AskReport::BalanceSheet => "balance-sheet",
            AskReport::DayBook => "day-book",
            AskReport::Gstr3b => {
                let books = ctx.books.as_ref().ok_or(NavError::NoCompany)?;
                let mut screen = navigate::gst_screen(books)?;
                screen.switch_tab(GstTab::Gstr3b);
                return Ok(Box::new(screen));
            }
            AskReport::BankReconciliation => navigate::BANK_RECON_KEY,
            AskReport::Ledgers => navigate::MASTERS_KEY,
        };
        navigate::open_target(&PaletteTarget::Report(key.to_owned()), ctx)
    }
}

/// The key bar entries in English: [`key_hints_in`] with the English file.
#[must_use]
pub fn key_hints() -> Vec<KeyHint> {
    key_hints_in(&Tr::default())
}

/// The key bar entries in the words of `tr`. The mock lists Enter "Ask" (index.html:1199); Enter
/// asks nothing in this build, so only the screen's name is listed. Esc is the shell's.
#[must_use]
pub fn key_hints_in(tr: &Tr) -> Vec<KeyHint> {
    vec![KeyHint::new("", tr.t("ask.title"))]
}

/// The Ask your books screen.
#[derive(Debug, Default)]
pub struct AskScreen {
    /// The company's AI tier, read through the worker.
    gate: TierGate,
    /// What the screen knows of the tier, as of the last frame.
    last: Option<Gate>,
    /// Why the last button did not open its screen.
    notice: Option<String>,
    /// The assistant half, used when the gate reads Ready.
    ai: crate::screens::ask_ai::AiAsk,
}

impl AskScreen {
    /// Whether the tier has been read (or cannot be: no company, or the read failed).
    #[must_use]
    pub fn loaded(&self) -> bool {
        if crate::release::ai_enabled() && !self.ai.loaded() {
            return false;
        }
        match &self.last {
            None | Some(Gate::Reading) => false,
            Some(
                Gate::NoCompany
                | Gate::Failed(_)
                | Gate::Unset
                | Gate::Off
                | Gate::Local
                | Gate::Cloud,
            ) => true,
        }
    }
}

/// The smallest width of a report button (`.launch`, index.html:193: `minmax(150px,1fr)`).
const TILE_MIN: f32 = 150.0;
/// The gap between report buttons (index.html:193).
const TILE_GAP: f32 = 6.0;

/// Draws the report buttons in rows that fill the width, and returns the one clicked.
#[expect(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "layout: the column count and button width follow the card's width"
)]
fn draw_reports(ui: &mut Ui, tr: &Tr) -> Option<AskReport> {
    let width = ui.available_width();
    let columns = (((width + TILE_GAP) / (TILE_MIN + TILE_GAP)).floor() as usize).max(1);
    let tile = (width - TILE_GAP * (columns as f32 - 1.0)) / columns as f32;
    let mut clicked = None;
    for row in AskReport::ALL.chunks(columns) {
        let height = row
            .iter()
            .map(|report| ai_gate::tile_height(ui, &report.label_in(tr), tile))
            .fold(ai_gate::TILE_HEIGHT, f32::max);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TILE_GAP;
            for report in row {
                let label = report.label_in(tr);
                if ai_gate::launch_tile(ui, report.icon(), &label, tile, height).clicked() {
                    clicked = Some(*report);
                }
            }
        });
        ui.add_space(TILE_GAP - ui.spacing().item_spacing.y);
    }
    clicked
}

impl Screen for AskScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("ask.title")
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        Some(tr.t("ask.subtitle"))
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        if crate::release::ai_enabled() && self.ai.ready() {
            return self.ai.keys();
        }
        key_hints_in(tr)
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if crate::release::ai_enabled() {
            self.ai.poll_gate(ctx);
            if self.ai.ready() {
                return self.ai.ui(ui, ctx);
            }
        }
        let tr = ctx.tr.clone();
        let gate = self.gate.gate(ui, ctx);
        self.last = Some(gate.clone());
        page_head(ui, &tr.t("ask.title"), Some(&tr.t("ask.subtitle")), |_| {});
        // The mock fills the box with a sample question; no sample is shown, since nothing
        // would answer it.
        ai_gate::input_box(ui, Icon::Spark, &tr.t("ask.question_hint"));
        ui.add_space(14.0);

        let mut action = ScreenAction::Stay;
        if ai_gate::gate_note_in(ui, Feature::Ask, &gate, &tr) {
            match ai_gate::open_settings(ctx) {
                Ok(open) => action = open,
                Err(why) => self.notice = Some(why),
            }
        }
        if let Some(line) = &self.notice {
            ui.add_space(8.0);
            note(ui, NoteKind::Warn, line);
        }
        ui.add_space(12.0);
        let t = tokens(ui);
        let picked = card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tr.t("ask.reports_title"))
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text),
            );
            ui.add_space(10.0);
            draw_reports(ui, &tr)
        })
        .inner;
        if let Some(report) = picked {
            match report.open(ctx) {
                Ok(screen) => action = ScreenAction::Push(screen),
                Err(e) => self.notice = Some(e.to_string()),
            }
        }
        action
    }
}

#[cfg(test)]
mod s14_tests {
    use super::AskReport;
    use crate::ui::icon::Icon;

    #[test]
    fn receivables_has_the_mocks_clock() {
        // provenance: index.html:1242 draws `Outstanding receivables` with the `clock` icon.
        assert_eq!(AskReport::Receivables.icon(), Icon::Clock);
    }
}
