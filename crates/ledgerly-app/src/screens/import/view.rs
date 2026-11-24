//! The screen: drawing and key handling for the wizard.

use std::path::PathBuf;

use egui::{Key, Modifiers, RichText, Sense, Stroke, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, DrCr, FinancialYear, Outcome, PeriodError, SignedAmount};

use super::direct::{DirectImport, DirectState};
use super::types::{
    DetectState, ImportError, ImportRequest, ImportSource, MAX_VOUCHERS_PER_REQUEST, MapReport,
    MapState, MapStatus, PullProgress, PullStage, PullState, ReconReport, ReconSectionReport,
    ReconState, RowStatus, Step, TALLY_PORT,
};
use super::wizard::ImportWizard;
use crate::theme::{RADIUS_M, RADIUS_S, TEXT_BODY, TEXT_H2, TEXT_META, border_width};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button, button_icon, style_combo};
use crate::ui::card::card;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::step_marker::{self, MARKER_SIZE, StepMarker};
use crate::ui::{a11y, bold_font, kind, para, tokens};
use crate::worker::ApiWorker;
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
    /// The one-call import path (see the `direct` module).
    direct: DirectImport,
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
            direct: DirectImport::default(),
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

    /// What the one-call import shows now.
    #[must_use]
    pub fn direct_state(&self) -> &DirectState {
        &self.direct.state
    }

    /// Sets the form of the one-call import: host, port text and Tally company name.
    pub fn set_direct_form(&mut self, host: &str, port: &str, company: &str) {
        host.clone_into(&mut self.direct.host);
        port.clone_into(&mut self.direct.port);
        company.clone_into(&mut self.direct.company);
    }

    /// Starts the one-call import on `worker`: `Api::import_tally_http` with the form's host,
    /// port and company. With no worker, or a bad field, the state becomes `Failed`.
    pub fn start_direct_import(&mut self, worker: Option<&ApiWorker>) {
        self.direct.start(worker);
    }

    /// Takes a finished one-call import answer, if there is one.
    pub fn tick(&mut self) {
        self.direct.tick();
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

/// Whether the host answers the step-by-step wizard's requests (detect, pull, map, commit,
/// reconcile). The shell does not run them yet, so the step-by-step controls are drawn disabled
/// with the reason; the one-step import is the path that works (`design/PARITY-GAPS.md`).
const STEP_BY_STEP_CONNECTED: bool = false;

/// The widest the screen gets, as the setup wizard's card.
const COLUMN_MAX_WIDTH: f32 = 980.0;

impl Screen for ImportScreen {
    fn title(&self) -> String {
        "Import from another app".to_owned()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick();
        if self.direct.state == DirectState::Running {
            ui.ctx().request_repaint();
        }
        let mut clicks = Vec::new();
        if ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            clicks.push(Click::Escape);
        }
        let width = ui.available_width().min(COLUMN_MAX_WIDTH);
        ui.vertical(|ui| {
            ui.set_max_width(width);
            page_head(
                ui,
                "Import from another app",
                Some("Bring your masters and vouchers in from another accounting app. Everything is read on this computer, and a report shows what moved."),
                |_| {},
            );
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                draw_steps(ui, &self.wizard);
                ui.add_space(6.0);
                rule(ui);
                ui.add_space(14.0);
                let no_company = ctx.api.is_none();
                match self.wizard.step() {
                    Step::Detect => {
                        if no_company {
                            note(
                                ui,
                                NoteKind::Warn,
                                "No company is open yet. An import fills an empty company file, so create a company first (Company setup), then come back here.",
                            );
                            ui.add_space(8.0);
                            if button(ui, "Back to setup", Some("Esc"), ButtonKind::Secondary)
                                .clicked()
                            {
                                clicks.push(Click::Close);
                            }
                            ui.add_space(14.0);
                        }
                        if draw_direct(ui, &mut self.direct, !no_company) {
                            self.direct.start(ctx.api.as_ref());
                        }
                        ui.add_space(12.0);
                        draw_detect(ui, &self.wizard, &mut self.export_path, &mut clicks);
                    }
                    Step::ChooseCompany => draw_choose(ui, &self.wizard, &mut clicks),
                    Step::Pull => draw_pull(ui, &self.wizard, &mut clicks),
                    Step::Map => draw_map(ui, &self.wizard, &mut clicks),
                    Step::Reconcile => {
                        draw_reconcile(ui, &self.wizard, &mut self.differences_only, &mut clicks);
                    }
                }
                if let Some(text) = &self.notice {
                    ui.add_space(12.0);
                    note(ui, NoteKind::Bad, text);
                }
            });
        });
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

