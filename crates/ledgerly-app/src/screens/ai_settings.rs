//! AI and privacy (mock `SCREENS['ai-settings']`, index.html:1896-1920; states
//! `ai-settings.local`, `ai-settings.cloud` and `ai-settings.off`).
//!
//! The screen shows the company's AI tier as stored in the company file (`ledgerly_api::AiTier`)
//! and lets the user choose it with 1, 2 or 3 or by a click. Three rules of spec 6.8 shape it:
//!
//! * AI only drafts. The "never" list says so, and no panel shows a figure.
//! * The cloud tier needs the user's own key, kept in the key vault (the OS keychain in the real
//!   window, attached when a company opens with AI in the release). Without a vault, a described
//!   provider and a saved key the engine refuses the cloud choice (`Outcome::Unsupported`); the
//!   screen shows that reason and stores nothing. The key never reaches the company file and is
//!   never drawn, only whether it is saved.
//! * The local panel names the model file the user chose (Ctrl+O); nothing is downloaded, and this
//!   build runs no model, so the reader and language rows say "Not installed".
//! * The log is the file's real assistant events (`AiActivityRecorded`, round s21) with the
//!   decision taken on each, or else the older `AiProposalRecorded` events. When it is empty the
//!   screen says so and draws no rows; Ctrl+E is offered and exports only a log that has rows.
//! * Each feature's switch is an audited setting of the company (`Api::set_ai_feature`).
//!
//! Every difference from the mock is listed in `design/PARITY-GAPS.md`.

use crate::i18n::Tr;
use std::path::PathBuf;

use egui::{Align, Key, Layout, Modifiers, RichText, Ui};
use egui_extras::{Size, StripBuilder};
use ledgerly_api::{
    AiDecision, AiFeature, AiLogEntry, AiLogRow, AiOutput, AiSettings, AiTier, Api, KeyState,
    LocalModelFile,
};
use ledgerly_core::Outcome;

