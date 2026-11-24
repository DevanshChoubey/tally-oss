//! AI and privacy (mock `SCREENS['ai-settings']`, index.html:1896-1920; states
//! `ai-settings.local`, `ai-settings.cloud` and `ai-settings.off`).
//!
//! The screen shows the company's AI tier as stored in the company file (`ledgerly_api::AiTier`)
//! and lets the user choose it with 1, 2 or 3 or by a click. Three rules of spec 6.8 shape it:
//!
//! * AI only drafts. The "never" list says so, and no panel shows a figure.
//! * The cloud tier needs the user's own key. No key store is wired into this build, so the
//!   engine refuses the cloud choice (`Outcome::Unsupported`). The screen shows that reason and
//!   stores nothing; the key never reaches the company file.
//! * The log is the file's real `AiProposalRecorded` events. When it is empty the screen says so
//!   and draws no rows; Ctrl+E is offered and exports only a log that has rows.
//!
//! Every difference from the mock is listed in `design/PARITY-GAPS.md`.

use std::path::PathBuf;

use egui::{Align, Key, Layout, Modifiers, RichText, Ui};
use egui_extras::{Size, StripBuilder};
use ledgerly_api::{AiLogEntry, AiTier, Api};
use ledgerly_core::Outcome;

use crate::screen::KeyHint;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::submit;
use crate::theme::{TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::card::{card, card_flush};
use crate::ui::icon::Icon;
use crate::ui::mock_table::{self, Cell, Width, panel_title};
use crate::ui::note::{NoteKind, note};
use crate::ui::option_card::{OptionCard, option_grid};
use crate::ui::page_head::page_head;
use crate::ui::{bold_font, para, para_text, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

/// The screen's title in the sidebar, the palette and the page head (index.html:891).
pub const TITLE: &str = "AI and privacy";

/// The page-head subtitle after the company name (index.html:1903).
pub const SUBTITLE_TAIL: &str = " only. Other companies keep their own setting.";

/// The "Off" option card: title and body (index.html:1905).
pub const OPTION_OFF: (&str, &str) = (
    "Off",
    "No AI. Exact and rule matching, GST checks and reports all still work.",
);
/// The "On this computer" option card (index.html:1906).
pub const OPTION_LOCAL: (&str, &str) = (
    "On this computer",
    "Local model. Nothing leaves this computer.",
);
/// The "Cloud, your own key" option card (index.html:1907).
pub const OPTION_CLOUD: (&str, &str) = (
    "Cloud, your own key",
    "You confirm each document. Redaction runs first.",
);

/// The note while AI is off (index.html:1061 and index.html:1911).
pub const OFF_NOTE: &str = "AI is off. Bill reading, plain-words entry, AI bank suggestions and Ask are hidden. Deterministic checks keep running: GSTIN check characters, due-date nudges, duplicate bills, Exact and Rule bank matching.";

/// The note while no tier is chosen. Not in the mock, whose first state is "On this computer" and
/// would claim a tier nobody chose (PARITY-GAPS).
pub const UNSET_NOTE: &str =
    "No AI tier is chosen for this company yet. Nothing is sent to any model until you choose one.";

/// The feature rows (index.html:1908): feature, what the model sees on this computer, and what
/// is sent to the provider in the cloud tier.
pub const FEATURES: [(&str, &str, &str); 5] = [
    (
        "Bill photo to draft purchase",
        "The image",
        "Image, cropped and redacted",
    ),
    (
        "Plain words to draft voucher",
        "Your sentence and ledger names",
        "Same, no balances",
    ),
    ("Bank line suggestions", "Narration, date, amount", "Same"),
    (
        "Ask your books",
        "Your question and the list of report types",
        "Same; never your figures",
    ),
    (
        "2B mismatch explanations",
        "The two records and the attached image",
        "Same, redacted",
    ),
];

/// The "What AI may never do here" list (index.html:1916).
pub const NEVER: [&str; 5] = [
    "Show a figure it worked out. Amounts come from the books, the document, your words or the engine.",
    "Post, alter, delete or file anything without your accept.",
    "Create a ledger, item or party on its own.",
    "Change a locked period or an IMS action.",
    "Send anything to the cloud without your confirmation.",
];

/// The label column of the local model panel, in points. The mock's table sizes it to its
/// content (index.html:1914), which comes to 226 in the capture.
const LOCAL_LABEL_COLUMN: f32 = 226.0;
/// The label column of the provider panel, in points (173 in the mock's capture, index.html:1912).
const PROVIDER_LABEL_COLUMN: f32 = 173.0;
/// The feature column of the features table, in points (197 in the mock's capture).
const FEATURE_COLUMN: f32 = 197.0;
/// The On column of the features table, in points (`th` width 60px in index.html:1908).
const ON_COLUMN: f32 = 60.0;
/// The gap between the two columns of a `.grid` (index.html:136).
const GRID_GAP: f32 = 12.0;
/// The narrowest the never card gets (`minmax(320px,1fr)`, index.html:138).
const NEVER_MIN: f32 = 320.0;
/// The share of the main row the left column takes before the gap: 1.55 of 2.55 parts.
const MAIN_LEFT_PARTS: f32 = 1.55;
/// The parts of the main row in all (index.html:138).
const MAIN_PARTS: f32 = 2.55;

/// The log card's title (index.html:1915).
pub const LOG_TITLE: &str = "AI activity log";
/// The log card's meta line when it has rows (index.html:1915).
pub const LOG_META: &str = "Kept with the company file. Ctrl+E exports it.";
/// The log card's meta line when it has no rows. The Ctrl+E promise does not hold then.
pub const LOG_META_EMPTY: &str = "Kept with the company file.";
/// The log card's text when it has no rows.
pub const LOG_EMPTY: &str = "No AI activity has been recorded for this company yet.";
/// The log's column headers. The mock also has "Feature" and "Input"; the file records the model
/// and an input hash, not the feature (PARITY-GAPS).
pub const LOG_HEADERS: [&str; 5] = ["When", "Model", "Input hash", "Output", "Your decision"];

/// The status of the cloud provider: the cloud tier is not available in this build.
pub const CLOUD_STATUS: &str = "Not available";
/// The reason shown when the engine has not yet said why.
pub const CLOUD_REASON_DEFAULT: &str = "No key store is wired into this build.";

/// Which panel the screen shows. It follows the stored tier, except that choosing Cloud shows
/// the cloud panel with the reason it is not available. The cloud choice itself is never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// No tier has been chosen for this company.
    Unset,
    /// AI is off.
    Off,
    /// The local model on this computer.
    Local,
    /// The cloud tier with the user's own key.
    Cloud,
}

/// The key bar entries for a panel (index.html:1920). Esc is the shell's. Ctrl+E is listed only
/// when the log has rows, since it exports nothing otherwise.
#[must_use]
pub fn key_hints(log_has_rows: bool) -> Vec<KeyHint> {
    let mut hints = vec![
        KeyHint::new("", TITLE),
        KeyHint::new("1", "Off"),
        KeyHint::new("2", "On this computer"),
        KeyHint::new("3", "Cloud"),
    ];
    if log_has_rows {
        hints.push(KeyHint::new("Ctrl+E", "Export log"));
    }
    hints
}

/// The worker's answer to the one call this screen waits on.
#[derive(Debug)]
enum Reply {
    /// The stored tier and the log, read together.
    Loaded(Result<(Option<AiTier>, Vec<AiLogEntry>), String>),
    /// The answer to choosing `tier`: stored, or refused with the engine's reason.
    Chosen {
        /// The tier that was chosen.
        tier: AiTier,
        /// The engine's answer.
        result: Result<Outcome<()>, String>,
    },
    /// The answer to exporting the log to `path`.
    Exported {
        /// Where the export was asked to write.
        path: PathBuf,
        /// The engine's answer.
        result: Result<Outcome<()>, String>,
    },
}

/// The AI and privacy screen.
#[derive(Debug, Default)]
pub struct AiSettingsScreen {
    /// The tier stored in the company file, once read.
    stored: Option<AiTier>,
    /// Whether the stored tier and the log have been read from the open company.
    loaded: bool,
    /// The log rows read from the company file.
    log: Vec<AiLogEntry>,
    /// The user chose Cloud, so the cloud panel is shown with the engine's reason.
    cloud_asked: bool,
    /// The engine's reason the cloud tier is not available, once it has said so.
    cloud_reason: Option<String>,
    /// A line for the user about the last choice, export or failed read.
    notice: Option<String>,
    /// The notice is the engine's refusal of the cloud tier, which the Provider card shows, so it
    /// is not drawn as a banner too.
    notice_is_cloud_refusal: bool,
    /// The one call in flight.
    latest: Latest<Reply>,
}

impl AiSettingsScreen {
    /// The panel to show now.
    #[must_use]
    pub fn panel(&self) -> Panel {
        if self.cloud_asked {
            return Panel::Cloud;
        }
        match self.stored {
            None => Panel::Unset,
            Some(AiTier::Off) => Panel::Off,
            Some(AiTier::Local) => Panel::Local,
            Some(AiTier::Cloud) => Panel::Cloud,
        }
    }

    /// Whether the stored tier and the log have been read from the open company.
    #[must_use]
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    /// The tier stored in the company file, once read.
    #[must_use]
    pub fn stored(&self) -> Option<AiTier> {
        self.stored
    }

    /// The log rows read from the company file.
    #[must_use]
    pub fn log(&self) -> &[AiLogEntry] {
        &self.log
    }

    /// The notice line, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// The notice to draw as a banner over the page. The cloud refusal is drawn in the Provider
    /// card instead (`notice()` still holds it), so it is not a banner.
    #[must_use]
    pub fn banner(&self) -> Option<&str> {
        if self.notice_is_cloud_refusal {
            None
        } else {
            self.notice()
        }
    }

    /// Records the engine's refusal of the cloud tier: kept as the notice, shown in the card.
    pub fn show_cloud_refusal(&mut self, line: &str) {
        self.notice = Some(line.to_owned());
        self.notice_is_cloud_refusal = true;
    }

    /// Records any other line for the user; it is drawn as a banner.
    pub fn show_notice(&mut self, line: &str) {
        self.notice = Some(line.to_owned());
        self.notice_is_cloud_refusal = false;
    }

    /// Starts reading the stored tier and the log.
    fn start_load(&mut self, worker: &ApiWorker) {
        self.latest.start(worker, |api: &mut Api| {
            let tier = api.ai_tier().map_err(|e| e.to_string());
            let log = api.ai_log().map_err(|e| e.to_string());
            Reply::Loaded(match (tier, log) {
                (Ok(tier), Ok(log)) => Ok((tier, log)),
                (Err(reason), _) | (_, Err(reason)) => Err(reason),
            })
        });
    }

    /// Starts choosing `tier`. Choosing Cloud is refused by the engine and stores nothing.
    fn choose(&mut self, worker: &ApiWorker, tier: AiTier) {
        self.latest
            .start(worker, move |api: &mut Api| Reply::Chosen {
                tier,
                result: api.set_ai_tier(tier).map_err(|e| e.to_string()),
            });
    }

    /// Starts exporting the log to `path`.
    fn export(&mut self, worker: &ApiWorker, path: PathBuf) {
        self.latest.start(worker, move |api: &mut Api| {
            let result = api.export_ai_log(&path).map_err(|e| e.to_string());
            Reply::Exported { path, result }
        });
    }

    /// Chooses `tier` from a key or a click. Cloud also shows the cloud panel at once.
    /// The keys this screen answers: 1, 2 and 3 choose a tier; Ctrl+E exports the log when it
    /// has rows.
    fn read_keys(&mut self, ui: &Ui, ctx: &AppCtx) {
        let (one, two, three, export) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Num1),
                i.consume_key(Modifiers::NONE, Key::Num2),
                i.consume_key(Modifiers::NONE, Key::Num3),
                i.consume_key(Modifiers::COMMAND, Key::E),
            )
        });
        let worker = ctx.api.as_ref();
        if one {
            self.pick(worker, AiTier::Off);
        } else if two {
            self.pick(worker, AiTier::Local);
        } else if three {
            self.pick(worker, AiTier::Cloud);
        }
        if export
            && !self.log.is_empty()
            && let (Some(worker), Some(path)) = (worker, save_dialog())
        {
            self.export(worker, path);
        }
    }

    fn pick(&mut self, worker: Option<&ApiWorker>, tier: AiTier) {
        self.cloud_asked = tier == AiTier::Cloud;
        if let Some(worker) = worker {
            self.choose(worker, tier);
        }
    }

    /// Takes the worker's answer when it has arrived and applies it to the screen and the shell.
    fn tick(&mut self, ctx: &mut AppCtx) -> bool {
        let reply = match self.latest.poll() {
            None => return false,
            Some(Done::Lost) => {
                self.show_notice("The company file stopped responding; reopen the company file.");
                return true;
            }
            Some(Done::Ready(reply)) => reply,
        };
        match reply {
            Reply::Loaded(Ok((tier, log))) => {
                self.stored = tier;
                self.log = log;
                self.loaded = true;
                ctx.ai_tier = tier;
            }
            Reply::Loaded(Err(reason)) => {
                self.loaded = true;
                self.show_notice(&format!("The AI settings could not be read: {reason}"));
            }
            Reply::Chosen { tier, result } => match result {
                Ok(Outcome::Value(())) => {
                    self.stored = Some(tier);
                    self.cloud_asked = false;
                    self.cloud_reason = None;
                    self.notice = None;
                    self.notice_is_cloud_refusal = false;
                    ctx.ai_tier = Some(tier);
                    // The top bar's chip and the books are read again by the shell.
                    submit::note_changed();
                }
                Ok(Outcome::Unsupported(u)) => {
                    let line = unsupported_text(&u);
                    if tier == AiTier::Cloud {
                        self.cloud_reason = Some(u.reason.clone());
                        self.show_cloud_refusal(&line);
                    } else {
                        self.show_notice(&line);
                    }
                }
                Err(reason) => self.show_notice(&reason),
            },
            Reply::Exported { path, result } => {
                let line = match result {
                    Ok(Outcome::Value(())) => {
                        format!("Exported the AI activity log to {}.", path.display())
                    }
                    Ok(Outcome::Unsupported(u)) => unsupported_text(&u),
                    Err(reason) => reason,
                };
                self.show_notice(&line);
            }
        }
        true
    }
}

