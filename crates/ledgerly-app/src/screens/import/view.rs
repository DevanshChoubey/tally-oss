//! The screen: drawing and key handling for the wizard.

use std::path::PathBuf;

use egui::{Key, Modifiers, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, DrCr, FinancialYear, Outcome, PeriodError, SignedAmount};

use super::types::{
    DetectState, ImportError, ImportRequest, ImportSource, MAX_VOUCHERS_PER_REQUEST, MapReport,
    MapState, MapStatus, PullProgress, PullStage, PullState, ReconReport, ReconSectionReport,
    ReconState, RowStatus, Step, TALLY_PORT,
};
use super::wizard::ImportWizard;
use crate::{AppCtx, Screen, ScreenAction};

// ---------------------------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------------------------

/// An amount with Indian grouping and its side, for example `12,34,567.00 Dr`.
#[must_use]
pub fn format_signed_amount(amount: SignedAmount) -> String {
    let side = match amount.side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    };
    format!("{} {side}", format_inr(amount.amount))
}

fn format_figure(figure: Option<SignedAmount>, absent: &str) -> String {
    figure.map_or_else(|| absent.to_owned(), format_signed_amount)
}

// ---------------------------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------------------------

/// What the user did in one frame.
#[derive(Clone, Copy)]
enum Click {
    Escape,
    Detect,
    OpenExport,
    Select(usize),
    Years(FinancialYear, FinancialYear),
    StartPull,
    Cancel,
    Back,
    ToMap,
    ConfirmMap,
    RetryReconcile,
    Close,
}

/// The import wizard as a screen (spec 6.7).
#[derive(Debug)]
pub struct ImportScreen {
    wizard: ImportWizard,
    export_path: String,
    differences_only: bool,
    notice: Option<String>,
}

impl ImportScreen {
    /// A wizard screen. `today` picks the financial years on offer.
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] when `today` is at the edge of the supported calendar.
    pub fn new(today: Date) -> Result<Self, PeriodError> {
        Ok(ImportScreen {
            wizard: ImportWizard::new(today)?,
            export_path: String::new(),
            differences_only: false,
            notice: None,
        })
    }

    /// The state machine, for the host to read.
    #[must_use]
    pub fn wizard(&self) -> &ImportWizard {
        &self.wizard
    }

    /// The state machine, for the host to answer requests.
    pub fn wizard_mut(&mut self) -> &mut ImportWizard {
        &mut self.wizard
    }

    /// Takes the work the screen wants done; see [`ImportWizard::take_requests`].
    pub fn take_requests(&mut self) -> Vec<ImportRequest> {
        self.wizard.take_requests()
    }

    /// The last refusal shown under the step, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    fn note_result(&mut self, result: Result<(), ImportError>) {
        self.notice = result.err().map(|e| e.to_string());
    }

    fn open_export(&mut self) -> Result<(), ImportError> {
        let text = self.export_path.trim();
        if text.is_empty() {
            return Err(ImportError::NoExportPath);
        }
        let path = PathBuf::from(text);
        self.wizard.start_detect(ImportSource::Export(path))
    }

    fn apply(&mut self, click: Click) -> ScreenAction {
        let result = match click {
            Click::Escape => return self.escape(),
            Click::Close => return self.leave(),
            Click::Detect => self
                .wizard
                .start_detect(ImportSource::Live { port: TALLY_PORT }),
            Click::OpenExport => self.open_export(),
            Click::Select(i) => self.wizard.select_company(i),
            Click::Years(first, last) => self.wizard.set_years(first, last),
            Click::StartPull => self.wizard.start_pull(),
            Click::Cancel => self.wizard.cancel(),
            Click::Back => self.wizard.back(),
            Click::ToMap => self.wizard.continue_to_map(),
            Click::ConfirmMap => self.wizard.confirm_map(),
            Click::RetryReconcile => self.wizard.retry_reconcile(),
        };
        self.note_result(result);
        ScreenAction::Stay
    }

    fn leave(&mut self) -> ScreenAction {
        if self.wizard.can_leave() {
            self.wizard.close();
            ScreenAction::Pop
        } else {
            self.notice =
                Some("wait: this step cannot be interrupted; cancel the pull instead".to_owned());
            ScreenAction::Stay
        }
    }

    fn escape(&mut self) -> ScreenAction {
        if matches!(self.wizard.pull_state(), PullState::Running(_)) {
            let result = self.wizard.cancel();
            self.note_result(result);
            ScreenAction::Stay
        } else {
            self.leave()
        }
    }
}