use crate::screen::KeyHint;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::submit;
use crate::theme::{TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::{card, card_flush};
use crate::ui::icon::Icon;
use crate::ui::mock_table::{self, Cell, Width, panel_title};
use crate::ui::note::{NoteKind, note};
use crate::ui::option_card::{OptionCard, option_grid};
use crate::ui::page_head::page_head;
use crate::ui::{bold_font, para, para_text, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

// The English texts of the screen, as the mock words them. The screen draws them through the
// translation files (`ai.settings.*`); the tests and the translation-file tests compare against
// these.
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
        "Nothing: an image cannot be redacted, so bills are read on this computer only",
    ),
    (
        "Plain words to draft voucher",
        "Your sentence and ledger names",
        "Same, redacted, no balances",
    ),
    (
        "Bank line suggestions",
        "Narration, date, amount",
        "Same, redacted",
    ),
    (
        "Ask your books",
        "Your question and the list of report types",
        "Same; never your figures",
    ),
    (
        "2B mismatch explanations",
        "The two records and the attached image",
        "The two records, redacted; never the image",
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

/// The log card's meta line in the words of `tr` (index.html:1915): with rows it says Ctrl+E
/// exports; without rows the Ctrl+E promise does not hold.
#[must_use]
pub fn log_meta_in(has_rows: bool, tr: &Tr) -> String {
    tr.t(if has_rows {
        "ai.settings.log.meta"
    } else {
        "ai.settings.log.meta_empty"
    })
}

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
    ];
    if crate::release::ai_enabled() {
        hints.push(KeyHint::new("2", tr.t("ai.settings.key.local")));
        hints.push(KeyHint::new("3", tr.t("ai.settings.key.cloud")));
    }
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
    /// The stored tier and the log, read together, with the AI settings and the assistant's log
    /// rows when the release carries AI.
    Loaded(Result<Box<Loaded>, String>),
    /// The answer to choosing a local model file.
    ModelFileChosen(Result<Outcome<()>, String>),
    /// The answer to switching a feature on or off.
    FeatureSwitched(Result<Outcome<()>, String>),
    /// The answer to saving the provider and the key from the form.
    ProviderSaved(Result<Outcome<()>, String>),
    /// The answer to deleting the key from the form.
    KeyDeleted(Result<Outcome<()>, String>),
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

/// What one read of the company gives the screen.
#[derive(Debug)]
struct Loaded {
    /// The stored tier.
    tier: Option<AiTier>,
    /// The older proposal log (`AiProposalRecorded`).
    log: Vec<AiLogEntry>,
    /// The model file, the cloud provider and the key state; `None` when AI is out of the
    /// release, so nothing is read for it.
    settings: Option<AiSettings>,
    /// The assistant's activity rows; empty when AI is out of the release.
    rows: Vec<AiLogRow>,
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
    /// The AI settings, once read (only when the release carries AI).
    settings: Option<AiSettings>,
    /// The assistant's activity rows read from the company file.
    rows: Vec<AiLogRow>,
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
    /// The provider/key form, while it is open (mock `ai.ai-settings.cloud-edit`).
    form: Option<ProviderForm>,
}

/// The provider/key form's fields as typed. The key field starts empty and is never filled from
/// the vault; an empty key on save keeps the saved one.
#[derive(Default)]
struct ProviderForm {
    /// The request format, one of [`ledgerly_api::CLOUD_FORMATS`].
    format: String,
    /// The endpoint (Messages) or base URL (chat) as typed.
    endpoint: String,
    /// The model for bill reading, plain words and other features, as typed.
    models: [String; 3],
    /// The key as typed, until a save moves it to the worker.
    key: String,
    /// Why the last save was refused.
    error: Option<String>,
}

impl std::fmt::Debug for ProviderForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The key is never printed, not even in a debug dump.
        f.debug_struct("ProviderForm")
            .field("format", &self.format)
            .field("endpoint", &self.endpoint)
            .field("models", &self.models)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
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

    /// Whether a call to the company is in flight.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.latest.is_pending()
    }

    /// Whether the provider/key form is open.
    #[must_use]
    pub fn editing_provider(&self) -> bool {
        self.form.is_some()
    }

    /// Fills the provider/key form as if typed; does nothing when the form is closed.
    /// `models` are the models for bill reading, plain words and other features.
    pub fn fill_provider_form(
        &mut self,
        format: &str,
        endpoint: &str,
        models: [&str; 3],
        key: &str,
    ) {
        if let Some(form) = self.form.as_mut() {
            format.clone_into(&mut form.format);
            endpoint.clone_into(&mut form.endpoint);
            form.models = models.map(str::to_owned);
            key.clone_into(&mut form.key);
        }
    }

    /// Opens the provider/key form from the stored provider, with the key field empty: the key
    /// is never read back from the vault.
    fn open_form(&mut self) {
        let cloud = self
            .settings
            .as_ref()
            .map_or_else(ledgerly_api::CloudSettings::default, |s| s.cloud.clone());
        self.form = Some(ProviderForm {
            format: cloud
                .format
                .unwrap_or_else(|| ledgerly_api::CLOUD_FORMATS[0].to_owned()),
            endpoint: cloud.endpoint.unwrap_or_default(),
            models: [cloud.bill_model, cloud.words_model, cloud.model]
                .map(Option::unwrap_or_default),
            key: String::new(),
            error: None,
        });
    }

    /// Starts storing the provider and, when one was typed, saving the key in the vault. The
    /// typed key is moved out of the form, so it is held only until the vault has it.
    fn start_save(&mut self, worker: &ApiWorker) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let given = |text: &str| {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        };
        let cloud = ledgerly_api::CloudSettings {
            endpoint: given(&form.endpoint),
            format: given(&form.format),
            model: given(&form.models[2]),
            bill_model: given(&form.models[0]),
            words_model: given(&form.models[1]),
        };
        let key = std::mem::take(&mut form.key);
        form.error = None;
        self.latest.start(worker, move |api: &mut Api| {
            Reply::ProviderSaved(save_provider(api, &cloud, &key).map_err(|e| e.to_string()))
        });
    }

    /// Takes the answer to a save or a delete: done closes the form with `done_line`; a refusal
    /// stays in the form. The settings are read again either way.
    fn form_answer(&mut self, result: Result<Outcome<()>, String>, done_line: &str) {
        let refused = match result {
            Ok(Outcome::Value(())) => {
                self.form = None;
                self.show_notice(done_line);
                None
            }
            Ok(Outcome::Unsupported(u)) => Some(unsupported_text(&u)),
            Err(reason) => Some(reason),
        };
        if let (Some(line), Some(form)) = (refused, self.form.as_mut()) {
            form.error = Some(line);
        }
        self.loaded = false;
    }

    /// Acts on what the form's or the panel's buttons asked for.
    fn form_asked(&mut self, asked: FormAsk, worker: Option<&ApiWorker>) {
        let worker = worker.filter(|_| !self.latest.is_pending());
        match (asked, worker) {
            (FormAsk::Open, _) => self.open_form(),
            (FormAsk::Cancel, _) => self.form = None,
            (FormAsk::Save, Some(worker)) => self.start_save(worker),
            (FormAsk::Delete, Some(worker)) => self.start_delete_key(worker),
            (FormAsk::Nothing, _) | (FormAsk::Save | FormAsk::Delete, None) => {}
        }
    }

    /// Starts deleting the saved key from the vault.
    fn start_delete_key(&mut self, worker: &ApiWorker) {
        self.latest.start(worker, |api: &mut Api| {
            Reply::KeyDeleted(api.delete_cloud_key().map_err(|e| e.to_string()))
        });
    }

    /// The form's keys: Ctrl+A saves, Alt+D deletes the key, Esc closes the form. Read before
    /// the form's fields are drawn, so a focused field does not take them.
    fn read_form_keys(&mut self, ui: &Ui, worker: Option<&ApiWorker>) {
        let (save, delete, close) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::A),
                i.consume_key(Modifiers::ALT, Key::D),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if close {
            self.form = None;
            return;
        }
        let Some(worker) = worker else { return };
        if self.latest.is_pending() {
            return;
        }
        if save {
            self.start_save(worker);
        } else if delete {
            self.start_delete_key(worker);
        }
    }

    /// The features switched off, once the AI settings are read (only with AI in the release).
    #[must_use]
    pub fn features_off(&self) -> Option<&[AiFeature]> {
        self.settings.as_ref().map(|s| s.features_off.as_slice())
    }

    /// Starts switching `feature` on or off; the settings are read again when it is stored.
    fn switch_feature(&mut self, worker: &ApiWorker, feature: AiFeature, on: bool) {
        self.latest.start(worker, move |api: &mut Api| {
            Reply::FeatureSwitched(api.set_ai_feature(feature, on).map_err(|e| e.to_string()))
        });
    }

    /// The assistant's activity rows read from the company file.
    #[must_use]
    pub fn activity_rows(&self) -> &[AiLogRow] {
        &self.rows
    }

    /// Whether the log card has anything to export.
    fn log_has_rows(&self) -> bool {
        !self.log.is_empty() || !self.rows.is_empty()
    }

    /// Starts reading the stored tier and the log, and, when the release carries AI, the AI
    /// settings and the assistant's rows. The release switch is the UI thread's, so it is read
    /// here and handed to the worker.
    fn start_load(&mut self, worker: &ApiWorker) {
        let with_ai = crate::release::ai_enabled();
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Loaded(
                read_all_settings(api, with_ai)
                    .map(Box::new)
                    .map_err(|e| e.to_string()),
            )
        });
    }

    /// Starts storing `path` as the local model file. Only the path is stored; nothing is read
    /// from the file but its size, and nothing is downloaded.
    fn choose_model_file(&mut self, worker: &ApiWorker, path: PathBuf) {
        self.latest.start(worker, move |api: &mut Api| {
            Reply::ModelFileChosen(
                api.set_local_model_file(Some(&path))
                    .map_err(|e| e.to_string()),
            )
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
        // The assistant's rows when there are any (the later release), else the older log.
        let activity = !self.rows.is_empty();
        self.latest.start(worker, move |api: &mut Api| {
            let result = if activity {
                api.export_ai_activity(&path)
            } else {
                api.export_ai_log(&path)
            }
            .map_err(|e| e.to_string());
            Reply::Exported { path, result }
        });
    }

    /// Chooses `tier` from a key or a click. Cloud also shows the cloud panel at once.
    /// The keys this screen answers: 1, 2 and 3 choose a tier; Ctrl+E exports the log when it
    /// has rows.
    fn read_keys(&mut self, ui: &Ui, ctx: &AppCtx) {
        if self.form.is_some() {
            self.read_form_keys(ui, ctx.api.as_ref());
            return;
        }
        if crate::release::ai_enabled()
            && self.can_edit_provider()
            && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter))
        {
            self.open_form();
            return;
        }
        let (one, two, three, export, model_file) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Num1),
                i.consume_key(Modifiers::NONE, Key::Num2),
                i.consume_key(Modifiers::NONE, Key::Num3),
                i.consume_key(Modifiers::COMMAND, Key::E),
                i.consume_key(Modifiers::COMMAND, Key::O),
            )
        });
        let worker = ctx.api.as_ref();
        if !crate::release::ai_enabled() {
            // Keys 1, 2 and 3 choose a tier; with no AI in this release the company stays Off.
            // The keys are still consumed, so no other screen reads them.
            let _ = (one, two, three);
        } else if one {
            self.pick(worker, AiTier::Off);
        } else if two {
            self.pick(worker, AiTier::Local);
        } else if three {
            self.pick(worker, AiTier::Cloud);
        }
        if export
            && self.log_has_rows()
            && let (Some(worker), Some(path)) = (worker, save_dialog(&ctx.tr))
        {
            self.export(worker, path);
        }
        if model_file
            && crate::release::ai_enabled()
            && self.panel() == Panel::Local
            && let (Some(worker), Some(path)) = (worker, model_dialog(&ctx.tr))
        {
            self.choose_model_file(worker, path);
        }
    }

    /// Whether the cloud panel shows the provider with a vault behind it, so the form can save.
    fn can_edit_provider(&self) -> bool {
        self.panel() == Panel::Cloud
            && self
                .settings
                .as_ref()
                .is_some_and(|s| s.key != KeyState::NoVault)
    }

    fn pick(&mut self, worker: Option<&ApiWorker>, tier: AiTier) {
        if !crate::release::ai_enabled() {
            // The company stays Off: nothing to store, and no card to pick.
            return;
        }
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
                self.show_notice(&tr.t("ai.settings.stopped"));
                return true;
            }
            Some(Done::Ready(reply)) => reply,
        };
        match reply {
            Reply::Loaded(Ok(loaded)) => {
                // A file stored with Local by an older build opens, and shows Off, while the
                // release carries no AI (see `crate::release`).
                let tier = crate::release::effective_tier(loaded.tier);
                self.stored = tier;
                self.log = loaded.log;
                self.settings = loaded.settings;
                self.rows = loaded.rows;
                self.loaded = true;
                ctx.ai_tier = tier;
            }
            Reply::Loaded(Err(reason)) => {
                self.loaded = true;
                self.show_notice(&tr.tf("ai.settings.read_failed", &[("reason", &reason)]));
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
            Reply::FeatureSwitched(result) => {
                match result {
                    Ok(Outcome::Value(())) => {}
                    Ok(Outcome::Unsupported(u)) => self.show_notice(&unsupported_text(&u)),
                    Err(reason) => self.show_notice(&reason),
                }
                self.loaded = false;
            }
            Reply::ProviderSaved(result) => {
                self.form_answer(result, &tr.t("ai.settings.form.saved"));
            }
            Reply::KeyDeleted(result) => {
                self.form_answer(result, &tr.t("ai.settings.form.deleted"));
            }
            Reply::ModelFileChosen(result) => {
                match result {
                    Ok(Outcome::Value(())) => self.notice = None,
                    Ok(Outcome::Unsupported(u)) => self.show_notice(&unsupported_text(&u)),
                    Err(reason) => self.show_notice(&reason),
                }
                // Read the settings again, so the row shows the file as it is on disk.
                self.loaded = false;
            }
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
    fn title(&self, tr: &Tr) -> String {
        tr.t("ai.settings.title")
    }

    fn key_bar_name(&self, tr: &Tr) -> String {
        if self.form.is_some() {
            tr.t("ai.settings.form.keys")
        } else {
            tr.t("ai.settings.title")
        }
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        if self.form.is_some() {
            // The form's own bar (mock `ai.ai-settings.cloud-edit`); the tier keys are off while
            // a field may be typed in.
            return vec![
                KeyHint::new("", tr.t("ai.settings.form.keys")),
                KeyHint::new("Ctrl+A", tr.t("ai.settings.form.save")),
                KeyHint::new("Alt+D", tr.t("ai.settings.form.delete")),
                KeyHint::new("Esc", tr.t("ai.settings.form.cancel")),
            ];
        }
        let mut hints = key_hints_in(self.log_has_rows(), tr);
        if crate::release::ai_enabled() && self.can_edit_provider() {
            let at = hints
                .iter()
                .position(|h| h.keys == "Ctrl+E")
                .unwrap_or(hints.len());
            hints.insert(
                at,
                KeyHint::new("Ctrl+Enter", tr.t("ai.settings.key.change")),
            );
        }
        if crate::release::ai_enabled() && self.panel() == Panel::Local {
            // The mock lists Ctrl+O after the tiers and before the export (index.html:2106).
            let at = hints
                .iter()
                .position(|h| h.keys == "Ctrl+E")
                .unwrap_or(hints.len());
            hints.insert(
                at,
                KeyHint::new("Ctrl+O", tr.t("ai.settings.key.model_file")),
            );
        }
        hints
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
        let subtitle = tr.tf("ai.settings.subtitle", &[("company", &company)]);
        // `page_head` ends with the head's own 16 point margin (index.html:121).
        let head = page_head(ui, &tr.t("ai.settings.title"), Some(&subtitle), |_| {});
        record_probe(ui, "ai-settings.head", head);

        if ctx.api.is_none() {
            note(ui, NoteKind::Info, &tr.t("ai.settings.open_company"));
            return ScreenAction::Stay;
        }
        if let Some(line) = self.banner() {
            let drawn = note(ui, NoteKind::Warn, line);
            record_probe(ui, "ai-settings.banner", drawn.rect);
            gap(ui, 12.0);
        }

        let panel = self.panel();
        if let Some(tier) = draw_options(ui, panel, self.stored, &tr) {
            self.pick(ctx.api.as_ref(), tier);
        }
        if !crate::release::ai_enabled() {
            // The one plain line under the cards: AI comes in a later version.
            gap(ui, 6.0);
            let color = tokens(ui).text_2;
            para(ui, &tr.t("ai.settings.later_note"), TEXT_BODY, color);
        }
        gap(ui, 14.0);

        let mut asked = FormAsk::Nothing;
        if let Some(form) = self.form.as_mut() {
            asked = draw_provider_form(ui, form, &tr);
            gap(ui, 14.0);
        } else if draw_panel(
            ui,
            panel,
            self.settings.as_ref(),
            self.cloud_reason.as_deref(),
            &tr,
        ) {
            asked = FormAsk::Open;
        }
        self.form_asked(asked, ctx.api.as_ref());

        let switched = draw_main_row(
            ui,
            panel,
            self.features_off(),
            &self.log,
            &self.rows,
            !self.loaded,
            &tr,
        );
        if let (Some((feature, on)), Some(worker)) = (switched, ctx.api.as_ref())
            && !self.latest.is_pending()
        {
            self.switch_feature(worker, feature, on);
        }
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
fn save_dialog(tr: &Tr) -> Option<PathBuf> {
    crate::ui::dialogs::save_file(
        &tr.t("ai.settings.export_dialog"),
        Some("ai-activity-log.csv"),
        &[("CSV", &["csv"])],
    )
}

/// The system open dialog for the local model file. Only the path is kept; nothing is copied or
/// downloaded.
fn model_dialog(tr: &Tr) -> Option<PathBuf> {
    crate::ui::dialogs::open_file(&tr.t("ai.settings.model_dialog"), &[])
}

/// Reads the stored tier and the older log, and with `with_ai` the AI settings and the
/// assistant's log rows.
fn read_all_settings(api: &Api, with_ai: bool) -> Result<Loaded, ledgerly_api::ApiError> {
    let tier = api.ai_tier()?;
    let log = api.ai_log()?;
    let (settings, rows) = if with_ai {
        (Some(api.ai_settings()?), api.ai_log_rows()?)
    } else {
        (None, Vec::new())
    };
    Ok(Loaded {
        tier,
        log,
        settings,
        rows,
    })
}

pub use crate::ui::probe::{read_probe, record_probe};

/// The panel under the option cards: what the chosen tier means, or the note for no tier.
/// Returns whether "Change provider or key" was clicked.
fn draw_panel(
    ui: &mut Ui,
    panel: Panel,
    settings: Option<&AiSettings>,
    cloud_reason: Option<&str>,
    tr: &Tr,
) -> bool {
    let mut change = false;
    match panel {
        Panel::Unset => {
            note(ui, NoteKind::Info, &unset_note_in(tr));
            gap(ui, 14.0);
        }
        Panel::Off => {
            note(ui, NoteKind::Info, &off_note_in(tr));
            gap(ui, 14.0);
        }
        Panel::Local => {
            draw_local(ui, settings.map(|s| &s.model_file), tr);
            gap(ui, 14.0);
        }
        Panel::Cloud => {
            match settings {
                Some(settings) if settings.key != KeyState::NoVault => {
                    change = draw_cloud_provider(ui, settings, tr);
                }
                Some(_) | None => draw_cloud(ui, cloud_reason, tr),
            }
            gap(ui, 14.0);
        }
    }
    change
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
fn draw_options(ui: &mut Ui, panel: Panel, stored: Option<AiTier>, tr: &Tr) -> Option<AiTier> {
    let t = tokens(ui);
    record_probe(ui, "ai-settings.accent", t.accent);
    record_probe(ui, "ai-settings.accent_soft", t.accent_soft);
    let options = [
        (AiTier::Off, Icon::Off),
        (AiTier::Local, Icon::Cpu),
        (AiTier::Cloud, Icon::Cloud),
    ];
    let shown = shown_tier(panel);
    let current_word = tr.t("ai.settings.option.current");
    let words = options.map(|(tier, _)| option_in(tier, tr));
    let ai_in_release = crate::release::ai_enabled();
    let not_in_release = tr.t("ai.settings.not_in_release");
    let cards: Vec<OptionCard<'_>> = options
        .iter()
        .zip(&words)
        .map(|(&(tier, glyph), (name, note))| {
            if !ai_in_release && tier != AiTier::Off {
                // Local and Cloud are shown, disabled and tagged, so the choice is visible but
                // cannot be made (index.html, AI_IN_RELEASE false).
                return OptionCard::new(glyph, name, note)
                    .disabled()
                    .badge(Some(not_in_release.as_str()));
            }
            let current_elsewhere = stored == Some(tier) && shown != Some(tier);
            OptionCard::new(glyph, name, note)
                .selected(shown == Some(tier))
                .badge(current_elsewhere.then_some(current_word.as_str()))
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
fn draw_local(ui: &mut Ui, model_file: Option<&LocalModelFile>, tr: &Tr) {
    let local = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        panel_title(ui, Some(Icon::Cpu), &tr.t("ai.settings.local.title"));
        let file_row = model_file.map(|file| {
            (
                tr.t("ai.settings.local.file_label"),
                model_file_text(file, tr),
            )
        });
        let words = [
            ("reader_label", "reader"),
            ("language_label", "language"),
            ("network_label", "network"),
        ]
        .map(|(label, value)| {
            (
                tr.t(&format!("ai.settings.local.{label}")),
                tr.t(&format!("ai.settings.local.{value}")),
            )
        });
        let rows = file_row
            .iter()
            .chain(words.iter())
            .map(|(label, value)| vec![mock_table::muted(label), mock_table::text(value)])
            .collect();
        mock_table::table(
            ui,
            &[Width::Fixed(LOCAL_LABEL_COLUMN), Width::Rest],
            None,
            rows,
        );
    });
    record_probe(ui, "ai-settings.local", local.response.rect);
}

/// The Model file row's text: none chosen, the file and its size, or a file that is gone.
fn model_file_text(file: &LocalModelFile, tr: &Tr) -> String {
    match file {
        LocalModelFile::NotChosen => tr.t("ai.settings.local.file_none"),
        LocalModelFile::Found { path, bytes } => tr.tf(
            "ai.settings.local.file_found",
            &[
                ("path", &path.display().to_string()),
                ("bytes", &bytes.to_string()),
            ],
        ),
        LocalModelFile::Missing(path) => tr.tf(
            "ai.settings.local.file_missing",
            &[("path", &path.display().to_string())],
        ),
    }
}

/// The request format in words: the two formats the cloud client speaks get the mock's words;
/// anything else the user typed is shown as typed.
fn format_text(format: &str, tr: &Tr) -> String {
    match format {
        "chat-completions" => tr.t("ai.settings.cloud.format_chat"),
        "messages" => tr.t("ai.settings.cloud.format_messages"),
        other => other.to_owned(),
    }
}

/// The Key row's words and badge: saved, not saved, or the vault could not be read.
fn key_words(key: &KeyState, tr: &Tr) -> (String, Option<(BadgeKind, String)>) {
    match key {
        KeyState::Set => (
            tr.t("ai.settings.cloud.key_stored"),
            Some((BadgeKind::Ok, tr.t("ai.settings.cloud.key_set"))),
        ),
        KeyState::NotSet => (
            tr.t("ai.settings.cloud.key_missing"),
            Some((BadgeKind::Warn, tr.t("ai.settings.cloud.key_not_set"))),
        ),
        KeyState::Unreadable(reason) => (
            tr.tf("ai.settings.cloud.key_unreadable", &[("reason", reason)]),
            None,
        ),
        KeyState::NoVault => (tr.t("ai.settings.cloud.key"), None),
    }
}

/// The cloud panel once a key vault is attached (index.html:2098): the provider as the user
/// described it and whether the key is saved, beside the Removed before sending card. The key
/// itself is never read here, only whether it is there.
/// Returns whether "Change provider or key" was clicked.
fn draw_cloud_provider(ui: &mut Ui, settings: &AiSettings, tr: &Tr) -> bool {
    let mut change = false;
    let plain = provider_rows(&settings.cloud, tr);
    let (key_text, key_badge) = key_words(&settings.key, tr);
    let labels =
        ["key_label", "ask_label", "region_label"].map(|k| tr.t(&format!("ai.settings.cloud.{k}")));
    let ask = tr.t("ai.settings.cloud.ask");
    let region = tr.t("ai.settings.cloud.region");
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = GRID_GAP;
        ui.columns(2, |cols| {
            let [left, right] = cols else { return };
            let provider = card(left, |ui| {
                ui.set_min_width(ui.available_width());
                panel_title(ui, Some(Icon::Cloud), &tr.t("ai.settings.cloud.title"));
                let key_cell: Cell<'_> = Box::new(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&key_text).size(TEXT_BODY));
                        if let Some((kind, word)) = &key_badge {
                            badge(ui, *kind, word);
                        }
                    });
                });
                let ask_cell: Cell<'_> = Box::new(|ui| {
                    ui.horizontal(|ui| {
                        mock_table::switch_cell(true, &ask)(ui);
                        ui.label(RichText::new(&ask).size(TEXT_BODY));
                    });
                });
                let mut rows: Vec<Vec<Cell<'_>>> = plain
                    .iter()
                    .map(|(label, value)| vec![mock_table::muted(label), mock_table::text(value)])
                    .collect();
                rows.push(vec![mock_table::muted(&labels[0]), key_cell]);
                rows.push(vec![mock_table::muted(&labels[1]), ask_cell]);
                rows.push(vec![
                    mock_table::muted(&labels[2]),
                    mock_table::muted(&region),
                ]);
                mock_table::table(
                    ui,
                    &[Width::Fixed(PROVIDER_LABEL_COLUMN), Width::Rest],
                    None,
                    rows,
                );
                ui.add_space(10.0);
                change = button(
                    ui,
                    &tr.t("ai.settings.form.title"),
                    Some("Ctrl+Enter"),
                    ButtonKind::Secondary,
                )
                .clicked();
            });
            record_probe(left, "ai-settings.provider", provider.response.rect);
            let removed = removed_card(right, tr);
            record_probe(right, "ai-settings.removed", removed);
        });
    });
    change
}

