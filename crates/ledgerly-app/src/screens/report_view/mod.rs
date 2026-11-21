//! Report viewer with drill-down. **Card U7** implements this module, wired to `ledgerly-api` in
//! card W7.
//!
//! Frozen interface (plan card U7):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `ReportView`. What spec 6.2 and 7 ask of a report, and where it lives:
//!
//! * Enter drills into the selected row (group, then ledger, then voucher); Esc returns with the
//!   selection preserved ([`ReportViewScreen::drill`]). A drill pushes a new screen, so the shell
//!   keeps this one, with its selection and scroll position, underneath; Esc is left to the shell,
//!   which pops the stack.
//! * Alt+F1 toggles condensed and detailed ([`ReportViewScreen::toggle_detail`]). The report
//!   reshapes in place and the selection stays on the same row when that row still exists.
//! * F2 edits the period ([`ReportViewScreen::open_period_editor`], [`parse_period_text`]).
//!
//! # Where the figures come from (rule C4)
//!
//! This screen sums nothing and rounds nothing. Every figure is copied from a `ledgerly-api`
//! report by [`convert::view_report`] and the screen only formats it ([`cell_text`]). A cell that
//! cannot be shown as a figure is shown as words ("Not supported yet: ..."), never as a number. A
//! report whose rows do not each carry one cell per column is rejected ([`ViewError::RowShape`])
//! rather than drawn.
//!
//! # Requests and staleness (review finding U7)
//!
//! Refresh and drill are asynchronous. Each is a [`ReportRequest`]:
//!
//! * With an API worker in the context ([`crate::AppCtx::api`]), the screen runs the request on the
//!   worker itself and polls it each frame. The UI thread never waits (rule D1).
//! * Without one (tests, or a host that runs requests elsewhere), the host takes the request with
//!   [`ReportViewScreen::take_request`] and answers it with [`ReportViewScreen::answer`].
//!
//! Only the newest request counts. A request that starts while an earlier one is still running
//! drops the earlier one, and an answer to an earlier request is ignored, so a slow answer never
//! replaces a newer one. A drill to a voucher is answered by the host with a screen
//! ([`ReportViewScreen::answer_drill_target`]), because only the shell can open the voucher form.
//!
//! Dr and Cr are written as words next to each amount, so no meaning rests on colour alone.

pub mod convert;
mod model;
mod text;
pub mod wire;

use egui::{Align, Context, Key, Layout, Modifiers, RichText, Ui};
use egui_extras::{Column, TableBuilder, TableRow};
use ledgerly_api::{Detail, ReportParams};
use ledgerly_core::{Outcome, Period};

use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};
use wire::{Done, Latest, unsupported_message, unsupported_text};

pub use convert::{drill_params, run_report, view_report, with_view};
pub use model::{
    DrillTarget, ReportRequest, ReportSource, ReportToken, RequestKind, ViewCell, ViewDetail,
    ViewError, ViewKey, ViewParams, ViewReport, ViewRow, ViewRowKind,
};
pub use text::{
    PeriodTextError, amount_text, cell_text, date_text, parse_period_date, parse_period_text,
    period_text,
};
use text::numeric_date_text;

/// Height of one table row, in points.
const ROW_HEIGHT: f32 = 22.0;
/// Rows moved by Page Up and Page Down.
const PAGE_ROWS: usize = 10;
/// Indent per nesting level of a row label, in points.
const INDENT: f32 = 16.0;

// ---- screen ----------------------------------------------------------------------------------

/// A message under the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// Information.
    Info(String),
    /// Something that did not happen, and why.
    Error(String),
}

impl Notice {
    /// The message.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Notice::Info(s) | Notice::Error(s) => s,
        }
    }
}

