//! Company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html` 1855-1895).
//!
//! The rules (steps, GSTIN and state, features, books date, what a company is) live in [`form`].
//! This file draws the steps and runs them. Creating the company file runs on a background
//! thread, as the welcome screen does, and the new company then replaces the open one.
//!
//! The keyboard preset has no store yet, so its step says so and saves nothing. The backup step's
//! folder, daily schedule and 7 restorable versions are stored with the new company (round s23).
//! The AI tier is stored with the new company, except the cloud tier, which the engine refuses
//! (see `design/PARITY-GAPS.md`).

pub mod form;

#[cfg(test)]
mod s14_tests;

use crate::i18n::Tr;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::Key;
use ledgerly_api::{AiTier, Api, GstFiling};
use ledgerly_core::{Clock, Date, StateCode};
use ledgerly_model::{EntityKind, Features};

use self::form::{
    CompanyForm, GstinCheck, KeyInput, KeyOutcome, Step, ai_notice, build_company, check_gstin,
    default_features, feature_rows, financial_year_end, financial_year_start, parse_books_begin,
    state_from_gstin, step_outcome, unsaved_notice,
};
use crate::shell::navigate::{self, MASTERS_KEY};
use crate::shell::{GatewayScreen, hand_over, spawn_file_job};
use crate::theme::{
    RADIUS_M, RADIUS_S, TEXT_BODY, TEXT_H1, TEXT_INPUT, TEXT_META, border_width, shadow_1,
};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::icon::{self, ICON_SIZE, Icon, icon};
use crate::ui::keycap;
use crate::ui::note::NoteKind;
use crate::ui::option_card::{OptionCard, option_grid};
use crate::ui::probe::record_probe;
use crate::ui::step_marker::{self, StepMarker};
use crate::ui::switch::switch;
use crate::ui::{bold_font, kind, para, tokens};
use crate::{AppCtx, Screen, ScreenAction};
use egui::FontId;

/// `true` when the frame's input has a press of `key`.
fn pressed_key(ui: &egui::Ui, key: Key) -> bool {
    ui.input(|i| {
        i.events
            .iter()
            .any(|e| is_key_event(e, key) && matches!(e, egui::Event::Key { pressed: true, .. }))
    })
}

/// `true` for a key event of `key`, pressed or released.
fn is_key_event(event: &egui::Event, key: Key) -> bool {
    matches!(event, egui::Event::Key { key: k, .. } if *k == key)
}

/// The wizard's screen title, as the sidebar and the breadcrumbs name it.
pub const SETUP_TITLE: &str = "Company setup";

/// The width of the step rail, as the mock's card gives it (index.html:311, 240 px).
const RAIL_WIDTH: f32 = 240.0;
/// The widest the card gets (`.setup{max-width:980px}`, index.html:311).
const CARD_MAX_WIDTH: f32 = 980.0;
/// The space above the card (`.setup{margin:12px auto}`).
const CARD_MARGIN_TOP: f32 = 12.0;
/// A rail row's height: the 22 px marker and 8 px of padding above and below (index.html:313).
const RAIL_ROW_HEIGHT: f32 = 38.0;
/// A rail row's padding at the left (`padding:8px 10px`).
const RAIL_PAD_X: f32 = 10.0;
/// The space between a marker and the step's name (`gap:10px`).
const RAIL_LABEL_GAP: f32 = 10.0;
/// A rail marker's diameter (`.steps button i{width:22px;height:22px}`, index.html:314).
const MARKER_SIZE: f32 = 22.0;
/// The space above Back and Continue (`margin-top:22px`, index.html:1861).
const NAV_GAP: f32 = 22.0;
/// The field grid's gap between columns (`.vhead{gap:12px 16px}`, index.html:234).
const FIELD_GAP_X: f32 = 16.0;
/// The field grid's gap between rows.
const FIELD_GAP_Y: f32 = 12.0;
/// A feature row's padding (`table.t td`).
const FEATURE_PAD: f32 = 8.0;
/// The space between a feature's switch and its text (the 44 px switch column).
const FEATURE_SWITCH_GAP: f32 = 10.0;
/// A launch card's height on Done (two lines of label and the padding).
const LAUNCH_HEIGHT: f32 = 58.0;
/// The gap between launch cards (`.launch{gap:6px}`, index.html:193).
const LAUNCH_GAP: f32 = 6.0;
/// A launch card's padding, left and right (`padding:9px 10px`, index.html:195).
const LAUNCH_PAD_X: f32 = 10.0;
/// A launch card's padding, top and bottom.
const LAUNCH_PAD_Y: f32 = 9.0;
/// The space between a launch card's icon and its label (`gap:10px`).
const LAUNCH_ICON_GAP: f32 = 10.0;

/// The answer of the background create: the open company and what the engine said about the
/// AI choice when it was not stored, or why the file could not be created.
type Created = Result<(Api, Option<String>), String>;

/// The company setup wizard. Opened from the palette or the sidebar.
pub struct SetupScreen {
    /// Clock for the financial year and the audit stamps of the new file.
    clock: Clock,
    /// The step on screen.
    step: Step,
    /// The financial year's first day (1 April, index.html:1870).
    fy_start: Date,
    /// Company name.
    name: String,
    /// Company type.
    kind: EntityKind,
    /// City, kept as the first address line.
    city: String,
    /// GSTIN text; may be blank.
    gstin: String,
    /// The GSTIN as it was when the state was last filled from it.
    gstin_seen: String,
    /// The state the form has; `None` until the user chooses one.
    state: Option<StateCode>,
    /// Where the company file is saved; blank means the default under Documents.
    file: String,
    /// The books begin date text, `1 Apr 2026` or `YYYY-MM-DD`.
    books_begin: String,
    /// The features switched on.
    features: Features,
    /// The keyboard preset the user picked. Not stored yet.
    keyboard_classic: bool,
    /// The AI tier the user picked, stored with the new company ([`form::create_company`]).
    ai: AiTier,
    /// How often GST returns are filed, stored with the new company.
    filing: GstFiling,
    /// The backup folder typed; empty means Backups beside the company file.
    backup_folder: String,
    /// What the last action said, shown on the step.
    notice: Option<String>,
    /// The name field takes focus when the company step opens, so typing goes there.
    focus_name: bool,
    /// The company file being created, if any.
    job: Option<Receiver<Created>>,
    /// The name of the company that was created, for the Done step.
    created_name: Option<String>,
    /// The card chosen on the Start step.
    start: StartChoice,
    /// The folder the sample company's throwaway file is made under.
    sample_base: PathBuf,
    /// The file dialog of "Open a Ledgerly file": the chosen path, or `None` when cancelled.
    picker: fn() -> Option<PathBuf>,
    /// The company file being opened, if any.
    open_job: Option<Receiver<Result<Api, String>>>,
    /// Where an opened or created company is recorded for the Start screen. `None` records
    /// nothing (tests, and the harness unless it asks).
    recent_dir: Option<PathBuf>,
    /// A company file was opened: the next frame goes to its home screen.
    opened: bool,
    /// Continue was pressed on the Import card: the next frame opens the import screen.
    open_screen: Option<OpenScreen>,
}

/// A screen the wizard opens on top of itself after the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenScreen {
    /// The import screen.
    Import,
}

