//! Company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html` 1855-1895).
//!
//! The rules (steps, GSTIN and state, features, books date, what a company is) live in [`form`].
//! This file draws the steps and runs them. Creating the company file runs on a background
//! thread, as the welcome screen does, and the new company then replaces the open one.
//!
//! Three choices have no store yet, so their steps say so and save nothing: the keyboard preset,
//! the AI tier and the backup folder (see `design/PARITY-GAPS.md`).

pub mod form;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::Key;
use ledgerly_api::Api;
use ledgerly_core::{Clock, Date, StateCode};
use ledgerly_model::{EntityKind, Features};

use self::form::{
    CompanyForm, GstinCheck, KeyInput, KeyOutcome, Step, build_company, check_gstin,
    default_features, feature_rows, financial_year_end, financial_year_start, parse_books_begin,
    state_from_gstin, step_outcome, unsaved_notice,
};
use crate::shell::navigate::{self, MASTERS_KEY};
use crate::shell::{GatewayScreen, hand_over, spawn_create};
use crate::{AppCtx, Screen, ScreenAction};

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
const RAIL_WIDTH: f32 = 220.0;

/// The AI choice on the AI step (index.html:1880-1882). It is not stored yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiChoice {
    /// No AI anywhere.
    Off,
    /// The local model on this computer.
    Local,
    /// A cloud provider with the user's key.
    Cloud,
}

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
    /// The AI choice the user picked. Not stored yet.
    ai: AiChoice,
    /// The backup folder typed. Not stored yet.
    backup_folder: String,
    /// What the last action said, shown on the step.
    notice: Option<String>,
    /// The name field takes focus when the company step opens, so typing goes there.
    focus_name: bool,
    /// The company file being created, if any.
    job: Option<Receiver<Result<Api, String>>>,
    /// The name of the company that was created, for the Done step.
    created_name: Option<String>,
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
            books_begin: fy_start.to_string(),
            features: default_features(),
            keyboard_classic: true,
            ai: AiChoice::Local,
            backup_folder: String::new(),
            notice: None,
            job: None,
            created_name: None,
            focus_name: false,
        }
    }

    /// The company form as the user has typed it. A GSTIN decides nothing here; [`check_gstin`]
    /// decides whether the step may go on.
    fn company_form(&self) -> Result<CompanyForm, String> {
        let state = self
            .state
            .ok_or_else(|| "Choose the state the company is in.".to_owned())?;
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
            return Some("Type a name for the company.".to_owned());
        }
        let Some(state) = self.state else {
            return Some("Choose the state the company is in.".to_owned());
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
        match spawn_create(path, company, self.clock) {
            Ok(rx) => {
                self.job = Some(rx);
                self.notice = Some("Working on the company file...".to_owned());
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
            .ok_or_else(|| {
                "Type where the company file goes; no home folder is known.".to_owned()
            })?;
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
            Err(TryRecvError::Disconnected) => Err(
                "the company file could not be created; try again or check the folder".to_owned(),
            ),
        };
        self.job = None;
        match answer {
            Ok(api) => self.open_created(ctx, api),
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Puts the new company on the worker and moves to Done. The wizard stays on Done until the
    /// user goes Home.
    fn open_created(&mut self, ctx: &mut AppCtx, api: Api) {
        match hand_over(ctx, api) {
            Ok(()) => {
                self.created_name = Some(self.name.trim().to_owned());
                self.notice = None;
                self.step = Step::Done;
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// The step rail, a list down the left of the card (index.html:1860). A step already passed
    /// can be revisited by clicking it, until the company exists: revisiting would create a second.
    fn rail(&mut self, ui: &mut egui::Ui) {
        for step in Step::ALL {
            let current = step == self.step;
            let done = step.index() < self.step.index();
            let mark = if done {
                "✓".to_owned()
            } else {
                (step.index() + 1).to_string()
            };
            let label = format!("{mark}  {}", step.label());
            let enabled = done && self.step != Step::Done;
            let button = egui::Button::new(label)
                .selected(current)
                .min_size(egui::vec2(RAIL_WIDTH, 28.0));
            if ui.add_enabled(enabled, button).clicked() {
                self.step = step;
                self.notice = None;
            }
        }
    }

    /// Back and Continue (index.html:1861).
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let back = self.step.back();
            let back_pressed = back.is_some() && ui.button("Back  (Esc)").clicked();
            if let Some(back) = back.filter(|_| back_pressed) {
                self.step = back;
                self.notice = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, enabled) = self.continue_label_and_gate();
                if ui
                    .add_enabled(enabled, egui::Button::new(format!("{label}  (Enter)")))
                    .clicked()
                {
                    self.continue_step();
                }
            });
        });
    }

    /// The Continue button's label and whether it is enabled on this step. On Company, a GSTIN
    /// that disagrees with the state disables it (index.html:1861).
    fn continue_label_and_gate(&self) -> (&'static str, bool) {
        match self.step {
            Step::Welcome => ("Create a company", true),
            Step::Company => ("Continue", self.company_block().is_none()),
            Step::Features | Step::Keyboard | Step::Ai => ("Continue", true),
            Step::Backup => ("Finish", self.job.is_none()),
            Step::Done => ("Go to Home", true),
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

    /// The Welcome step. Create is the one choice that works; the others say why they wait.
    fn draw_welcome(ui: &mut egui::Ui) {
        ui.heading("Welcome to Ledgerly");
        ui.label("Accounting, GST and bank reconciliation on your own computer. Your books are a file you own.");
        ui.add_space(12.0);
        ui.columns(2, |cols| {
            let [c0, c1] = cols else { return };
            option_card(
                c0,
                true,
                true,
                "Create a company",
                "About 3 minutes. Name, GSTIN, financial year.",
            );
            option_card(
                c1,
                false,
                false,
                "Import from another accounting app",
                "Not available in this wizard yet (PARITY-GAPS).",
            );
        });
        ui.columns(2, |cols| {
            let [c0, c1] = cols else { return };
            option_card(
                c0,
                false,
                false,
                "Open a Ledgerly file",
                "Not available in this wizard yet (PARITY-GAPS).",
            );
            option_card(
                c1,
                false,
                false,
                "Try the sample company",
                "No sample data ships yet (PARITY-GAPS).",
            );
        });
    }

    /// The Company step (index.html:1866-1874).
    fn draw_company(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your company");
        ui.label("You can change all of this later except the financial year start.");
        ui.add_space(8.0);
        egui::Grid::new("setup-company")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Company name");
                let name = ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .id_salt("setup-name")
                        .desired_width(360.0),
                );
                if std::mem::take(&mut self.focus_name) {
                    name.request_focus();
                }
                ui.end_row();

                ui.label("Type");
                egui::ComboBox::from_id_salt("setup-kind")
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
                ui.end_row();

                ui.label("City");
                ui.add(egui::TextEdit::singleline(&mut self.city).desired_width(360.0));
                ui.end_row();

                ui.label("GSTIN");
                ui.add(
                    egui::TextEdit::singleline(&mut self.gstin)
                        .hint_text("15 characters, or blank")
                        .desired_width(360.0),
                );
                ui.end_row();
            });
        self.fill_state_from_gstin();
        ui.add_space(4.0);
        let check = self.state.map(|state| check_gstin(&self.gstin, state));
        match check {
            None => ui.weak("Choose the state below."),
            Some(GstinCheck::Blank) => {
                ui.weak("Leave blank if the business is not registered for GST yet.")
            }
            Some(GstinCheck::Valid(gstin)) => ui.weak(format!(
                "Check character OK. PAN {} · fills the state.",
                gstin.pan_str()
            )),
            Some(GstinCheck::Invalid(reason)) => {
                ui.colored_label(ui.visuals().error_fg_color, reason)
            }
            Some(GstinCheck::Mismatch { message, .. }) => {
                ui.colored_label(ui.visuals().error_fg_color, message)
            }
        };
        self.draw_company_rest(ui);
    }

    /// The state, filing, financial year, books date and file of the company step.
    fn draw_company_rest(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        egui::Grid::new("setup-company-2")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("State");
                let selected = self.state.map_or_else(
                    || "Choose a state".to_owned(),
                    |s| format!("{:02} {}", s.get(), s.name()),
                );
                egui::ComboBox::from_id_salt("setup-state")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for state in StateCode::all() {
                            let label = format!("{:02} {}", state.get(), state.name());
                            ui.selectable_value(&mut self.state, Some(state), label);
                        }
                    });
                ui.end_row();

                ui.label("GST filing");
                ui.weak("Monthly. Not saved yet: the model has no filing frequency (PARITY-GAPS).");
                ui.end_row();

                ui.label("Financial year");
                let fy_end = financial_year_end(self.fy_start)
                    .map_or_else(|_| "the end of March".to_owned(), long_date);
                ui.label(format!("{} to {fy_end}", long_date(self.fy_start)));
                ui.end_row();

                ui.label("Books begin from");
                ui.add(
                    egui::TextEdit::singleline(&mut self.books_begin)
                        .hint_text("YYYY-MM-DD")
                        .desired_width(160.0),
                );
                ui.end_row();

                ui.label("Company file");
                ui.add(
                    egui::TextEdit::singleline(&mut self.file)
                        .hint_text("blank: Documents/Ledgerly/<company>.ledgerly")
                        .desired_width(360.0),
                );
                ui.end_row();
            });
        ui.weak(
            "Use a later date if you are moving in mid-year; opening balances go on this date.",
        );
    }

    /// The Features step (index.html:1871-1878). Only the switches the model has are working.
    fn draw_features(&mut self, ui: &mut egui::Ui) {
        ui.heading("What do you need?");
        ui.label("Turn on only what you use. Each one adds fields and screens.");
        ui.add_space(8.0);
        for row in feature_rows() {
            if let Some(switch) = row.switch {
                let flag = switch(&mut self.features);
                ui.checkbox(flag, row.label);
                ui.weak(row.hint);
            } else {
                let mut off = false;
                ui.add_enabled(false, egui::Checkbox::new(&mut off, row.label));
                let reason = row.unavailable.unwrap_or("not built yet");
                ui.weak(format!("{} Not available: {reason}", row.hint));
            }
            ui.add_space(6.0);
        }
    }

    /// The Keyboard step (index.html:1883-1886). The preset is not stored yet.
    fn draw_keyboard(&mut self, ui: &mut egui::Ui) {
        ui.heading("Which keys do you know?");
        ui.label("Both presets have every action on a key. Mouse works everywhere too.");
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            let [c0, c1] = cols else { return };
            let classic = option_card(
                c0,
                self.keyboard_classic,
                true,
                "Classic accountant keys (recommended)",
                "F8 sales, F9 purchase, F5 payment, F6 receipt, Ctrl+A save, Enter to move on, Esc to go back.",
            );
            if classic.clicked() {
                self.keyboard_classic = true;
            }
            let standard = option_card(
                c1,
                !self.keyboard_classic,
                true,
                "Standard desktop keys",
                "Ctrl+S save, Ctrl+N new, Tab to move on. Function keys still work as aliases.",
            );
            if standard.clicked() {
                self.keyboard_classic = false;
            }
        });
        ui.add_space(8.0);
        if let Some(notice) = unsaved_notice(Step::Keyboard) {
            ui.weak(notice);
        }
    }

    /// The AI step (index.html:1879-1884). The choice is not stored yet.
    fn draw_ai(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI and privacy");
        ui.label("AI only drafts. You accept every draft, and the engine computes every amount. Pick per company; change any time.");
        ui.add_space(8.0);
        ui.columns(3, |cols| {
            let [c0, c1, c2] = cols else { return };
            let off = option_card(c0, self.ai == AiChoice::Off, true, "Off", "No AI anywhere. Exact and rule matching still work.");
            if off.clicked() {
                self.ai = AiChoice::Off;
            }
            let local = option_card(c1, self.ai == AiChoice::Local, true, "On this computer", "A small local model. Nothing leaves this computer. Slower on old machines.");
            if local.clicked() {
                self.ai = AiChoice::Local;
            }
            let cloud = option_card(c2, self.ai == AiChoice::Cloud, true, "Cloud, with your own key", "Better reading of poor photos. You confirm each document before it is sent; bank numbers, PAN and phone numbers are removed first.");
            if cloud.clicked() {
                self.ai = AiChoice::Cloud;
            }
        });
        ui.add_space(8.0);
        if let Some(notice) = unsaved_notice(Step::Ai) {
            ui.weak(notice);
        }
    }

    /// The Backup step (index.html:1885). The folder is not stored and no backup runs on a schedule.
    fn draw_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backups");
        ui.label(
            "Ledgerly saves as you type. A backup is a separate copy, made every day you work.",
        );
        ui.add_space(8.0);
        egui::Grid::new("setup-backup")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Backup folder");
                ui.add(
                    egui::TextEdit::singleline(&mut self.backup_folder)
                        .hint_text("Documents/Ledgerly backups")
                        .desired_width(360.0),
                );
                ui.end_row();
                ui.label("Keep");
                ui.weak("Last 30 days. Not saved yet.");
                ui.end_row();
                ui.label("When");
                ui.weak("On close, daily. Not saved yet.");
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.weak("Companies that keep books electronically must keep a daily backup on a server located in India. This firm is a partnership, but the habit is good. Ledgerly makes the file; where you copy it is your choice.");
        if let Some(notice) = unsaved_notice(Step::Backup) {
            ui.weak(notice);
        }
    }

    /// The Done step (index.html:1889-1894). Returns the screen to open, if a launch was chosen.
    fn draw_done(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &AppCtx,
    ) -> Result<Option<ScreenAction>, String> {
        let name = self.created_name.clone().unwrap_or_default();
        ui.heading(format!("{name} is ready"));
        ui.label("Three good first steps. Each has a key.");
        ui.add_space(8.0);
        let mut chosen = None;
        ui.horizontal(|ui| {
            if ui.button("Enter opening balances  (Masters)").clicked() {
                chosen = Some(MASTERS_KEY);
            }
            if ui.button("Import a bank statement").clicked() {
                chosen = Some(navigate::BANK_RECON_KEY);
            }
            ui.add_enabled(false, egui::Button::new("Make your first invoice"));
        });
        match chosen {
            None => Ok(None),
            Some(key) => match navigate::screen_for_key(key, ctx) {
                Some(Ok(screen)) => Ok(Some(ScreenAction::Push(screen))),
                Some(Err(e)) => Err(e.to_string()),
                None => Err("that screen is not available yet".to_owned()),
            },
        }
    }
}