impl Screen for AiSettingsScreen {
    fn title(&self) -> String {
        TITLE.to_owned()
    }

    fn keys(&self) -> Vec<KeyHint> {
        key_hints(!self.log.is_empty())
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.tick(ctx) {
            // The top bar chip was drawn before this frame's reply arrived; draw once more.
            ui.ctx().request_repaint();
        }
        if let Some(worker) = ctx.api.as_ref()
            && !self.loaded
            && !self.latest.is_pending()
        {
            self.start_load(worker);
        }

        self.read_keys(ui, ctx);

        let company = ctx
            .books
            .as_ref()
            .map_or_else(|| "this company".to_owned(), |b| b.company.name.clone());
        let subtitle = format!("For {company}{SUBTITLE_TAIL}");
        // `page_head` ends with the head's own 16 point margin (index.html:121).
        let head = page_head(ui, TITLE, Some(&subtitle), |_| {});
        record_probe(ui, "ai-settings.head", head);

        if ctx.api.is_none() {
            note(
                ui,
                NoteKind::Info,
                "Open a company file to see and change its AI tier.",
            );
            return ScreenAction::Stay;
        }
        if let Some(line) = self.banner() {
            let drawn = note(ui, NoteKind::Warn, line);
            record_probe(ui, "ai-settings.banner", drawn.rect);
            gap(ui, 12.0);
        }

        let panel = self.panel();
        if let Some(tier) = draw_options(ui, panel, self.stored) {
            self.pick(ctx.api.as_ref(), tier);
        }
        gap(ui, 14.0);

        draw_panel(ui, panel, self.cloud_reason.as_deref());

        draw_main_row(ui, panel, &self.log, !self.loaded);
        ScreenAction::Stay
    }
}