/// What the Start step's Continue button does: the cards that can be chosen (index.html:1863).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartChoice {
    /// Create a company: the wizard goes on to the Company step.
    Create,
    /// Import from another app: the import screen opens on top of the wizard.
    Import,
    /// Open a Ledgerly file: the system file dialog, then the company opens.
    Open,
    /// Try the sample company: a throwaway company file is built and opened.
    Sample,
}

impl StartChoice {
    /// Where an arrow key moves the choice. The cards sit in two columns: Create and Import on
    /// the first row, Open and the sample on the second. A key that would leave the grid stays.
    fn moved(self, key: Key) -> Self {
        match (self, key) {
            (StartChoice::Create, Key::ArrowRight) | (StartChoice::Sample, Key::ArrowUp) => {
                StartChoice::Import
            }
            (StartChoice::Create | StartChoice::Open, Key::ArrowUp)
            | (StartChoice::Import, Key::ArrowLeft) => StartChoice::Create,
            (StartChoice::Create, Key::ArrowDown) | (StartChoice::Sample, Key::ArrowLeft) => {
                StartChoice::Open
            }
            (StartChoice::Import, Key::ArrowDown) | (StartChoice::Open, Key::ArrowRight) => {
                StartChoice::Sample
            }
            (choice, _) => choice,
        }
    }
}

/// The folder the sample company's file is made under: the user's cache folder, else the
/// system temp folder. Never the user's documents.
fn default_sample_base() -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".cache"))
                .filter(|p| p.is_absolute())
        });
    cache.unwrap_or_else(std::env::temp_dir)
}

/// Lets the app raise native file dialogs (see [`crate::ui::dialogs`]). Only `shell::run` calls
/// this.
pub fn enable_system_dialogs() {
    crate::ui::dialogs::enable_system_dialogs();
}

/// `true` once [`enable_system_dialogs`] was called.
#[must_use]
pub fn system_dialogs_enabled() -> bool {
    crate::ui::dialogs::system_dialogs_enabled()
}

/// The wizard's default file dialog: the system dialog when dialogs were switched on, otherwise
/// "cancelled" at once.
#[must_use]
pub fn default_file_dialog() -> Option<PathBuf> {
    crate::ui::dialogs::open_file(
        &crate::i18n::t("setup.welcome.open"),
        &[("Ledgerly company file", &["ledgerly"])],
    )
}

/// A home folder to use instead of `$HOME` for the default company file (captures and tests).
static HOME_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Makes the default company file go under `home` instead of the user's home folder. The first
/// call wins; the real window never calls it.
pub fn set_home_override(home: PathBuf) {
    let _first = HOME_OVERRIDE.set(home);
}

/// The path the company file gets when the field is left blank, as the field shows it.
#[must_use]
pub fn default_file_label(name: &str) -> String {
    let slug = file_slug(name);
    if slug.is_empty() {
        "~/Documents/Ledgerly/<company name>.ledgerly".to_owned()
    } else {
        format!("~/Documents/Ledgerly/{slug}.ledgerly")
    }
}

/// The company name as a file name: lower case, each run of other characters one hyphen, no
/// hyphen at either end.
fn file_slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

impl Default for SetupScreen {
    fn default() -> Self {
        Self::with_clock(Clock::System)
    }
}

