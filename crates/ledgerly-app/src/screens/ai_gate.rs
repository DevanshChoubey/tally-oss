//! The AI tier gate shared by the three assistant screens: Ask your books (mock `ask`),
//! Plain-words entry (`quick-entry`) and the bill photo (`purchase-bill`).
//!
//! Each of those screens reads the company's AI tier through the worker (`Api::ai_tier`) and
//! says, in the mock's layout, why it cannot do its job in this build. Spec 6.8 lists the three
//! features (question to `ReportQuery`, plain words to `VoucherDraft`, bill image to
//! `VoucherDraft`); none of them can run today:
//!
//! * no local model client exists (`ledgerly_assist::nl_entry::ModelClient` has only the recorded
//!   test double),
//! * no OCR engine turns an image into `ledgerly_assist::ocr::OcrOutput`, so the bill reader has
//!   nothing to read,
//! * the cloud tier is refused by `Api::set_ai_tier`, since no keychain store is wired in.
//!
//! So the screens never show a reading, a draft or a figure. What each says follows the stored
//! tier ([`Gate`]): Off takes the mock's AI-off wording (index.html:1169, 1431), Local says the
//! model is not installed, and no tier says none is chosen. Every case is listed; none defaults
//! to another.

use egui::{FontId, Frame, Margin, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, vec2};
use ledgerly_api::AiTier;

use crate::keymap::Action;
use crate::palette::PaletteTarget;
use crate::screens::wire::{Done, Latest};
use crate::shell::{navigate, sidebar, submit};
use crate::theme::{RADIUS_M, RADIUS_S, TEXT_BODY, TEXT_INPUT, TEXT_META, border_width};
use crate::ui::button::{ButtonKind, button};
use crate::ui::icon::{self as icons, ICON_SIZE, Icon, icon};
use crate::ui::note::{NoteKind, colors as note_colors};
use crate::ui::{kind, para_text, tokens};
use crate::{AppCtx, ScreenAction, worker::ApiWorker};

/// The three assistant features of spec 6.8 that have a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// A question about the books, turned into a report query (Ask your books).
    Ask,
    /// A sentence in plain words, turned into a draft voucher (Plain-words entry).
    PlainWords,
    /// A bill photo, read into a draft purchase (Purchase from bill photo).
    BillReading,
}

/// What a screen knows about the company's AI tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// The tier is being read; nothing is said about it yet.
    Reading,
    /// No company file is open.
    NoCompany,
    /// The tier could not be read. The text says why.
    Failed(String),
    /// No tier has been chosen for the company.
    Unset,
    /// AI is off for the company.
    Off,
    /// The local tier is chosen, and no local model is installed.
    Local,
    /// The cloud tier is stored, and this build cannot use it.
    Cloud,
}

impl Gate {
    /// The gate for a tier read from the company file.
    #[must_use]
    pub fn from_tier(tier: Option<AiTier>) -> Self {
        match tier {
            None => Gate::Unset,
            Some(AiTier::Off) => Gate::Off,
            Some(AiTier::Local) => Gate::Local,
            Some(AiTier::Cloud) => Gate::Cloud,
        }
    }

    /// Whether the screen should offer the AI and privacy button: when a tier was read.
    #[must_use]
    pub fn offers_settings(&self) -> bool {
        match self {
            Gate::Unset | Gate::Off | Gate::Local | Gate::Cloud => true,
            Gate::Reading | Gate::NoCompany | Gate::Failed(_) => false,
        }
    }
}

/// What the feature cannot do, in the words that follow "so" in [`reason`].
fn cannot(feature: Feature) -> &'static str {
    match feature {
        Feature::Ask => "questions cannot be turned into queries",
        Feature::PlainWords => "a sentence cannot be drafted into a voucher",
        Feature::BillReading => "a bill photo cannot be read",
    }
}

/// What the user can do instead, the sentence after the reason.
fn instead(feature: Feature) -> &'static str {
    match feature {
        Feature::Ask => "These reports answer most questions directly.",
        Feature::PlainWords => "Type the voucher yourself in the payment or receipt form.",
        Feature::BillReading => "Type the bill in the purchase form instead.",
    }
}

/// The screen's name, for the line shown when no company is open.
fn screen_name(feature: Feature) -> &'static str {
    match feature {
        Feature::Ask => "ask about your books",
        Feature::PlainWords => "enter a voucher in plain words",
        Feature::BillReading => "read a bill photo",
    }
}