/// The provider's rows as the API resolves them: the format, where requests go, and the model
/// each feature is sent to (the owner's defaults with the Messages format when none is named).
fn provider_rows(cloud: &ledgerly_api::CloudSettings, tr: &Tr) -> Vec<(String, String)> {
    let not_described = tr.t("ai.settings.cloud.not_described");
    let or_not = |v: Option<String>| v.unwrap_or_else(|| not_described.clone());
    vec![
        (
            tr.t("ai.settings.cloud.format_label"),
            or_not(cloud.format.as_deref().map(|f| format_text(f, tr))),
        ),
        (
            tr.t("ai.settings.cloud.url_label"),
            or_not(ledgerly_api::cloud_request_url(cloud)),
        ),
        (
            tr.t("ai.settings.cloud.bill_model_label"),
            or_not(ledgerly_api::cloud_model_for(cloud, AiFeature::BillReading)),
        ),
        (
            tr.t("ai.settings.cloud.words_model_label"),
            or_not(ledgerly_api::cloud_model_for(cloud, AiFeature::PlainWords)),
        ),
        (
            tr.t("ai.settings.cloud.other_model_label"),
            or_not(ledgerly_api::cloud_model_for(cloud, AiFeature::Ask)),
        ),
    ]
}

/// The Removed before sending card; returns its rectangle.
fn removed_card(ui: &mut Ui, tr: &Tr) -> egui::Rect {
    let removed_lines = removed_in(tr);
    card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        panel_title(ui, Some(Icon::Shield), &tr.t("ai.settings.cloud.removed"));
        let rows = removed_lines
            .iter()
            .map(|line| vec![mock_table::text(line), mock_table::switch_cell(true, line)])
            .collect();
        mock_table::table(ui, &[Width::Rest, Width::Fixed(ON_COLUMN)], None, rows);
    })
    .response
    .rect
}

