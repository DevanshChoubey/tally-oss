//! Audit views (shows RP10 outputs only). **Card U12** implements this module (spec 6.6).
//!
//! Frozen interface (plan card U12):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! [`AuditScreen`] is the auditor's window onto the audit trail: a kind selector (object history,
//! edit log, deleted and altered vouchers, back-dated entries, edits after lock, integrity
//! certificate), a period, a table, and a text filter. It computes nothing (rule C4). Every cell
//! it shows is a string that card RP10 already produced; the screen only lays the strings out,
//! folds child rows (the field-by-field diff under an event), filters rows, and copies the visible
//! rows as tab-separated text.
//!
//! # Why the table is a local type
//!
//! `ledgerly-app` does not depend on `ledgerly-reports` (the manifest is frozen), so the screen
//! cannot name `ledgerly_reports::Report`. It reads [`AuditTable`] instead: column labels, rows
//! with a nesting depth, and notes. The host (the Wave 2 wiring card) builds an [`AuditTable`] from
//! the `Report` that `audit_report` or `integrity_certificate` returned, one cell per `Cell`
//! rendered to text, with `Cell::Unsupported` kept visible rather than blanked. Card U12's report
//! asks the controller for either a `ledgerly-reports` dependency here or an `AuditTable`
//! conversion in `ledgerly-api`.
//!
//! # Running a report
//!
//! The screen never reads the audit log. When the user picks a kind or presses Run, the screen
//! queues an [`AuditRequest`]; the host takes it with [`AuditScreen::take_request`], runs the
//! report, and answers with [`AuditScreen::set_table`], [`AuditScreen::set_unsupported`] or
//! [`AuditScreen::set_failed`]. An `Unsupported` outcome from RP10 is shown as such, with its
//! reason, never as an empty table.

use crate::i18n::Tr;
use std::collections::BTreeSet;

use egui_extras::{Column, TableBuilder};
use ledgerly_core::{Date, Month, Period, PeriodError};
use thiserror::Error;

use crate::{AppCtx, Screen, ScreenAction};

/// Height of a table row in points.
const ROW_HEIGHT: f32 = 20.0;

/// Indent of a child row per level of depth, in points.
const INDENT: f32 = 14.0;

/// The auditor reports of spec 6.6 that card RP10 produces.
///
/// This mirrors `ledgerly_reports::AuditReportKind` (see the module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditViewKind {
    /// Every event of the selected objects with a field-by-field diff.
    ObjectHistory,
    /// Every event of the period.
    EditLog,
    /// Voucher alterations, cancellations and deletions.
    DeletedAltered,
    /// Vouchers posted with a date earlier than an earlier-posted voucher.
    BackDated,
    /// Voucher events that touch a locked date.
    EditsAfterLock,
    /// The result of verifying the hash chain.
    IntegrityCertificate,
}

impl AuditViewKind {
    /// Every kind, in the order the selector shows them.
    pub const ALL: [AuditViewKind; 6] = [
        AuditViewKind::ObjectHistory,
        AuditViewKind::EditLog,
        AuditViewKind::DeletedAltered,
        AuditViewKind::BackDated,
        AuditViewKind::EditsAfterLock,
        AuditViewKind::IntegrityCertificate,
    ];

    /// Short label for the selector.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            AuditViewKind::ObjectHistory => "Object history",
            AuditViewKind::EditLog => "Edit log",
            AuditViewKind::DeletedAltered => "Deleted and altered",
            AuditViewKind::BackDated => "Back-dated entries",
            AuditViewKind::EditsAfterLock => "Edits after lock",
            AuditViewKind::IntegrityCertificate => "Integrity certificate",
        }
    }

    /// One sentence saying what the report lists.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            AuditViewKind::ObjectHistory => {
                "Every change to each object, with the fields that changed."
            }
            AuditViewKind::EditLog => "Every change recorded in the period, with who made it.",
            AuditViewKind::DeletedAltered => {
                "Vouchers that were altered, cancelled or deleted, with what changed."
            }
            AuditViewKind::BackDated => {
                "Vouchers posted with a date earlier than a voucher posted before them."
            }
            AuditViewKind::EditsAfterLock => {
                "Voucher changes that touched a date on or before the lock date in force."
            }
            AuditViewKind::IntegrityCertificate => {
                "Whether the recorded history has been altered after the fact."
            }
        }
    }

    /// Whether the report depends on the period. The certificate covers the whole log.
    #[must_use]
    pub fn uses_period(self) -> bool {
        match self {
            AuditViewKind::ObjectHistory
            | AuditViewKind::EditLog
            | AuditViewKind::DeletedAltered
            | AuditViewKind::BackDated
            | AuditViewKind::EditsAfterLock => true,
            AuditViewKind::IntegrityCertificate => false,
        }
    }
}

