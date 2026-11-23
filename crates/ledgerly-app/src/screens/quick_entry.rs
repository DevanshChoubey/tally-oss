//! Plain-words entry (mock `SCREENS['quick-entry']`, index.html:1516-1555).
//!
//! Spec 6.8: a sentence such as "paid rent 25k to Sharma Properties by HDFC" becomes a
//! `VoucherDraft` that the engine balances and a person accepts. `ledgerly_assist::nl_entry`
//! parses a model's answer, but no model client exists in this build (its only `ModelClient` is
//! the recorded test double), so nothing can be drafted. The sentence box is drawn disabled, the
//! note says why for the company's stored tier ([`crate::screens::ai_gate`]), and the screen
//! offers the voucher forms the mock's rejected state offers ("Type it myself", index.html:1554):
//! F5 Payment and F6 Receipt, the two kinds `nl_entry` drafts. No draft, sample or figure is
//! drawn.
//!
//! All five mock states (`quick-entry.proposal`, `.hinglish`, `.unknown-ledger`, `.loading`,
//! `.rejected`) need a draft and are listed in `design/PARITY-GAPS.md`.

use egui::{Key, Modifiers, RichText, Ui};
use egui_extras::{Size, StripBuilder};

use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::screens::ai_gate::{self, Feature, Gate, TierGate};
use crate::theme::{TEXT_BODY, TEXT_H2};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::{bold_font, para_text, tokens};
use crate::{AppCtx, Screen, ScreenAction};

/// The screen's title (index.html:1542).
pub const TITLE: &str = "Plain-words entry";
/// The page head's purpose line (index.html:1542).
pub const SUBTITLE: &str = "Type it the way you would say it. You get a draft voucher; the engine balances it; you accept.";
/// The disabled sentence box's hint. The mock fills it with a sample sentence; none is shown,
/// since nothing would draft it.
pub const SENTENCE_HINT: &str = "Sentences are not drafted into vouchers in this build";
/// The rules card's title (index.html:1545).
pub const RULES_TITLE: &str = "What the assistant may do here";
/// The rules card's lines (index.html:1545).
pub const RULES: [&str; 3] = [
    "Copy amounts you typed (25k, 1 lakh) exactly. It never works one out.",
    "Pick ledgers that already exist. It never creates one without you.",
    "Suggest a date from a matching bank line.",
];
/// The card title over the voucher forms (index.html:1554: "Type it myself").
pub const TYPE_IT_TITLE: &str = "Type it myself";

/// The key bar entries: the screen's name, then the two voucher forms. The mock's Ctrl+A, Enter
/// and Alt+D act on a draft (index.html:1554), which never exists here. Esc is the shell's.
#[must_use]
pub fn key_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("", TITLE),
        KeyHint::new("F5", "Payment"),
        KeyHint::new("F6", "Receipt"),
    ]
}

/// The Plain-words entry screen.
#[derive(Debug, Default)]
pub struct QuickEntryScreen {
    /// The company's AI tier, read through the worker.
    gate: TierGate,
    /// What the screen knows of the tier, as of the last frame.
    last: Option<Gate>,
    /// Why the last button or key did not open its screen.
    notice: Option<String>,
}

impl QuickEntryScreen {
    /// Whether the tier has been read (or cannot be: no company, or the read failed).
    #[must_use]
    pub fn loaded(&self) -> bool {
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

    /// Turns a request to open a screen into the stack's action, keeping the reason when it
    /// does not open.
    fn opened(&mut self, result: Result<ScreenAction, String>) -> ScreenAction {
        match result {
            Ok(action) => action,
            Err(why) => {
                self.notice = Some(why);
                ScreenAction::Stay
            }
        }
    }
}

/// The share of the row the left card takes (`.g-main`, index.html:138: 1.55fr against 1fr).
const LEFT_SHARE: f32 = 0.6;

/// What the left card asked for.
enum Ask {
    /// Nothing.
    Nothing,
    /// Open a voucher form.
    Voucher(Action),
    /// Open AI and privacy.
    Settings,
}

/// The left card: the note with its AI and privacy button, then the voucher forms.
fn draw_left(ui: &mut Ui, gate: &Gate, notice: Option<&str>) -> Ask {
    let t = tokens(ui);
    let mut asked = Ask::Nothing;
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        if ai_gate::gate_note(ui, Feature::PlainWords, gate) {
            asked = Ask::Settings;
        }
        if let Some(line) = notice {
            ui.add_space(8.0);
            note(ui, NoteKind::Warn, line);
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new(TYPE_IT_TITLE)
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if button(ui, "Payment", Some("F5"), ButtonKind::Secondary).clicked() {
                asked = Ask::Voucher(Action::NewPayment);
            }
            if button(ui, "Receipt", Some("F6"), ButtonKind::Secondary).clicked() {
                asked = Ask::Voucher(Action::NewReceipt);
            }
        });
    });
    asked
}

/// The right card: what the assistant may do here (index.html:1545). The mock's "Try" sentences
/// are left out, since nothing would draft them.
fn draw_rules(ui: &mut Ui) {
    let t = tokens(ui);
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(RULES_TITLE)
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(8.0);
        for line in RULES {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("•").color(t.text_2));
                ui.label(para_text(line, TEXT_BODY, t.text_2));
            });
        }
    });
}

impl Screen for QuickEntryScreen {
    fn title(&self) -> String {
        TITLE.to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        Some(SUBTITLE.to_owned())
    }

    fn keys(&self) -> Vec<KeyHint> {
        key_hints()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        let gate = self.gate.gate(ctx);
        self.last = Some(gate.clone());
        let (f5, f6) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F5),
                i.consume_key(Modifiers::NONE, Key::F6),
            )
        });
        if f5 {
            return self.opened(ai_gate::open_voucher(Action::NewPayment, ctx));
        }
        if f6 {
            return self.opened(ai_gate::open_voucher(Action::NewReceipt, ctx));
        }

        page_head(ui, TITLE, Some(SUBTITLE), |_| {});
        ai_gate::input_box(ui, Icon::Chat, SENTENCE_HINT);
        ui.add_space(14.0);

        let notice = self.notice.clone();
        let mut asked = Ask::Nothing;
        StripBuilder::new(ui)
            .size(Size::relative(LEFT_SHARE))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| asked = draw_left(ui, &gate, notice.as_deref()));
                strip.cell(draw_rules);
            });
        match asked {
            Ask::Nothing => ScreenAction::Stay,
            Ask::Voucher(action) => self.opened(ai_gate::open_voucher(action, ctx)),
            Ask::Settings => self.opened(ai_gate::open_settings(ctx)),
        }
    }
}