impl std::fmt::Debug for SetupScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetupScreen")
            .field("step", &self.step)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl SetupScreen {
    /// The tier selected on the AI step.
    #[must_use]
    pub fn ai_choice(&self) -> AiTier {
        self.ai
    }

    /// A wizard whose financial year and audit stamps come from `clock`.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        let today = clock.now().date();
        let fy_start = financial_year_start(today).unwrap_or(today);
        Self {
            clock,
            step: Step::Welcome,
            fy_start,
            name: String::new(),
            kind: EntityKind::Partnership,
            city: String::new(),
            gstin: String::new(),
            gstin_seen: String::new(),
            state: None,
            file: String::new(),
            books_begin: long_date(fy_start),
            features: default_features(),
            keyboard_classic: true,
            ai: AiTier::Off,
            filing: GstFiling::Monthly,
            backup_folder: String::new(),
            notice: None,
            job: None,
            created_name: None,
            focus_name: false,
            start: StartChoice::Create,
            sample_base: default_sample_base(),
            picker: default_file_dialog,
            open_job: None,
            recent_dir: None,
            opened: false,
            open_screen: None,
        }
    }

    /// The same wizard making the sample company's throwaway file under `base`.
    #[must_use]
    pub fn with_sample_base(mut self, base: PathBuf) -> Self {
        self.sample_base = base;
        self
    }

    /// The same wizard recording the companies it opens or creates in the list in `dir`.
    #[must_use]
    pub fn with_recent_dir(mut self, dir: PathBuf) -> Self {
        self.recent_dir = Some(dir);
        self
    }

    /// The same wizard with `picker` as the file dialog of "Open a Ledgerly file".
    #[must_use]
    pub fn with_picker(mut self, picker: fn() -> Option<PathBuf>) -> Self {
        self.picker = picker;
        self
    }

    /// The company form as the user has typed it. A GSTIN decides nothing here; [`check_gstin`]
    /// decides whether the step may go on.
    fn company_form(&self) -> Result<CompanyForm, String> {
        let state = self
            .state
            .ok_or_else(|| crate::i18n::t("setup.notice.choose_state"))?;
        let fy_end = financial_year_end(self.fy_start)?;
        let books_begin = parse_books_begin(&self.books_begin, self.fy_start, fy_end)?;
        Ok(CompanyForm {
            name: self.name.clone(),
            kind: self.kind,
            city: self.city.clone(),
            gstin: self.gstin.clone(),
            state,
            fy_start: self.fy_start,
            books_begin,
        })
    }

    /// Fills the state from a GSTIN the user has just typed (index.html:1868 "fills the state").
    fn fill_state_from_gstin(&mut self) {
        if self.gstin == self.gstin_seen {
            return;
        }
        if let Some(state) = state_from_gstin(&self.gstin) {
            self.state = Some(state);
        }
        self.gstin_seen = self.gstin.clone();
    }

    /// Why the company step cannot go on, or `None` when it can.
    fn company_block(&self) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some(crate::i18n::t("setup.notice.type_name"));
        }
        let Some(state) = self.state else {
            return Some(crate::i18n::t("setup.notice.choose_state"));
        };
        match check_gstin(&self.gstin, state) {
            GstinCheck::Blank | GstinCheck::Valid(_) => {}
            GstinCheck::Invalid(reason) => return Some(reason),
            GstinCheck::Mismatch { message, .. } => return Some(message),
        }
        self.company_form().err()
    }

    /// Continues from the current step. On Backup, that creates the company.
    fn continue_step(&mut self) {
        match self.step {
            Step::Welcome => match self.start {
                StartChoice::Create => {
                    self.step = Step::Company;
                    self.focus_name = true;
                }
                StartChoice::Import => self.open_screen = Some(OpenScreen::Import),
                StartChoice::Open => self.open_file(),
                StartChoice::Sample => self.open_sample(),
            },
            Step::Company => {
                if let Some(reason) = self.company_block() {
                    self.notice = Some(reason);
                    return;
                }
                self.notice = None;
                self.step = Step::Features;
            }
            Step::Features => self.step = Step::Keyboard,
            Step::Keyboard => self.step = Step::Ai,
            Step::Ai => self.step = Step::Backup,
            Step::Backup => self.create(),
            Step::Done => {}
        }
    }

    /// Asks for a company file in the system file dialog and opens it in the background. A
    /// cancelled dialog changes nothing. Home replaces the wizard when the company is open.
    fn open_file(&mut self) {
        if self.open_job.is_some() {
            return;
        }
        let Some(path) = (self.picker)() else {
            return;
        };
        let dir = self.recent_dir.clone();
        match spawn_file_job(move || crate::recent::open_and_record(&path, dir.as_deref())) {
            Ok(rx) => {
                self.open_job = Some(rx);
                self.notice = Some(crate::i18n::t("setup.notice.opening_file"));
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Builds the sample company in a throwaway file under the sample folder and opens it in the
    /// background. Home replaces the wizard when it is open.
    fn open_sample(&mut self) {
        if self.open_job.is_some() {
            return;
        }
        let (base, clock) = (self.sample_base.clone(), self.clock);
        let tr = crate::i18n::current();
        match spawn_file_job(move || {
            ledgerly_sample::create_under(&base, clock)
                .map(|(api, _path)| api)
                .map_err(|e| tr.tf("setup.notice.sample_failed", &[("error", &e.to_string())]))
        }) {
            Ok(rx) => {
                self.open_job = Some(rx);
                self.notice = Some(crate::i18n::t("setup.notice.opening_sample"));
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Checks the background open and, when it has answered, hands the company to the worker.
    fn collect_open(&mut self, ctx: &mut AppCtx) {
        let Some(rx) = self.open_job.as_ref() else {
            return;
        };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(crate::i18n::t("setup.notice.open_failed")),
        };
        self.open_job = None;
        match answer.and_then(|api| hand_over(ctx, api)) {
            Ok(()) => {
                self.notice = None;
                self.opened = true;
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Starts creating the company file in the background. The wizard moves to Done when the
    /// company is open.
    fn create(&mut self) {
        if self.job.is_some() {
            return;
        }
        let company = match self
            .company_form()
            .and_then(|form| build_company(&form, self.features))
        {
            Ok(company) => company,
            Err(reason) => {
                self.notice = Some(reason);
                return;
            }
        };
        let path = match self.file_path() {
            Ok(path) => path,
            Err(reason) => {
                self.notice = Some(reason);
                return;
            }
        };
        let (clock, ai, filing) = (self.clock, form::stored_ai_tier(self.ai), self.filing);
        let dir = self.recent_dir.clone();
        let backup_folder = self.backup_folder.clone();
        match spawn_file_job(move || {
            let (mut api, note) =
                form::create_company_with_filing(&path, &company, clock, ai, filing)?;
            let note = note.or_else(|| form::store_backup_settings(&mut api, &backup_folder));
            let made = (api, note);
            crate::recent::note_opened(&made.0, &path, dir.as_deref());
            Ok(made)
        }) {
            Ok(rx) => {
                self.job = Some(rx);
                self.notice = Some(crate::i18n::t("setup.notice.working"));
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// The file the company is saved in: what the user typed, or the default under Documents.
    fn file_path(&self) -> Result<PathBuf, String> {
        if !self.file.trim().is_empty() {
            return Ok(PathBuf::from(self.file.trim()));
        }
        let home = HOME_OVERRIDE
            .get()
            .cloned()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .ok_or_else(|| crate::i18n::t("setup.notice.no_home"))?;
        let slug = file_slug(&self.name);
        Ok(home
            .join("Documents")
            .join("Ledgerly")
            .join(format!("{slug}.ledgerly")))
    }

    /// Checks the background create and, when it has answered, opens the company.
    fn collect(&mut self, ctx: &mut AppCtx) {
        self.collect_open(ctx);
        let Some(rx) = self.job.as_ref() else {
            return;
        };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(crate::i18n::t("setup.notice.create_failed")),
        };
        self.job = None;
        match answer {
            Ok((api, note)) => self.open_created(ctx, api, note),
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Puts the new company on the worker and moves to Done. The wizard stays on Done until the
    /// user goes Home.
    /// `note` is what the engine said when the AI choice could not be stored; Done shows it.
    fn open_created(&mut self, ctx: &mut AppCtx, api: Api, note: Option<String>) {
        match hand_over(ctx, api) {
            Ok(()) => {
                self.created_name = Some(self.name.trim().to_owned());
                self.notice = note;
                self.step = Step::Done;
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// The step rail down the left of the card (index.html:1860, CSS 312-317). A step already
    /// passed can be revisited by clicking it, until the company exists: revisiting would create
    /// a second.
    fn rail(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        for step in Step::ALL {
            let marker = rail_marker(step, self.step);
            let clickable = marker == Marker::Passed && self.step != Step::Done;
            if rail_row(ui, step, marker, clickable).clicked() {
                self.step = step;
                self.notice = None;
            }
        }
    }

    /// Back and Continue (index.html:1861): a secondary Back with its Esc keycap at the left,
    /// the primary Continue with its Enter keycap at the right.
    fn footer(&mut self, ui: &mut egui::Ui) {
        nav_gap(ui);
        ui.horizontal(|ui| {
            if let Some(back) = self.step.back() {
                let pressed = button(
                    ui,
                    &crate::i18n::t("chrome.back"),
                    Some("Esc"),
                    ButtonKind::Secondary,
                );
                record_probe(ui, "setup.back", pressed.rect);
                if pressed.clicked() {
                    self.step = back;
                    self.notice = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, enabled) = self.continue_label_and_gate();
                let pressed = ui
                    .add_enabled_ui(enabled, |ui| {
                        button(ui, &label, Some("Enter"), ButtonKind::Primary)
                    })
                    .inner;
                record_probe(ui, "setup.continue", pressed.rect);
                if pressed.clicked() {
                    self.continue_step();
                }
            });
        });
    }

    /// The Continue button's label and whether it is enabled on this step. On Company, a GSTIN
    /// that disagrees with the state disables it (index.html:1861).
    fn continue_label_and_gate(&self) -> (String, bool) {
        match self.step {
            Step::Welcome => match self.start {
                StartChoice::Create => (crate::i18n::t("setup.welcome.create"), true),
                StartChoice::Import => (crate::i18n::t("import.title"), true),
                StartChoice::Open => (
                    crate::i18n::t("setup.welcome.open"),
                    self.open_job.is_none(),
                ),
                StartChoice::Sample => (
                    crate::i18n::t("setup.welcome.sample"),
                    self.open_job.is_none(),
                ),
            },
            Step::Company => (
                crate::i18n::t("setup.key.continue"),
                self.company_block().is_none(),
            ),
            Step::Features | Step::Keyboard | Step::Ai => {
                (crate::i18n::t("setup.key.continue"), true)
            }
            Step::Backup => (crate::i18n::t("setup.nav.finish"), self.job.is_none()),
            Step::Done => (crate::i18n::t("setup.nav.go_home"), true),
        }
    }

    /// The arrow keys on the Start step move the choice between the cards that can be chosen.
    fn take_start_keys(&mut self, ui: &mut egui::Ui) {
        if self.step != Step::Welcome || ui.ctx().text_edit_focused() {
            return;
        }
        let arrows = [
            Key::ArrowDown,
            Key::ArrowUp,
            Key::ArrowLeft,
            Key::ArrowRight,
        ];
        if let Some(arrow) = arrows.iter().copied().find(|k| pressed_key(ui, *k)) {
            self.start = self.start.moved(arrow);
            ui.input_mut(|i| {
                i.events
                    .retain(|e| !arrows.iter().any(|k| is_key_event(e, *k)));
            });
        }
    }

    /// Enter and Escape for this step (index.html:1887-1888), consuming the ones the wizard uses.
    fn take_keys(&self, ui: &mut egui::Ui) -> KeyOutcome {
        let typing = ui.ctx().text_edit_focused();
        let enter =
            pressed_key(ui, Key::Enter).then(|| step_outcome(self.step, typing, KeyInput::Enter));
        let escape =
            pressed_key(ui, Key::Escape).then(|| step_outcome(self.step, typing, KeyInput::Escape));
        let (key, outcome) = match (enter, escape) {
            (Some(outcome), _) => (Key::Enter, outcome),
            (None, Some(outcome)) => (Key::Escape, outcome),
            (None, None) => return KeyOutcome::Pass,
        };
        // A key the wizard acts on is taken out of the input, press and release both, so the
        // focused Continue or Back button does not act on it a second time.
        if outcome != KeyOutcome::Pass {
            ui.input_mut(|i| i.events.retain(|e| !is_key_event(e, key)));
        }
        outcome
    }

    /// The launch a key opens on Done (index.html:1889: M the masters, B a bank statement), when
    /// no field has focus. The key is taken out of the input.
    fn take_launch_key(&self, ui: &mut egui::Ui) -> Option<&'static str> {
        if self.step != Step::Done || ui.ctx().text_edit_focused() {
            return None;
        }
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, Key::M) {
                Some(MASTERS_KEY)
            } else if i.consume_key(egui::Modifiers::NONE, Key::F8) {
                Some(navigate::SALES_KEY)
            } else if i.consume_key(egui::Modifiers::NONE, Key::B) {
                Some(navigate::BANK_RECON_KEY)
            } else {
                None
            }
        })
    }

    /// The Welcome step (index.html:1863). Create is the one choice that works; the others say
    /// why they wait.
    fn draw_welcome(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("setup.welcome.title"),
            &crate::i18n::t("setup.welcome.lead"),
        );
        let cards = [
            OptionCard::new(
                Icon::Plus,
                crate::i18n::t("setup.welcome.create"),
                crate::i18n::t("setup.welcome.create_hint"),
            )
            .selected(self.start == StartChoice::Create),
            OptionCard::new(
                Icon::Upload,
                crate::i18n::t("setup.welcome.import"),
                crate::i18n::t("setup.welcome.import_hint"),
            )
            .selected(self.start == StartChoice::Import),
            OptionCard::new(
                Icon::File,
                crate::i18n::t("setup.welcome.open"),
                crate::i18n::t("setup.welcome.open_hint"),
            )
            .selected(self.start == StartChoice::Open),
            OptionCard::new(
                Icon::Eye,
                crate::i18n::t("setup.welcome.sample"),
                crate::i18n::t("setup.welcome.sample_hint"),
            )
            .selected(self.start == StartChoice::Sample),
        ];
        let drawn = option_grid(ui, egui::Id::unique("setup-welcome"), 2, &cards);
        for (n, card) in drawn.iter().enumerate() {
            record_probe(ui, &format!("setup.option.{n}"), card.card);
        }
        for (n, choice) in [
            (0, StartChoice::Create),
            (1, StartChoice::Import),
            (2, StartChoice::Open),
            (3, StartChoice::Sample),
        ] {
            if drawn.get(n).is_some_and(|card| card.response.clicked()) {
                self.start = choice;
            }
        }
    }

    /// The Company step (index.html:1866-1874): the mock's four-column field grid, each label
    /// above its field. The widgets are made in the order name, type, city, GSTIN, state, books
    /// date, file, so Tab walks them in that order.
    fn draw_company(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("setup.company.title"),
            &crate::i18n::t("setup.company.lead"),
        );
        let col = field_column(ui);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(
                ui,
                span(col, 2),
                &crate::i18n::t("setup.field.name"),
                |ui, w| {
                    let name = crate::ui::a11y::text_edit(
                        ui,
                        &crate::i18n::t("setup.field.name"),
                        input(&mut self.name).id_salt("setup-name").desired_width(w),
                    );
                    if std::mem::take(&mut self.focus_name) {
                        name.request_focus();
                    }
                },
            );
            field(ui, col, &crate::i18n::t("setup.field.type"), |ui, w| {
                self.draw_kind_box(ui, w);
            });
            field(ui, col, &crate::i18n::t("setup.field.city"), |ui, w| {
                crate::ui::a11y::text_edit(
                    ui,
                    &crate::i18n::t("setup.field.city"),
                    input(&mut self.city).desired_width(w),
                );
            });
        });
        ui.add_space(FIELD_GAP_Y);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(ui, span(col, 2), "GSTIN", |ui, w| {
                let gstin_box = crate::ui::a11y::text_edit(
                    ui,
                    "GSTIN",
                    input(&mut self.gstin)
                        .hint_text(crate::i18n::t("setup.field.gstin_hint"))
                        .desired_width(w),
                );
                self.fill_state_from_gstin();
                let check = self.state.map(|s| check_gstin(&self.gstin, s));
                if matches!(
                    check,
                    Some(GstinCheck::Invalid(_) | GstinCheck::Mismatch { .. })
                ) {
                    outline_bad(ui, gstin_box.rect);
                }
                self.gstin_hint(ui);
            });
            field(ui, col, &crate::i18n::t("masters.field.state"), |ui, w| {
                self.draw_state_box(ui, w);
            });
            field(
                ui,
                col,
                &crate::i18n::t("setup.field.gst_filing"),
                |ui, w| self.draw_filing_box(ui, w),
            );
        });
        self.draw_company_dates(ui, col);
    }

    /// The entity type list of the company step.
    fn draw_kind_box(&mut self, ui: &mut egui::Ui, w: f32) {
        let kind_box = egui::ComboBox::from_id_salt("setup-kind")
            .width(w)
            .selected_text(kind_label(self.kind))
            .show_ui(ui, |ui| {
                for kind in [
                    EntityKind::Company,
                    EntityKind::Llp,
                    EntityKind::Partnership,
                    EntityKind::Proprietorship,
                    EntityKind::Other,
                ] {
                    ui.selectable_value(&mut self.kind, kind, kind_label(kind));
                }
            });
        crate::ui::a11y::name(
            ui,
            &kind_box.response,
            egui::accesskit::Role::ComboBox,
            &crate::i18n::t("setup.field.type"),
        );
    }

    /// The GST filing list of the company step.
    fn draw_filing_box(&mut self, ui: &mut egui::Ui, w: f32) {
        let filing_box = egui::ComboBox::from_id_salt("setup-filing")
            .width(w)
            .selected_text(self.filing.label())
            .show_ui(ui, |ui| {
                for filing in [GstFiling::Monthly, GstFiling::Quarterly] {
                    ui.selectable_value(&mut self.filing, filing, filing.label());
                }
            });
        crate::ui::a11y::name(
            ui,
            &filing_box.response,
            egui::accesskit::Role::ComboBox,
            &crate::i18n::t("setup.field.gst_filing"),
        );
    }

    /// The State list of the company step, outlined in red when it disagrees with the GSTIN.
    fn draw_state_box(&mut self, ui: &mut egui::Ui, w: f32) {
        let selected = self.state.map_or_else(
            || crate::i18n::t("setup.field.state_pick"),
            |s| format!("{:02} {}", s.get(), s.name()),
        );
        let state_box = egui::ComboBox::from_id_salt("setup-state")
            .width(w)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for state in StateCode::all() {
                    let label = format!("{:02} {}", state.get(), state.name());
                    ui.selectable_value(&mut self.state, Some(state), label);
                }
            });
        crate::ui::a11y::name(
            ui,
            &state_box.response,
            egui::accesskit::Role::ComboBox,
            &crate::i18n::t("masters.field.state"),
        );
        let mismatch = self.state.map(|s| check_gstin(&self.gstin, s));
        if matches!(mismatch, Some(GstinCheck::Mismatch { .. })) {
            outline_bad(ui, state_box.response.rect);
        }
    }

    /// The financial year, books date and file rows of the company step.
    fn draw_company_dates(&mut self, ui: &mut egui::Ui, col: f32) {
        ui.add_space(FIELD_GAP_Y);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(
                ui,
                span(col, 2),
                &crate::i18n::t("setup.field.fy"),
                |ui, _| {
                    let fy_end = financial_year_end(self.fy_start)
                        .map_or_else(|_| crate::i18n::t("setup.value.fy_end"), long_date);
                    ui.horizontal(|ui| {
                        read_only(
                            ui,
                            &crate::i18n::tf(
                                "setup.fy_range",
                                &[("from", &long_date(self.fy_start)), ("to", &fy_end)],
                            ),
                        );
                        rule_pill(ui, &crate::i18n::t("setup.rule.april_march"));
                    });
                },
            );
            field(
                ui,
                span(col, 2),
                &crate::i18n::t("setup.field.books_begin"),
                |ui, w| {
                    crate::ui::a11y::text_edit(
                        ui,
                        &crate::i18n::t("setup.field.books_begin"),
                        input(&mut self.books_begin)
                            .hint_text(crate::i18n::t("setup.placeholder.books_begin"))
                            .desired_width(w),
                    );
                    hint(ui, &crate::i18n::t("setup.hint.books_begin"));
                },
            );
        });
        ui.add_space(FIELD_GAP_Y);
        field(
            ui,
            span(col, 4),
            &crate::i18n::t("setup.field.file"),
            |ui, w| {
                crate::ui::a11y::text_edit(
                    ui,
                    &crate::i18n::t("setup.field.file"),
                    input(&mut self.file)
                        .hint_text(default_file_label(&self.name))
                        .desired_width(w),
                );
            },
        );
    }

    /// The line under the GSTIN: the check result, or why the GSTIN cannot be used
    /// (index.html:1868).
    fn gstin_hint(&self, ui: &mut egui::Ui) {
        let t = tokens(ui);
        match self.state.map(|state| check_gstin(&self.gstin, state)) {
            None => hint(ui, &crate::i18n::t("setup.hint.choose_state")),
            Some(GstinCheck::Blank) => {
                hint(ui, &crate::i18n::t("setup.hint.gstin_blank"));
            }
            Some(GstinCheck::Valid(gstin)) => {
                ui.horizontal(|ui| {
                    badge(ui, BadgeKind::Ok, &crate::i18n::t("masters.check_ok"));
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::i18n::tf(
                                "setup.gstin.pan_fills",
                                &[("pan", gstin.pan_str())],
                            ))
                            .size(TEXT_META)
                            .color(t.text_3),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            }
            Some(
                GstinCheck::Invalid(reason)
                | GstinCheck::Mismatch {
                    message: reason, ..
                },
            ) => {
                para(ui, &reason, TEXT_META, t.bad);
            }
        }
    }

    /// The Features step (index.html:1871-1878): a row per feature with its switch, bold title
    /// and muted line. Only the switches the model has work; the others say why.
    fn draw_features(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("setup.features.title"),
            &crate::i18n::t("setup.features.lead"),
        );
        let t = tokens(ui);
        let rule = egui::Stroke::new(border_width(kind(ui)), t.border);
        for row in feature_rows() {
            let drawn = ui.horizontal_top(|ui| {
                ui.add_space(FEATURE_PAD);
                ui.vertical(|ui| {
                    ui.add_space(FEATURE_PAD);
                    if let Some(flip) = row.switch {
                        let flag = flip(&mut self.features);
                        if switch(ui, *flag, egui::Sense::click(), &row.label).clicked() {
                            *flag = !*flag;
                        }
                    } else {
                        switch(ui, false, egui::Sense::hover(), &row.label);
                    }
                });
                ui.add_space(FEATURE_SWITCH_GAP);
                ui.vertical(|ui| {
                    ui.add_space(FEATURE_PAD);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(&row.label)
                            .font(bold_font(ui, TEXT_BODY))
                            .color(t.text),
                    );
                    let line = match (row.switch, row.unavailable.clone()) {
                        (Some(_), _) => row.hint.clone(),
                        (None, reason) => crate::i18n::tf(
                            "setup.features.unavailable",
                            &[
                                ("hint", &row.hint),
                                (
                                    "reason",
                                    &reason.unwrap_or_else(|| {
                                        crate::i18n::t("setup.features.not_built")
                                    }),
                                ),
                            ],
                        ),
                    };
                    para(ui, &line, TEXT_BODY, t.text_2);
                    ui.add_space(FEATURE_PAD);
                });
            });
            let r = drawn.response.rect;
            ui.painter()
                .hline(r.left()..=ui.max_rect().right(), r.bottom(), rule);
        }
    }

    /// The Keyboard step (index.html:1883-1886). The preset is not stored yet.
    fn draw_keyboard(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("setup.keyboard.title"),
            &crate::i18n::t("setup.keyboard.note"),
        );
        let cards = [
            OptionCard::new(
                Icon::Kbd,
                crate::i18n::t("setup.keyboard.classic"),
                crate::i18n::t("setup.keyboard.classic_hint"),
            )
            .selected(self.keyboard_classic)
            .keys(&["F8", "F5", "Ctrl+A", "Enter", "Esc"]),
            OptionCard::new(
                Icon::Kbd,
                crate::i18n::t("setup.keyboard.standard"),
                crate::i18n::t("setup.keyboard.standard_hint"),
            )
            .selected(!self.keyboard_classic)
            .keys(&["Ctrl+S", "Ctrl+N", "Tab"]),
        ];
        let drawn = option_grid(ui, egui::Id::unique("setup-keyboard"), 2, &cards);
        for (n, card) in drawn.iter().enumerate() {
            record_probe(ui, &format!("setup.option.{n}"), card.card);
            if card.response.clicked() {
                self.keyboard_classic = n == 0;
            }
        }
        if let Some(notice) = unsaved_notice(Step::Keyboard) {
            step_text(ui, &notice);
        }
    }

    /// The AI step (index.html:1879-1884). The tier is stored when the company is created; the
    /// cloud tier is refused by the engine, and [`ai_notice`] says so.
    fn draw_ai(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("topbar.ai_privacy_hover"),
            &crate::i18n::t("setup.ai.lead"),
        );
        let tiers = [AiTier::Off, AiTier::Local, AiTier::Cloud];
        let cards = [
            OptionCard::new(
                Icon::Off,
                crate::i18n::t("setup.ai.off"),
                crate::i18n::t("setup.ai.off_hint"),
            ),
            OptionCard::new(
                Icon::Cpu,
                crate::i18n::t("setup.ai.local"),
                crate::i18n::t("setup.ai.local_hint"),
            ),
            OptionCard::new(
                Icon::Cloud,
                crate::i18n::t("setup.ai.cloud"),
                crate::i18n::t("setup.ai.cloud_hint"),
            ),
        ];
        let ai_in_release = crate::release::ai_enabled();
        let not_in_release = crate::i18n::t("ai.settings.not_in_release");
        let cards = cards
            .into_iter()
            .zip(tiers)
            .map(|(card, tier)| {
                if !ai_in_release && tier != AiTier::Off {
                    // Shown, tagged and disabled: the company stays Off (`crate::release`).
                    card.disabled().badge(Some(not_in_release.as_str()))
                } else {
                    card.selected(self.ai == tier)
                }
            })
            .collect::<Vec<_>>();
        let drawn = option_grid(ui, egui::Id::unique("setup-ai"), 3, &cards);
        for (n, (card, tier)) in drawn.iter().zip(tiers).enumerate() {
            record_probe(ui, &format!("setup.option.{n}"), card.card);
            if card.response.clicked() && (ai_in_release || tier == AiTier::Off) {
                self.ai = tier;
            }
        }
        step_text(
            ui,
            &crate::i18n::t(if ai_in_release {
                "setup.ai.later"
            } else {
                "ai.settings.later_note"
            }),
        );
        if let Some(notice) = ai_notice(self.ai) {
            step_text(ui, &notice);
        }
    }

    /// The Backup step (mock `setup.backup`). The folder, a daily schedule and 7 restorable versions
    /// are stored with the company when it is created ([`form::store_backup_settings`]).
    fn draw_backup(&mut self, ui: &mut egui::Ui) {
        pane_head(
            ui,
            &crate::i18n::t("setup.backup.title"),
            &crate::i18n::t("setup.backup.lead"),
        );
        let col = field_column(ui);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(
                ui,
                span(col, 2),
                &crate::i18n::t("setup.field.backup_folder"),
                |ui, w| {
                    crate::ui::a11y::text_edit(
                        ui,
                        &crate::i18n::t("setup.field.backup_folder"),
                        input(&mut self.backup_folder)
                            .hint_text(crate::i18n::t("setup.field.backup_hint"))
                            .desired_width(w),
                    );
                },
            );
            field(ui, col, &crate::i18n::t("setup.field.versions"), |ui, _| {
                read_only(ui, &crate::i18n::t("setup.value.versions"));
            });
            field(ui, col, &crate::i18n::t("setup.field.when"), |ui, _| {
                read_only(ui, &crate::i18n::t("setup.value.daily"));
            });
        });
        ui.add_space(14.0);
        crate::ui::note::note_with_icon(ui, NoteKind::Info, Icon::Info, |ui, ink| {
            para(ui, &crate::i18n::t("setup.backup.law"), TEXT_META, ink);
        });
    }

    /// The Done step (index.html:1889-1894): three launch cards, each with its icon, bold label
    /// and key, and the primary "Go to Home". Returns the launch or Home chosen, if any.
    fn draw_done(ui: &mut egui::Ui, name: &str) -> Option<DoneChoice> {
        let t = tokens(ui);
        ui.horizontal(|ui| {
            icon(ui, Icon::Check, t.text);
            ui.label(
                egui::RichText::new(crate::i18n::tf("setup.done.ready", &[("name", name)]))
                    .font(bold_font(ui, TEXT_H1))
                    .color(t.text),
            );
        });
        ui.add_space(6.0);
        para(ui, &crate::i18n::t("setup.done.lead"), TEXT_BODY, t.text_2);
        ui.add_space(18.0);
        let launches = [
            Launch {
                glyph: Icon::Users,
                label: crate::i18n::t("setup.done.masters"),
                place: Some(crate::i18n::t("sidebar.masters")),
                key: "M",
                choice: Some(DoneChoice::Open(MASTERS_KEY)),
            },
            Launch {
                glyph: Icon::Sale,
                label: crate::i18n::t("setup.done.invoice"),
                place: None,
                key: "F8",
                choice: Some(DoneChoice::Open(navigate::SALES_KEY)),
            },
            Launch {
                glyph: Icon::Bank,
                label: crate::i18n::t("setup.done.bank"),
                place: None,
                key: "B",
                choice: Some(DoneChoice::Open(navigate::BANK_RECON_KEY)),
            },
        ];
        let mut chosen = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = LAUNCH_GAP;
            ui.columns(launches.len(), |cols| {
                for (n, (launch, col)) in launches.iter().zip(cols.iter_mut()).enumerate() {
                    let response = launch_card(col, launch);
                    record_probe(col, &format!("setup.launch.{n}"), response.rect);
                    if response.clicked() {
                        chosen = launch.choice;
                    }
                }
            });
        });
        nav_gap(ui);
        // Inside a row, so the right-to-left layout is one button high, not the pane's height.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let home = button(
                    ui,
                    &crate::i18n::t("setup.nav.go_home"),
                    Some("Enter"),
                    ButtonKind::Primary,
                );
                record_probe(ui, "setup.home", home.rect);
                if home.clicked() {
                    chosen = Some(DoneChoice::Home);
                }
            });
        });
        chosen
    }
}