/// What the host should run: one report over one period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditRequest {
    /// Which report.
    pub kind: AuditViewKind,
    /// The period typed on the screen (the certificate ignores it).
    pub period: Period,
}

/// Why the period typed on the screen cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuditInputError {
    /// A date box is empty.
    #[error("enter the {which} date")]
    Empty {
        /// `"from"` or `"to"`.
        which: &'static str,
    },
    /// A date box does not read as `DD-MM-YYYY` or `YYYY-MM-DD`.
    #[error("the {which} date '{text}' is not a date; write it as 31-03-2026")]
    Format {
        /// `"from"` or `"to"`.
        which: &'static str,
        /// What was typed.
        text: String,
    },
    /// The two dates do not make a period.
    #[error(transparent)]
    Period(#[from] PeriodError),
}

/// Parses `DD-MM-YYYY`, `DD/MM/YYYY` or `YYYY-MM-DD` (a full date; no day-month shorthand, because
/// an audit period must never depend on a guessed year).
fn parse_full_date(which: &'static str, text: &str) -> Result<Date, AuditInputError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AuditInputError::Empty { which });
    }
    let bad = || AuditInputError::Format {
        which,
        text: text.to_owned(),
    };
    let parts: Vec<&str> = text.split(['-', '/']).collect();
    if !parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(bad());
    }
    let (day, month, year) = match parts.as_slice() {
        [d, m, y] if y.len() == 4 => (*d, *m, *y),
        [y, m, d] if y.len() == 4 => (*d, *m, *y),
        _ => return Err(bad()),
    };
    let day: u8 = day.parse().map_err(|_| bad())?;
    let month = Month::try_from(month.parse::<u8>().map_err(|_| bad())?).map_err(|_| bad())?;
    let year: i32 = year.parse().map_err(|_| bad())?;
    Date::from_calendar_date(year, month, day).map_err(|_| bad())
}

/// Writes a date the way the period boxes read it (`31-03-2026`).
#[must_use]
pub fn format_period_date(date: Date) -> String {
    format!(
        "{:02}-{:02}-{}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// One row of an audit table. A row with `depth > 0` belongs to the nearest earlier row with a
/// smaller depth (a changed field under its event).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    /// Nesting depth; 0 is a top-level row.
    pub depth: u8,
    /// Cell text, one per column, already formatted by the host.
    pub cells: Vec<String>,
}

impl AuditRow {
    /// Whether any cell contains `needle` (already lower-cased), ignoring case.
    fn matches(&self, needle: &str) -> bool {
        self.cells.iter().any(|c| c.to_lowercase().contains(needle))
    }
}

/// The outcome of a certificate check as the report states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditVerdict {
    /// The report's result row reads exactly `PASS`.
    Pass,
    /// The report's result row reads exactly `FAIL`.
    Fail,
}

/// A report as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditTable {
    /// Report title.
    pub title: String,
    /// Column labels.
    pub columns: Vec<String>,
    /// Rows in display order.
    pub rows: Vec<AuditRow>,
    /// Notes shown under the table.
    pub notes: Vec<String>,
}

impl AuditTable {
    /// Whether the row at `index` has rows nested under it.
    #[must_use]
    pub fn has_children(&self, index: usize) -> bool {
        self.rows
            .get(index)
            .zip(self.rows.get(index.saturating_add(1)))
            .is_some_and(|(row, next)| next.depth > row.depth)
    }