/// `margin-bottom` between blocks: `points` from the block above to the one below, whatever
/// spacing the layout adds between items.
fn gap(ui: &mut Ui, points: f32) {
    #[expect(
        clippy::float_arithmetic,
        reason = "a margin less the layout's own spacing"
    )]
    let extra = (points - ui.spacing().item_spacing.y).max(0.0);
    ui.add_space(extra);
}

/// The system save dialog for the log export, CSV only.
fn save_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export AI activity log")
        .set_file_name("ai-activity-log.csv")
        .add_filter("CSV", &["csv"])
        .save_file()
}

pub use crate::ui::probe::{read_probe, record_probe};

/// The panel under the option cards: what the chosen tier means, or the note for no tier.
fn draw_panel(ui: &mut Ui, panel: Panel, cloud_reason: Option<&str>) {
    match panel {
        Panel::Unset => {
            note(ui, NoteKind::Info, UNSET_NOTE);
            gap(ui, 14.0);
        }
        Panel::Off => {
            note(ui, NoteKind::Info, OFF_NOTE);
            gap(ui, 14.0);
        }
        Panel::Local => {
            draw_local(ui);
            gap(ui, 14.0);
        }
        Panel::Cloud => {
            draw_cloud(ui, cloud_reason);
            gap(ui, 14.0);
        }
    }
}