/// What Done's launches and button lead to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoneChoice {
    /// The screen with this palette key.
    Open(&'static str),
    /// Home.
    Home,
}

/// One launch card of Done (index.html:1892).
struct Launch {
    /// The icon at the card's left.
    glyph: Icon,
    /// The bold label.
    label: String,
    /// The place named before the key, such as "Masters".
    place: Option<String>,
    /// The key that opens it.
    key: &'static str,
    /// Where it leads; `None` while there is no route from the wizard (PARITY-GAPS).
    choice: Option<DoneChoice>,
}

/// Draws a launch card (`.launch button`, index.html:193-197): a bordered row with the icon, the
/// bold label and, at the right, the place and the key. A card with no route senses hover only
/// and draws its label muted.
fn launch_card(ui: &mut egui::Ui, launch: &Launch) -> egui::Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let sense = if launch.choice.is_some() {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), LAUNCH_HEIGHT), sense);
    let edge = if response.hovered() && launch.choice.is_some() {
        t.border_strong
    } else {
        t.border
    };
    let painter = ui.painter();
    if response.hovered() && launch.choice.is_some() {
        painter.rect_filled(rect, RADIUS_S, t.hover);
    }
    painter.rect_stroke(
        rect,
        RADIUS_S,
        egui::Stroke::new(bw, edge),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(egui::vec2(LAUNCH_PAD_X, LAUNCH_PAD_Y));
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(inner.left() + ICON_SIZE / 2.0, inner.center().y),
        egui::Vec2::splat(ICON_SIZE),
    );
    icon::paint(painter, icon_rect, launch.glyph, t.text_2);
    // The key at the right edge, the place above it when there is one, as the mock's span wraps.
    let key_w = keycap::width(ui, launch.key);
    let place_galley = launch.place.as_deref().map(|place| {
        painter.layout_no_wrap(place.to_owned(), FontId::proportional(TEXT_BODY), t.text)
    });
    let right_w = place_galley
        .as_ref()
        .map_or(key_w, |g| g.size().x.max(key_w));
    let right = inner.right();
    let key_top = match &place_galley {
        Some(g) => {
            let top = inner.center().y - f32::midpoint(g.size().y, keycap::KEYCAP_HEIGHT);
            painter.galley(egui::pos2(right - g.size().x, top), g.clone(), t.text);
            top + g.size().y
        }
        None => inner.center().y - keycap::KEYCAP_HEIGHT / 2.0,
    };
    let key_rect = egui::Rect::from_min_size(
        egui::pos2(right - key_w, key_top),
        egui::vec2(key_w, keycap::KEYCAP_HEIGHT),
    );
    keycap::paint(
        painter,
        key_rect,
        launch.key,
        t.surface_2,
        t.border,
        t.text_2,
    );
    let label_left = icon_rect.right() + LAUNCH_ICON_GAP;
    let label_w = (right - right_w - LAUNCH_ICON_GAP - label_left).max(0.0);
    let ink = if launch.choice.is_some() {
        t.text
    } else {
        t.text_3
    };
    let galley = painter.layout(launch.label.clone(), bold_font(ui, TEXT_BODY), ink, label_w);
    let top = inner.center().y - galley.size().y / 2.0;
    painter.galley(egui::pos2(label_left, top), galley, ink);
    if launch.choice.is_none() {
        return response.on_hover_text(crate::i18n::t("setup.tip.no_route"));
    }
    response
}