impl Screen for ImportScreen {
    fn title(&self) -> String {
        "Import from Tally".to_owned()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
        let mut clicks = Vec::new();
        if ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            clicks.push(Click::Escape);
        }
        draw_steps(ui, &self.wizard);
        ui.separator();
        match self.wizard.step() {
            Step::Detect => draw_detect(ui, &self.wizard, &mut self.export_path, &mut clicks),
            Step::ChooseCompany => draw_choose(ui, &self.wizard, &mut clicks),
            Step::Pull => draw_pull(ui, &self.wizard, &mut clicks),
            Step::Map => draw_map(ui, &self.wizard, &mut clicks),
            Step::Reconcile => {
                draw_reconcile(ui, &self.wizard, &mut self.differences_only, &mut clicks);
            }
        }
        if let Some(text) = &self.notice {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {text}"));
        }
        let mut action = ScreenAction::Stay;
        for click in clicks {
            if matches!(action, ScreenAction::Pop) {
                break;
            }
            action = self.apply(click);
        }
        action
    }
}

fn step_number(step: Step) -> usize {
    Step::ALL
        .iter()
        .position(|s| *s == step)
        .map_or(0, |p| p.saturating_add(1))
}

fn draw_steps(ui: &mut Ui, wizard: &ImportWizard) {
    ui.horizontal_wrapped(|ui| {
        for step in Step::ALL {
            let state = if step == wizard.step() {
                "current"
            } else if step < wizard.step() {
                "done"
            } else {
                "to do"
            };
            let text = format!("{}. {} ({state})", step_number(step), step.label());
            if step == wizard.step() {
                ui.strong(text);
            } else {
                ui.label(text);
            }
        }
    });
}

fn draw_detect(
    ui: &mut Ui,
    wizard: &ImportWizard,
    export_path: &mut String,
    clicks: &mut Vec<Click>,
) {
    ui.heading("Import from Tally");
    ui.label(format!(
        "Ledgerly looks for Tally on this computer, port {TALLY_PORT}. Nothing is sent anywhere else."
    ));
    let checking = *wizard.detect_state() == DetectState::Checking;
    if ui
        .add_enabled(!checking, egui::Button::new("Look for Tally"))
        .clicked()
    {
        clicks.push(Click::Detect);
    }
    ui.label("Or read a file you exported from Tally:");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(export_path);
        if ui
            .add_enabled(!checking, egui::Button::new("Read exported file"))
            .clicked()
        {
            clicks.push(Click::OpenExport);
        }
    });
    match wizard.detect_state() {
        DetectState::NotChecked | DetectState::Found => {}
        DetectState::Checking => {
            ui.label("Looking for companies...");
        }
        DetectState::NotRunning { port } => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!(
                    "Tally is not running or not accepting connections on port {port}. Start Tally, enable its XML interface, and try again."
                ),
            );
        }
        DetectState::Failed(reason) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {reason}"));
        }
    }
}