struct PeriodEdit {
    from: String,
    to: String,
    focus_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyCmd {
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
    Open,
    ToggleDetail,
    EditPeriod,
    ApplyPeriod,
    CancelPeriod,
}

#[derive(Default)]
struct TableEvents {
    clicked: Option<usize>,
    opened: Option<usize>,
}

/// The request whose answer the screen is waiting for.
#[derive(Debug, Clone)]
struct Awaiting {
    /// Its sequence number.
    seq: u64,
    /// Refresh or drill.
    kind: RequestKind,
    /// The parameters it runs with.
    params: ReportParams,
}

/// Shows one report, with row selection, drill-down, the detail toggle and the period editor.
pub struct ReportViewScreen {
    report: ViewReport,
    /// The parameters the shown report was run with; a refresh or drill starts from them.
    source: ReportSource,
    selected: usize,
    scroll_pending: bool,
    period_edit: Option<PeriodEdit>,
    notice: Option<Notice>,
    /// The request nobody has taken yet. Only the newest one is kept.
    queued: Option<ReportRequest>,
    /// The request whose answer is wanted.
    awaiting: Option<Awaiting>,
    /// The call running on the API worker, if any.
    latest: Latest<Result<Outcome<ViewReport>, ViewError>>,
    /// A screen a drill opened, pushed on the next frame.
    pending_push: Option<Box<dyn Screen>>,
}

impl ReportViewScreen {
    /// A screen for `report`, run with `source`, with the first row selected.
    ///
    /// # Errors
    /// [`ViewError::RowShape`] or [`ViewError::TotalsShape`] when the report is not rectangular.
    pub fn new(report: ViewReport, source: ReportSource) -> Result<Self, ViewError> {
        report.check_shape()?;
        Ok(Self {
            report,
            source,
            selected: 0,
            scroll_pending: false,
            period_edit: None,
            notice: None,
            queued: None,
            awaiting: None,
            latest: Latest::default(),
            pending_push: None,
        })
    }

    /// The report on show.
    #[must_use]
    pub fn report(&self) -> &ViewReport {
        &self.report
    }

    /// Index of the selected row (0 when the report has no rows).
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The selected row, if the report has rows.
    #[must_use]
    pub fn selected_row(&self) -> Option<&ViewRow> {
        self.report.rows.get(self.selected)
    }

    /// The message under the header, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// `true` while the period editor is open.
    #[must_use]
    pub fn period_editor_open(&self) -> bool {
        self.period_edit.is_some()
    }

    /// `true` while a request is waiting for its answer.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.awaiting.is_some()
    }

    /// The request the host should run, when no worker is attached. Taking it leaves the screen
    /// waiting for an answer with the same sequence number.
    pub fn take_request(&mut self) -> Option<ReportRequest> {
        self.queued.take()
    }

    fn last_row(&self) -> usize {
        self.report.rows.len().saturating_sub(1)
    }