/// How a rail marker is drawn (index.html:1860).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    /// A step already passed: a green check.
    Passed,
    /// The step on screen: its number on the accent.
    Current,
    /// A step still ahead: its number in a circle.
    Ahead,
}

/// The marker of `step` while `on` is the step on screen.
fn rail_marker(step: Step, on: Step) -> Marker {
    match step.index().cmp(&on.index()) {
        std::cmp::Ordering::Less => Marker::Passed,
        std::cmp::Ordering::Equal => Marker::Current,
        std::cmp::Ordering::Greater => Marker::Ahead,
    }
}

/// Draws one rail row (`.steps button`, index.html:313-317): the round marker, then the step's
/// name. The current row is accent-soft with the accent-ink name in bold.
#[expect(clippy::float_arithmetic, reason = "laying out the marker in its row")]
fn rail_row(ui: &mut egui::Ui, step: Step, marker: Marker, clickable: bool) -> egui::Response {
    let t = tokens(ui);
    let sense = if clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), RAIL_ROW_HEIGHT), sense);
    let n = step.index();
    record_probe(ui, &format!("setup.rail.row.{n}"), rect);
    let painter = ui.painter();
    if marker == Marker::Current {
        painter.rect_filled(rect, RADIUS_S, t.accent_soft);
    } else if clickable && response.hovered() {
        painter.rect_filled(rect, RADIUS_S, t.hover);
    }
    let dot = egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + RAIL_PAD_X,
            rect.center().y - MARKER_SIZE / 2.0,
        ),
        egui::Vec2::splat(MARKER_SIZE),
    );
    record_probe(ui, &format!("setup.rail.marker.{n}"), dot);
    let kit = match marker {
        Marker::Passed => StepMarker::Passed,
        Marker::Current => StepMarker::Current,
        Marker::Ahead => StepMarker::Ahead,
    };
    step_marker::paint(ui, dot.center(), kit, &(n + 1).to_string());
    let painter = ui.painter();
    let (font, ink) = if marker == Marker::Current {
        (bold_font(ui, TEXT_BODY), t.accent_ink)
    } else {
        (FontId::proportional(TEXT_BODY), t.text_2)
    };
    painter.text(
        egui::pos2(dot.right() + RAIL_LABEL_GAP, rect.center().y),
        egui::Align2::LEFT_CENTER,
        step.label(),
        font,
        ink,
    );
    response
}