/// A hairline across the card.
fn rule(ui: &mut Ui) {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), bw), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.border);
}

/// The step line: a marker and the step's name for each of the five steps, a hairline joining
/// them, as the setup wizard's rail marks its steps (check for passed, accent for the current).
#[expect(clippy::float_arithmetic, reason = "laying five steps along a row")]
#[expect(clippy::cast_precision_loss, reason = "five steps, exact in f32")]
fn draw_steps(ui: &mut Ui, wizard: &ImportWizard) {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), Sense::hover());
    let seg = rect.width() / Step::ALL.len() as f32;
    for (i, step) in Step::ALL.iter().enumerate() {
        let (marker, ink, font) = match step.cmp(&wizard.step()) {
            std::cmp::Ordering::Less => (
                StepMarker::Passed,
                t.text_2,
                egui::FontId::proportional(TEXT_BODY),
            ),
            std::cmp::Ordering::Equal => {
                (StepMarker::Current, t.accent_ink, bold_font(ui, TEXT_BODY))
            }
            std::cmp::Ordering::Greater => (
                StepMarker::Ahead,
                t.text_2,
                egui::FontId::proportional(TEXT_BODY),
            ),
        };
        let x0 = rect.left() + seg * i as f32;
        let center = egui::pos2(x0 + MARKER_SIZE / 2.0, rect.center().y);
        step_marker::paint(ui, center, marker, &step_number(*step).to_string());
        let label_rect = ui.painter().text(
            egui::pos2(center.x + MARKER_SIZE / 2.0 + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            step.label(),
            font,
            ink,
        );
        let next_marker = x0 + seg;
        if i + 1 < Step::ALL.len() && next_marker - label_rect.right() > 28.0 {
            ui.painter().hline(
                (label_rect.right() + 10.0)..=(next_marker - 10.0),
                rect.center().y,
                Stroke::new(bw, t.border),
            );
        }
        let name = format!("Step {} of {}: {}", i + 1, Step::ALL.len(), step.label());
        let r = ui.interact(
            egui::Rect::from_min_max(
                egui::pos2(x0, rect.top()),
                egui::pos2(label_rect.right(), rect.bottom()),
            ),
            ui.scope_id().with(("import-step", i)),
            Sense::hover(),
        );
        a11y::name_selected(ui, &r, egui::Role::ListItem, *step == wizard.step(), &name);
    }
}

/// A section heading inside the card (`h2`).
fn heading(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.label(
        RichText::new(text)
            .font(bold_font(ui, TEXT_H2))
            .color(t.text),
    );
    ui.add_space(2.0);
}

/// A muted line under a heading.
fn meta(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    para(ui, text, TEXT_META, t.text_2);
    ui.add_space(8.0);
}

/// A bordered panel inside the card, filled with the second surface.
fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    egui::Frame::new()
        .fill(t.surface_2)
        .stroke(Stroke::new(bw, t.border))
        .corner_radius(RADIUS_M)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A field: the label above, then the text box the full width of the column.
fn labelled_field(ui: &mut Ui, label: &str, text: &mut String, hint: &str, enabled: bool) {
    let t = tokens(ui);
    ui.add_space(4.0);
    ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
    ui.add_enabled_ui(enabled, |ui| {
        a11y::text_edit(
            ui,
            label,
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .desired_width(ui.available_width()),
        );
    });
}

/// A secondary or primary kit button that may be disabled. Returns `true` when clicked.
fn kit_button(
    ui: &mut Ui,
    label: &str,
    key: Option<&str>,
    kind_of: ButtonKind,
    enabled: bool,
) -> bool {
    ui.add_enabled_ui(enabled, |ui| button(ui, label, key, kind_of))
        .inner
        .clicked()
}