/// The tier a panel shows, as the option card it checks.
fn shown_tier(panel: Panel) -> Option<AiTier> {
    match panel {
        Panel::Unset => None,
        Panel::Off => Some(AiTier::Off),
        Panel::Local => Some(AiTier::Local),
        Panel::Cloud => Some(AiTier::Cloud),
    }
}

/// Draws the three option cards in a row and returns the tier clicked this frame, if any.
/// The checked card is the one whose panel is shown (`aria-checked`, index.html:1905-1907). The
/// "Current" badge marks the tier stored in the company file when that is not the card checked:
/// a refused cloud choice leaves the stored tier unchanged.
fn draw_options(ui: &mut Ui, panel: Panel, stored: Option<AiTier>) -> Option<AiTier> {
    let t = tokens(ui);
    record_probe(ui, "ai-settings.accent", t.accent);
    record_probe(ui, "ai-settings.accent_soft", t.accent_soft);
    let options = [
        (AiTier::Off, Icon::Off, OPTION_OFF),
        (AiTier::Local, Icon::Cpu, OPTION_LOCAL),
        (AiTier::Cloud, Icon::Cloud, OPTION_CLOUD),
    ];
    let shown = shown_tier(panel);
    let cards = options.map(|(tier, glyph, (title, body))| {
        let current_elsewhere = stored == Some(tier) && shown != Some(tier);
        OptionCard::new(glyph, title, body)
            .selected(shown == Some(tier))
            .badge(current_elsewhere.then_some("Current"))
    });
    let drawn = option_grid(ui, egui::Id::unique("ai-option"), 3, &cards);
    let mut clicked = None;
    for (n, (card, (tier, _, _))) in drawn.iter().zip(options).enumerate() {
        record_probe(ui, &format!("ai-settings.icon.{n}"), card.icon);
        record_probe(ui, &format!("ai-settings.title.{n}"), card.title);
        record_probe(ui, &format!("ai-settings.card.{n}"), card.card);
        if card.response.clicked() {
            clicked = Some(tier);
        }
    }
    clicked
}