/// A date as the mock writes it: `1 Apr 2026`.
fn long_date(date: Date) -> String {
    let month: String = date.month().to_string().chars().take(3).collect();
    format!("{} {month} {}", date.day(), date.year())
}

/// The name of a company type, as the mock's "Type" field shows it.
fn kind_label(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Company => "Company",
        EntityKind::Llp => "Limited liability partnership",
        EntityKind::Partnership => "Partnership firm",
        EntityKind::Proprietorship => "Sole proprietorship",
        EntityKind::Other => "Trust, society or other",
    }
}

/// One option card: a selectable button with the title and a line of text (the mock's `optCard`).
fn option_card(
    ui: &mut egui::Ui,
    selected: bool,
    enabled: bool,
    title: &str,
    text: &str,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(format!("{title}\n{text}"))
            .selected(selected)
            .min_size(egui::vec2(ui.available_width(), 72.0)),
    )
}

impl Screen for SetupScreen {
    fn title(&self) -> String {
        "Company setup".to_owned()
    }

    fn keys(&self) -> Vec<crate::shell::KeyHint> {
        vec![
            crate::shell::KeyHint::new("Enter", "Continue"),
            crate::shell::KeyHint::new("Esc", "Back"),
        ]
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
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
        let mut action = ScreenAction::Stay;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(RAIL_WIDTH);
                    self.rail(ui);
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_min_width(ui.available_width());
                    match self.step {
                        Step::Welcome => Self::draw_welcome(ui),
                        Step::Company => self.draw_company(ui),
                        Step::Features => self.draw_features(ui),
                        Step::Keyboard => self.draw_keyboard(ui),
                        Step::Ai => self.draw_ai(ui),
                        Step::Backup => self.draw_backup(ui),
                        Step::Done => match self.draw_done(ui, ctx) {
                            Ok(Some(next)) => action = next,
                            Ok(None) => {}
                            Err(reason) => self.notice = Some(reason),
                        },
                    }
                    if let Some(notice) = &self.notice {
                        ui.colored_label(ui.visuals().error_fg_color, notice.as_str());
                    }
                    if self.step != Step::Done {
                        self.footer(ui);
                    }
                });
            });
        });
        action
    }
}