fn draw_detect(
    ui: &mut Ui,
    wizard: &ImportWizard,
    export_path: &mut String,
    clicks: &mut Vec<Click>,
) {
    panel(ui, |ui| {
        heading(ui, "Step by step, with a report");
        meta(
            ui,
            &format!(
                "Ledgerly looks for a running app on this computer, port {TALLY_PORT}, or reads an XML file exported from your previous accounting app. Nothing is sent anywhere else."
            ),
        );
        let usable = STEP_BY_STEP_CONNECTED && *wizard.detect_state() != DetectState::Checking;
        if kit_button(
            ui,
            "Look for a running app",
            None,
            ButtonKind::Secondary,
            usable,
        ) {
            clicks.push(Click::Detect);
        }
        labelled_field(
            ui,
            "Exported XML file",
            export_path,
            "path of the exported file",
            STEP_BY_STEP_CONNECTED,
        );
        ui.add_space(8.0);
        if kit_button(
            ui,
            "Read exported file",
            None,
            ButtonKind::Secondary,
            usable,
        ) {
            clicks.push(Click::OpenExport);
        }
        ui.add_space(10.0);
        if !STEP_BY_STEP_CONNECTED {
            note(
                ui,
                NoteKind::Info,
                "Not available yet: the step-by-step import and its reconciliation report are not connected to the app. Use the one-step import below.",
            );
        }
        match wizard.detect_state() {
            DetectState::NotChecked | DetectState::Found => {}
            DetectState::Checking => {
                note(ui, NoteKind::Info, "Looking for companies...");
            }
            DetectState::NotRunning { port } => {
                note(
                    ui,
                    NoteKind::Bad,
                    &format!(
                        "The other app is not running or not accepting connections on port {port}. Start it, enable its XML interface, and try again."
                    ),
                );
            }
            DetectState::Failed(reason) => {
                note(ui, NoteKind::Bad, &format!("Error: {reason}"));
            }
        }
    });
}

/// The one-call import form. Returns `true` when the user pressed the button.
fn draw_direct(ui: &mut Ui, direct: &mut DirectImport, can_run: bool) -> bool {
    let mut pressed = false;
    panel(ui, |ui| {
        heading(ui, "Import in one step");
        meta(
            ui,
            "Reads the whole company from a running app into the open, empty company file.",
        );
        // Plain rows, not an `egui::Grid`: a grid's first layout asks egui to run the frame again,
        // and the second pass would no longer see a key (Esc) the first pass consumed.
        ui.columns(3, |cols| {
            if let [host, port, company] = cols {
                labelled_field(host, "Host", &mut direct.host, "", can_run);
                labelled_field(port, "Port", &mut direct.port, "", can_run);
                labelled_field(
                    company,
                    "Company in the other app",
                    &mut direct.company,
                    "its exact name",
                    can_run,
                );
            }
        });
        ui.add_space(10.0);
        let running = direct.state == DirectState::Running;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(can_run && !running, |ui| {
                pressed = button_icon(
                    ui,
                    Some(Icon::Upload),
                    "Import the company",
                    None,
                    ButtonKind::Primary,
                )
                .clicked();
            });
        });
        if let Some(text) = direct.state.text() {
            ui.add_space(10.0);
            let kind_of = match &direct.state {
                DirectState::Failed(_) | DirectState::Refused(_) => NoteKind::Bad,
                DirectState::Imported(_) => NoteKind::Ok,
                DirectState::Idle | DirectState::Running => NoteKind::Info,
            };
            note(ui, kind_of, &text);
        }
    });
    pressed
}

/// One company of the list: a row that shows the accent when chosen.
fn choice_row(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
    let (fill, stroke) = if selected {
        (t.accent_soft, Stroke::new(bw + 1.0, t.accent))
    } else {
        (t.surface, Stroke::new(bw, t.border))
    };
    ui.painter().rect_filled(rect, RADIUS_S, fill);
    ui.painter()
        .rect_stroke(rect, RADIUS_S, stroke, egui::StrokeKind::Inside);
    let font = if selected {
        bold_font(ui, TEXT_BODY)
    } else {
        egui::FontId::proportional(TEXT_BODY)
    };
    let ink = if selected { t.accent_ink } else { t.text };
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        ink,
    );
    a11y::name_selected(ui, &response, egui::Role::RadioButton, selected, text);
    a11y::focus_ring(ui, &response, rect);
    response
}