/// The local panel: the local model's reader, language and network rows, across the full width
/// of the option cards (index.html:1906 and the local-model card at 1914).
fn draw_local(ui: &mut Ui) {
    let local = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        panel_title(ui, Some(Icon::Cpu), "Local model");
        let rows = [
            (
                "Reader",
                "Not installed. This build has no local bill reader.",
            ),
            ("Language", "Not installed. This build has no local model."),
            ("Network", "Nothing is sent: no AI feature is built yet."),
        ]
        .map(|(label, value)| vec![mock_table::muted(label), mock_table::text(value)]);
        mock_table::table(
            ui,
            &[Width::Fixed(LOCAL_LABEL_COLUMN), Width::Rest],
            None,
            rows.into(),
        );
    });
    record_probe(ui, "ai-settings.local", local.response.rect);
}

/// The cloud panel (index.html:1912): the Provider card and the Removed before sending card, side
/// by side. The provider is not available in this build, and the Provider card says why in the
/// mock's place for the key status.
fn draw_cloud(ui: &mut Ui, reason: Option<&str>) {
    let id = egui::Id::unique("ai-cloud-height");
    let wanted = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let mut tallest = 0.0_f32;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = GRID_GAP;
        ui.columns(2, |cols| {
            let [left, right] = cols else { return };
            let provider = card(left, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(wanted);
                panel_title(ui, Some(Icon::Cloud), "Provider");
                let status: Cell<'_> = Box::new(|ui| {
                    badge(ui, BadgeKind::Warn, CLOUD_STATUS);
                });
                let rows = vec![
                    vec![mock_table::muted("Status"), status],
                    vec![
                        mock_table::muted("Why"),
                        mock_table::text(reason.unwrap_or(CLOUD_REASON_DEFAULT)),
                    ],
                    vec![
                        mock_table::muted("Key"),
                        mock_table::text(
                            "Never written to the company file. It belongs in the OS keychain.",
                        ),
                    ],
                    vec![
                        mock_table::muted("Region and retention"),
                        mock_table::muted("Set by your provider's terms. Check them."),
                    ],
                ];
                mock_table::table(
                    ui,
                    &[Width::Fixed(PROVIDER_LABEL_COLUMN), Width::Rest],
                    None,
                    rows,
                );
                tallest = tallest.max(ui.min_rect().height());
            });
            record_probe(left, "ai-settings.provider", provider.response.rect);
            let removed = card(right, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(wanted);
                panel_title(ui, Some(Icon::Shield), "Removed before sending");
                let rows = REMOVED
                    .iter()
                    .map(|line| vec![mock_table::text(line), mock_table::switch_cell(true, line)])
                    .collect();
                mock_table::table(ui, &[Width::Rest, Width::Fixed(ON_COLUMN)], None, rows);
                let t = tokens(ui);
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Not built yet")
                        .size(TEXT_META)
                        .color(t.text_3),
                );
                tallest = tallest.max(ui.min_rect().height());
            });
            record_probe(right, "ai-settings.removed", removed.response.rect);
        });
    });
    #[expect(clippy::float_arithmetic, reason = "how far the height moved")]
    if (tallest - wanted).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, tallest));
        ui.ctx().request_repaint();
    }
}