/// The note a screen shows for `gate`: why the feature does not run, then what to do instead.
/// `None` while the tier is still being read, so no tier is claimed before it is known.
#[must_use]
pub fn reason(feature: Feature, gate: &Gate) -> Option<String> {
    let then = instead(feature);
    let so = cannot(feature);
    let text = match gate {
        Gate::Reading => return None,
        Gate::NoCompany => format!("Open a company file to {}.", screen_name(feature)),
        Gate::Failed(why) => format!("The company's AI tier could not be read: {why}"),
        Gate::Off => match feature {
            // index.html:1430: the bill photo's own AI-off words.
            Feature::BillReading => format!("Bill reading is off for this company. {then}"),
            // index.html:1169: Ask's AI-off words; plain-words entry says the same of itself.
            Feature::Ask | Feature::PlainWords => {
                format!("AI is off for this company, so {so}. {then}")
            }
        },
        Gate::Unset => format!("No AI tier is chosen for this company, so {so}. {then}"),
        Gate::Local => match feature {
            Feature::BillReading => {
                format!("No bill reader is installed on this computer, so {so}. {then}")
            }
            Feature::Ask | Feature::PlainWords => {
                format!("No local model is installed on this computer, so {so}. {then}")
            }
        },
        Gate::Cloud => format!(
            "The cloud tier is not available in this build: the provider key must be kept in the \
             system keychain, and no keychain store is wired in. So {so}. {then}"
        ),
    };
    Some(text)
}

/// Reads the company's AI tier through the worker, and again after any saved change (choosing a
/// tier in AI and privacy is one), so a screen returned to shows the tier now stored.
#[derive(Debug, Default)]
pub struct TierGate {
    /// The answer of the last read, once it came.
    read: Option<Result<Option<AiTier>, String>>,
    /// The change count when the last read was started.
    seen: Option<u64>,
    /// The read in flight.
    latest: Latest<Result<Option<AiTier>, String>>,
}

impl TierGate {
    /// Starts a read on `worker`.
    fn start(&mut self, worker: &ApiWorker) {
        self.seen = Some(submit::change_count());
        self.latest
            .start(worker, |api| api.ai_tier().map_err(|e| e.to_string()));
    }

    /// Takes the worker's answer when it has come, starts a read when none was made or a change
    /// was saved since, and returns what is known now. A tier read here is also put in
    /// [`AppCtx::ai_tier`], so the top bar's chip agrees with the screen. While a read runs it
    /// asks `ui` for another frame, so the answer is drawn when it comes.
    pub fn gate(&mut self, ui: &Ui, ctx: &mut AppCtx) -> Gate {
        let Some(worker) = ctx.api.as_ref() else {
            return Gate::NoCompany;
        };
        match self.latest.poll() {
            None => {}
            Some(Done::Ready(Ok(tier))) => {
                self.read = Some(Ok(tier));
                ctx.ai_tier = tier;
            }
            Some(Done::Ready(Err(reason))) => self.read = Some(Err(reason)),
            Some(Done::Lost) => {
                self.read = Some(Err(
                    "the company file stopped responding; reopen the company file".to_owned(),
                ));
            }
        }
        let stale = self.seen != Some(submit::change_count());
        if !self.latest.is_pending() && (self.read.is_none() || stale) {
            self.start(worker);
        }
        if self.latest.is_pending() {
            ui.ctx().request_repaint();
            return Gate::Reading;
        }
        match &self.read {
            None => Gate::Reading,
            Some(Ok(tier)) => Gate::from_tier(*tier),
            Some(Err(reason)) => Gate::Failed(reason.clone()),
        }
    }
}

/// The question or sentence box (`.qe` in the mock, index.html:190): an icon and a one-line field
/// in a bordered row. It is drawn disabled, because nothing in this build reads what is typed.
pub fn input_box(ui: &mut Ui, which: Icon, hint: &str) {
    let t = tokens(ui);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(kind(ui)), t.border_strong))
        .corner_radius(RADIUS_M)
        .inner_margin(Margin {
            left: 12,
            right: 8,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, which, t.text_2);
                ui.add_space(8.0);
                let mut empty = String::new();
                ui.add_enabled(
                    false,
                    TextEdit::singleline(&mut empty)
                        .hint_text(hint)
                        .font(egui::FontId::proportional(TEXT_INPUT))
                        .frame(Frame::NONE)
                        .desired_width(f32::INFINITY),
                );
            });
        });
}

/// The words on the button that opens AI and privacy (index.html:1169, 1430).
pub const SETTINGS_BUTTON: &str = "AI and privacy";

/// The note's icon for `gate` (the mock's `off` for AI off, index.html:1169).
fn gate_icon(gate: &Gate) -> Icon {
    match gate {
        Gate::Off | Gate::Unset | Gate::Reading | Gate::NoCompany | Gate::Failed(_) => Icon::Off,
        Gate::Local => Icon::Cpu,
        Gate::Cloud => Icon::Cloud,
    }
}