/// What the provider form's buttons asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormAsk {
    /// Nothing was clicked.
    Nothing,
    /// "Change provider or key" on the provider card.
    Open,
    /// Save (Ctrl+A).
    Save,
    /// Delete key (Alt+D).
    Delete,
    /// Cancel (Esc).
    Cancel,
}

/// The provider/key form (mock `ai.ai-settings.cloud-edit`): endpoint, request format, model
/// and a masked key field, beside the Removed before sending card.
fn draw_provider_form(ui: &mut Ui, form: &mut ProviderForm, tr: &Tr) -> FormAsk {
    let mut asked = FormAsk::Nothing;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = GRID_GAP;
        ui.columns(2, |cols| {
            let [left, right] = cols else { return };
            let drawn = card(left, |ui| {
                ui.set_min_width(ui.available_width());
                panel_title(ui, Some(Icon::Cloud), &tr.t("ai.settings.form.title"));
                form_fields(ui, form, tr);
                if let Some(error) = &form.error {
                    ui.add_space(6.0);
                    note(ui, NoteKind::Bad, error);
                }
                ui.add_space(10.0);
                asked = form_buttons(ui, tr);
            });
            record_probe(left, "ai-settings.form", drawn.response.rect);
            let _removed = removed_card(right, tr);
        });
    });
    asked
}