fn draw_choose(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    ui.heading("Choose a company");
    for (i, name) in wizard.companies().iter().enumerate() {
        if ui
            .selectable_label(wizard.selected() == Some(i), name.as_str())
            .clicked()
        {
            clicks.push(Click::Select(i));
        }
    }
    ui.separator();
    ui.label("Years to pull (Indian financial years, April to March)");
    let (mut first, mut last) = wizard.years();
    ui.horizontal(|ui| {
        year_combo(
            ui,
            "u10-first-year",
            "From",
            wizard.fy_options(),
            &mut first,
        );
        year_combo(ui, "u10-last-year", "To", wizard.fy_options(), &mut last);
    });
    if (first, last) != wizard.years() {
        clicks.push(Click::Years(first, last));
    }
    if let Err(e) = wizard.period() {
        ui.colored_label(ui.visuals().error_fg_color, format!("Error: {e}"));
    }
    ui.weak(format!(
        "Vouchers are fetched in batches of at most {MAX_VOUCHERS_PER_REQUEST} per request."
    ));
    ui.horizontal(|ui| {
        if ui.button("Back").clicked() {
            clicks.push(Click::Back);
        }
        if ui
            .add_enabled(wizard.selected().is_some(), egui::Button::new("Start pull"))
            .clicked()
        {
            clicks.push(Click::StartPull);
        }
    });
}

fn year_combo(
    ui: &mut Ui,
    id: &'static str,
    label: &str,
    options: &[FinancialYear],
    value: &mut FinancialYear,
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.label())
        .show_ui(ui, |ui| {
            for option in options {
                ui.selectable_value(value, *option, option.label());
            }
        });
}

fn draw_pull(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    ui.heading("Pull");
    match wizard.pull_state() {
        PullState::Idle => {
            ui.label("Not started.");
        }
        PullState::Running(progress) => {
            draw_progress(ui, progress);
            if ui.button("Cancel (Esc)").clicked() {
                clicks.push(Click::Cancel);
            }
        }
        PullState::Cancelling(progress) => {
            draw_progress(ui, progress);
            ui.label("Cancelling: waiting for the current request to stop. Nothing is imported.");
        }
        PullState::Done(summary) => {
            ui.label(format!(
                "Pulled {} vouchers in {} batches. Raw XML saved in {}. Nothing is imported yet.",
                summary.vouchers,
                summary.batches,
                summary.capture_dir.display()
            ));
            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    clicks.push(Click::Back);
                }
                if ui.button("Continue to mapping").clicked() {
                    clicks.push(Click::ToMap);
                }
            });
        }
        PullState::Cancelled { batches_kept } => {
            ui.label(format!(
                "Pull cancelled. Nothing was imported. {batches_kept} batches of raw XML stay on disk."
            ));
            if ui.button("Back").clicked() {
                clicks.push(Click::Back);
            }
        }
        PullState::Failed(reason) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {reason}"));
            if ui.button("Back").clicked() {
                clicks.push(Click::Back);
            }
        }
    }
}

fn draw_progress(ui: &mut Ui, progress: &PullProgress) {
    for stage in PullStage::ALL {
        let state = if stage < progress.stage {
            "done"
        } else if stage == progress.stage {
            "now"
        } else {
            "waiting"
        };
        ui.label(format!("{}: {state}", stage.label()));
    }
    if progress.stage == PullStage::Vouchers {
        let total = progress
            .batches_total
            .map_or_else(|| "?".to_owned(), |t| t.to_string());
        let text = format!(
            "{} of {total} batches, {} vouchers",
            progress.batches_done, progress.vouchers_pulled
        );
        match progress.batch_percent() {
            Some(percent) => {
                ui.add(egui::ProgressBar::new(f32::from(percent) / 100.0).text(text));
            }
            None => {
                ui.label(text);
            }
        }
    }
}

fn draw_map(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    ui.heading("Map");
    match wizard.map_state() {
        MapState::NotStarted => {
            ui.label("Not started.");
        }
        MapState::Working => {
            ui.label("Staging and mapping...");
        }
        MapState::Failed(reason) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {reason}"));
            if ui.button("Back").clicked() {
                clicks.push(Click::Back);
            }
        }
        MapState::Ready(report) => draw_map_report(ui, report, clicks),
    }
}

