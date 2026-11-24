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

use crate::i18n::Tr;
use std::path::PathBuf;

use egui::{Align, Key, Layout, Modifiers, RichText, Sense, Stroke, Ui};
use egui_extras::{Column, Size, StripBuilder};
use ledgerly_api::{AiLogEntry, AiTier, Api};
use ledgerly_core::Outcome;

use crate::screen::KeyHint;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::submit;
use crate::theme::{TEXT_BODY, TEXT_META};
use crate::ui::card::card;
use crate::ui::icon::{Icon, icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::option_card::{OptionCard, option_grid};
use crate::ui::page_head::{HEAD_GAP, page_head};
use crate::ui::switch::switch;
use crate::ui::table::{self, ROW_HEIGHT};
use crate::ui::{bold_font, para_text, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

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

/// The "What AI may never do here" list (index.html:1916).
pub const NEVER: [&str; 5] = [
    "Show a figure it worked out. Amounts come from the books, the document, your words or the engine.",
    "Post, alter, delete or file anything without your accept.",
    "Create a ledger, item or party on its own.",
    "Change a locked period or an IMS action.",
    "Send anything to the cloud without your confirmation.",
];

/// The label column of the local and provider panels, in points.
const LABEL_COLUMN: f32 = 120.0;
/// The feature column of the features table, in points.
const FEATURE_COLUMN: f32 = 180.0;
/// The On column of the features table, in points (`th` width 60px in index.html:1908).
const ON_COLUMN: f32 = 64.0;

/// The log card's meta line when it has rows (index.html:1915).
pub const LOG_META: &str = "Kept with the company file. Ctrl+E exports it.";
/// The log card's meta line when it has no rows. The Ctrl+E promise does not hold then.
pub const LOG_META_EMPTY: &str = "Kept with the company file.";
/// The log card's text when it has no rows.
pub const LOG_EMPTY: &str = "No AI activity has been recorded for this company yet.";

/// The status of the cloud provider: the cloud tier is not available in this build.
pub const CLOUD_STATUS: &str = "Not available";

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

/// The option card of `tier` in the words of `tr`: title and body (index.html:1905-1907).
#[must_use]
pub fn option_in(tier: AiTier, tr: &Tr) -> (String, String) {
    let prefix = match tier {
        AiTier::Off => "ai.settings.option.off",
        AiTier::Local => "ai.settings.option.local",
        AiTier::Cloud => "ai.settings.option.cloud",
    };
    (
        tr.t(&format!("{prefix}.title")),
        tr.t(&format!("{prefix}.body")),
    )
}

/// The note while AI is off, in the words of `tr` (index.html:1061 and 1911).
#[must_use]
pub fn off_note_in(tr: &Tr) -> String {
    tr.t("ai.settings.off_note")
}

/// The note while no tier is chosen, in the words of `tr` (PARITY-GAPS).
#[must_use]
pub fn unset_note_in(tr: &Tr) -> String {
    tr.t("ai.settings.unset_note")
}

/// The "What AI may never do here" list in the words of `tr` (index.html:1916).
#[must_use]
pub fn never_in(tr: &Tr) -> [String; 5] {
    [
        tr.t("ai.settings.never.1"),
        tr.t("ai.settings.never.2"),
        tr.t("ai.settings.never.3"),
        tr.t("ai.settings.never.4"),
        tr.t("ai.settings.never.5"),
    ]
}

/// The feature rows in the words of `tr` (index.html:1908): feature, what the model sees on this
/// computer, and what is sent to the provider in the cloud tier.
#[must_use]
pub fn features_in(tr: &Tr) -> [(String, String, String); 5] {
    let row = |key: &str| {
        (
            tr.t(&format!("ai.settings.feature.{key}.name")),
            tr.t(&format!("ai.settings.feature.{key}.sees")),
            tr.t(&format!("ai.settings.feature.{key}.sent")),
        )
    };
    [
        row("bill"),
        row("plain"),
        row("bank"),
        row("ask"),
        row("mismatch"),
    ]
}

/// The key bar entries for a panel in the words of `tr` (index.html:1920). Esc is the shell's.
/// Ctrl+E is listed only when the log has rows, since it exports nothing otherwise.
#[must_use]
pub fn key_hints_in(log_has_rows: bool, tr: &Tr) -> Vec<KeyHint> {
    let mut hints = vec![
        KeyHint::new("", tr.t("ai.settings.title")),
        KeyHint::new("1", tr.t("ai.settings.key.off")),
        KeyHint::new("2", tr.t("ai.settings.key.local")),
        KeyHint::new("3", tr.t("ai.settings.key.cloud")),
    ];
    if log_has_rows {
        hints.push(KeyHint::new("Ctrl+E", tr.t("ai.settings.key.export")));
    }
    hints
}

/// The key bar entries for a panel, in English: [`key_hints_in`] with the English file.
#[must_use]
pub fn key_hints(log_has_rows: bool) -> Vec<KeyHint> {
    key_hints_in(log_has_rows, &Tr::default())
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
            && let (Some(worker), Some(path)) = (worker, save_dialog(&ctx.tr))
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
        let tr = ctx.tr.clone();
        let reply = match self.latest.poll() {
            None => return false,
            Some(Done::Lost) => {
                self.notice = Some(tr.t("ai.settings.stopped"));
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
                self.notice =
                    Some(tr.tf("ai.settings.read_failed", &[("reason", reason.as_str())]));
            }
            Reply::Chosen { tier, result } => match result {
                Ok(Outcome::Value(())) => {
                    self.stored = Some(tier);
                    self.cloud_asked = false;
                    self.cloud_reason = None;
                    self.notice = None;
                    ctx.ai_tier = Some(tier);
                    // The top bar's chip and the books are read again by the shell.
                    submit::note_changed();
                }
                Ok(Outcome::Unsupported(u)) => {
                    self.cloud_reason = Some(u.reason.clone());
                    self.notice = Some(unsupported_text(&u));
                }
                Err(reason) => self.notice = Some(reason),
            },
            Reply::Exported { path, result } => {
                self.notice = Some(match result {
                    Ok(Outcome::Value(())) => tr.tf(
                        "ai.settings.exported",
                        &[("path", path.display().to_string().as_str())],
                    ),
                    Ok(Outcome::Unsupported(u)) => unsupported_text(&u),
                    Err(reason) => reason,
                });
            }
        }
        true
    }
}

impl Screen for AiSettingsScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("ai.settings.title")
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        key_hints_in(!self.log.is_empty(), tr)
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        let tr = ctx.tr.clone();
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

        let company = ctx.books.as_ref().map_or_else(
            || tr.t("ai.settings.company_fallback"),
            |b| b.company.name.clone(),
        );
        let subtitle = tr.tf("ai.settings.subtitle", &[("company", company.as_str())]);
        page_head(ui, &tr.t("ai.settings.title"), Some(&subtitle), |_| {});
        ui.add_space(HEAD_GAP);

        if ctx.api.is_none() {
            note(ui, NoteKind::Info, &tr.t("ai.settings.open_company"));
            return ScreenAction::Stay;
        }
        if let Some(line) = &self.notice {
            note(ui, NoteKind::Warn, line);
            ui.add_space(12.0);
        }

        let panel = self.panel();
        if let Some(tier) = draw_options(ui, self.stored, &tr) {
            self.pick(ctx.api.as_ref(), tier);
        }
        ui.add_space(14.0);

        draw_panel(ui, panel, self.cloud_reason.as_deref(), &tr);
        ui.add_space(14.0);

        StripBuilder::new(ui)
            .size(Size::relative(TABLE_SHARE))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|left| {
                    draw_features(left, panel, &tr);
                    left.add_space(14.0);
                    draw_log(left, &self.log, !self.loaded, &tr);
                });
                strip.cell(|right| draw_never(right, &tr));
            });
        ScreenAction::Stay
    }
}

