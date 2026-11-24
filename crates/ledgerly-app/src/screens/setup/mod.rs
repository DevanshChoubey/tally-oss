//! Company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html` 1855-1895).
//!
//! The rules (steps, GSTIN and state, features, books date, what a company is) live in [`form`].
//! This file draws the steps and runs them. Creating the company file runs on a background
//! thread, as the welcome screen does, and the new company then replaces the open one.
//!
//! Two choices have no store yet, so their steps say so and save nothing: the keyboard preset and
//! the backup folder. The AI tier is stored with the new company, except the cloud tier, which
//! the engine refuses (see `design/PARITY-GAPS.md`).

pub mod form;

use crate::i18n::Tr;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::Key;
use ledgerly_api::{AiTier, Api};
use ledgerly_core::{Clock, Date, StateCode};
use ledgerly_model::{EntityKind, Features};

use self::form::{
    CompanyForm, GstinCheck, KeyInput, KeyOutcome, Step, ai_notice_in, build_company_in,
    check_gstin_in, default_features, feature_rows, financial_year_end_in, financial_year_start_in,
    parse_books_begin_in, state_from_gstin, step_label_in, step_outcome, unsaved_notice_in,
};
use crate::shell::navigate::{self, MASTERS_KEY};
use crate::shell::{GatewayScreen, hand_over, spawn_file_job};
use crate::theme::{
    RADIUS_M, RADIUS_S, TEXT_BODY, TEXT_CAP, TEXT_H1, TEXT_INPUT, TEXT_META, border_width, shadow_1,
};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::icon::{self, ICON_SIZE, Icon, icon};
use crate::ui::keycap;
use crate::ui::note::{NoteKind, note};
use crate::ui::option_card::{OptionCard, option_grid};
use crate::ui::probe::record_probe;
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
/// The check inside a passed step's marker.
const CHECK_SIZE: f32 = 14.0;
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
    /// The books begin date text, `YYYY-MM-DD`.
    books_begin: String,
    /// The features switched on.
    features: Features,
    /// The keyboard preset the user picked. Not stored yet.
    keyboard_classic: bool,
    /// The AI tier the user picked, stored with the new company ([`form::create_company_in`]).
    ai: AiTier,
    /// The backup folder typed. Not stored yet.
    backup_folder: String,
    /// What the last action said, shown on the step.
    notice: Option<String>,
    /// The name field takes focus when the company step opens, so typing goes there.
    focus_name: bool,
    /// The company file being created, if any.
    job: Option<Receiver<Created>>,
    /// The name of the company that was created, for the Done step.
    created_name: Option<String>,
    /// The strings of the language the user chose, set each frame from the app context.
    tr: Tr,
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
    /// A wizard whose financial year and audit stamps come from `clock`.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        let today = clock.now().date();
        let fy_start = financial_year_start_in(today, &Tr::default()).unwrap_or(today);
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
            books_begin: fy_start.to_string(),
            features: default_features(),
            keyboard_classic: true,
            ai: AiTier::Local,
            backup_folder: String::new(),
            notice: None,
            job: None,
            created_name: None,
            focus_name: false,
            tr: Tr::default(),
        }
    }

    /// The company form as the user has typed it. A GSTIN decides nothing here; [`check_gstin_in`]
    /// decides whether the step may go on.
    fn company_form(&self) -> Result<CompanyForm, String> {
        let state = self
            .state
            .ok_or_else(|| self.tr.t("setup.notice.choose_state"))?;
        let fy_end = financial_year_end_in(self.fy_start, &self.tr)?;
        let books_begin = parse_books_begin_in(&self.books_begin, self.fy_start, fy_end, &self.tr)?;
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
            return Some(self.tr.t("setup.notice.type_name"));
        }
        let Some(state) = self.state else {
            return Some(self.tr.t("setup.notice.choose_state"));
        };
        match check_gstin_in(&self.gstin, state, &self.tr) {
            GstinCheck::Blank | GstinCheck::Valid(_) => {}
            GstinCheck::Invalid(reason) => return Some(reason),
            GstinCheck::Mismatch { message, .. } => return Some(message),
        }
        self.company_form().err()
    }

    /// Continues from the current step. On Backup, that creates the company.
    fn continue_step(&mut self) {
        match self.step {
            Step::Welcome => {
                self.step = Step::Company;
                self.focus_name = true;
            }
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

    /// Starts creating the company file in the background. The wizard moves to Done when the
    /// company is open.
    fn create(&mut self) {
        if self.job.is_some() {
            return;
        }
        let company = match self
            .company_form()
            .and_then(|form| build_company_in(&form, self.features, &self.tr))
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
        let (clock, ai) = (self.clock, self.ai);
        let tr = self.tr.clone();
        match spawn_file_job(move || form::create_company_in(&path, &company, clock, ai, &tr)) {
            Ok(rx) => {
                self.job = Some(rx);
                self.notice = Some(self.tr.t("setup.notice.working"));
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// The file the company is saved in: what the user typed, or the default under Documents.
    fn file_path(&self) -> Result<PathBuf, String> {
        if !self.file.trim().is_empty() {
            return Ok(PathBuf::from(self.file.trim()));
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| self.tr.t("setup.notice.no_home"))?;
        let slug: String = self
            .name
            .trim()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        Ok(PathBuf::from(home)
            .join("Documents")
            .join("Ledgerly")
            .join(format!("{slug}.ledgerly")))
    }

    /// Checks the background create and, when it has answered, opens the company.
    fn collect(&mut self, ctx: &mut AppCtx) {
        let Some(rx) = self.job.as_ref() else {
            return;
        };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(self.tr.t("setup.notice.create_failed")),
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
        let tr = self.tr.clone();
        for step in Step::ALL {
            let marker = rail_marker(step, self.step);
            let clickable = marker == Marker::Passed && self.step != Step::Done;
            if rail_row(ui, step, marker, clickable, &tr).clicked() {
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
                    &self.tr.t("setup.nav.back"),
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
        let tr = &self.tr;
        match self.step {
            Step::Welcome => (tr.t("setup.welcome.create"), true),
            Step::Company => (tr.t("setup.nav.continue"), self.company_block().is_none()),
            Step::Features | Step::Keyboard | Step::Ai => (tr.t("setup.nav.continue"), true),
            Step::Backup => (tr.t("setup.nav.finish"), self.job.is_none()),
            Step::Done => (tr.t("setup.nav.go_home"), true),
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
            } else if i.consume_key(egui::Modifiers::NONE, Key::B) {
                Some(navigate::BANK_RECON_KEY)
            } else {
                None
            }
        })
    }

    /// The Welcome step (index.html:1863). Create is the one choice that works; the others say
    /// why they wait.
    fn draw_welcome(ui: &mut egui::Ui, tr: &Tr) {
        let s_welcome_title = tr.t("setup.welcome.title");
        let s_welcome_lead = tr.t("setup.welcome.lead");
        let s_welcome_create = tr.t("setup.welcome.create");
        let s_welcome_create_hint = tr.t("setup.welcome.create_hint");
        let s_welcome_import = tr.t("setup.welcome.import");
        let s_welcome_import_hint = tr.t("setup.welcome.import_hint");
        let s_welcome_unavailable = tr.t("setup.welcome.unavailable");
        let s_welcome_open = tr.t("setup.welcome.open");
        let s_welcome_open_hint = tr.t("setup.welcome.open_hint");
        let s_welcome_sample = tr.t("setup.welcome.sample");
        let s_welcome_sample_hint = tr.t("setup.welcome.sample_hint");
        let s_welcome_sample_unavailable = tr.t("setup.welcome.sample_unavailable");

        pane_head(ui, &s_welcome_title, &s_welcome_lead);
        let cards = [
            OptionCard::new(Icon::Plus, &s_welcome_create, &s_welcome_create_hint).selected(true),
            OptionCard::new(Icon::Upload, &s_welcome_import, &s_welcome_import_hint)
                .unavailable(&s_welcome_unavailable),
            OptionCard::new(Icon::File, &s_welcome_open, &s_welcome_open_hint)
                .unavailable(&s_welcome_unavailable),
            OptionCard::new(Icon::Eye, &s_welcome_sample, &s_welcome_sample_hint)
                .unavailable(&s_welcome_sample_unavailable),
        ];
        let drawn = option_grid(ui, egui::Id::unique("setup-welcome"), 2, &cards);
        for (n, card) in drawn.iter().enumerate() {
            record_probe(ui, &format!("setup.option.{n}"), card.card);
        }
    }

    /// The Company step (index.html:1866-1874): the mock's four-column field grid, each label
    /// above its field. The widgets are made in the order name, type, city, GSTIN, state, books
    /// date, file, so Tab walks them in that order.
    fn draw_company(&mut self, ui: &mut egui::Ui) {
        let tr = self.tr.clone();
        pane_head(ui, tr.t("setup.company.title"), tr.t("setup.company.lead"));
        let col = field_column(ui);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(ui, span(col, 2), tr.t("setup.field.name"), |ui, w| {
                let name = ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .id_salt("setup-name")
                        .desired_width(w),
                );
                if std::mem::take(&mut self.focus_name) {
                    name.request_focus();
                }
            });
            field(ui, col, tr.t("setup.field.type"), |ui, w| {
                egui::ComboBox::from_id_salt("setup-kind")
                    .width(w)
                    .selected_text(kind_label_in(self.kind, &tr))
                    .show_ui(ui, |ui| {
                        for kind in [
                            EntityKind::Company,
                            EntityKind::Llp,
                            EntityKind::Partnership,
                            EntityKind::Proprietorship,
                            EntityKind::Other,
                        ] {
                            ui.selectable_value(&mut self.kind, kind, kind_label_in(kind, &tr));
                        }
                    });
            });
            field(ui, col, tr.t("setup.field.city"), |ui, w| {
                ui.add(egui::TextEdit::singleline(&mut self.city).desired_width(w));
            });
        });
        ui.add_space(FIELD_GAP_Y);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(ui, span(col, 2), "GSTIN", |ui, w| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.gstin)
                        .hint_text(tr.t("setup.field.gstin_hint"))
                        .desired_width(w),
                );
                self.fill_state_from_gstin();
                self.gstin_hint(ui);
            });
            field(ui, col, tr.t("setup.field.state"), |ui, w| {
                let selected = self.state.map_or_else(
                    || tr.t("setup.field.state_pick"),
                    |s| format!("{:02} {}", s.get(), s.name()),
                );
                egui::ComboBox::from_id_salt("setup-state")
                    .width(w)
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for state in StateCode::all() {
                            let label = format!("{:02} {}", state.get(), state.name());
                            ui.selectable_value(&mut self.state, Some(state), label);
                        }
                    });
            });
            field(ui, col, tr.t("setup.field.gst_filing"), |ui, _| {
                read_only(ui, tr.t("setup.value.monthly"));
                hint(ui, tr.t("setup.hint.filing_unsaved"));
            });
        });
        self.draw_company_dates(ui, col);
    }

    /// The financial year, books date and file rows of the company step.
    fn draw_company_dates(&mut self, ui: &mut egui::Ui, col: f32) {
        let tr = self.tr.clone();
        ui.add_space(FIELD_GAP_Y);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(ui, span(col, 2), tr.t("setup.field.fy"), |ui, _| {
                let fy_end = financial_year_end_in(self.fy_start, &tr)
                    .map_or_else(|_| tr.t("setup.value.fy_end"), long_date);
                read_only(ui, format!("{} to {fy_end}", long_date(self.fy_start)));
            });
            field(
                ui,
                span(col, 2),
                tr.t("setup.field.books_begin"),
                |ui, w| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.books_begin)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(w),
                    );
                    hint(ui, tr.t("setup.hint.books_begin"));
                },
            );
        });
        ui.add_space(FIELD_GAP_Y);
        field(ui, span(col, 4), tr.t("setup.field.file"), |ui, w| {
            ui.add(
                egui::TextEdit::singleline(&mut self.file)
                    .hint_text(tr.t("setup.field.file_hint"))
                    .desired_width(w),
            );
        });
    }

    /// The line under the GSTIN: the check result, or why the GSTIN cannot be used
    /// (index.html:1868).
    fn gstin_hint(&self, ui: &mut egui::Ui) {
        let tr = self.tr.clone();
        let t = tokens(ui);
        match self
            .state
            .map(|state| check_gstin_in(&self.gstin, state, &tr))
        {
            None => hint(ui, tr.t("setup.hint.choose_state")),
            Some(GstinCheck::Blank) => {
                hint(ui, tr.t("setup.hint.gstin_blank"));
            }
            Some(GstinCheck::Valid(gstin)) => {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, BadgeKind::Ok, &tr.t("setup.badge.check_ok"));
                    ui.label(
                        egui::RichText::new(
                            self.tr.tf("setup.hint.pan", &[("pan", gstin.pan_str())]),
                        )
                        .size(TEXT_META)
                        .color(t.text_3),
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
        let tr = self.tr.clone();
        pane_head(
            ui,
            tr.t("setup.features.title"),
            tr.t("setup.features.lead"),
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
                        if switch(ui, *flag, egui::Sense::click()).clicked() {
                            *flag = !*flag;
                        }
                    } else {
                        switch(ui, false, egui::Sense::hover());
                    }
                });
                ui.add_space(FEATURE_SWITCH_GAP);
                ui.vertical(|ui| {
                    ui.add_space(FEATURE_PAD);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(tr.t(&format!("setup.feature.{}.label", row.key)))
                            .font(bold_font(ui, TEXT_BODY))
                            .color(t.text),
                    );
                    let hint = tr.t(&format!("setup.feature.{}.hint", row.key));
                    let line = match (row.switch, row.unavailable) {
                        (Some(_), _) => hint,
                        (None, unavailable) => {
                            let reason = if unavailable.is_some() {
                                tr.t(&format!("setup.feature.{}.reason", row.key))
                            } else {
                                tr.t("setup.features.not_built")
                            };
                            tr.tf(
                                "setup.features.unavailable",
                                &[("hint", hint.as_str()), ("reason", reason.as_str())],
                            )
                        }
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
        let tr = self.tr.clone();
        let s_keyboard_title = tr.t("setup.keyboard.title");
        let s_keyboard_classic = tr.t("setup.keyboard.classic");
        let s_keyboard_classic_hint = tr.t("setup.keyboard.classic_hint");
        let s_keyboard_standard = tr.t("setup.keyboard.standard");
        let s_keyboard_standard_hint = tr.t("setup.keyboard.standard_hint");
        pane_head(ui, &s_keyboard_title, tr.t("setup.keyboard.note"));
        let cards = [
            OptionCard::new(Icon::Kbd, &s_keyboard_classic, &s_keyboard_classic_hint)
                .selected(self.keyboard_classic)
                .keys(&["F8", "F5", "Ctrl+A", "Enter", "Esc"]),
            OptionCard::new(Icon::Kbd, &s_keyboard_standard, &s_keyboard_standard_hint)
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
        if let Some(notice) = unsaved_notice_in(Step::Keyboard, &tr) {
            step_text(ui, notice);
        }
    }

    /// The AI step (index.html:1879-1884). The tier is stored when the company is created; the
    /// cloud tier is refused by the engine, and [`ai_notice_in`] says so.
    fn draw_ai(&mut self, ui: &mut egui::Ui) {
        let tr = self.tr.clone();
        let s_step_ai = tr.t("setup.step.ai");
        let s_ai_lead = tr.t("setup.ai.lead");
        let s_ai_off = tr.t("setup.ai.off");
        let s_ai_off_hint = tr.t("setup.ai.off_hint");
        let s_ai_local = tr.t("setup.ai.local");
        let s_ai_local_hint = tr.t("setup.ai.local_hint");
        let s_ai_cloud = tr.t("setup.ai.cloud");
        let s_ai_cloud_hint = tr.t("setup.ai.cloud_hint");
        let s_ai_later = tr.t("setup.ai.later");
        pane_head(ui, &s_step_ai, &s_ai_lead);
        let tiers = [AiTier::Off, AiTier::Local, AiTier::Cloud];
        let cards = [
            OptionCard::new(Icon::Off, &s_ai_off, &s_ai_off_hint),
            OptionCard::new(Icon::Cpu, &s_ai_local, &s_ai_local_hint),
            OptionCard::new(Icon::Cloud, &s_ai_cloud, &s_ai_cloud_hint),
        ];
        let cards = cards
            .into_iter()
            .zip(tiers)
            .map(|(card, tier)| card.selected(self.ai == tier))
            .collect::<Vec<_>>();
        let drawn = option_grid(ui, egui::Id::unique("setup-ai"), 3, &cards);
        for (n, (card, tier)) in drawn.iter().zip(tiers).enumerate() {
            record_probe(ui, &format!("setup.option.{n}"), card.card);
            if card.response.clicked() {
                self.ai = tier;
            }
        }
        step_text(ui, &s_ai_later);
        if let Some(notice) = ai_notice_in(self.ai, &tr) {
            step_text(ui, notice);
        }
    }

    /// The Backup step (index.html:1885). The folder is not stored and no backup runs on a schedule.
    fn draw_backup(&mut self, ui: &mut egui::Ui) {
        let tr = self.tr.clone();
        pane_head(ui, tr.t("setup.backup.title"), tr.t("setup.backup.lead"));
        let col = field_column(ui);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_GAP_X;
            field(
                ui,
                span(col, 2),
                tr.t("setup.field.backup_folder"),
                |ui, w| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.backup_folder)
                            .hint_text(tr.t("setup.field.backup_hint"))
                            .desired_width(w),
                    );
                },
            );
            field(ui, col, tr.t("setup.field.keep"), |ui, _| {
                read_only(ui, tr.t("setup.value.keep"));
                hint(ui, tr.t("setup.hint.not_saved"));
            });
            field(ui, col, tr.t("setup.field.when"), |ui, _| {
                read_only(ui, tr.t("setup.value.when"));
                hint(ui, tr.t("setup.hint.not_saved"));
            });
        });
        ui.add_space(14.0);
        note(ui, NoteKind::Info, &tr.t("setup.backup.law"));
        if let Some(notice) = unsaved_notice_in(Step::Backup, &tr) {
            step_text(ui, notice);
        }
    }

    /// The Done step (index.html:1889-1894): three launch cards, each with its icon, bold label
    /// and key, and the primary "Go to Home". Returns the launch or Home chosen, if any.
    fn draw_done(ui: &mut egui::Ui, name: &str, tr: &Tr) -> Option<DoneChoice> {
        let t = tokens(ui);
        ui.horizontal(|ui| {
            icon(ui, Icon::Check, t.text);
            ui.label(
                egui::RichText::new(format!("{name} is ready"))
                    .font(bold_font(ui, TEXT_H1))
                    .color(t.text),
            );
        });
        ui.add_space(6.0);
        para(ui, &tr.t("setup.done.lead"), TEXT_BODY, t.text_2);
        ui.add_space(18.0);
        let launches = [
            Launch {
                glyph: Icon::Users,
                label: tr.t("setup.done.masters"),
                place: Some(tr.t("setup.done.masters_place")),
                key: "M",
                choice: Some(DoneChoice::Open(MASTERS_KEY)),
            },
            Launch {
                glyph: Icon::Sale,
                label: tr.t("setup.done.invoice"),
                place: None,
                key: "F8",
                choice: None,
            },
            Launch {
                glyph: Icon::Bank,
                label: tr.t("setup.done.bank"),
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
                    let response = launch_card(col, launch, tr);
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
                    &tr.t("setup.nav.go_home"),
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
fn launch_card(ui: &mut egui::Ui, launch: &Launch, tr: &Tr) -> egui::Response {
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
        return response.on_hover_text(tr.t("setup.tip.no_route"));
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
fn rail_row(
    ui: &mut egui::Ui,
    step: Step,
    marker: Marker,
    clickable: bool,
    tr: &Tr,
) -> egui::Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
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
    let radius = MARKER_SIZE / 2.0;
    let number = (n + 1).to_string();
    let cap = bold_font(ui, TEXT_CAP);
    match marker {
        Marker::Passed => {
            painter.circle_filled(dot.center(), radius, t.ok);
            let check = egui::Rect::from_center_size(dot.center(), egui::Vec2::splat(CHECK_SIZE));
            icon::paint(painter, check, Icon::Check, t.surface);
        }
        Marker::Current => {
            painter.circle_filled(dot.center(), radius, t.accent);
            painter.text(
                dot.center(),
                egui::Align2::CENTER_CENTER,
                number,
                cap,
                t.on_accent,
            );
        }
        Marker::Ahead => {
            painter.circle_stroke(
                dot.center(),
                radius - bw / 2.0,
                egui::Stroke::new(bw, t.border_strong),
            );
            painter.text(
                dot.center(),
                egui::Align2::CENTER_CENTER,
                number,
                cap,
                t.text_2,
            );
        }
    }
    let (font, ink) = if marker == Marker::Current {
        (bold_font(ui, TEXT_BODY), t.accent_ink)
    } else {
        (FontId::proportional(TEXT_BODY), t.text_2)
    };
    painter.text(
        egui::pos2(dot.right() + RAIL_LABEL_GAP, rect.center().y),
        egui::Align2::LEFT_CENTER,
        step_label_in(step, tr),
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
fn pane_head(ui: &mut egui::Ui, title: impl AsRef<str>, sub: impl AsRef<str>) {
    let (title, sub) = (title.as_ref(), sub.as_ref());
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
fn step_text(ui: &mut egui::Ui, text: impl AsRef<str>) {
    let text = text.as_ref();
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
fn field(
    ui: &mut egui::Ui,
    width: f32,
    label: impl AsRef<str>,
    add: impl FnOnce(&mut egui::Ui, f32),
) {
    let label = label.as_ref();
    let t = tokens(ui);
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(label).size(TEXT_META).color(t.text_2));
        add(ui, width);
    });
}

/// A value the user cannot change, in the field's place (`.ro`).
fn read_only(ui: &mut egui::Ui, text: impl AsRef<str>) {
    let text = text.as_ref();
    let t = tokens(ui);
    ui.label(egui::RichText::new(text).size(TEXT_INPUT).color(t.text));
}

/// A field's muted hint (`.fld .hint`).
fn hint(ui: &mut egui::Ui, text: impl AsRef<str>) {
    let text = text.as_ref();
    let t = tokens(ui);
    para(ui, text, TEXT_META, t.text_3);
}

/// A date as the mock writes it: `1 Apr 2026`.
fn long_date(date: Date) -> String {
    let month: String = date.month().to_string().chars().take(3).collect();
    format!("{} {month} {}", date.day(), date.year())
}

/// The name of a company type in the words of `tr`, as the mock's "Type" field shows it.
#[must_use]
pub fn kind_label_in(kind: EntityKind, tr: &Tr) -> String {
    match kind {
        EntityKind::Company => tr.t("setup.kind.company"),
        EntityKind::Llp => tr.t("setup.kind.llp"),
        EntityKind::Partnership => tr.t("setup.kind.partnership"),
        EntityKind::Proprietorship => tr.t("setup.kind.proprietorship"),
        EntityKind::Other => tr.t("setup.kind.other"),
    }
}

/// The key bar's hints after the screen's name, in the words of `tr`: Enter, Esc and Tab
/// (index.html:1891).
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

    /// The key bar's name (index.html:1891): the step's place among the seven.
    fn key_bar_name(&self, tr: &Tr) -> String {
        tr.tf(
            "setup.key_bar",
            &[
                ("n", (self.step.index() + 1).to_string().as_str()),
                ("total", Step::ALL.len().to_string().as_str()),
            ],
        )
    }

    /// The key bar (index.html:1891): the step's place, then Enter, Esc and Tab.
    fn keys(&self, tr: &Tr) -> Vec<crate::shell::KeyHint> {
        let mut hints = vec![crate::shell::KeyHint::new("", self.key_bar_name(tr))];
        hints.extend(key_hints_in(tr));
        hints
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tr = ctx.tr.clone();
        self.collect(ctx);
        if self.job.is_some() {
            ui.ctx().request_repaint();
        }
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
                    self.notice = Some(self.tr.t("setup.notice.screen_unavailable"));
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
            Step::Welcome => Self::draw_welcome(ui, &self.tr),
            Step::Company => self.draw_company(ui),
            Step::Features => self.draw_features(ui),
            Step::Keyboard => self.draw_keyboard(ui),
            Step::Ai => self.draw_ai(ui),
            Step::Backup => self.draw_backup(ui),
            Step::Done => {
                let name = self.created_name.clone().unwrap_or_default();
                chosen = Self::draw_done(ui, &name, &self.tr);
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