fn draw_map_report(ui: &mut Ui, report: &MapReport, clicks: &mut Vec<Click>) {
    let counts: Vec<String> = report
        .kind_counts()
        .iter()
        .map(|(kind, n)| format!("{} {n}", kind.label()))
        .collect();
    ui.label(counts.join(", "));
    let unmapped = report.unmapped_count();
    let warnings = report.warning_count();
    ui.label(format!("{unmapped} unmapped, {warnings} with warnings."));
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("u10-map-grid")
            .striped(true)
            .show(ui, |ui| {
                for row in &report.rows {
                    ui.label(row.kind.label());
                    ui.label(row.tally_name.as_str());
                    match &row.status {
                        MapStatus::Mapped => ui.label("Mapped"),
                        MapStatus::Warning(w) => ui.label(format!("Warning: {w}")),
                        MapStatus::Unmapped(w) => {
                            ui.colored_label(ui.visuals().error_fg_color, format!("UNMAPPED: {w}"))
                        }
                    };
                    ui.end_row();
                }
            });
        for warning in &report.warnings {
            ui.label(format!("Warning: {warning}"));
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Back").clicked() {
            clicks.push(Click::Back);
        }
        if ui
            .add_enabled(unmapped == 0, egui::Button::new("Import into books"))
            .clicked()
        {
            clicks.push(Click::ConfirmMap);
        }
    });
}

fn draw_reconcile(
    ui: &mut Ui,
    wizard: &ImportWizard,
    differences_only: &mut bool,
    clicks: &mut Vec<Click>,
) {
    ui.heading("Reconciliation");
    match wizard.recon_state() {
        ReconState::NotStarted => {
            ui.label("Not started.");
        }
        ReconState::Committing => {
            ui.label("Writing to the books. This cannot be interrupted.");
        }
        ReconState::CommitFailed(reason) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Error: {reason}. Nothing was written."),
            );
            if ui.button("Back").clicked() {
                clicks.push(Click::Back);
            }
        }
        ReconState::Reconciling => {
            ui.label("Imported. Comparing with Tally's own reports...");
        }
        ReconState::Failed(reason) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!(
                    "Error: {reason}. The data is in the books; the comparison did not finish."
                ),
            );
            if ui.button("Run the comparison again").clicked() {
                clicks.push(Click::RetryReconcile);
            }
        }
        ReconState::Ready(report) => draw_report(ui, report, differences_only),
    }
    if ui.button("Close (Esc)").clicked() {
        clicks.push(Click::Close);
    }
}

fn draw_report(ui: &mut Ui, report: &ReconReport, differences_only: &mut bool) {
    let summary = report.summary();
    if summary.all_match() {
        ui.strong(format!(
            "All {} figures match Tally's own reports.",
            summary.rows
        ));
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            format!(
                "{} of {} figures differ from Tally's own reports. {} sections were not compared.",
                summary.differing_rows,
                summary.rows,
                summary.not_compared.len()
            ),
        );
    }
    for section in &summary.not_compared {
        ui.label(format!("Not compared: {}", section.label()));
    }
    ui.checkbox(differences_only, "Show only differences");
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (n, section) in report.sections.iter().enumerate() {
            draw_section(ui, n, section, *differences_only);
        }
    });
}

fn draw_section(ui: &mut Ui, n: usize, section: &ReconSectionReport, differences_only: bool) {
    ui.strong(section.section.label());
    match &section.outcome {
        Outcome::Unsupported(u) => {
            ui.label(format!("Not compared: {} ({})", u.what, u.reason));
        }
        Outcome::Value(rows) => {
            egui::Grid::new(("u10-recon-grid", n))
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Item");
                    ui.strong("Ledgerly");
                    ui.strong("Tally");
                    ui.strong("Result");
                    ui.end_row();
                    for row in rows {
                        let status = row.status();
                        if differences_only && status == RowStatus::Match {
                            continue;
                        }
                        ui.label(row.label.as_str());
                        ui.label(format_figure(row.ours, "none"));
                        ui.label(format_figure(row.theirs, "none"));
                        ui.label(status.label());
                        ui.end_row();
                    }
                });
        }
    }
}