/// Draws the note for `gate` across the width (`.note`, index.html:1169 and 1430): its icon, the
/// reason and, when a tier was read, the AI and privacy button after the text. Draws nothing
/// while the tier is being read. Returns `true` in the frame the button is clicked.
pub fn gate_note(ui: &mut Ui, feature: Feature, gate: &Gate) -> bool {
    let Some(text) = reason(feature, gate) else {
        return false;
    };
    let kind_of = match gate {
        Gate::Failed(_) => NoteKind::Warn,
        Gate::Reading | Gate::NoCompany | Gate::Unset | Gate::Off | Gate::Local | Gate::Cloud => {
            NoteKind::Info
        }
    };
    settings_note(ui, kind_of, gate_icon(gate), &text, gate.offers_settings())
}

/// Draws a note across the width with `icon`, `text` and, when `offer` is set, the AI and privacy
/// button after the text. Returns `true` in the frame the button is clicked. [`gate_note`] and
/// the bank screen's AI-off note (index.html:1830) share it.
pub fn settings_note(ui: &mut Ui, kind_of: NoteKind, glyph: Icon, text: &str, offer: bool) -> bool {
    let t = tokens(ui);
    let (bg, fg, edge) = note_colors(kind_of, &t);
    let mut clicked = false;
    Frame::new()
        .fill(bg)
        .stroke(Stroke::new(border_width(kind(ui)), edge))
        .corner_radius(RADIUS_S)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                icon(ui, glyph, fg);
                ui.add_space(4.0);
                ui.label(para_text(text, TEXT_META, fg));
                if offer {
                    clicked = button(ui, SETTINGS_BUTTON, None, ButtonKind::Secondary).clicked();
                }
            });
        });
    clicked
}

/// What opening AI and privacy does: the screen on top.
///
/// # Errors
/// A line saying why the screen did not open.
pub fn open_settings(ctx: &AppCtx) -> Result<ScreenAction, String> {
    match navigate::screen_for_key(navigate::AI_SETTINGS_KEY, ctx) {
        Some(Ok(screen)) => Ok(ScreenAction::Push(screen)),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("AI and privacy has no screen in this build".to_owned()),
    }
}

/// What opening the voucher form of `action` does (F5 Payment, F6 Receipt, F9 Purchase): the
/// form on top, as the sidebar opens it.
///
/// # Errors
/// A line saying why the form did not open, such as no company being open.
pub fn open_voucher(action: Action, ctx: &AppCtx) -> Result<ScreenAction, String> {
    sidebar::screen_for(&PaletteTarget::Command(action), ctx)
        .map(ScreenAction::Push)
        .map_err(|e| e.to_string())
}

/// The least height of a report button in the launch grid (`.launch button`, index.html:195:
/// 9 px padding around a 16 px icon and a line of text).
pub const TILE_HEIGHT: f32 = 38.0;
/// The padding inside a report button, left and right of its parts (index.html:195).
const TILE_PAD_X: f32 = 10.0;
/// The padding above and below a report button's label (index.html:195).
const TILE_PAD_Y: f32 = 9.0;

/// The label of a report button laid out to wrap within a button `width` wide.
#[expect(
    clippy::float_arithmetic,
    reason = "layout: the label wraps in what is left after the icon and the padding"
)]
fn tile_label(ui: &Ui, label: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    let wrap = (width - 3.0 * TILE_PAD_X - ICON_SIZE).max(1.0);
    ui.painter().layout(
        label.to_owned(),
        FontId::proportional(TEXT_BODY),
        tokens(ui).text,
        wrap,
    )
}

/// The height a report button `width` wide needs for `label`: [`TILE_HEIGHT`], or more when the
/// label wraps (the mock's labels wrap to two lines, index.html:1170).
#[must_use]
#[expect(
    clippy::float_arithmetic,
    reason = "layout: the label's height plus the padding"
)]
pub fn tile_height(ui: &Ui, label: &str, width: f32) -> f32 {
    (tile_label(ui, label, width).size().y + 2.0 * TILE_PAD_Y).max(TILE_HEIGHT)
}

/// Draws one report button of the launch grid (`.launch button`, index.html:195): an outlined
/// box `width` by `height` with the icon and the label, wrapped when long. Returns its response.
#[expect(
    clippy::float_arithmetic,
    reason = "layout: the icon and label sit at fixed offsets inside the button"
)]
pub fn launch_tile(ui: &mut Ui, which: Icon, label: &str, width: f32, height: f32) -> Response {
    let t = tokens(ui);
    let galley = tile_label(ui, label, width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let hovered = response.hovered() || response.has_focus();
    let (fill, edge) = if hovered {
        (t.hover, t.border_strong)
    } else {
        (t.surface, t.border)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(
        rect,
        RADIUS_S,
        Stroke::new(border_width(kind(ui)), edge),
        StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_size(
        rect.left_center() + vec2(TILE_PAD_X, -ICON_SIZE / 2.0),
        vec2(ICON_SIZE, ICON_SIZE),
    );
    icons::paint(painter, icon_rect, which, t.text_2);
    let text_at = rect.left_center() + vec2(2.0 * TILE_PAD_X + ICON_SIZE, -galley.size().y / 2.0);
    painter.galley(text_at, galley, t.text);
    response
}