/// The 22 points above Back and Continue or Go to Home (index.html:1861, 1893), less the item
/// spacing egui adds after the widget before it.
fn nav_gap(ui: &mut egui::Ui) {
    #[expect(clippy::float_arithmetic, reason = "the gap less the item spacing")]
    let gap = (NAV_GAP - ui.spacing().item_spacing.y).max(0.0);
    ui.add_space(gap);
}

/// The pane's heading and the line under it (`<h1>` and `<p class="t2" style="margin:6px 0
/// 18px">`, index.html:1863).
fn pane_head(ui: &mut egui::Ui, title: &str, sub: &str) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(title)
            .font(bold_font(ui, TEXT_H1))
            .color(t.text),
    );
    ui.add_space(6.0);
    para(ui, sub, TEXT_BODY, t.text_2);
    ui.add_space(18.0);
}

/// A muted line under a step's cards (`<p class="meta" style="margin-top:12px">`,
/// index.html:1882).
fn step_text(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    ui.add_space(12.0);
    para(ui, text, TEXT_META, t.text_3);
}

/// The width of one column of the pane's four-column field grid (`.vhead`, index.html:234:
/// four equal columns, 16 points apart).
#[expect(clippy::float_arithmetic, reason = "four columns and three gaps")]
fn field_column(ui: &egui::Ui) -> f32 {
    ((ui.available_width() - 3.0 * FIELD_GAP_X) / 4.0).max(60.0)
}

