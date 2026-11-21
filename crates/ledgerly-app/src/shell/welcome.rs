//! The welcome screen: create a company file or open one (spec 7).
//!
//! Opening and creating touch the disk, so they run on a background thread. The screen polls the
//! thread each frame and, when it returns an [`Api`], moves it onto a new [`ApiWorker`] in
//! [`AppCtx::api`]. The previous worker, if any, is dropped on another thread, because dropping a
//! worker waits for its queued jobs.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use ledgerly_api::Api;
use ledgerly_core::{Clock, CompanyId, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::{Date, Month};

use crate::screens::item_invoice::parse_place;
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

/// The answer of a background open or create: the opened company, or the reason it failed.
type JobResult = Result<Api, String>;

/// Root screen of a new tab: opens or creates a company file.
#[derive(Debug)]
pub struct WelcomeScreen {
    /// Clock that stamps the audit records of a new company file.
    clock: Clock,
    /// Path typed in the "open" row.
    open_path: String,
    /// Name typed in the "create" row.
    name: String,
    /// Path typed in the "create" row.
    new_path: String,
    /// State code or state name typed in the "create" row.
    state: String,
    /// The financial year starting on 1 April, as a year (for example `2026` for 2026-27).
    fy_year: String,
    /// What the last action said, shown under the forms.
    notice: Option<String>,
    /// The background job in progress, if any.
    job: Option<Receiver<JobResult>>,
}

impl Default for WelcomeScreen {
    fn default() -> Self {
        Self::with_clock(Clock::System)
    }
}

impl WelcomeScreen {
    /// A welcome screen whose new company files are stamped by `clock`.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            open_path: String::new(),
            name: String::new(),
            new_path: String::new(),
            state: String::new(),
            fy_year: String::new(),
            notice: None,
            job: None,
        }
    }

    /// `true` while a background open or create has not answered yet.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Starts opening the company file at the path in the "open" row. Returns at once.
    pub fn start_open(&mut self) {
        let path = PathBuf::from(self.open_path.trim());
        if self.open_path.trim().is_empty() {
            self.notice = Some("Type the path of a company file to open.".to_owned());
            return;
        }
        self.spawn_job(move || Api::open(&path).map_err(|e| e.to_string()));
    }

    /// Starts creating the company described by the "create" row. Returns at once.
    ///
    /// Nothing is started when the form does not describe a company; the reason is shown.
    pub fn start_create(&mut self) {
        let company = match company_from_form(&self.name, &self.state, &self.fy_year) {
            Ok(company) => company,
            Err(reason) => {
                self.notice = Some(reason);
                return;
            }
        };
        if self.new_path.trim().is_empty() {
            self.notice = Some("Type the path of the new company file.".to_owned());
            return;
        }
        let path = PathBuf::from(self.new_path.trim());
        let clock = self.clock;
        self.spawn_job(move || Api::create(&path, &company, clock).map_err(|e| e.to_string()));
    }

    /// Runs `job` on a new thread and remembers where its answer will arrive.
    fn spawn_job<F>(&mut self, job: F)
    where
        F: FnOnce() -> JobResult + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<JobResult>();
        let spawned = std::thread::Builder::new()
            .name("ledgerly-file".to_owned())
            .spawn(move || {
                // The screen may have gone; then the answer is not wanted.
                let _unwanted = tx.send(job());
            });
        match spawned {
            Ok(_handle) => {
                self.job = Some(rx);
                self.notice = Some("Working on the company file...".to_owned());
            }
            Err(e) => self.notice = Some(format!("Could not start the file work: {e}")),
        }
    }

    /// Checks the background job and, when it has answered, hands the company to the worker.
    fn collect(&mut self, ctx: &mut AppCtx) {
        let Some(rx) = self.job.as_ref() else {
            return;
        };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(
                "the company file could not be read; try again or check the path".to_owned(),
            ),
        };
        self.job = None;
        match answer {
            Ok(api) => self.install(ctx, api),
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Puts `api` on a new worker in `ctx`. The previous worker stops on a separate thread.
    fn install(&mut self, ctx: &mut AppCtx, api: Api) {
        match ApiWorker::spawn(api) {
            Ok(worker) => {
                if let Some(previous) = ctx.api.replace(worker) {
                    let _stopping = std::thread::spawn(move || drop(previous));
                }
                self.notice = Some("The company file is open.".to_owned());
            }
            Err(e) => self.notice = Some(format!("Could not start the worker: {e}")),
        }
    }
}

/// Builds the company a "create" form describes. The company has no addresses or GST
/// registrations yet; the user adds them in the masters.
///
/// `state` is a state code (for example `27`) or a state name. `fy_year` is the year the financial
/// year starts, on 1 April (for example `2026` for 2026-27). Books begin on the first day of the
/// financial year.
///
/// # Errors
/// A message naming the field that is not acceptable.
pub fn company_from_form(name: &str, state: &str, fy_year: &str) -> Result<Company, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name for the company.".to_owned());
    }
    let state: StateCode = parse_place(state).map_err(|e| e.to_string())?;
    let year: i32 = fy_year
        .trim()
        .parse()
        .map_err(|_| "The financial year must be a year, for example 2026.".to_owned())?;
    let fy_start = Date::from_calendar_date(year, Month::April, 1)
        .map_err(|_| "That financial year is outside the supported range.".to_owned())?;
    Ok(Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: name.to_owned(),
        mailing_name: None,
        address: Address::default(),
        state,
        pan: None,
        registrations: Vec::new(),
        fy_start,
        books_begin: fy_start,
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    })
}

impl Screen for WelcomeScreen {
    fn title(&self) -> String {
        "Home".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.collect(ctx);
        ui.heading("Ledgerly");
        if ctx.api.is_some() {
            ui.label("A company is open.");
        } else {
            ui.label("No company is open.");
        }
        if self.is_busy() {
            ui.label("Working on the company file...");
            ui.ctx().request_repaint();
        }
        let busy = self.is_busy();
        ui.separator();
        ui.label("Open a company file");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.open_path)
                    .hint_text("path to a .ledgerly file")
                    .desired_width(360.0),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Open"))
                .clicked()
            {
                self.start_open();
            }
        });
        ui.separator();
        ui.label("Create a company file");
        egui::Grid::new("welcome-create").show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(280.0));
            ui.end_row();
            ui.label("File");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_path)
                    .hint_text("path for the new .ledgerly file")
                    .desired_width(280.0),
            );
            ui.end_row();
            ui.label("State");
            ui.add(
                egui::TextEdit::singleline(&mut self.state)
                    .hint_text("state code, for example 27")
                    .desired_width(280.0),
            );
            ui.end_row();
            ui.label("Financial year from 1 April");
            ui.add(
                egui::TextEdit::singleline(&mut self.fy_year)
                    .hint_text("for example 2026")
                    .desired_width(280.0),
            );
            ui.end_row();
        });
        if ui
            .add_enabled(!busy, egui::Button::new("Create"))
            .clicked()
        {
            self.start_create();
        }
        if let Some(notice) = &self.notice {
            ui.label(notice.as_str());
        }
        ScreenAction::Stay
    }
}