/// The share of the row the features table and the log take; the never card takes the rest.
const TABLE_SHARE: f32 = 0.66;

/// The system save dialog for the log export, CSV only.
fn save_dialog(tr: &Tr) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(tr.t("ai.settings.export_dialog"))
        .set_file_name("ai-activity-log.csv")
        .add_filter("CSV", &["csv"])
        .save_file()
}

pub use crate::ui::probe::{read_probe, record_probe};

/// The panel under the option cards: what the chosen tier means, or the note for no tier.
fn draw_panel(ui: &mut Ui, panel: Panel, cloud_reason: Option<&str>, tr: &Tr) {
    match panel {
        Panel::Unset => {
            note(ui, NoteKind::Info, &unset_note_in(tr));
        }
        Panel::Off => {
            note(ui, NoteKind::Info, &off_note_in(tr));
        }
        Panel::Local => draw_local(ui, tr),
        Panel::Cloud => draw_cloud(ui, cloud_reason, tr),
    }
}

/// Draws the three option cards in a row and returns the tier clicked this frame, if any.
/// The "Current" badge marks the tier stored in the company file, not the panel shown: a refused
/// cloud choice leaves the stored tier unchanged.
fn draw_options(ui: &mut Ui, stored: Option<AiTier>, tr: &Tr) -> Option<AiTier> {
    let t = tokens(ui);
    record_probe(ui, "ai-settings.accent", t.accent);
    record_probe(ui, "ai-settings.accent_soft", t.accent_soft);
    let options = [
        (AiTier::Off, Icon::Off),
        (AiTier::Local, Icon::Cpu),
        (AiTier::Cloud, Icon::Cloud),
    ];
    // The shown tier has the accent border and fill (`aria-checked` in index.html:1905-1907) and
    // a "Current" badge.
    let current_word = tr.t("ai.settings.option.current");
    let texts = options.map(|(tier, _)| option_in(tier, tr));
    let cards: Vec<OptionCard<'_>> = options
        .iter()
        .zip(texts.iter())
        .map(|((tier, glyph), (title, body))| {
            let current = stored == Some(*tier);
            OptionCard::new(*glyph, title, body)
                .selected(current)
                .badge(current.then_some(current_word.as_str()))
        })
        .collect();
    let drawn = option_grid(ui, egui::Id::unique("ai-option"), 3, &cards);
    let mut clicked = None;
    for (n, (card, (tier, _))) in drawn.iter().zip(options).enumerate() {
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
fn draw_local(ui: &mut Ui, tr: &Tr) {
    let local = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        panel_heading(ui, Icon::Cpu, &tr.t("ai.settings.local.title"));
        rule_rows(
            ui,
            &[
                (
                    tr.t("ai.settings.local.reader_label"),
                    tr.t("ai.settings.local.reader"),
                ),
                (
                    tr.t("ai.settings.local.language_label"),
                    tr.t("ai.settings.local.language"),
                ),
                (
                    tr.t("ai.settings.local.network_label"),
                    tr.t("ai.settings.local.network"),
                ),
            ],
        );
    });
    record_probe(ui, "ai-settings.local", local.response.rect);
}