/// The width of a field spanning `columns` grid columns, gaps included.
#[expect(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "columns is at most 4, exact in f32"
)]
fn span(col: f32, columns: usize) -> f32 {
    let n = columns as f32;
    col * n + FIELD_GAP_X * (n - 1.0)
}

/// One field of the grid (`.fld`, index.html:235-237): the label above, then what `add` draws
/// in the field's width.
fn field(ui: &mut egui::Ui, width: f32, label: &str, add: impl FnOnce(&mut egui::Ui, f32)) {
    let t = tokens(ui);
    ui.vertical(|ui| {
        ui.set_width(width);
        crate::ui::button::style_combo(ui);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(label).size(TEXT_META).color(t.text_2));
        add(ui, width);
    });
}

/// The red outline of a field that fails its check (`.invalid`, index.html:236).
fn outline_bad(ui: &egui::Ui, rect: egui::Rect) {
    let t = tokens(ui);
    ui.painter().rect_stroke(
        rect,
        crate::theme::RADIUS_S,
        egui::Stroke::new(border_width(kind(ui)).max(1.5), t.bad),
        egui::StrokeKind::Inside,
    );
}

/// A single-line text box as the mock draws its inputs (`.fld input`, index.html:236): a 32 point
/// box with 10 points of padding and the input size of text.
fn input(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .margin(egui::Margin::symmetric(10, 7))
        .font(FontId::proportional(TEXT_INPUT))
}

/// A small label that says where a value comes from (`.prov.rule`, a deterministic rule): the
/// sunken fill, the muted ink, 10.5 points, bold.
fn rule_pill(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.sunken)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(5, 0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .font(bold_font(ui, 10.5))
                    .color(t.text_2),
            );
        });
}