/// What redaction takes out before anything is sent (index.html:1912). The mock also lists email
/// addresses; the redaction module recognises PAN, GSTIN, Aadhaar, bank account numbers, IFSC
/// codes and phone numbers, not email addresses, so the card names exactly those.
const REMOVED: [&str; 4] = [
    "Bank account numbers and IFSC codes",
    "PAN, GSTIN and Aadhaar numbers",
    "Phone numbers",
    "Ledger balances and report figures (never sent)",
];

/// The main row (`.g-main`, index.html:1918): the features table over the log on the left, the
/// never card on the right.
fn draw_main_row(ui: &mut Ui, panel: Panel, log: &[AiLogEntry], reading: bool) {
    #[expect(clippy::float_arithmetic, reason = "the grid's fractions of the row")]
    let (left, right) = {
        let room = (ui.available_width() - GRID_GAP).max(0.0);
        let right = (room * (MAIN_PARTS - MAIN_LEFT_PARTS) / MAIN_PARTS).max(NEVER_MIN.min(room));
        (room - right, right)
    };
    // The strip's own cell spacing is the grid gap, not the layout's item spacing.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        StripBuilder::new(ui)
            .size(Size::exact(left))
            .size(Size::exact(GRID_GAP))
            .size(Size::exact(right))
            .horizontal(|mut strip| {
                strip.cell(|col| {
                    draw_features(col, panel);
                    gap(col, 12.0);
                    draw_log(col, log, reading);
                });
                strip.empty();
                strip.cell(draw_never);
            });
    });
}