/// The cloud panel: the provider is not available, with the engine's reason.
fn draw_cloud(ui: &mut Ui, reason: Option<&str>, tr: &Tr) {
    card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        panel_heading(ui, Icon::Cloud, &tr.t("ai.settings.cloud.title"));
        let why = reason.map_or_else(|| tr.t("ai.settings.cloud.why_default"), str::to_owned);
        rule_rows(
            ui,
            &[
                (
                    tr.t("ai.settings.cloud.status_label"),
                    tr.t("ai.settings.cloud.status"),
                ),
                (tr.t("ai.settings.cloud.why_label"), why),
                (
                    tr.t("ai.settings.cloud.key_label"),
                    tr.t("ai.settings.cloud.key"),
                ),
            ],
        );
    });
}

/// A panel heading: its icon, then the title in the bold family.
fn panel_heading(ui: &mut Ui, glyph: Icon, title: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        icon(ui, glyph, t.text_2);
        ui.add_space(6.0);
        ui.label(
            RichText::new(title)
                .font(bold_font(ui, TEXT_BODY))
                .color(t.text),
        );
    });
    ui.add_space(6.0);
}

/// Label and text rows across the card, a rule under each row. The label column is fixed; the
/// value takes the rest. Each cell draws its own part of the rule, so the rules join up.
fn rule_rows(ui: &mut Ui, items: &[(String, String)]) {
    let t = tokens(ui);
    table::builder(ui)
        .column(Column::exact(LABEL_COLUMN))
        .column(Column::remainder())
        .body(|mut body| {
            for (label, value) in items {
                body.row(ROW_HEIGHT, |mut row| {
                    row.col(|ui| {
                        ui.label(
                            RichText::new(label.as_str())
                                .size(TEXT_META)
                                .color(t.text_2),
                        );
                        cell_rule(ui);
                    });
                    row.col(|ui| {
                        table::text(ui, value);
                        cell_rule(ui);
                    });
                });
            }
        });
}

/// Draws the rule along the bottom of a table cell (`table.t` lines in index.html).
fn cell_rule(ui: &Ui) {
    let t = tokens(ui);
    let rect = ui.max_rect();
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, t.border),
    );
}