/// The form's fields: the request format, the endpoint or base URL, a model per feature and
/// the masked key.
fn form_fields(ui: &mut Ui, form: &mut ProviderForm, tr: &Tr) {
    let t = tokens(ui);
    let width = ui.available_width();
    let label = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).size(TEXT_META).color(t.text_2));
    };
    let meta = |ui: &mut Ui, text: &str| {
        ui.add(egui::Label::new(para_text(text, TEXT_META, t.text_2)).wrap());
    };
    label(ui, &tr.t("ai.settings.form.format_label"));
    egui::ComboBox::from_id_salt("ai-provider-format")
        .selected_text(format_text(&form.format, tr))
        .width(width)
        .show_ui(ui, |ui| {
            for format in ledgerly_api::CLOUD_FORMATS {
                let on = form.format == format;
                if ui.selectable_label(on, format_text(format, tr)).clicked() {
                    format.clone_into(&mut form.format);
                }
            }
        });
    ui.add_space(6.0);
    let messages = form.format == ledgerly_api::CLOUD_FORMATS[1];
    let endpoint_label = if messages {
        tr.t("ai.settings.cloud.endpoint_label")
    } else {
        tr.t("ai.settings.form.base_url_label")
    };
    label(ui, &endpoint_label);
    ui.add(egui::TextEdit::singleline(&mut form.endpoint).desired_width(width));
    meta(ui, &tr.t("ai.settings.form.url_note"));
    ui.add_space(6.0);
    let model_labels = ["bill_model_label", "words_model_label", "other_model_label"];
    for (n, (field, key)) in form.models.iter_mut().zip(model_labels).enumerate() {
        label(ui, &tr.t(&format!("ai.settings.form.{key}")));
        let hint = if n < 2 {
            tr.t("ai.settings.form.model_hint")
        } else {
            String::new()
        };
        ui.add(
            egui::TextEdit::singleline(field)
                .hint_text(hint)
                .desired_width(width),
        );
    }
    meta(ui, &tr.t("ai.settings.form.defaults_note"));
    ui.add_space(6.0);
    label(ui, &tr.t("ai.settings.cloud.key_label"));
    ui.add(
        egui::TextEdit::singleline(&mut form.key)
            .password(true)
            .hint_text(tr.t("ai.settings.form.key_hint"))
            .desired_width(width),
    );
    ui.add_space(6.0);
    meta(ui, &tr.t("ai.settings.form.key_note"));
}