    /// The certificate verdict, read from the row labelled `Result`. Anything other than exactly
    /// `PASS` or `FAIL` gives `None`: the screen never invents a pass.
    #[must_use]
    pub fn verdict(&self) -> Option<AuditVerdict> {
        let row = self
            .rows
            .iter()
            .find(|r| r.cells.first().is_some_and(|c| c == "Result"))?;
        match row.cells.get(1).map(String::as_str) {
            Some("PASS") => Some(AuditVerdict::Pass),
            Some("FAIL") => Some(AuditVerdict::Fail),
            Some(_) | None => None,
        }
    }

    /// The header and the rows at `visible` as tab-separated text, one line per row (tabs and
    /// line breaks inside a cell become spaces).
    #[must_use]
    pub fn to_tsv(&self, visible: &[usize]) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let mut lines = vec![
            self.columns
                .iter()
                .map(|c| clean(c))
                .collect::<Vec<_>>()
                .join("\t"),
        ];
        for row in visible.iter().filter_map(|i| self.rows.get(*i)) {
            lines.push(
                row.cells
                    .iter()
                    .map(|c| clean(c))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

/// End (exclusive) of the group that starts at `start`: the next top-level row, or the end.
fn group_end(rows: &[AuditRow], start: usize) -> usize {
    rows.iter()
        .enumerate()
        .skip(start.saturating_add(1))
        .find(|(_, r)| r.depth == 0)
        .map_or(rows.len(), |(i, _)| i)
}

/// Indices of the rows to draw.
///
/// Rows are grouped (a top-level row and everything nested under it). With a non-blank `filter`,
/// a group is kept whole when any of its rows contains the text (case-insensitive). Rows nested
/// under a row in `collapsed` are hidden.
#[must_use]
pub fn visible_rows(rows: &[AuditRow], collapsed: &BTreeSet<usize>, filter: &str) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    let mut out = Vec::new();
    let mut start = 0_usize;
    while start < rows.len() {
        let end = group_end(rows, start);
        let group = rows.get(start..end).unwrap_or_default();
        if needle.is_empty() || group.iter().any(|r| r.matches(&needle)) {
            let mut hide_deeper_than: Option<u8> = None;
            for (offset, row) in group.iter().enumerate() {
                if let Some(depth) = hide_deeper_than {
                    if row.depth > depth {
                        continue;
                    }
                    hide_deeper_than = None;
                }
                let index = start.saturating_add(offset);
                out.push(index);
                if collapsed.contains(&index) {
                    hide_deeper_than = Some(row.depth);
                }
            }
        }
        start = end.max(start.saturating_add(1));
    }
    out
}

/// What the screen is showing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuditState {
    /// Nothing has been requested yet.
    #[default]
    NotRun,
    /// A request is with the host.
    Running,
    /// The report.
    Loaded(AuditTable),
    /// The report cannot be produced for this request (rule A1: shown, never hidden).
    Unsupported {
        /// The construct that is not handled.
        what: String,
        /// Why, and what the user can do.
        reason: String,
    },
    /// The report failed with an error.
    Failed(String),
}

/// The audit trail screen.
#[derive(Debug)]
pub struct AuditScreen {
    kind: AuditViewKind,
    from_text: String,
    to_text: String,
    filter: String,
    state: AuditState,
    collapsed: BTreeSet<usize>,
    request: Option<AuditRequest>,
    input_error: Option<AuditInputError>,
}

impl AuditScreen {
    /// A screen on the edit log for `period` (the books' financial year, say). Nothing is
    /// requested until [`AuditScreen::run`] is called or the user presses Run.
    #[must_use]
    pub fn new(period: Period) -> Self {
        Self {
            kind: AuditViewKind::EditLog,
            from_text: format_period_date(period.from()),
            to_text: format_period_date(period.to()),
            filter: String::new(),
            state: AuditState::NotRun,
            collapsed: BTreeSet::new(),
            request: None,
            input_error: None,
        }
    }

    /// The selected report.
    #[must_use]
    pub fn kind(&self) -> AuditViewKind {
        self.kind
    }

    /// What the screen is showing.
    #[must_use]
    pub fn state(&self) -> &AuditState {
        &self.state
    }

    /// The row filter text.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Sets the row filter text.
    pub fn set_filter(&mut self, text: &str) {
        text.clone_into(&mut self.filter);
    }

    /// The problem with the typed period, if the last Run failed on it.
    #[must_use]
    pub fn input_error(&self) -> Option<&AuditInputError> {
        self.input_error.as_ref()
    }