/// A value the user cannot change, in the field's place (`.ro`).
fn read_only(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    ui.label(egui::RichText::new(text).size(TEXT_INPUT).color(t.text));
}

/// A field's muted hint (`.fld .hint`).
fn hint(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    para(ui, text, TEXT_META, t.text_3);
}

/// A date as the mock writes it: `1 Apr 2026`, with the month word of the current language.
fn long_date(date: Date) -> String {
    crate::ui::format::date_in(date, &crate::i18n::current())
}

/// The name of a company type, as the mock's "Type" field shows it, in the current language.
fn kind_label(kind: EntityKind) -> String {
    kind_label_in(kind, &crate::i18n::current())
}

/// The name of a company type in the words of `tr`.
#[must_use]
pub fn kind_label_in(kind: EntityKind, tr: &Tr) -> String {
    tr.t(match kind {
        EntityKind::Company => "setup.kind.company",
        EntityKind::Llp => "setup.kind.llp",
        EntityKind::Partnership => "setup.kind.partnership",
        EntityKind::Proprietorship => "setup.kind.proprietorship",
        EntityKind::Other => "setup.kind.other",
    })
}

/// The key bar's Enter, Esc and Tab hints in the words of `tr` (index.html:1891).
#[must_use]
pub fn key_hints_in(tr: &Tr) -> Vec<crate::shell::KeyHint> {
    vec![
        crate::shell::KeyHint::new("Enter", tr.t("setup.key.continue")),
        crate::shell::KeyHint::new("Esc", tr.t("setup.key.back")),
        crate::shell::KeyHint::new("Tab", tr.t("setup.key.next_field")),
    ]
}

impl Screen for SetupScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("setup.title")
    }

    fn shows_breadcrumbs(&self) -> bool {
        false
    }

    /// The key bar's name (index.html:1891): the step's place among the seven.
    fn key_bar_name(&self, tr: &Tr) -> String {
        tr.tf(
            "setup.key_bar",
            &[
                ("n", &(self.step.index() + 1).to_string()),
                ("total", &Step::ALL.len().to_string()),
            ],
        )
    }

    /// The key bar (index.html:1891): the step's place, then Enter, Esc and Tab.
    fn keys(&self, tr: &Tr) -> Vec<crate::shell::KeyHint> {
        let mut keys = vec![crate::shell::KeyHint::new("", self.key_bar_name(tr))];
        keys.extend(key_hints_in(tr));
        keys
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.collect(ctx);
        if std::mem::take(&mut self.opened) {
            return ScreenAction::Replace(Box::new(GatewayScreen::new()));
        }
        if self.job.is_some() || self.open_job.is_some() {
            ui.ctx().request_repaint();
        }
        self.take_start_keys(ui);
        match self.take_keys(ui) {
            KeyOutcome::Continue => {
                if self.step == Step::Done {
                    return ScreenAction::Replace(Box::new(GatewayScreen::new()));
                }
                self.continue_step();
            }
            KeyOutcome::Back => {
                if let Some(back) = self.step.back() {
                    self.step = back;
                    self.notice = None;
                }
            }
            KeyOutcome::Pass => {}
        }
        if self.open_screen.take() == Some(OpenScreen::Import) {
            match navigate::screen_for_key(navigate::IMPORT_KEY, ctx) {
                Some(Ok(screen)) => return ScreenAction::Push(screen),
                Some(Err(e)) => self.notice = Some(e.to_string()),
                None => self.notice = Some(crate::i18n::t("setup.notice.screen_unavailable")),
            }
        }
        let mut chosen = self.take_launch_key(ui).map(DoneChoice::Open);
        if let Some(next) = self.draw_card(ui) {
            chosen = Some(next);
        }
        match chosen {
            None => ScreenAction::Stay,
            Some(DoneChoice::Home) => ScreenAction::Replace(Box::new(GatewayScreen::new())),
            Some(DoneChoice::Open(key)) => match navigate::screen_for_key(key, ctx) {
                Some(Ok(screen)) => ScreenAction::Push(screen),
                Some(Err(e)) => {
                    self.notice = Some(e.to_string());
                    ScreenAction::Stay
                }
                None => {
                    self.notice = Some(crate::i18n::t("setup.notice.screen_unavailable"));
                    ScreenAction::Stay
                }
            },
        }
    }
}

impl SetupScreen {
    /// The wizard's card (index.html:1894, CSS 311-318): the rail, the rule and the step's pane,
    /// at most 980 points wide and centred. Returns a Done choice.
    fn draw_card(&mut self, ui: &mut egui::Ui) -> Option<DoneChoice> {
        let mut chosen = None;
        let t = tokens(ui);
        let k = kind(ui);
        let width = ui.available_width().min(CARD_MAX_WIDTH);
        #[expect(clippy::float_arithmetic, reason = "centring the card")]
        let left = ((ui.available_width() - width) / 2.0).max(0.0);
        ui.add_space(CARD_MARGIN_TOP);
        ui.horizontal_top(|ui| {
            ui.add_space(left);
            let card = egui::Frame::new()
                .fill(t.surface)
                .stroke(egui::Stroke::new(border_width(k), t.border))
                .corner_radius(RADIUS_M)
                .shadow(shadow_1(k))
                .show(ui, |ui| {
                    // The card's outline is `width` wide, its border included.
                    #[expect(clippy::float_arithmetic, reason = "the border, both sides")]
                    let inner = width - 2.0 * border_width(k);
                    ui.set_width(inner);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let row = ui.horizontal_top(|ui| {
                        let rail = ui.vertical(|ui| {
                            ui.set_width(RAIL_WIDTH);
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(8, 18))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    self.rail(ui);
                                });
                        });
                        record_probe(ui, "setup.rail", rail.response.rect);
                        ui.vertical(|ui| {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(28, 22))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if let Some(next) = self.draw_pane(ui) {
                                        chosen = Some(next);
                                    }
                                });
                        });
                        rail.response.rect.right()
                    });
                    // The rule between the rail and the pane, the card's full height
                    // (index.html:312).
                    let x = row.inner;
                    ui.painter().vline(
                        x,
                        ui.min_rect().y_range(),
                        egui::Stroke::new(border_width(k), t.border),
                    );
                });
            record_probe(ui, "setup.card", card.response.rect);
        });
        chosen
    }

    /// The step's pane: its content, any notice, and Back and Continue. Returns a Done choice.
    fn draw_pane(&mut self, ui: &mut egui::Ui) -> Option<DoneChoice> {
        let mut chosen = None;
        match self.step {
            Step::Welcome => self.draw_welcome(ui),
            Step::Company => self.draw_company(ui),
            Step::Features => self.draw_features(ui),
            Step::Keyboard => self.draw_keyboard(ui),
            Step::Ai => self.draw_ai(ui),
            Step::Backup => self.draw_backup(ui),
            Step::Done => {
                let name = self.created_name.clone().unwrap_or_default();
                chosen = Self::draw_done(ui, &name);
            }
        }
        if let Some(notice) = &self.notice {
            let t = tokens(ui);
            ui.add_space(12.0);
            para(ui, notice, TEXT_META, t.bad);
        }
        if self.step != Step::Done {
            self.footer(ui);
        }
        chosen
    }
}