    /// Selects row `index`, or the last row when `index` is past the end.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.last_row());
        self.scroll_pending = true;
    }

    /// Moves the selection down by `rows`, stopping at the last row.
    pub fn select_next(&mut self, rows: usize) {
        self.select(self.selected.saturating_add(rows));
    }

    /// Moves the selection up by `rows`, stopping at the first row.
    pub fn select_prev(&mut self, rows: usize) {
        self.select(self.selected.saturating_sub(rows));
    }

    fn info(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice::Info(text.into()));
    }

    fn fail(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice::Error(text.into()));
    }

    /// Asks for what the selected row drills to. The drilled report or screen arrives later and
    /// then opens on top of this screen, so this screen stays below with its selection. A row that
    /// opens nothing, and a drill this build cannot answer, say why in
    /// [`ReportViewScreen::notice`].
    pub fn drill(&mut self) {
        self.notice = None;
        let Some(row) = self.report.rows.get(self.selected) else {
            self.info("This report has no rows to open.");
            return;
        };
        let Some(key) = row.key.clone() else {
            let label = row.label.clone();
            self.info(format!("\"{label}\" has nothing to open."));
            return;
        };
        match key {
            ViewKey::Voucher(_) => {
                let params = self.source.base().clone();
                self.queue(RequestKind::Drill(key), params);
            }
            ViewKey::Item(_) | ViewKey::Bill { .. } => self.fail(unsupported_message(
                "drill into stock items and bills",
                "no report exists for them yet",
            )),
            ViewKey::Group(_) | ViewKey::Ledger(_) => {
                match drill_params(self.source.base(), &key) {
                    Some(params) => self.queue(RequestKind::Drill(key), params),
                    None => self.fail(unsupported_message(
                        "this drill",
                        "no report exists for this row yet",
                    )),
                }
            }
        }
    }

    /// Switches between condensed and detailed (Alt+F1) and re-runs the report in place.
    pub fn toggle_detail(&mut self) {
        let base = self.source.base();
        let params = ReportParams {
            detail: match base.detail {
                Detail::Condensed => Detail::Detailed,
                Detail::Detailed => Detail::Condensed,
            },
            ..base.clone()
        };
        self.queue(RequestKind::Refresh, params);
    }

    /// Re-runs the report for `period` and keeps the detail level.
    pub fn set_period(&mut self, period: Period) {
        let base = self.source.base();
        let view = ViewParams {
            period,
            detail: convert::view_detail(base.detail),
        };
        let params = with_view(base, view, period.to());
        self.queue(RequestKind::Refresh, params);
    }

    /// Opens the period editor (F2), filled with the current period.
    pub fn open_period_editor(&mut self) {
        let p = self.report.params.period;
        self.period_edit = Some(PeriodEdit {
            from: numeric_date_text(p.from()),
            to: numeric_date_text(p.to()),
            focus_pending: true,
        });
    }

    /// Sets the text of the two period fields. Does nothing unless the editor is open.
    pub fn set_period_text(&mut self, from: &str, to: &str) {
        if let Some(edit) = self.period_edit.as_mut() {
            from.clone_into(&mut edit.from);
            to.clone_into(&mut edit.to);
        }
    }

    /// Applies the typed period and closes the editor. When the text is not a period, the editor
    /// stays open and the notice says what is wrong.
    pub fn apply_period_edit(&mut self) {
        let parsed = match self.period_edit.as_ref() {
            Some(edit) => parse_period_text(&edit.from, &edit.to),
            None => return,
        };
        match parsed {
            Ok(period) => {
                self.period_edit = None;
                self.set_period(period);
            }
            Err(e) => self.fail(format!("Period: {e}")),
        }
    }

    /// Closes the period editor without changing anything.
    pub fn cancel_period_edit(&mut self) {
        self.period_edit = None;
    }

    /// Answers the request with sequence number `seq`. An answer to a request that is no longer
    /// the newest one is ignored. A refresh replaces the report; a drill opens its report on top.
    pub fn answer(&mut self, seq: u64, result: Result<Outcome<ViewReport>, ViewError>) {
        let Some(awaited) = self.take_awaited(seq) else {
            return;
        };
        match awaited.kind {
            RequestKind::Refresh => self.apply_refresh(result, awaited.params),
            RequestKind::Drill(_) => {
                self.finish_drill(result.map(|o| o.map(DrillTarget::Report)), awaited.params);
            }
        }
    }

    /// Answers a drill with the screen it opens, for drills the host answers itself (a voucher).
    /// Ignored when `seq` is not the newest request.
    pub fn answer_drill_target(
        &mut self,
        seq: u64,
        result: Result<Outcome<DrillTarget>, ViewError>,
    ) {
        let Some(awaited) = self.take_awaited(seq) else {
            return;
        };
        self.finish_drill(result, awaited.params);
    }

    /// Takes the awaited request when `seq` is that request and still the newest one. Clears the
    /// wait, so an answer is applied once.
    fn take_awaited(&mut self, seq: u64) -> Option<Awaiting> {
        let matches = matches!(&self.awaiting, Some(a) if a.seq == seq);
        if !matches || !self.latest.is_current(seq) {
            return None;
        }
        self.awaiting.take()
    }

    fn apply_refresh(
        &mut self,
        result: Result<Outcome<ViewReport>, ViewError>,
        params: ReportParams,
    ) {
        self.notice = None;
        match result {
            Err(e) => self.fail(e.to_string()),
            Ok(Outcome::Unsupported(u)) => self.fail(unsupported_text(&u)),
            Ok(Outcome::Value(report)) => match report.check_shape() {
                Ok(()) => {
                    self.replace_report(report);
                    self.source = ReportSource::new(params);
                }
                Err(e) => self.fail(e.to_string()),
            },
        }
    }

    fn finish_drill(
        &mut self,
        result: Result<Outcome<DrillTarget>, ViewError>,
        params: ReportParams,
    ) {
        self.notice = None;
        match result {
            Err(e) => self.fail(e.to_string()),
            Ok(Outcome::Unsupported(u)) => self.fail(unsupported_text(&u)),
            Ok(Outcome::Value(DrillTarget::Screen(screen))) => self.pending_push = Some(screen),
            Ok(Outcome::Value(DrillTarget::Report(report))) => {
                match ReportViewScreen::new(report, ReportSource::new(params)) {
                    Ok(screen) => self.pending_push = Some(Box::new(screen)),
                    Err(e) => self.fail(e.to_string()),
                }
            }
        }
    }

    /// Queues a request, replacing any earlier one that nobody has taken. The request becomes
    /// the one whose answer is wanted.
    fn queue(&mut self, kind: RequestKind, params: ReportParams) {
        self.notice = None;
        let seq = self.latest.next_seq();
        self.awaiting = Some(Awaiting {
            seq,
            kind: kind.clone(),
            params: params.clone(),
        });
        self.queued = Some(ReportRequest { seq, kind, params });
    }

    /// Swaps in a reshaped report and keeps the selection on the same row: the row with the same
    /// drill key, or for a keyless row the same kind and label; otherwise the same position, or
    /// the last row.
    fn replace_report(&mut self, report: ViewReport) {
        let old_index = self.selected;
        let old = self
            .report
            .rows
            .get(old_index)
            .map(|r| (r.key.clone(), r.label.clone(), r.kind));
        self.report = report;
        let found = old.and_then(|(key, label, kind)| {
            self.report.rows.iter().position(|r| match &key {
                Some(k) => r.key.as_ref() == Some(k),
                None => r.key.is_none() && r.label == label && r.kind == kind,
            })
        });
        self.selected = found.unwrap_or_else(|| old_index.min(self.last_row()));
        self.scroll_pending = true;
    }

    fn run(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Up => self.select_prev(1),
            KeyCmd::Down => self.select_next(1),
            KeyCmd::PageUp => self.select_prev(PAGE_ROWS),
            KeyCmd::PageDown => self.select_next(PAGE_ROWS),
            KeyCmd::First => self.select(0),
            KeyCmd::Last => self.select(self.last_row()),
            KeyCmd::Open => self.drill(),
            KeyCmd::ToggleDetail => self.toggle_detail(),
            KeyCmd::EditPeriod => self.open_period_editor(),
            KeyCmd::ApplyPeriod => self.apply_period_edit(),
            KeyCmd::CancelPeriod => self.cancel_period_edit(),
        }
    }

    /// Starts the queued request on the worker when there is one, and applies a finished answer.
    /// A voucher drill stays queued for the host, because only the shell opens the voucher form.
    fn pump(&mut self, ctx: &Context, worker: Option<&ApiWorker>) {
        let Some(worker) = worker else {
            return;
        };
        let host_only = matches!(
            self.queued.as_ref().map(|r| &r.kind),
            Some(RequestKind::Drill(ViewKey::Voucher(_)))
        );
        if !host_only && let Some(request) = self.queued.take() {
            let params = request.params.clone();
            self.latest
                .start_as(request.seq, worker, move |api| run_report(api, &params));
        }
        match self.latest.poll() {
            Some(Done::Ready(seq, result)) => self.answer(seq, result),
            Some(Done::Lost(seq)) => {
                let message = "the API worker stopped before the report ran".to_owned();
                self.answer(seq, Err(ViewError::Source { message }));
            }
            None => {
                if self.latest.is_pending() {
                    ctx.request_repaint();
                }
            }
        }
    }

    fn draw(&mut self, ui: &mut Ui) -> ScreenAction {
        let keys = read_keys(ui.ctx(), self.period_edit.is_some());
        for cmd in &keys {
            self.run(*cmd);
        }
        ui.heading(self.report.title.as_str());
        ui.label(format!(
            "{}  |  {}",
            period_text(self.report.params.period),
            self.report.params.detail.label()
        ));
        if self.is_loading() {
            ui.weak("Running the report...");
        }
        match &self.notice {
            Some(Notice::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {s}"));
            }
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
        self.draw_period_editor(ui);
        let events = self.draw_table(ui);
        for note in &self.report.notes {
            ui.label(note.as_str());
        }
        ui.weak("Enter: open   Esc: back   Alt+F1: condensed or detailed   F2: period");
        self.scroll_pending = false;
        if let Some(i) = events.clicked {
            self.selected = i.min(self.last_row());
        }
        if let Some(i) = events.opened {
            self.selected = i.min(self.last_row());
            self.drill();
        }
        if !keys.is_empty() {
            ui.ctx().request_repaint();
        }
        self.pending_push
            .take()
            .map_or(ScreenAction::Stay, ScreenAction::Push)
    }

    fn draw_period_editor(&mut self, ui: &mut Ui) {
        let base = ui.scope_id().with("u7-period");
        let Some(edit) = self.period_edit.as_mut() else {
            return;
        };
        let (mut apply, mut cancel) = (false, false);
        ui.group(|ui| {
            ui.label("Period (DD-MM-YYYY)");
            ui.horizontal(|ui| {
                ui.label("From");
                let from = ui.add(
                    egui::TextEdit::singleline(&mut edit.from)
                        .id(base.with("from"))
                        .desired_width(110.0),
                );
                if edit.focus_pending {
                    from.request_focus();
                    edit.focus_pending = false;
                }
                ui.label("To");
                ui.add(
                    egui::TextEdit::singleline(&mut edit.to)
                        .id(base.with("to"))
                        .desired_width(110.0),
                );
            });
            ui.horizontal(|ui| {
                apply = ui.button("Apply (Enter)").clicked();
                cancel = ui.button("Cancel (Esc)").clicked();
            });
        });
        if apply {
            self.apply_period_edit();
        } else if cancel {
            self.cancel_period_edit();
        }
    }

    /// Draws header, rows and totals as one virtualised table, so a long register costs only the
    /// rows in view (spec 8).
    fn draw_table(&self, ui: &mut Ui) -> TableEvents {
        let mut events = TableEvents::default();
        let report = &self.report;
        let total_rows = report
            .rows
            .len()
            .saturating_add(usize::from(!report.totals.is_empty()));
        let height = (ui.available_height() - 48.0).max(120.0);
        let mut table = TableBuilder::new(ui)
            .id_salt(("u7-report-table", report.token.0.as_str()))
            .striped(true)
            .sense(egui::Sense::click())
            .max_scroll_height(height)
            .auto_shrink([false, false])
            .column(Column::initial(300.0).at_least(160.0).clip(true));
        for _ in &report.columns {
            table = table.column(Column::initial(170.0).at_least(110.0).clip(true));
        }
        if self.scroll_pending && !report.rows.is_empty() {
            table = table.scroll_to_row(self.selected, None);
        }
        table
            .header(ROW_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.strong("Particulars");
                });
                for column in &report.columns {
                    header.col(|ui| right_aligned(ui, RichText::new(column.as_str()).strong()));
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, total_rows, |mut row| {
                    let index = row.index();
                    match report.rows.get(index) {
                        Some(data) => {
                            row.set_selected(index == self.selected);
                            draw_data_row(&mut row, data);
                            let response = row.response();
                            if response.clicked() {
                                events.clicked = Some(index);
                            }
                            if response.double_clicked() {
                                events.opened = Some(index);
                            }
                        }
                        None => draw_totals_row(&mut row, &report.totals),
                    }
                });
            });
        events
    }
}