/// Save, Delete key and Cancel; returns the one clicked.
fn form_buttons(ui: &mut Ui, tr: &Tr) -> FormAsk {
    let mut asked = FormAsk::Nothing;
    ui.horizontal_wrapped(|ui| {
        for (word, key, kind, ask) in [
            (
                "ai.settings.form.save",
                "Ctrl+A",
                ButtonKind::Primary,
                FormAsk::Save,
            ),
            (
                "ai.settings.form.delete",
                "Alt+D",
                ButtonKind::Danger,
                FormAsk::Delete,
            ),
            (
                "ai.settings.form.cancel",
                "Esc",
                ButtonKind::Secondary,
                FormAsk::Cancel,
            ),
        ] {
            if button(ui, &tr.t(word), Some(key), kind).clicked() {
                asked = ask;
            }
        }
    });
    asked
}

/// Stores the provider, then saves the key when one was typed (an empty key keeps the saved
/// one). A refused provider stops before the key is touched.
fn save_provider(
    api: &mut Api,
    cloud: &ledgerly_api::CloudSettings,
    key: &str,
) -> Result<Outcome<()>, ledgerly_api::ApiError> {
    if let Outcome::Unsupported(u) = api.set_cloud_settings(cloud)? {
        return Ok(Outcome::Unsupported(u));
    }
    if key.trim().is_empty() {
        return Ok(Outcome::Value(()));
    }
    api.save_cloud_key(key)
}