    /// Types a period into the two boxes.
    pub fn set_period_text(&mut self, from: &str, to: &str) {
        from.clone_into(&mut self.from_text);
        to.clone_into(&mut self.to_text);
    }

    /// Reads the period boxes.
    ///
    /// # Errors
    /// [`AuditInputError`] when a box is empty or is not a date, or when `from` is after `to`.
    pub fn period(&self) -> Result<Period, AuditInputError> {
        let from = parse_full_date("from", &self.from_text)?;
        let to = parse_full_date("to", &self.to_text)?;
        Ok(Period::new(from, to)?)
    }

    /// Picks a report and asks for it. When the report changes, the old table, any error and any
    /// queued request for the old report are dropped, even when the new period is bad: a table is
    /// never shown under the wrong heading, and the selector, the description and the table always
    /// name the same report.
    pub fn select_kind(&mut self, kind: AuditViewKind) {
        if self.kind != kind {
            self.state = AuditState::NotRun;
            self.request = None;
        }
        self.kind = kind;
        self.run();
    }

    /// Queues a request for the selected report over the typed period. A bad period sets
    /// [`AuditScreen::input_error`] and queues nothing. The certificate does not read the period,
    /// so a bad period does not stop it.
    pub fn run(&mut self) {
        self.collapsed.clear();
        // The certificate covers the whole log, so it does not read the period boxes; the request
        // carries the widest period only because the request type has one.
        let period = match self.period() {
            Ok(period) => period,
            Err(e) if self.kind.uses_period() => {
                self.input_error = Some(e);
                return;
            }
            Err(_) => match Period::new(Date::MIN, Date::MAX) {
                Ok(period) => period,
                Err(e) => {
                    self.input_error = Some(AuditInputError::Period(e));
                    return;
                }
            },
        };
        self.input_error = None;
        self.state = AuditState::Running;
        self.request = Some(AuditRequest {
            kind: self.kind,
            period,
        });
    }

    /// Hands the queued request to the host, once.
    pub fn take_request(&mut self) -> Option<AuditRequest> {
        self.request.take()
    }

    /// The host's answer: the report.
    pub fn set_table(&mut self, table: AuditTable) {
        self.collapsed.clear();
        self.state = AuditState::Loaded(table);
    }

    /// The host's answer: the report is `Unsupported` for this request.
    pub fn set_unsupported(&mut self, what: &str, reason: &str) {
        self.state = AuditState::Unsupported {
            what: what.to_owned(),
            reason: reason.to_owned(),
        };
    }

    /// The host's answer: the report failed.
    pub fn set_failed(&mut self, message: &str) {
        self.state = AuditState::Failed(message.to_owned());
    }

    /// Folds or unfolds the rows nested under row `index`.
    pub fn toggle_row(&mut self, index: usize) {
        if !self.collapsed.remove(&index) {
            self.collapsed.insert(index);
        }
    }

    /// Whether the rows under `index` are folded.
    #[must_use]
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.contains(&index)
    }

    /// Indices of the rows of the loaded table that are drawn now (none unless a table is loaded).
    #[must_use]
    pub fn visible(&self) -> Vec<usize> {
        match &self.state {
            AuditState::Loaded(table) => visible_rows(&table.rows, &self.collapsed, &self.filter),
            AuditState::NotRun
            | AuditState::Running
            | AuditState::Unsupported { .. }
            | AuditState::Failed(_) => Vec::new(),
        }
    }