fn right_aligned(ui: &mut Ui, text: RichText) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(text);
    });
}

fn draw_data_row(row: &mut TableRow<'_, '_>, data: &ViewRow) {
    let emphasised = match data.kind {
        ViewRowKind::Group | ViewRowKind::Subtotal | ViewRowKind::Difference => true,
        ViewRowKind::Ledger
        | ViewRowKind::Voucher
        | ViewRowKind::Item
        | ViewRowKind::Bill
        | ViewRowKind::Text => false,
    };
    row.col(|ui| {
        ui.add_space(f32::from(data.depth) * INDENT);
        let label = RichText::new(data.label.as_str());
        ui.label(if emphasised { label.strong() } else { label });
    });
    for cell in &data.cells {
        row.col(|ui| {
            let text = RichText::new(cell_text(cell));
            right_aligned(ui, if emphasised { text.strong() } else { text });
        });
    }
}

fn draw_totals_row(row: &mut TableRow<'_, '_>, totals: &[ViewCell]) {
    row.col(|ui| {
        ui.strong("Total");
    });
    for cell in totals {
        row.col(|ui| right_aligned(ui, RichText::new(cell_text(cell)).strong()));
    }
}

/// Reads this frame's keys. While the period editor is open only Enter and Esc count, so the
/// arrows belong to the text fields. Alt+F1 comes first in the table because egui ignores extra
/// Alt and Shift when it matches a plain key.
fn read_keys(ctx: &Context, editing: bool) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        if editing {
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                out.push(KeyCmd::ApplyPeriod);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                out.push(KeyCmd::CancelPeriod);
            }
            return;
        }
        let table = [
            (Modifiers::ALT, Key::F1, KeyCmd::ToggleDetail),
            (Modifiers::NONE, Key::F2, KeyCmd::EditPeriod),
            (Modifiers::NONE, Key::Enter, KeyCmd::Open),
            (Modifiers::NONE, Key::ArrowUp, KeyCmd::Up),
            (Modifiers::NONE, Key::ArrowDown, KeyCmd::Down),
            (Modifiers::NONE, Key::PageUp, KeyCmd::PageUp),
            (Modifiers::NONE, Key::PageDown, KeyCmd::PageDown),
            (Modifiers::NONE, Key::Home, KeyCmd::First),
            (Modifiers::NONE, Key::End, KeyCmd::Last),
        ];
        for (modifiers, key, cmd) in table {
            if i.consume_key(modifiers, key) {
                out.push(cmd);
            }
        }
    });
    out
}

impl Screen for ReportViewScreen {
    fn title(&self) -> String {
        self.report.title.clone()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.pump(ui.ctx(), ctx.api.as_ref());
        self.draw(ui)
    }
}