/// The features table: what each feature sees, and whether it runs (index.html:1908). The On
/// switch is read-only: nothing runs yet (PARITY-GAPS).
fn draw_features(ui: &mut Ui, panel: Panel) {
    let off = matches!(panel, Panel::Off | Panel::Unset);
    let cloud = panel == Panel::Cloud;
    let t = tokens(ui);
    let features = card_flush(ui, |ui| {
        let second = if cloud {
            "What is sent to your provider"
        } else {
            "What the model sees"
        };
        let texts: Vec<String> = FEATURES
            .iter()
            .map(|(_, sees, sent)| {
                if off {
                    "Nothing: off".to_owned()
                } else if cloud {
                    (*sent).to_owned()
                } else {
                    format!("{sees}, on this computer")
                }
            })
            .collect();
        let rows = FEATURES
            .iter()
            .zip(&texts)
            .map(|((feature, _, _), text)| {
                vec![
                    mock_table::text(feature),
                    mock_table::muted(text),
                    mock_table::switch_cell(!off, feature),
                ]
            })
            .collect();
        mock_table::table(
            ui,
            &[
                Width::Fixed(FEATURE_COLUMN),
                Width::Rest,
                Width::Fixed(ON_COLUMN),
            ],
            Some(&["Feature", second, "On"]),
            rows,
        );
        if !off {
            // Nothing runs in this build; the switch says the feature is on, this line says why
            // nothing happens (PARITY-GAPS).
            ui.horizontal(|ui| {
                ui.add_space(crate::ui::table::CELL_PAD_X);
                ui.label(
                    RichText::new("Not built yet")
                        .size(TEXT_META)
                        .color(t.text_3),
                );
            });
            ui.add_space(6.0);
        }
    });
    record_probe(ui, "ai-settings.features", features.response.rect);
}

/// The AI activity log: the file's real rows, or a line saying there are none.
fn draw_log(ui: &mut Ui, log: &[AiLogEntry], reading: bool) {
    card_flush(ui, |ui| {
        let t = tokens(ui);
        // `.spread` with `padding:12px 16px 0` (index.html:1915).
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(LOG_TITLE)
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                let meta = if log.is_empty() {
                    LOG_META_EMPTY
                } else {
                    LOG_META
                };
                ui.label(RichText::new(meta).size(TEXT_META).color(t.text_3));
            });
        });
        ui.add_space(6.0);
        if reading || log.is_empty() {
            let line = if reading {
                "Reading the log..."
            } else {
                LOG_EMPTY
            };
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(para_text(line, TEXT_BODY, t.text_2));
            });
            ui.add_space(12.0);
            return;
        }
        let texts: Vec<[String; 5]> = log
            .iter()
            .map(|entry| {
                [
                    when_text(entry),
                    entry.model.clone(),
                    short_hash(&entry.input_hash),
                    entry.output_json.clone(),
                    if entry.accepted {
                        "Accepted"
                    } else {
                        "Not accepted"
                    }
                    .to_owned(),
                ]
            })
            .collect();
        let rows = texts
            .iter()
            .map(|cells| {
                vec![
                    mock_table::text(&cells[0]),
                    mock_table::text(&cells[1]),
                    mock_table::muted(&cells[2]),
                    mock_table::text(&cells[3]),
                    mock_table::text(&cells[4]),
                ]
            })
            .collect();
        mock_table::table(
            ui,
            &[
                Width::Fixed(140.0),
                Width::Fixed(110.0),
                Width::Fixed(130.0),
                Width::Rest,
                Width::Fixed(110.0),
            ],
            Some(&LOG_HEADERS),
            rows,
        );
    });
}

/// The log's time as the CSV export writes it: `YYYY-MM-DD HH:MM` in the entry's own offset.
fn when_text(entry: &AiLogEntry) -> String {
    let (y, mo, d) = entry.at.to_calendar_date();
    let (h, mi, _) = entry.at.to_hms();
    format!("{y:04}-{:02}-{d:02} {h:02}:{mi:02}", u8::from(mo))
}

/// The first twelve characters of a hash, then an ellipsis (index.html:1896 shows `9f3c…e1`).
fn short_hash(hash: &str) -> String {
    if hash.chars().count() <= 12 {
        return hash.to_owned();
    }
    let head: String = hash.chars().take(12).collect();
    format!("{head}…")
}

/// The "What AI may never do here" card (index.html:1916): a bold title over a bulleted list,
/// 18 points in, 6 apart.
fn draw_never(ui: &mut Ui) {
    let never = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let t = tokens(ui);
        ui.label(
            RichText::new("What AI may never do here")
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for line in NEVER {
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("•").size(TEXT_BODY).color(t.text_2));
                ui.vertical(|ui| {
                    para(ui, line, TEXT_BODY, t.text_2);
                });
            });
        }
    });
    record_probe(ui, "ai-settings.never", never.response.rect);
}