/// Back at the left and the primary action at the right.
fn nav(
    ui: &mut Ui,
    back: Option<Click>,
    forward: Option<(&str, Click, bool)>,
    clicks: &mut Vec<Click>,
) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        if let Some(click) = back
            && button(ui, "Back", None, ButtonKind::Secondary).clicked()
        {
            clicks.push(click);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((label, click, enabled)) = forward
                && kit_button(ui, label, None, ButtonKind::Primary, enabled)
            {
                clicks.push(click);
            }
        });
    });
}

fn draw_choose(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    heading(ui, "Choose a company");
    meta(
        ui,
        "The companies the other app lists. Pick the one to bring in.",
    );
    for (i, name) in wizard.companies().iter().enumerate() {
        if choice_row(ui, name.as_str(), wizard.selected() == Some(i)).clicked() {
            clicks.push(Click::Select(i));
        }
        ui.add_space(4.0);
    }
    ui.add_space(8.0);
    rule(ui);
    ui.add_space(10.0);
    heading(ui, "Years to pull");
    meta(ui, "Indian financial years, April to March.");
    let (mut first, mut last) = wizard.years();
    ui.horizontal(|ui| {
        style_combo(ui);
        year_combo(
            ui,
            "u10-first-year",
            "From",
            wizard.fy_options(),
            &mut first,
        );
        ui.add_space(12.0);
        year_combo(ui, "u10-last-year", "To", wizard.fy_options(), &mut last);
    });
    if (first, last) != wizard.years() {
        clicks.push(Click::Years(first, last));
    }
    if let Err(e) = wizard.period() {
        ui.add_space(8.0);
        note(ui, NoteKind::Bad, &format!("Error: {e}"));
    }
    ui.add_space(8.0);
    let t = tokens(ui);
    para(
        ui,
        &format!(
            "Vouchers are fetched in batches of at most {MAX_VOUCHERS_PER_REQUEST} per request."
        ),
        TEXT_META,
        t.text_3,
    );
    nav(
        ui,
        Some(Click::Back),
        Some(("Start pull", Click::StartPull, wizard.selected().is_some())),
        clicks,
    );
}

fn year_combo(
    ui: &mut Ui,
    id: &'static str,
    label: &str,
    options: &[FinancialYear],
    value: &mut FinancialYear,
) {
    let t = tokens(ui);
    ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.label())
        .show_ui(ui, |ui| {
            for option in options {
                ui.selectable_value(value, *option, option.label());
            }
        });
}

fn draw_pull(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    heading(ui, "Pull");
    match wizard.pull_state() {
        PullState::Idle => {
            meta(ui, "Not started.");
        }
        PullState::Running(progress) => {
            draw_progress(ui, progress);
            ui.add_space(10.0);
            if button(ui, "Cancel", Some("Esc"), ButtonKind::Secondary).clicked() {
                clicks.push(Click::Cancel);
            }
        }
        PullState::Cancelling(progress) => {
            draw_progress(ui, progress);
            ui.add_space(8.0);
            note(
                ui,
                NoteKind::Info,
                "Cancelling: waiting for the current request to stop. Nothing is imported.",
            );
        }
        PullState::Done(summary) => {
            note(
                ui,
                NoteKind::Ok,
                &format!(
                    "Pulled {} vouchers in {} batches. Raw XML saved in {}. Nothing is imported yet.",
                    summary.vouchers,
                    summary.batches,
                    summary.capture_dir.display()
                ),
            );
            nav(
                ui,
                Some(Click::Back),
                Some(("Continue to mapping", Click::ToMap, true)),
                clicks,
            );
        }
        PullState::Cancelled { batches_kept } => {
            note(
                ui,
                NoteKind::Warn,
                &format!(
                    "Pull cancelled. Nothing was imported. {batches_kept} batches of raw XML stay on disk."
                ),
            );
            nav(ui, Some(Click::Back), None, clicks);
        }
        PullState::Failed(reason) => {
            note(ui, NoteKind::Bad, &format!("Error: {reason}"));
            nav(ui, Some(Click::Back), None, clicks);
        }
    }
}