/// The features table: what each feature sees, and whether it runs (index.html:1908). The On
/// switch is read-only: nothing runs yet (PARITY-GAPS).
fn draw_features(ui: &mut Ui, panel: Panel, tr: &Tr) {
    let off = matches!(panel, Panel::Off | Panel::Unset);
    let cloud = panel == Panel::Cloud;
    let t = tokens(ui);
    let features = card(ui, |ui| {
        table::builder(ui)
            .column(Column::initial(FEATURE_COLUMN).at_least(160.0).clip(true))
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::exact(ON_COLUMN))
            .header(ROW_HEIGHT, |mut row| {
                table::header(&mut row, &tr.t("ai.settings.features.feature"), false);
                let second = if cloud {
                    tr.t("ai.settings.features.sent")
                } else {
                    tr.t("ai.settings.features.sees")
                };
                table::header(&mut row, &second, false);
                table::header(&mut row, &tr.t("ai.settings.features.on"), false);
            })
            .body(|mut body| {
                for (feature, sees, sent) in features_in(tr) {
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label(
                                RichText::new(feature.as_str())
                                    .size(TEXT_BODY)
                                    .color(t.text),
                            );
                            cell_rule(ui);
                        });
                        row.col(|ui| {
                            let text = if off {
                                tr.t("ai.settings.features.nothing_off")
                            } else if cloud {
                                sent
                            } else {
                                tr.tf(
                                    "ai.settings.features.on_computer",
                                    &[("sees", sees.as_str())],
                                )
                            };
                            table::text(ui, &text);
                            cell_rule(ui);
                        });
                        row.col(|ui| {
                            switch(ui, !off, Sense::hover());
                            cell_rule(ui);
                        });
                    });
                }
            });
        if !off {
            // Nothing runs in this build; the switch says the feature is on, this line says why
            // nothing happens (PARITY-GAPS).
            ui.add_space(6.0);
            ui.label(
                RichText::new(tr.t("ai.settings.features.not_built"))
                    .size(TEXT_META)
                    .color(t.text_3),
            );
        }
    });
    record_probe(ui, "ai-settings.features", features.response.rect);
}

/// The AI activity log: the file's real rows, or a line saying there are none.
fn draw_log(ui: &mut Ui, log: &[AiLogEntry], reading: bool, tr: &Tr) {
    card(ui, |ui| {
        let t = tokens(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr.t("ai.settings.log.title")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let meta = if log.is_empty() {
                    tr.t("ai.settings.log.meta_empty")
                } else {
                    tr.t("ai.settings.log.meta")
                };
                ui.label(RichText::new(meta).size(TEXT_META).color(t.text_3));
            });
        });
        if reading {
            ui.label(para_text(
                &tr.t("ai.settings.log.reading"),
                TEXT_BODY,
                t.text_2,
            ));
            return;
        }
        if log.is_empty() {
            ui.label(para_text(
                &tr.t("ai.settings.log.empty"),
                TEXT_BODY,
                t.text_2,
            ));
            return;
        }
        table::builder(ui)
            .column(Column::auto().at_least(120.0))
            .column(Column::auto().at_least(100.0))
            .column(Column::auto().at_least(110.0))
            .column(Column::remainder())
            .column(Column::auto().at_least(110.0))
            .header(ROW_HEIGHT, |mut row| {
                let headers = [
                    tr.t("ai.settings.log.when"),
                    tr.t("ai.settings.log.model"),
                    tr.t("ai.settings.log.input_hash"),
                    tr.t("ai.settings.log.output"),
                    tr.t("ai.settings.log.decision"),
                ];
                for label in &headers {
                    table::header(&mut row, label, false);
                }
            })
            .body(|mut body| {
                for entry in log {
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| table::text(ui, &when_text(entry)));
                        row.col(|ui| table::text(ui, &entry.model));
                        row.col(|ui| table::text(ui, &short_hash(&entry.input_hash)));
                        row.col(|ui| table::text(ui, &entry.output_json));
                        let decision = if entry.accepted {
                            tr.t("ai.settings.log.accepted")
                        } else {
                            tr.t("ai.settings.log.not_accepted")
                        };
                        row.col(|ui| table::text(ui, &decision));
                    });
                }
            });
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

/// The "What AI may never do here" card (index.html:1916).
fn draw_never(ui: &mut Ui, tr: &Tr) {
    let never = card(ui, |ui| {
        let t = tokens(ui);
        ui.label(RichText::new(tr.t("ai.settings.never.title")).strong());
        ui.add_space(6.0);
        for line in never_in(tr) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("•").color(t.text_2));
                ui.label(para_text(&line, TEXT_BODY, t.text_2));
            });
        }
    });
    record_probe(ui, "ai-settings.never", never.response.rect);
}
