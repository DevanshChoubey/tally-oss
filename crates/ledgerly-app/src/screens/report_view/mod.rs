//! Report viewer with drill-down. **Card U7** implements this module.
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
//! This screen sums nothing and rounds nothing. Every figure arrives in a [`ViewReport`] that the
//! host built from a `ledgerly-reports` `Report`, and the screen only formats it
//! ([`cell_text`]). `ledgerly-app` does not depend on `ledgerly-reports` yet (the manifest is
//! frozen), so [`ViewReport`], [`ViewRow`], [`ViewCell`] and [`ViewKey`] mirror that crate's
//! model one to one, using the shared `ledgerly-core` types for money and ids. The host (Wave 2
//! wiring, cards W6 and W7) converts, and supplies a [`ReportSource`]: two functions, one that
//! re-runs the report with new parameters and one that opens what a row drills to.
//!
//! A cell that cannot be shown as a figure is shown as words ("Unsupported: ..."), never as a
//! number. A report whose rows do not each carry one cell per column is rejected
//! ([`ViewError::RowShape`]) rather than drawn, because a figure under the wrong heading is a
//! wrong number. When a refresh fails or is unsupported, the previous report stays on screen with
//! its own period and detail in the header, and the reason is shown above it.
//!
//! Dr and Cr are written as words next to each amount, so no meaning rests on colour alone.

mod model;
mod text;

use egui::{Align, Context, Key, Layout, Modifiers, RichText, Ui};
use egui_extras::{Column, TableBuilder, TableRow};
use ledgerly_core::{Outcome, Period};

use crate::{AppCtx, Screen, ScreenAction};

pub use model::{
    DrillTarget, ReportSource, ReportToken, ViewCell, ViewDetail, ViewError, ViewKey, ViewParams,
    ViewReport, ViewRow, ViewRowKind,
};
pub use text::{
    PeriodTextError, amount_text, cell_text, date_text, parse_period_date, parse_period_text,
    period_text,
};
use text::{numeric_date_text, unsupported_text};

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

/// Shows one report, with row selection, drill-down, the detail toggle and the period editor.
pub struct ReportViewScreen {
    report: ViewReport,
    source: ReportSource,
    selected: usize,
    scroll_pending: bool,
    period_edit: Option<PeriodEdit>,
    notice: Option<Notice>,
}

impl ReportViewScreen {
    /// A screen for `report`, with the first row selected.
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

    /// Opens what the selected row drills to: a report screen, or another screen such as a
    /// voucher. Returns [`ScreenAction::Push`], so this screen stays below with its selection.
    /// A row that opens nothing, an unsupported drill and a host failure leave the screen as it is
    /// and say why in [`ReportViewScreen::notice`].
    pub fn drill(&mut self) -> ScreenAction {
        self.notice = None;
        let (key, label) = if let Some(row) = self.report.rows.get(self.selected) {
            (row.key.clone(), row.label.clone())
        } else {
            self.info("This report has no rows to open.");
            return ScreenAction::Stay;
        };
        let Some(key) = key else {
            self.info(format!("\"{label}\" has nothing to open."));
            return ScreenAction::Stay;
        };
        match (self.source.drill)(&self.report.token, &key, self.report.params) {
            Err(e) => {
                self.fail(e.to_string());
                ScreenAction::Stay
            }
            Ok(Outcome::Unsupported(u)) => {
                self.fail(unsupported_text(&u));
                ScreenAction::Stay
            }
            Ok(Outcome::Value(DrillTarget::Screen(screen))) => ScreenAction::Push(screen),
            Ok(Outcome::Value(DrillTarget::Report(report))) => {
                match ReportViewScreen::new(report, self.source.clone()) {
                    Ok(screen) => ScreenAction::Push(Box::new(screen)),
                    Err(e) => {
                        self.fail(e.to_string());
                        ScreenAction::Stay
                    }
                }
            }
        }
    }

    /// Switches between condensed and detailed (Alt+F1) and re-runs the report in place.
    pub fn toggle_detail(&mut self) {
        let params = ViewParams {
            detail: self.report.params.detail.toggled(),
            ..self.report.params
        };
        self.refresh(params);
    }

    /// Re-runs the report for `period` and keeps the detail level.
    pub fn set_period(&mut self, period: Period) {
        let params = ViewParams {
            period,
            ..self.report.params
        };
        self.refresh(params);
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

    fn refresh(&mut self, params: ViewParams) {
        self.notice = None;
        match (self.source.refresh)(&self.report.token, params) {
            Err(e) => self.fail(e.to_string()),
            Ok(Outcome::Unsupported(u)) => self.fail(unsupported_text(&u)),
            Ok(Outcome::Value(report)) => match report.check_shape() {
                Ok(()) => self.replace_report(report),
                Err(e) => self.fail(e.to_string()),
            },
        }
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

    fn run(&mut self, cmd: KeyCmd) -> ScreenAction {
        match cmd {
            KeyCmd::Up => self.select_prev(1),
            KeyCmd::Down => self.select_next(1),
            KeyCmd::PageUp => self.select_prev(PAGE_ROWS),
            KeyCmd::PageDown => self.select_next(PAGE_ROWS),
            KeyCmd::First => self.select(0),
            KeyCmd::Last => self.select(self.last_row()),
            KeyCmd::Open => return self.drill(),
            KeyCmd::ToggleDetail => self.toggle_detail(),
            KeyCmd::EditPeriod => self.open_period_editor(),
            KeyCmd::ApplyPeriod => self.apply_period_edit(),
            KeyCmd::CancelPeriod => self.cancel_period_edit(),
        }
        ScreenAction::Stay
    }

    fn draw(&mut self, ui: &mut Ui) -> ScreenAction {
        let keys = read_keys(ui.ctx(), self.period_edit.is_some());
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if let ScreenAction::Push(screen) = self.run(*cmd) {
                action = ScreenAction::Push(screen);
            }
        }
        ui.heading(self.report.title.as_str());
        ui.label(format!(
            "{}  |  {}",
            period_text(self.report.params.period),
            self.report.params.detail.label()
        ));
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
            if let ScreenAction::Push(screen) = self.drill() {
                action = ScreenAction::Push(screen);
            }
        }
        if !keys.is_empty() {
            ui.ctx().request_repaint();
        }
        action
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

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
        self.draw(ui)
    }
}