fn draw_progress(ui: &mut Ui, progress: &PullProgress) {
    let t = tokens(ui);
    for stage in PullStage::ALL {
        let (kind_of, word) = match stage.cmp(&progress.stage) {
            std::cmp::Ordering::Less => (BadgeKind::Ok, "Done"),
            std::cmp::Ordering::Equal => (BadgeKind::Accent, "Now"),
            std::cmp::Ordering::Greater => (BadgeKind::Neutral, "Waiting"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(stage.label()).size(TEXT_BODY).color(t.text));
            badge(ui, kind_of, word);
        });
        ui.add_space(2.0);
    }
    if progress.stage == PullStage::Vouchers {
        ui.add_space(8.0);
        let total = progress
            .batches_total
            .map_or_else(|| "?".to_owned(), |t| t.to_string());
        let text = format!(
            "{} of {total} batches, {} vouchers",
            progress.batches_done, progress.vouchers_pulled
        );
        match progress.batch_percent() {
            Some(percent) => {
                ui.add(
                    egui::ProgressBar::new(f32::from(percent) / 100.0)
                        .fill(t.accent)
                        .text(text),
                );
            }
            None => {
                para(ui, &text, TEXT_BODY, t.text_2);
            }
        }
    }
}

fn draw_map(ui: &mut Ui, wizard: &ImportWizard, clicks: &mut Vec<Click>) {
    heading(ui, "Map");
    match wizard.map_state() {
        MapState::NotStarted => {
            meta(ui, "Not started.");
        }
        MapState::Working => {
            note(ui, NoteKind::Info, "Staging and mapping...");
        }
        MapState::Failed(reason) => {
            note(ui, NoteKind::Bad, &format!("Error: {reason}"));
            nav(ui, Some(Click::Back), None, clicks);
        }
        MapState::Ready(report) => draw_map_report(ui, report, clicks),
    }
}

fn draw_map_report(ui: &mut Ui, report: &MapReport, clicks: &mut Vec<Click>) {
    let t = tokens(ui);
    let counts: Vec<String> = report
        .kind_counts()
        .iter()
        .map(|(kind, n)| format!("{} {n}", kind.label()))
        .collect();
    meta(ui, &counts.join(", "));
    let unmapped = report.unmapped_count();
    let warnings = report.warning_count();
    let (kind_of, text) = if unmapped == 0 {
        (
            NoteKind::Ok,
            format!("{unmapped} unmapped, {warnings} with warnings."),
        )
    } else {
        (
            NoteKind::Bad,
            format!(
                "{unmapped} unmapped, {warnings} with warnings. Nothing is imported until none is unmapped."
            ),
        )
    };
    note(ui, kind_of, &text);
    ui.add_space(10.0);
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            egui::Grid::new("u10-map-grid")
                .striped(true)
                .spacing(egui::vec2(18.0, 8.0))
                .show(ui, |ui| {
                    for row in &report.rows {
                        ui.label(
                            RichText::new(row.kind.label())
                                .size(TEXT_META)
                                .color(t.text_2),
                        );
                        ui.label(
                            RichText::new(row.tally_name.as_str())
                                .size(TEXT_BODY)
                                .color(t.text),
                        );
                        match &row.status {
                            MapStatus::Mapped => {
                                badge(ui, BadgeKind::Ok, "Mapped");
                            }
                            MapStatus::Warning(w) => {
                                badge(ui, BadgeKind::Warn, &format!("Warning: {w}"));
                            }
                            MapStatus::Unmapped(w) => {
                                badge(ui, BadgeKind::Bad, &format!("UNMAPPED: {w}"));
                            }
                        }
                        ui.end_row();
                    }
                });
            for warning in &report.warnings {
                ui.add_space(6.0);
                note(ui, NoteKind::Warn, &format!("Warning: {warning}"));
            }
        });
    nav(
        ui,
        Some(Click::Back),
        Some(("Import into books", Click::ConfirmMap, unmapped == 0)),
        clicks,
    );
}