    fn controls_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for kind in AuditViewKind::ALL {
                let selected = self.kind == kind;
                if ui
                    .selectable_label(selected, kind.label())
                    .on_hover_text(kind.description())
                    .clicked()
                    && !selected
                {
                    self.select_kind(kind);
                }
            }
        });
        ui.label(self.kind.description());
        ui.horizontal(|ui| {
            if self.kind.uses_period() {
                ui.label(crate::i18n::lit("From"));
                ui.add(egui::TextEdit::singleline(&mut self.from_text).desired_width(90.0));
                ui.label("to");
                ui.add(egui::TextEdit::singleline(&mut self.to_text).desired_width(90.0));
            } else {
                ui.label(crate::i18n::lit("Covers the whole log."));
            }
            if ui.button("Run").clicked() {
                self.run();
            }
        });
        if let Some(error) = &self.input_error {
            ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        }
    }

    fn filter_ui(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::lit("Find"));
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(crate::i18n::lit("text in any column"))
                    .desired_width(180.0),
            );
            if let AuditState::Loaded(table) = &self.state {
                ui.weak(format!("{} of {} rows", visible.len(), table.rows.len()));
                if ui.button("Copy rows").clicked() {
                    ui.ctx().copy_text(table.to_tsv(visible));
                }
            }
        });
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        if matches!(self.state, AuditState::Loaded(_)) {
            self.table_ui(ui);
            return;
        }
        match &self.state {
            AuditState::NotRun => {
                ui.label(crate::i18n::lit("Choose a report and press Run."));
            }
            AuditState::Running => {
                ui.label(crate::i18n::lit("Reading the audit log..."));
            }
            AuditState::Unsupported { what, reason } => {
                ui.colored_label(ui.visuals().warn_fg_color, format!("Unsupported: {what}"));
                ui.label(reason.as_str());
            }
            AuditState::Failed(message) => {
                ui.colored_label(ui.visuals().error_fg_color, message.as_str());
            }
            AuditState::Loaded(_) => {}
        }
    }

    fn table_ui(&mut self, ui: &mut egui::Ui) {
        let visible = self.visible();
        self.filter_ui(ui, &visible);
        let toggled = {
            let AuditState::Loaded(table) = &self.state else {
                return;
            };
            ui.heading(table.title.as_str());
            if self.kind == AuditViewKind::IntegrityCertificate {
                verdict_ui(ui, table.verdict());
            }
            let toggled = draw_table(ui, table, &visible, &self.collapsed);
            for note in &table.notes {
                ui.weak(note.as_str());
            }
            toggled
        };
        if let Some(index) = toggled {
            self.toggle_row(index);
        }
    }
}

fn verdict_ui(ui: &mut egui::Ui, verdict: Option<AuditVerdict>) {
    let (text, color) = match verdict {
        Some(AuditVerdict::Pass) => ("Chain check: PASS", ui.visuals().text_color()),
        Some(AuditVerdict::Fail) => ("Chain check: FAIL", ui.visuals().error_fg_color),
        None => (
            "Chain check: result not stated by the report",
            ui.visuals().warn_fg_color,
        ),
    };
    ui.label(egui::RichText::new(text).strong().color(color));
}

/// Draws `table`'s `visible` rows; returns the row whose fold button was pressed.
fn draw_table(
    ui: &mut egui::Ui,
    table: &AuditTable,
    visible: &[usize],
    collapsed: &BTreeSet<usize>,
) -> Option<usize> {
    if table.columns.is_empty() {
        ui.label(crate::i18n::lit("The report has no columns."));
        return None;
    }
    let mut toggled: Option<usize> = None;
    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .max_scroll_height(480.0);
    for _ in &table.columns {
        builder = builder.column(Column::auto().at_least(48.0).clip(true));
    }
    let _output = builder
        .header(ROW_HEIGHT, |mut header| {
            for label in &table.columns {
                header.col(|ui| {
                    ui.strong(label.as_str());
                });
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, visible.len(), |mut row| {
                let Some(index) = visible.get(row.index()).copied() else {
                    return;
                };
                let Some(data) = table.rows.get(index) else {
                    return;
                };
                for (column, text) in data.cells.iter().enumerate() {
                    row.col(|ui| {
                        if column == 0 {
                            ui.add_space(INDENT * f32::from(data.depth));
                            if table.has_children(index) {
                                let (mark, hint) = if collapsed.contains(&index) {
                                    ("+", "Show the changed fields")
                                } else {
                                    ("-", "Hide the changed fields")
                                };
                                if ui.small_button(mark).on_hover_text(hint).clicked() {
                                    toggled = Some(index);
                                }
                            }
                        }
                        ui.label(text.as_str());
                    });
                }
            });
        });
    toggled
}

impl Screen for AuditScreen {
    fn title(&self, _tr: &Tr) -> String {
        "Audit trail".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        self.controls_ui(ui);
        ui.separator();
        self.body_ui(ui);
        ScreenAction::Stay
    }
}