/// The cloud panel (index.html:1912): the Provider card and the Removed before sending card, side
/// by side. The provider is not available in this build, and the Provider card says why in the
/// mock's place for the key status.
fn draw_cloud(ui: &mut Ui, reason: Option<&str>, tr: &Tr) {
    let removed_lines = removed_in(tr);
    let why_default = tr.t("ai.settings.cloud.why_default");
    let status_word = tr.t("ai.settings.cloud.status");
    let status_label = tr.t("ai.settings.cloud.status_label");
    let why_label = tr.t("ai.settings.cloud.why_label");
    let key_label = tr.t("ai.settings.cloud.key_label");
    let key_text = tr.t("ai.settings.cloud.key");
    let region_label = tr.t("ai.settings.cloud.region_label");
    let region_text = tr.t("ai.settings.cloud.region");
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
                panel_title(ui, Some(Icon::Cloud), &tr.t("ai.settings.cloud.title"));
                let status: Cell<'_> = Box::new(|ui| {
                    badge(ui, BadgeKind::Warn, &status_word);
                });
                let rows = vec![
                    vec![mock_table::muted(&status_label), status],
                    vec![
                        mock_table::muted(&why_label),
                        mock_table::text(reason.unwrap_or(&why_default)),
                    ],
                    vec![mock_table::muted(&key_label), mock_table::text(&key_text)],
                    vec![
                        mock_table::muted(&region_label),
                        mock_table::muted(&region_text),
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
                panel_title(ui, Some(Icon::Shield), &tr.t("ai.settings.cloud.removed"));
                let rows = removed_lines
                    .iter()
                    .map(|line| vec![mock_table::text(line), mock_table::switch_cell(true, line)])
                    .collect();
                mock_table::table(ui, &[Width::Rest, Width::Fixed(ON_COLUMN)], None, rows);
                let t = tokens(ui);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(tr.t("ai.settings.features.not_built"))
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

/// What redaction takes out before anything is sent (index.html:1912). The redaction module
/// recognises PAN, GSTIN, Aadhaar, bank account numbers, IFSC codes, phone numbers and email
/// addresses, and the card names exactly those.
fn removed_in(tr: &Tr) -> [String; 4] {
    [
        tr.t("ai.settings.removed.1"),
        tr.t("ai.settings.removed.2"),
        tr.t("ai.settings.removed.3"),
        tr.t("ai.settings.removed.4"),
    ]
}

/// The main row (`.g-main`, index.html:1918): the features table over the log on the left, the
/// never card on the right.
/// Returns the feature whose switch was clicked this frame and the state it was switched to.
fn draw_main_row(
    ui: &mut Ui,
    panel: Panel,
    features_off: Option<&[AiFeature]>,
    log: &[AiLogEntry],
    rows: &[AiLogRow],
    reading: bool,
    tr: &Tr,
) -> Option<(AiFeature, bool)> {
    let mut switched = None;
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
                    switched = draw_features(col, panel, features_off, tr);
                    gap(col, 12.0);
                    if rows.is_empty() {
                        draw_log(col, log, reading, tr);
                    } else {
                        draw_activity(col, rows, tr);
                    }
                });
                strip.empty();
                strip.cell(|col| draw_never(col, tr));
            });
    });
    switched
}

/// The features table: what each feature sees, and whether it runs (index.html:1908). The On
/// switch is read-only: nothing runs yet (PARITY-GAPS).
/// With the AI settings read (`features_off` is `Some`) and a tier on, each switch can be clicked;
/// the feature clicked and its new state are returned.
fn draw_features(
    ui: &mut Ui,
    panel: Panel,
    features_off: Option<&[AiFeature]>,
    tr: &Tr,
) -> Option<(AiFeature, bool)> {
    let off = matches!(panel, Panel::Off | Panel::Unset);
    let mut switched = None;
    let cloud = panel == Panel::Cloud;
    let t = tokens(ui);
    let features = card_flush(ui, |ui| {
        let second = if cloud {
            &tr.t("ai.settings.features.sent")
        } else {
            &tr.t("ai.settings.features.sees")
        };
        let features_words = features_in(tr);
        let texts: Vec<String> = features_words
            .iter()
            .map(|(_, sees, sent)| {
                if off {
                    tr.t("ai.settings.features.nothing_off")
                } else if cloud {
                    sent.clone()
                } else {
                    tr.tf("ai.settings.features.on_computer", &[("sees", sees)])
                }
            })
            .collect();
        let clicked = std::cell::Cell::new(None);
        let rows = features_words
            .iter()
            .zip(&texts)
            .zip(AiFeature::ALL)
            .enumerate()
            .map(|(n, (((name, _, _), text), feature))| {
                let switch: Cell<'_> = match features_off {
                    Some(list) if !off => {
                        let on = !list.contains(&feature);
                        let clicked = &clicked;
                        Box::new(move |ui: &mut Ui| {
                            let response =
                                crate::ui::switch::switch(ui, on, egui::Sense::click(), name);
                            record_probe(ui, &format!("ai-settings.switch.{n}"), response.rect);
                            if response.clicked() {
                                clicked.set(Some((feature, !on)));
                            }
                        })
                    }
                    Some(_) | None => mock_table::switch_cell(!off, name),
                };
                vec![mock_table::text(name), mock_table::muted(text), switch]
            })
            .collect();
        mock_table::table(
            ui,
            &[
                Width::Fixed(FEATURE_COLUMN),
                Width::Rest,
                Width::Fixed(ON_COLUMN),
            ],
            Some(&[
                &tr.t("ai.settings.features.feature"),
                second,
                &tr.t("masters.field.on"),
            ]),
            rows,
        );
        switched = clicked.get();
        if !off {
            // Nothing runs in this build; the switch says the feature is on, this line says why
            // nothing happens (PARITY-GAPS).
            ui.horizontal(|ui| {
                ui.add_space(crate::ui::table::CELL_PAD_X);
                ui.label(
                    RichText::new(tr.t("ai.settings.features.not_built"))
                        .size(TEXT_META)
                        .color(t.text_3),
                );
            });
            ui.add_space(6.0);
        }
    });
    record_probe(ui, "ai-settings.features", features.response.rect);
    switched
}