fn draw_reconcile(
    ui: &mut Ui,
    wizard: &ImportWizard,
    differences_only: &mut bool,
    clicks: &mut Vec<Click>,
) {
    heading(ui, "Reconciliation");
    match wizard.recon_state() {
        ReconState::NotStarted => {
            meta(ui, "Not started.");
        }
        ReconState::Committing => {
            note(
                ui,
                NoteKind::Info,
                "Writing to the books. This cannot be interrupted.",
            );
        }
        ReconState::CommitFailed(reason) => {
            note(
                ui,
                NoteKind::Bad,
                &format!("Error: {reason}. Nothing was written."),
            );
            nav(ui, Some(Click::Back), None, clicks);
        }
        ReconState::Reconciling => {
            note(
                ui,
                NoteKind::Info,
                "Imported. Comparing with the source app's own reports...",
            );
        }
        ReconState::Failed(reason) => {
            note(
                ui,
                NoteKind::Bad,
                &format!(
                    "Error: {reason}. The data is in the books; the comparison did not finish."
                ),
            );
            ui.add_space(8.0);
            if button(ui, "Run the comparison again", None, ButtonKind::Secondary).clicked() {
                clicks.push(Click::RetryReconcile);
            }
        }
        ReconState::Ready(report) => draw_report(ui, report, differences_only),
    }
    nav(ui, None, Some(("Close", Click::Close, true)), clicks);
}

fn draw_report(ui: &mut Ui, report: &ReconReport, differences_only: &mut bool) {
    let summary = report.summary();
    if summary.all_match() {
        note(
            ui,
            NoteKind::Ok,
            &format!(
                "All {} figures match the source app's own reports.",
                summary.rows
            ),
        );
    } else {
        note(
            ui,
            NoteKind::Bad,
            &format!(
                "{} of {} figures differ from the source app's own reports. {} sections were not compared.",
                summary.differing_rows,
                summary.rows,
                summary.not_compared.len()
            ),
        );
    }
    let t = tokens(ui);
    for section in &summary.not_compared {
        ui.add_space(4.0);
        para(
            ui,
            &format!("Not compared: {}", section.label()),
            TEXT_META,
            t.text_2,
        );
    }
    ui.add_space(8.0);
    ui.checkbox(differences_only, "Show only differences");
    ui.add_space(6.0);
    egui::ScrollArea::vertical()
        .max_height(420.0)
        .show(ui, |ui| {
            for (n, section) in report.sections.iter().enumerate() {
                draw_section(ui, n, section, *differences_only);
            }
        });
}

fn draw_section(ui: &mut Ui, n: usize, section: &ReconSectionReport, differences_only: bool) {
    let t = tokens(ui);
    ui.add_space(8.0);
    ui.label(
        RichText::new(section.section.label())
            .font(bold_font(ui, TEXT_H2))
            .color(t.text),
    );
    ui.add_space(4.0);
    match &section.outcome {
        Outcome::Unsupported(u) => {
            para(
                ui,
                &format!("Not compared: {} ({})", u.what, u.reason),
                TEXT_BODY,
                t.text_2,
            );
        }
        Outcome::Value(rows) => {
            egui::Grid::new(("u10-recon-grid", n))
                .striped(true)
                .spacing(egui::vec2(18.0, 8.0))
                .show(ui, |ui| {
                    for head in ["Item", "Ledgerly", "Source app", "Result"] {
                        ui.label(
                            RichText::new(head)
                                .font(bold_font(ui, TEXT_META))
                                .color(t.text_2),
                        );
                    }
                    ui.end_row();
                    for row in rows {
                        let status = row.status();
                        if differences_only && status == RowStatus::Match {
                            continue;
                        }
                        ui.label(
                            RichText::new(row.label.as_str())
                                .size(TEXT_BODY)
                                .color(t.text),
                        );
                        ui.label(
                            RichText::new(format_figure(row.ours, "none"))
                                .size(TEXT_BODY)
                                .color(t.text),
                        );
                        ui.label(
                            RichText::new(format_figure(row.theirs, "none"))
                                .size(TEXT_BODY)
                                .color(t.text),
                        );
                        let badge_kind = match status {
                            RowStatus::Match => BadgeKind::Ok,
                            RowStatus::Differs | RowStatus::OnlyInTally | RowStatus::OnlyInOurs => {
                                BadgeKind::Bad
                            }
                            RowStatus::NotReported => BadgeKind::Warn,
                        };
                        badge(ui, badge_kind, status.label());
                        ui.end_row();
                    }
                });
        }
    }
}