/// The AI activity log: the file's real rows, or a line saying there are none.
#[allow(
    clippy::too_many_lines,
    reason = "one drawing pass of the screen, a line for each part of it"
)]
fn draw_log(ui: &mut Ui, log: &[AiLogEntry], reading: bool, tr: &Tr) {
    card_flush(ui, |ui| {
        let t = tokens(ui);
        // `.spread` with `padding:12px 16px 0` (index.html:1915).
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(tr.t("ai.settings.log.title"))
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                let meta = if log.is_empty() {
                    tr.t("ai.settings.log.meta_empty")
                } else {
                    tr.t("ai.settings.log.meta")
                };
                ui.label(RichText::new(meta).size(TEXT_META).color(t.text_3));
            });
        });
        ui.add_space(6.0);
        if reading || log.is_empty() {
            let line = tr.t(if reading {
                "ai.settings.log.reading"
            } else {
                "ai.settings.log.empty"
            });
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(para_text(&line, TEXT_BODY, t.text_2));
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
                    tr.t(if entry.accepted {
                        "ai.settings.log.accepted"
                    } else {
                        "ai.settings.log.not_accepted"
                    }),
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
        let log_headers = [
            tr.t("ai.settings.log.when"),
            tr.t("ai.settings.log.model"),
            tr.t("ai.settings.log.input_hash"),
            tr.t("ai.settings.log.output"),
            tr.t("ai.settings.log.decision"),
        ];
        mock_table::table(
            ui,
            &[
                Width::Fixed(140.0),
                Width::Fixed(110.0),
                Width::Fixed(130.0),
                Width::Rest,
                Width::Fixed(110.0),
            ],
            Some(&log_headers.each_ref().map(String::as_str)),
            rows,
        );
    });
}

/// The assistant's activity log (index.html:2077 and 1915): when, which feature, what it was
/// given, what it produced and what you decided. Every cell is a stored fact; none is a figure.
fn draw_activity(ui: &mut Ui, rows: &[AiLogRow], tr: &Tr) {
    card_flush(ui, |ui| {
        let t = tokens(ui);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(tr.t("ai.settings.log.title"))
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(tr.t("ai.settings.log.meta"))
                        .size(TEXT_META)
                        .color(t.text_3),
                );
            });
        });
        ui.add_space(6.0);
        let texts: Vec<[String; 5]> = rows
            .iter()
            .map(|row| {
                [
                    short_when(row),
                    tr.t(feature_key(row.feature)),
                    tr.tf(
                        "ai.settings.log.input_text",
                        &[
                            ("label", &row.input_label),
                            ("hash", &short_hash(&row.input_hash)),
                        ],
                    ),
                    output_text(&row.output, tr),
                    decision_text(&row.decision, tr),
                ]
            })
            .collect();
        let cells = texts
            .iter()
            .map(|c| {
                vec![
                    mock_table::muted(&c[0]),
                    mock_table::text(&c[1]),
                    mock_table::text(&c[2]),
                    mock_table::text(&c[3]),
                    mock_table::text(&c[4]),
                ]
            })
            .collect();
        let headers = [
            tr.t("ai.settings.log.when"),
            tr.t("ai.settings.log.feature"),
            tr.t("ai.settings.log.input"),
            tr.t("ai.settings.log.output"),
            tr.t("ai.settings.log.decision"),
        ];
        mock_table::table(
            ui,
            &[
                Width::Fixed(100.0),
                Width::Fixed(110.0),
                Width::Rest,
                Width::Fixed(150.0),
                Width::Fixed(130.0),
            ],
            Some(&headers.each_ref().map(String::as_str)),
            cells,
        );
    });
}

/// The words of a feature in the log (index.html:2077).
fn feature_key(feature: AiFeature) -> &'static str {
    match feature {
        AiFeature::BillReading => "ai.settings.log.feature.bill",
        AiFeature::PlainWords => "ai.settings.log.feature.plain",
        AiFeature::BankSuggestion => "ai.settings.log.feature.bank",
        AiFeature::Ask => "ai.settings.log.feature.ask",
        AiFeature::Gstr2bExplanation => "ai.settings.log.feature.gstr2b",
    }
}

/// What an event produced, in words.
fn output_text(output: &AiOutput, tr: &Tr) -> String {
    match output {
        AiOutput::DraftVoucher(kind) => tr.t(crate::screens::review::draft_kind_key(*kind)),
        AiOutput::MatchSuggestion => tr.t("ai.settings.log.out.match"),
        AiOutput::ReportQuery => tr.t("ai.settings.log.out.query"),
        AiOutput::MappingSuggestion => tr.t("ai.settings.log.out.mapping"),
        AiOutput::Answered => tr.t("ai.settings.log.out.answered"),
        AiOutput::ReadFailed(reason) => {
            tr.tf("ai.settings.log.out.read_failed", &[("reason", reason)])
        }
        AiOutput::CloudConsent(true) => tr.t("ai.settings.log.out.consent_given"),
        AiOutput::CloudConsent(false) => tr.t("ai.settings.log.out.consent_refused"),
    }
}

/// What the person decided, in words.
fn decision_text(decision: &AiDecision, tr: &Tr) -> String {
    match decision {
        AiDecision::Waiting => tr.t("ai.settings.log.waiting"),
        AiDecision::Accepted(Some(number)) => {
            tr.tf("ai.settings.log.accepted_as", &[("number", number)])
        }
        AiDecision::Accepted(None) => tr.t("ai.settings.log.accepted"),
        AiDecision::Rejected(_) => tr.t("ai.settings.log.rejected"),
        AiDecision::NoDecision => tr.t("ai.settings.log.no_decision"),
    }
}

/// The log's time as the mock shows it: `21 Nov 10:42` in the row's own offset.
fn short_when(row: &AiLogRow) -> String {
    let month = match row.at.month() {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    };
    let (h, mi, _) = row.at.to_hms();
    format!("{:02} {month} {h:02}:{mi:02}", row.at.day())
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
fn draw_never(ui: &mut Ui, tr: &Tr) {
    let never = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let t = tokens(ui);
        ui.label(
            RichText::new(tr.t("ai.settings.never.title"))
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for line in never_in(tr) {
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("•").size(TEXT_BODY).color(t.text_2));
                ui.vertical(|ui| {
                    para(ui, &line, TEXT_BODY, t.text_2);
                });
            });
        }
    });
    record_probe(ui, "ai-settings.never", never.response.rect);
}
