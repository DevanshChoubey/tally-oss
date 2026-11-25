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
//! * F2 edits the period ([`ReportViewScreen::open_period_editor`], [`parse_period_text`]). In
//!   the day book F2 asks for one date and Alt+F2 for a period, as TallyPrime's Day Book does
//!   ([`ReportViewScreen::open_date_editor`]).
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

pub mod balance;
pub mod day_book;
pub mod generic;
mod ledger_side;
mod mock;
mod model;
mod remote;
mod text;
mod voucher_view;

use std::path::PathBuf;

use egui::{Align, Context, Key, Layout, Modifiers, Rect, Ui};
use ledgerly_api::{ExportFormat, ReportKind, ReportParams};
use ledgerly_core::{Outcome, Period};

use crate::screen::KeyHint;
use crate::screens::print_job::PrintJob;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::icon::Icon;
use crate::ui::keycap::keycap;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::{page_head, page_head_with};
use crate::ui::para_text;
use crate::ui::tabs::{tab_strip, tab_strip_width};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

pub use model::{
    DrillTarget, ReportSource, ReportToken, ViewCell, ViewDetail, ViewError, ViewKey, ViewParams,
    ViewReport, ViewRow, ViewRowKind,
};
use remote::{drill_params, mock_lines, run_report, view_detail, with_view};
use text::numeric_date_text;
pub use text::{
    PeriodTextError, amount_text, cell_text, date_text, parse_period_date, parse_period_text,
    period_text,
};
pub use voucher_view::{
    VoucherView, VoucherViewError, VoucherViewItem, VoucherViewLine, VoucherViewScreen,
    audit_lines, item_texts, posting_columns, voucher_title, voucher_view,
};

/// Rows moved by Page Up and Page Down.
const PAGE_ROWS: usize = 10;

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
    kind: EditorKind,
    from: String,
    to: String,
    focus_pending: bool,
}

/// What the open editor asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    /// One date: the day book's F2 (TallyPrime's Day Book, F2 Date).
    Date,
    /// A From and a To date: F2 elsewhere, Alt+F2 in the day book.
    Period,
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
    EditDate,
    EditPeriod,
    ApplyPeriod,
    CancelPeriod,
    Export,
    Print,
    /// F10 on the trial balance.
    ProfitLoss,
}

#[derive(Default)]
struct TableEvents {
    clicked: Option<usize>,
    opened: Option<usize>,
}

/// The save dialog Export opens: given the suggested file name, the path chosen, or `None` when
/// the user cancelled. [`ReportViewScreen::set_export_picker`] replaces it (tests do).
pub type ExportPicker = Box<dyn FnMut(&str) -> Option<PathBuf>>;

/// The system save dialog, offering CSV and XLSX (index.html:1624).
fn save_dialog(suggested: &str) -> Option<PathBuf> {
    crate::ui::dialogs::save_file(
        "Export report",
        Some(suggested),
        &[("CSV", &["csv"]), ("Excel workbook", &["xlsx"])],
    )
}

/// What an engine report cell shows on screen (see [`text::cell_text`]): the palette's Recent
/// rows show a ledger's closing balance with it.
#[must_use]
pub fn engine_cell_text(cell: &ledgerly_api::Cell) -> String {
    text::cell_text(&remote::view_cell(cell))
}

/// The file name the save dialog suggests for a report titled `title`: the title with path
/// separators and the characters Windows refuses (`/ \\ < > : " | ? *`) turned into `-`, edge
/// spaces and dots dropped, and `.csv` added. An empty result is "Report".
#[must_use]
pub fn export_file_name(title: &str) -> String {
    let safe: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = safe.trim_matches(|c: char| c == ' ' || c == '.');
    let stem = if trimmed.is_empty() {
        "Report"
    } else {
        trimmed
    };
    format!("{stem}.{}", ExportFormat::Csv.extension())
}

/// What an export brings back: the file, and whether it was written.
type Exported = (PathBuf, Result<Outcome<()>, String>);

/// The answer a worker-mode request brings back: the parameters it ran with, and the report.
/// The engine's answer, and for the reports drawn to the mock the lines to draw.
type Answer = (
    ReportParams,
    Result<Outcome<ViewReport>, ViewError>,
    Option<ViewReport>,
);

/// What a worker-mode screen sends to the API worker and waits for.
struct WorkerRun {
    /// The engine parameters of the report on show. Its balance date is kept across period and
    /// detail changes.
    engine: ReportParams,
    /// A period or detail change the worker has not been asked for yet.
    want: Option<ViewParams>,
    /// The request in flight. A newer request drops the older one's ticket.
    latest: Latest<Answer>,
}

/// The layout a report is drawn in. It is read from the engine's report kind, which a worker-mode
/// screen holds; a host-mode screen has no kind and draws the generic table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    /// The generic table: particulars, then one column per engine column.
    Generic,
    /// The balance sheet, drawn one section under the other (see [`balance`]).
    BalanceSheet,
    /// The day book: vouchers with a period tab (see [`day_book`]).
    DayBook,
    /// A generic table under the mock's own head: title, subtitle, keys and actions (see
    /// [`mock`]).
    Mock(mock::Mock),
}

/// The look for an engine report kind. Every kind is listed, so a new kind fails to compile until
/// it has a look.
fn look_of(kind: &ReportKind) -> Look {
    match kind {
        ReportKind::BalanceSheet => Look::BalanceSheet,
        ReportKind::DayBook => Look::DayBook,
        ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::Register(_)
        | ReportKind::TrialBalance
        | ReportKind::ProfitAndLoss
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => mock::Mock::of(kind).map_or(Look::Generic, Look::Mock),
    }
}

/// The title a drill level is shown under: the engine's title without its report-kind prefix, so
/// a group summary reads as the group's name and a ledger's vouchers as the ledger's (index.html:
/// 1570-1574). Every kind is listed, so a new kind fails to compile until it is placed.
#[must_use]
pub fn drill_title(title: &str, kind: &ReportKind) -> String {
    let prefix = match kind {
        ReportKind::GroupSummary(_) => "Group Summary: ",
        ReportKind::GroupLedgers(_) => "Group Ledgers: ",
        ReportKind::LedgerVouchers(_) => "Ledger Vouchers: ",
        ReportKind::BalanceSheet
        | ReportKind::DayBook
        | ReportKind::TrialBalance
        | ReportKind::ProfitAndLoss
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => return title.to_owned(),
    };
    title.strip_prefix(prefix).unwrap_or(title).to_owned()
}

/// The subtitle of a drill level (index.html:1577).
#[must_use]
pub fn drill_subtitle(as_of: ledgerly_core::Date) -> String {
    balance::drill_subtitle(as_of)
}

/// Shows one report, with row selection, drill-down, the detail toggle and the period editor.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is a separate one-frame request (scroll, export, print, alter) raised by a key or button and cleared after the frame"
)]
pub struct ReportViewScreen {
    report: ViewReport,
    source: ReportSource,
    selected: usize,
    scroll_pending: bool,
    period_edit: Option<PeriodEdit>,
    notice: Option<Notice>,
    /// Present only for a worker-mode screen (see [`ReportSource::worker`]).
    worker: Option<WorkerRun>,
    /// The footnote area's rectangle in the last frame drawn (see [`ReportViewScreen::footer_rect`]).
    footer: Option<Rect>,
    /// The selected line's rectangle in the last frame, when the generic table laid it out.
    selected_line: Option<Rect>,
    /// Ctrl+E or the Export button was pressed; the save dialog opens after the frame is drawn.
    export_wanted: bool,
    /// The save dialog.
    export_picker: ExportPicker,
    /// For a screen a drill opened, the sidebar title of the screen the drill started from.
    origin: Option<String>,
    /// The export in flight.
    export: Latest<Exported>,
    /// Ctrl+P was pressed; the print starts after the frame is drawn.
    print_wanted: bool,
    /// A head button opens another report (the trial balance's "Profit and loss"): its palette
    /// key. The screen opens after the frame.
    goto: Option<&'static str>,
    /// A head button asked for what Enter does on the selected row.
    drill_wanted: bool,
    /// A voucher button of the empty day book was pressed; the voucher opens after the frame.
    open_wanted: Option<crate::keymap::Action>,
    /// The print in flight, and its save dialog (see [`PrintJob`]).
    print: PrintJob,
    /// The side card of a ledger drill (group, GSTIN, credit days, open bills, Alter ledger).
    side: ledger_side::LedgerSide,
    /// The lines drawn to the mock for the trial balance, the outstanding bills and the stock
    /// summary, made from [`ReportViewScreen::report`] (see `mock.rs`). `None` draws the report.
    lines: Option<ViewReport>,
    /// Alter ledger was clicked; the masters screen opens after the frame.
    alter_wanted: bool,
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
            worker: None,
            footer: None,
            selected_line: None,
            export_wanted: false,
            export_picker: Box::new(save_dialog),
            origin: None,
            export: Latest::default(),
            print_wanted: false,
            goto: None,
            drill_wanted: false,
            lines: None,
            open_wanted: None,
            print: PrintJob::default(),
            side: ledger_side::LedgerSide::default(),
            alter_wanted: false,
        })
    }

    /// The rectangle of the footnote area (the report's notes, and the day book's and balance
    /// sheet's lines under the table) in the last frame drawn. The area is pinned to the bottom of
    /// the screen, so it stays above the key bar; `None` before the first frame.
    #[must_use]
    pub fn footer_rect(&self) -> Option<Rect> {
        self.footer
    }

    /// The selected line's rectangle in the last frame drawn, when the generic table (a report
    /// without a look of its own) laid it out; `None` for the other looks and while the line is
    /// out of view.
    #[must_use]
    pub fn selected_line_rect(&self) -> Option<Rect> {
        self.selected_line
    }

    /// A worker-mode screen for the report `source` describes. The report is requested from the
    /// API worker on the first frame; until it arrives the screen shows "Loading the report".
    ///
    /// Returns `None` when `source` is not a worker source.
    #[must_use]
    pub fn worker(source: ReportSource) -> Option<Self> {
        let engine = source.worker_base()?.clone();
        let want = ViewParams {
            period: engine.period,
            detail: view_detail(engine.detail),
        };
        Some(Self {
            report: pending_report(want),
            source,
            selected: 0,
            scroll_pending: false,
            period_edit: None,
            notice: None,
            worker: Some(WorkerRun {
                engine,
                want: Some(want),
                latest: Latest::default(),
            }),
            footer: None,
            selected_line: None,
            export_wanted: false,
            export_picker: Box::new(save_dialog),
            origin: None,
            export: Latest::default(),
            print_wanted: false,
            goto: None,
            drill_wanted: false,
            lines: None,
            open_wanted: None,
            print: PrintJob::default(),
            side: ledger_side::LedgerSide::default(),
            alter_wanted: false,
        })
    }

    /// Points the side card at the ledger a ledger drill shows (none for any other report), and
    /// moves its open bills along.
    fn follow_side(&mut self, ctx: &AppCtx) {
        let (info, period) = match self.worker.as_ref() {
            Some(run) => match (&run.engine.kind, ctx.books.as_ref()) {
                (ReportKind::LedgerVouchers(id), Some(books)) => {
                    (ledger_side::side_info(books, *id), run.engine.period)
                }
                _ => (None, run.engine.period),
            },
            None => return,
        };
        self.side.follow(info, period);
        self.side.tick(ctx.api.as_ref());
    }

    /// Advances a worker-mode screen by one step: applies a finished report and sends a waiting
    /// period or detail change to `worker`. Does nothing in host mode. A change with no worker
    /// shows an error and sends nothing.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        let finished = self.worker.as_mut().and_then(|run| run.latest.poll());
        match finished {
            Some(Done::Ready((params, answer, lines))) => self.apply(params, answer, lines),
            Some(Done::Lost) => self.fail(
                "the engine stopped before it answered; check the books and open the report again",
            ),
            None => {}
        }
        let Some(view) = self.worker.as_mut().and_then(|run| run.want.take()) else {
            return;
        };
        let Some(handle) = worker else {
            self.fail("No company is open, so the report cannot run.");
            return;
        };
        let Some(run) = self.worker.as_mut() else {
            return;
        };
        let as_of = if view.period == run.engine.period {
            run.engine.as_of
        } else {
            view.period.to()
        };
        let params = with_view(&run.engine, view, as_of);
        run.latest.start(handle, move |api| {
            let answer = run_report(api, &params);
            let lines = match &answer {
                Ok(Outcome::Value(view)) => mock_lines(api, &params, view),
                Ok(Outcome::Unsupported(_)) | Err(_) => None,
            };
            (params, answer, lines)
        });
    }

    /// Shows the answer to a worker request. A failed or unsupported answer keeps the previous
    /// report on screen, with the reason.
    fn apply(
        &mut self,
        params: ReportParams,
        answer: Result<Outcome<ViewReport>, ViewError>,
        lines: Option<ViewReport>,
    ) {
        match answer {
            Err(e) => self.fail(e.to_string()),
            Ok(Outcome::Unsupported(u)) => self.fail(unsupported_text(&u)),
            Ok(Outcome::Value(report)) => match report.check_shape() {
                Ok(()) => {
                    if let Some(run) = self.worker.as_mut() {
                        run.engine = params;
                    }
                    self.replace_report(report, lines);
                }
                Err(e) => self.fail(e.to_string()),
            },
        }
    }

    /// Replaces the save dialog Export opens.
    pub fn set_export_picker(&mut self, picker: ExportPicker) {
        self.export_picker = picker;
    }

    /// Starts a print of the report on the worker when Ctrl+P was pressed. Reports have no print
    /// layout yet, so the engine answers `Unsupported` and its reason is shown as the notice.
    fn run_print_request(&mut self, worker: Option<&ApiWorker>) {
        if !std::mem::take(&mut self.print_wanted) {
            return;
        }
        let Some(handle) = worker else {
            self.fail("No company is open, so the report cannot be printed.");
            return;
        };
        let Some(run) = self.worker.as_ref() else {
            self.fail("This report was not run by the engine, so it cannot be printed.");
            return;
        };
        let params = run.engine.clone();
        let title = self.report.title.clone();
        self.print
            .start(handle, &title, move |api| api.print_report(&params));
    }

    /// `true` while an export is being written.
    #[must_use]
    pub fn is_exporting(&self) -> bool {
        self.export.is_pending()
    }

    /// Writes the report on screen to `path` on `worker`, as CSV or XLSX by the file's extension
    /// (index.html:1624). The engine runs the same parameters again, so the file holds exactly the
    /// report shown. The screen says where the file went, or why it was not written: a name that
    /// is neither `.csv` nor `.xlsx`, a report still loading, a host-mode screen or no company.
    pub fn export_to(&mut self, path: PathBuf, worker: Option<&ApiWorker>) {
        let Some(format) = ExportFormat::from_path(&path) else {
            self.fail(format!(
                "\"{}\" was not written: choose a file name ending in .csv or .xlsx.",
                path.display()
            ));
            return;
        };
        if self.is_loading() {
            self.notice = Some(Notice::Info(
                "The report is still computing; export it once it is shown.".to_owned(),
            ));
            return;
        }
        let Some(run) = self.worker.as_ref() else {
            self.fail("This report was not run by the engine, so it cannot be exported.");
            return;
        };
        let Some(handle) = worker else {
            self.fail("No company is open, so the report cannot be exported.");
            return;
        };
        let params = run.engine.clone();
        self.export.start(handle, move |api| {
            let done = api
                .export_report(&params, format, &path)
                .map_err(|e| e.to_string());
            (path, done)
        });
    }

    /// Shows how a finished export went.
    fn poll_export(&mut self) {
        match self.export.poll() {
            Some(Done::Ready((path, Ok(Outcome::Value(()))))) => {
                self.notice = Some(Notice::Info(format!("Exported to {}.", path.display())));
            }
            Some(Done::Ready((_, Ok(Outcome::Unsupported(u))))) => self.fail(unsupported_text(&u)),
            Some(Done::Ready((path, Err(e)))) => {
                self.fail(format!("\"{}\" was not written: {e}", path.display()));
            }
            Some(Done::Lost) => {
                self.fail("the engine stopped before the export finished; nothing was written");
            }
            None => {}
        }
    }

    /// Opens the save dialog for a pending Export and starts the export on the file chosen.
    fn run_export_request(&mut self, worker: Option<&ApiWorker>) {
        if !std::mem::take(&mut self.export_wanted) {
            return;
        }
        let suggested = export_file_name(&self.report.title);
        if let Some(path) = (self.export_picker)(&suggested) {
            self.export_to(path, worker);
        }
    }

    /// `true` while a worker-mode screen waits for a report.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|run| run.want.is_some() || run.latest.is_pending())
    }

    /// `true` while the worker is running a request and no report has arrived yet: the key bar
    /// then says "Computing" and offers Esc to cancel (index.html:1579). A request that is only
    /// waiting to be sent does not count.
    fn is_computing(&self) -> bool {
        self.report.rows.is_empty()
            && self
                .worker
                .as_ref()
                .is_some_and(|run| run.latest.is_pending())
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
        self.shown().rows.get(self.selected)
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
        self.shown().rows.len().saturating_sub(1)
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
        let (key, label) = if let Some(row) = self.shown().rows.get(self.selected) {
            (row.key.clone(), row.label.clone())
        } else {
            self.info("This report has no rows to open.");
            return ScreenAction::Stay;
        };
        let Some(key) = key else {
            self.info(format!("\"{label}\" has nothing to open."));
            return ScreenAction::Stay;
        };
        let Some(drill) = self.source.host_drill() else {
            return self.drill_worker(&key);
        };
        match drill(&self.report.token, &key, self.report.params) {
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
                    Ok(screen) => ScreenAction::Push(Box::new(self.drilled(screen))),
                    Err(e) => {
                        self.fail(e.to_string());
                        ScreenAction::Stay
                    }
                }
            }
        }
    }

    /// `screen`, opened by a drill from this one: it keeps this screen's sidebar entry.
    fn drilled(&self, mut screen: ReportViewScreen) -> ReportViewScreen {
        screen.origin = Some(self.sidebar_title());
        screen
    }

    /// Drill in worker mode: a group or ledger opens a new worker-mode report with the same
    /// period and detail; a voucher opens the read-only voucher screen. The new screen asks the worker itself on its first frame.
    fn drill_worker(&mut self, key: &ViewKey) -> ScreenAction {
        // A voucher is a screen, not a report: open it read-only (see `voucher_view.rs`).
        if let ViewKey::Voucher(id) = key {
            let view = VoucherViewScreen::new(*id).with_origin(self.sidebar_title());
            return ScreenAction::Push(Box::new(view));
        }
        let base = self
            .worker
            .as_ref()
            .map(|run| with_view(&run.engine, self.report.params, run.engine.as_of));
        let Some(base) = base else {
            self.fail("This report has no worker to run a drill with.");
            return ScreenAction::Stay;
        };
        match drill_params(&base, key) {
            Err(notice) => {
                self.notice = Some(notice);
                ScreenAction::Stay
            }
            Ok(params) => {
                if let Some(screen) = ReportViewScreen::worker(ReportSource::worker(params)) {
                    ScreenAction::Push(Box::new(self.drilled(screen)))
                } else {
                    self.fail("The drilled report could not be set up.");
                    ScreenAction::Stay
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
            kind: EditorKind::Period,
            from: numeric_date_text(p.from()),
            to: numeric_date_text(p.to()),
            focus_pending: true,
        });
    }

    /// Opens the date editor (the day book's F2), filled with the day the period ends on.
    /// Applying it shows that one day.
    pub fn open_date_editor(&mut self) {
        let day = numeric_date_text(self.report.params.period.to());
        self.period_edit = Some(PeriodEdit {
            kind: EditorKind::Date,
            from: day.clone(),
            to: day,
            focus_pending: true,
        });
    }

    /// What the open editor asks for, or `None` when no editor is open.
    #[must_use]
    pub fn editor_kind(&self) -> Option<EditorKind> {
        self.period_edit.as_ref().map(|edit| edit.kind)
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
            Some(edit) => match edit.kind {
                EditorKind::Date => parse_period_text(&edit.from, &edit.from),
                EditorKind::Period => parse_period_text(&edit.from, &edit.to),
            },
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
        if let Some(run) = self.worker.as_mut() {
            run.want = Some(params);
            return;
        }
        let Some(refresh) = self.source.host_refresh() else {
            self.fail("This report has no source to run it with.");
            return;
        };
        match refresh(&self.report.token, params) {
            Err(e) => self.fail(e.to_string()),
            Ok(Outcome::Unsupported(u)) => self.fail(unsupported_text(&u)),
            Ok(Outcome::Value(report)) => match report.check_shape() {
                Ok(()) => self.replace_report(report, None),
                Err(e) => self.fail(e.to_string()),
            },
        }
    }

    /// Swaps in a reshaped report and keeps the selection on the same row: the row with the same
    /// drill key, or for a keyless row the same kind and label; otherwise the same position, or
    /// the last row.
    fn replace_report(&mut self, report: ViewReport, lines: Option<ViewReport>) {
        let old_index = self.selected;
        let old = self
            .shown()
            .rows
            .get(old_index)
            .map(|r| (r.key.clone(), r.label.clone(), r.kind));
        self.report = report;
        self.lines = lines;
        let found = old.and_then(|(key, label, kind)| {
            self.shown().rows.iter().position(|r| match &key {
                Some(k) => r.key.as_ref() == Some(k),
                None => r.key.is_none() && r.label == label && r.kind == kind,
            })
        });
        self.selected = found.unwrap_or_else(|| old_index.min(self.last_row()));
        self.scroll_pending = true;
    }

    /// What the table draws and the cursor moves over: the lines made for the mock when there
    /// are some, otherwise the engine's report.
    fn shown(&self) -> &ViewReport {
        self.lines.as_ref().unwrap_or(&self.report)
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
            KeyCmd::EditDate => self.open_date_editor(),
            KeyCmd::EditPeriod => self.open_period_editor(),
            KeyCmd::ApplyPeriod => self.apply_period_edit(),
            KeyCmd::CancelPeriod => self.cancel_period_edit(),
            KeyCmd::Export => self.export_wanted = true,
            KeyCmd::Print => self.print_wanted = true,
            KeyCmd::ProfitLoss => self.goto = Some("profit-and-loss"),
        }
        ScreenAction::Stay
    }

    /// What Alt+F1 and the button switch to: the other detail level.
    fn detail_switch(&self) -> &'static str {
        match self.report.params.detail {
            ViewDetail::Condensed => "Detailed",
            ViewDetail::Detailed => "Condensed",
        }
    }

    /// `true` for a report that a drill opens (a group summary, a ledger's vouchers). Every kind
    /// is listed, so a new kind fails to compile until it is placed.
    fn is_drill_level(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|run| match run.engine.kind {
                ReportKind::GroupSummary(_)
                | ReportKind::GroupLedgers(_)
                | ReportKind::LedgerVouchers(_) => true,
                ReportKind::BalanceSheet
                | ReportKind::DayBook
                | ReportKind::TrialBalance
                | ReportKind::ProfitAndLoss
                | ReportKind::Register(_)
                | ReportKind::Outstanding { .. }
                | ReportKind::StockSummary
                | ReportKind::Audit(_) => false,
            })
    }

    /// The title of a report drawn as a generic table: a drill level reads as the name drilled.
    fn generic_title(&self) -> String {
        match self.worker.as_ref() {
            Some(run) => drill_title(&self.report.title, &run.engine.kind),
            None => self.report.title.clone(),
        }
    }

    /// The layout this screen draws in.
    fn look(&self) -> Look {
        match self.worker.as_ref() {
            Some(run) => look_of(&run.engine.kind),
            None => Look::Generic,
        }
    }

    /// The page header, the day book's tabs, the loading line, the notice and the period editor.
    fn draw_head(&mut self, ui: &mut Ui, look: Look) {
        let title = self.page_title();
        let subtitle = self.subtitle();
        let detail_switch = self.detail_switch();
        let mut toggle = false;
        let mut tab_clicked = None;
        if look == Look::DayBook {
            // index.html:1643-1645: the subtitle carries keycaps and the tabs sit at the right of
            // the head.
            let period = self.report.params.period;
            let labels = day_book::tab_labels(period);
            let labels = [labels[0].as_str(), labels[1].as_str()];
            page_head_with(
                ui,
                &title,
                |ui| draw_sub_parts(ui, &day_book::subtitle_parts(period)),
                |ui| {
                    let size = egui::vec2(tab_strip_width(ui, &labels), ui.available_height());
                    tab_clicked = ui
                        .allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
                            tab_strip(ui, &labels, day_book::tab_of(period)).clicked
                        })
                        .inner;
                },
            );
        } else if let Look::Mock(m) = look {
            self.draw_mock_head(ui, m, &title, subtitle.as_deref());
        } else {
            let mut export = false;
            // index.html:1577: a drill level (a group, a ledger) has no action buttons; its keys
            // still offer Ctrl+E (index.html:1634).
            let drill_level = self.is_drill_level();
            page_head(ui, &title, subtitle.as_deref(), |ui| {
                // index.html:1576: the detail switch, then Export at the right. The actions are
                // laid out right to left, so Export goes first.
                if !drill_level {
                    export = button_icon(
                        ui,
                        Some(Icon::Upload),
                        "Export",
                        Some("Ctrl+E"),
                        ButtonKind::Secondary,
                    )
                    .clicked();
                }
                if look == Look::BalanceSheet {
                    toggle =
                        button(ui, detail_switch, Some("Alt+F1"), ButtonKind::Secondary).clicked();
                }
            });
            if export {
                self.export_wanted = true;
            }
        }
        if toggle {
            self.toggle_detail();
        }
        if let Some(tab) = tab_clicked
            && let Some(period) = day_book::period_of_tab(self.report.params.period, tab)
        {
            self.set_period(period);
        }
        if self.is_loading() {
            ui.ctx().request_repaint();
        }
        match &self.notice {
            Some(Notice::Error(s)) => draw_error(ui, s),
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
        self.draw_period_editor(ui);
    }

    /// The head of a report drawn to the mock: tabs for the audit screen and the outstanding
    /// bills, and the buttons the mock puts at the right.
    #[allow(
        clippy::too_many_lines,
        reason = "one head: tabs, the buttons of each report and their clicks"
    )]
    fn draw_mock_head(&mut self, ui: &mut Ui, m: mock::Mock, title: &str, subtitle: Option<&str>) {
        let tabs: Vec<(&str, bool)> = match m {
            mock::Mock::Audit(current) => mock::AuditTab::ALL
                .iter()
                .map(|t| (t.label(), *t == current))
                .collect(),
            mock::Mock::Outstanding { receivable } => {
                vec![("Receivables", receivable), ("Payables", !receivable)]
            }
            mock::Mock::Trial | mock::Mock::Stock | mock::Mock::ProfitLoss => Vec::new(),
        };
        let (mut export, mut print, mut goto, mut drill) = (false, false, false, false);
        let mut clicked = None;
        let sub = subtitle.unwrap_or_default().to_owned();
        let t = crate::ui::tokens(ui);
        let drawing_tabs = !tabs.is_empty();
        let labels: Vec<&str> = tabs.iter().map(|(l, _)| *l).collect();
        let selected = tabs.iter().position(|(_, on)| *on);
        page_head_with(
            ui,
            title,
            |ui| {
                ui.label(para_text(&sub, crate::theme::TEXT_BODY, t.text_2));
            },
            |ui| {
                if drawing_tabs {
                    let size = egui::vec2(tab_strip_width(ui, &labels), ui.available_height());
                    clicked = ui
                        .allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
                            tab_strip(ui, &labels, selected).clicked
                        })
                        .inner;
                    return;
                }
                match m {
                    mock::Mock::Trial => {
                        goto = button(ui, "Profit and loss", Some("F10"), ButtonKind::Secondary)
                            .clicked();
                    }
                    mock::Mock::Stock => {
                        drill = button(ui, "Stock items", Some("Enter"), ButtonKind::Secondary)
                            .clicked();
                    }
                    mock::Mock::ProfitLoss => {
                        export = button_icon(
                            ui,
                            Some(Icon::Upload),
                            "Export",
                            Some("Ctrl+E"),
                            ButtonKind::Secondary,
                        )
                        .clicked();
                        print = button_icon(
                            ui,
                            Some(Icon::Upload),
                            "Print preview",
                            Some("Ctrl+P"),
                            ButtonKind::Secondary,
                        )
                        .clicked();
                    }
                    mock::Mock::Outstanding { .. } | mock::Mock::Audit(_) => {}
                }
            },
        );
        if export {
            self.export_wanted = true;
        }
        if print {
            self.print_wanted = true;
        }
        if goto {
            self.goto = Some("profit-and-loss");
        }
        if drill {
            self.drill_wanted = true;
        }
        if let Some(i) = clicked {
            let kind = match m {
                mock::Mock::Audit(_) => mock::AuditTab::ALL.get(i).map(|t| t.kind()),
                mock::Mock::Outstanding { .. } => {
                    Some(ReportKind::Outstanding { receivable: i == 0 })
                }
                mock::Mock::Trial | mock::Mock::Stock | mock::Mock::ProfitLoss => None,
            };
            if let Some(kind) = kind {
                self.switch_kind(kind);
            }
        }
    }

    /// Shows another report of the same family (a tab): the placeholder until the engine answers,
    /// so the old report is never shown under the new tab.
    fn switch_kind(&mut self, kind: ReportKind) {
        let Some(run) = self.worker.as_mut() else {
            return;
        };
        if run.engine.kind == kind {
            return;
        }
        run.engine.kind = kind;
        // The period asked for, not the one the previous report carried (the audit reports carry
        // the whole calendar).
        let params = ViewParams {
            period: run.engine.period,
            ..self.report.params
        };
        self.report = pending_report(params);
        self.lines = None;
        self.selected = 0;
        self.refresh(params);
    }

    fn draw(&mut self, ui: &mut Ui) -> ScreenAction {
        let keys = read_keys(ui.ctx(), self.period_edit.is_some(), self.look());
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if let ScreenAction::Push(screen) = self.run(*cmd) {
                action = ScreenAction::Push(screen);
            }
        }
        let look = self.look();
        self.draw_head(ui, look);
        if self.report.columns.is_empty()
            && self.report.rows.is_empty()
            && !self.is_loading()
            && matches!(self.notice, Some(Notice::Error(_)))
        {
            // The engine refused the report: the note above says why, and there is no table to
            // draw (no empty card, no figure).
            self.footer = None;
            self.selected_line = None;
            return action;
        }
        if self.is_loading() && self.report.rows.is_empty() {
            // Nothing has arrived yet: say so, and draw no figure.
            draw_loading(ui);
            self.footer = None;
            self.selected_line = None;
            return action;
        }

        let balance = if look == Look::BalanceSheet {
            balance::vertical_balance(&self.report)
        } else {
            None
        };
        let day_cols = if look == Look::DayBook {
            day_book::day_columns(&self.report.columns)
        } else {
            None
        };
        let (selected, scroll) = (self.selected, self.scroll_pending);
        let day_card = day_cols.filter(|_| !self.report.rows.is_empty());
        let events = if let Some(cols) = day_card {
            // The day book draws its own card with the footnote under it (B7 holds).
            let (events, foot) = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    day_book::draw(ui, &self.report, cols, selected, scroll)
                })
                .inner;
            self.footer = Some(foot);
            events
        } else if let Some(b) = &balance {
            // The balance sheet draws its own card with the check and the notes pinned under the
            // statement (B7 holds inside the card).
            let (events, foot) = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    balance::draw(ui, &self.report, b, selected, scroll)
                })
                .inner;
            self.footer = Some(foot);
            events
        } else {
            self.draw_plain(ui, look, selected, scroll)
        };
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

    /// The generic table, or the empty day book's card, with the footnote under it.
    fn draw_plain(
        &mut self,
        ui: &mut Ui,
        look: Look,
        selected: usize,
        scroll: bool,
    ) -> TableEvents {
        let empty_day = look == Look::DayBook && self.report.rows.is_empty();
        if empty_day {
            // The empty day book is a card with its voucher buttons, the footnote under it.
            let (chosen, foot) = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| day_book::draw_empty(ui, &self.report))
                .inner;
            self.footer = Some(foot);
            self.open_wanted = chosen;
            TableEvents::default()
        } else {
            // The footnote is a bottom panel added before the table, so the table takes the
            // space left above it and the footnote stays inside the screen, above the key bar
            // (B7).
            let footer = egui::Panel::bottom("u7-footer")
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 6)))
                .show(ui, |ui| self.draw_footer(ui, look))
                .response
                .rect;
            self.footer = Some(footer);
            let report = self.shown();
            let side = &self.side;
            let mut alter = false;
            let drawn = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    if side.ledger().is_some() {
                        let mut drawn = None;
                        crate::screens::home::draw::split(
                            ui,
                            |ui| drawn = Some(generic::draw(ui, report, selected, scroll)),
                            |ui| alter = side.draw(ui),
                        );
                        drawn.unwrap_or_else(|| generic::draw(ui, report, selected, scroll))
                    } else {
                        generic::draw(ui, report, selected, scroll)
                    }
                })
                .inner;
            if alter {
                self.alter_wanted = true;
            }
            self.selected_line = drawn.selected;
            drawn.events
        }
    }

    /// The footnote under a table: the report's notes, and what the day book does not show yet.
    /// The balance sheet draws its own (see [`balance::draw`]).
    fn draw_footer(&self, ui: &mut Ui, look: Look) {
        let t = crate::ui::tokens(ui);
        for line in &self.report.notes {
            ui.label(
                egui::RichText::new(line.as_str())
                    .size(crate::theme::TEXT_META)
                    .color(t.text_3),
            );
        }
        if look == Look::DayBook {
            note(ui, NoteKind::Info, day_book::NOT_RECORDED);
        }
    }

    fn draw_period_editor(&mut self, ui: &mut Ui) {
        let base = ui.scope_id().with("u7-period");
        let Some(edit) = self.period_edit.as_mut() else {
            return;
        };
        let (mut apply, mut cancel) = (false, false);
        let kind = edit.kind;
        ui.group(|ui| {
            ui.label(match kind {
                EditorKind::Date => "Date (DD-MM-YYYY)",
                EditorKind::Period => "Period (DD-MM-YYYY)",
            });
            ui.horizontal(|ui| {
                ui.label(match kind {
                    EditorKind::Date => "Date",
                    EditorKind::Period => "From",
                });
                let from = ui.add(
                    egui::TextEdit::singleline(&mut edit.from)
                        .id(base.with("from"))
                        .desired_width(110.0),
                );
                if edit.focus_pending {
                    from.request_focus();
                    edit.focus_pending = false;
                }
                match kind {
                    EditorKind::Date => {}
                    EditorKind::Period => {
                        ui.label("To");
                        ui.add(
                            egui::TextEdit::singleline(&mut edit.to)
                                .id(base.with("to"))
                                .desired_width(110.0),
                        );
                    }
                }
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
}

/// Percent widths of the loading skeleton rows (index.html:1579).
const LOADING_BARS: [u8; 8] = [60, 45, 70, 40, 55, 80, 35, 65];

/// The report while it computes (index.html:1579): a card with a line that says so, then
/// placeholder bars that never look like figures.
fn draw_loading(ui: &mut Ui) {
    let t = crate::ui::tokens(ui);
    let k = crate::ui::kind(ui);
    egui::Frame::new()
        .fill(t.surface)
        .stroke(egui::Stroke::new(crate::theme::border_width(k), t.border))
        .corner_radius(crate::theme::RADIUS_M)
        .shadow(crate::theme::shadow_1(k))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // index.html:1579: a clock, the line, an Esc keycap, then the rest of the sentence.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = crate::theme::SPACE_2;
                crate::ui::icon::icon(ui, crate::ui::icon::Icon::Clock, t.text_2);
                ui.label(
                    egui::RichText::new("Computing from all vouchers\u{2026}")
                        .size(crate::theme::TEXT_BODY)
                        .color(t.text_2),
                );
                keycap(ui, "Esc");
                ui.label(
                    egui::RichText::new("cancels. The previous result is not shown as current.")
                        .size(crate::theme::TEXT_BODY)
                        .color(t.text_2),
                );
            });
            ui.add_space(14.0);
            let width = ui.available_width();
            // index.html:1579: twelve rows, their widths cycling through LOADING_BARS.
            for i in 0..12_usize {
                let pct = f32::from(
                    LOADING_BARS
                        .get(i % LOADING_BARS.len())
                        .copied()
                        .unwrap_or(50),
                );
                crate::ui::skeleton::skeleton_bar(ui, width * pct / 100.0);
                ui.add_space(crate::theme::SPACE_2);
            }
        });
}

/// Draws a subtitle made of text and keycaps on one line, in the subtitle's ink.
fn draw_sub_parts(ui: &mut Ui, parts: &[day_book::SubPart]) {
    let t = crate::ui::tokens(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for part in parts {
            match part {
                day_book::SubPart::Text(text) => {
                    ui.label(para_text(text, crate::theme::TEXT_BODY, t.text_2));
                }
                day_book::SubPart::Key(key) => {
                    keycap(ui, key);
                }
            }
        }
    });
}

/// Reads this frame's keys. While the period editor is open only Enter and Esc count, so the
/// arrows belong to the text fields. Alt+F1 comes first in the table because egui ignores extra
/// Alt and Shift when it matches a plain key.
fn read_keys(ctx: &Context, editing: bool, look: Look) -> Vec<KeyCmd> {
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
        // TallyHelp "Viewing Day Book": in the day book F2 changes the date and Alt+F2 the
        // period; elsewhere F2 edits the period. Alt+F2 comes before F2 for the reason above.
        let f2 = match look {
            Look::DayBook => KeyCmd::EditDate,
            Look::BalanceSheet | Look::Generic | Look::Mock(_) => KeyCmd::EditPeriod,
        };
        let table = [
            (Modifiers::ALT, Key::F1, KeyCmd::ToggleDetail),
            (Modifiers::ALT, Key::F2, KeyCmd::EditPeriod),
            (Modifiers::NONE, Key::F2, f2),
            (Modifiers::NONE, Key::Enter, KeyCmd::Open),
            (Modifiers::NONE, Key::ArrowUp, KeyCmd::Up),
            (Modifiers::NONE, Key::ArrowDown, KeyCmd::Down),
            // index.html:1634 and 1656: "J / K" Move.
            (Modifiers::NONE, Key::J, KeyCmd::Down),
            (Modifiers::NONE, Key::K, KeyCmd::Up),
            (Modifiers::NONE, Key::PageUp, KeyCmd::PageUp),
            (Modifiers::NONE, Key::PageDown, KeyCmd::PageDown),
            (Modifiers::NONE, Key::Home, KeyCmd::First),
            (Modifiers::NONE, Key::End, KeyCmd::Last),
            // index.html:1624 and 1634: Ctrl+E exports.
            (Modifiers::COMMAND, Key::E, KeyCmd::Export),
            // Ctrl+P prints the open report (index.html:1839); see design/PARITY-GAPS.md.
            (Modifiers::COMMAND, Key::P, KeyCmd::Print),
        ];
        if look == Look::Mock(mock::Mock::Trial) && i.consume_key(Modifiers::NONE, Key::F10) {
            out.push(KeyCmd::ProfitLoss);
        }
        for (modifiers, key, cmd) in table {
            if i.consume_key(modifiers, key) {
                out.push(cmd);
            }
        }
    });
    out
}

impl Screen for ReportViewScreen {
    fn cursor_label(&self, _ctx: &AppCtx) -> Option<String> {
        let row = self.selected_row()?;
        // A voucher line is named by its voucher number (its label is the other ledger's name,
        // which many vouchers share).
        if row.kind == ViewRowKind::Voucher {
            let number = self
                .report
                .columns
                .iter()
                .position(|c| c == "Voucher No")
                .and_then(|i| row.cells.get(i));
            if let Some(ViewCell::Text(number)) = number {
                return Some(number.clone());
            }
        }
        Some(row.label.clone())
    }

    fn title(&self) -> String {
        // The crumb uses the page title's wording for the screens drawn to the mock
        // (index.html:1572: "Balance sheet").
        match self.look() {
            Look::BalanceSheet | Look::DayBook => self.page_title(),
            Look::Mock(m) => m.crumb().to_owned(),
            Look::Generic => self.generic_title(),
        }
    }

    fn page_title(&self) -> String {
        match self.look() {
            Look::BalanceSheet => "Balance sheet".to_owned(),
            Look::DayBook => "Day book".to_owned(),
            Look::Mock(m) => m.page_title().to_owned(),
            Look::Generic => self.generic_title(),
        }
    }

    fn subtitle(&self) -> Option<String> {
        let period = self.report.params.period;
        match self.look() {
            Look::BalanceSheet => {
                let as_of = self
                    .worker
                    .as_ref()
                    .map_or(period.to(), |run| run.engine.as_of);
                Some(balance::subtitle(as_of))
            }
            Look::DayBook => Some(day_book::subtitle(period)),
            Look::Mock(m) => {
                let as_of = self
                    .worker
                    .as_ref()
                    .map_or(period.to(), |run| run.engine.as_of);
                // The period asked for, not the one the report carries: the audit reports carry
                // the whole calendar.
                let asked = self.worker.as_ref().map_or(period, |run| run.engine.period);
                Some(m.subtitle(as_of, asked))
            }
            Look::Generic if self.is_drill_level() => {
                let as_of = self
                    .worker
                    .as_ref()
                    .map_or(period.to(), |run| run.engine.as_of);
                Some(balance::drill_subtitle(as_of))
            }
            Look::Generic => Some(format!(
                "{}  |  {}",
                period_text(period),
                self.report.params.detail.label()
            )),
        }
    }

    fn sidebar_title(&self) -> String {
        self.origin.clone().unwrap_or_else(|| self.title())
    }

    fn key_bar_name(&self) -> String {
        if self.is_computing() {
            // index.html:1579: while the report computes the bar says so and offers only Esc.
            return "Computing".to_owned();
        }
        match self.look() {
            Look::Mock(m) => m.key_bar_name().to_owned(),
            Look::BalanceSheet | Look::DayBook | Look::Generic => self.page_title(),
        }
    }

    fn keys(&self) -> Vec<KeyHint> {
        if self.is_computing() {
            return vec![KeyHint::new("Esc", "Cancel")];
        }
        match self.look() {
            Look::BalanceSheet => vec![
                KeyHint::new("J / K", "Move"),
                KeyHint::new("Enter", "Drill in"),
                KeyHint::new("Esc", "Back up"),
                KeyHint::new("Alt+F1", self.detail_switch()),
                KeyHint::new("Ctrl+E", "Export"),
                KeyHint::new("F2", "Period"),
            ],
            Look::DayBook => vec![
                KeyHint::new("J / K", "Move"),
                KeyHint::new("Enter", "Open voucher"),
                KeyHint::new("F2", "Date"),
                KeyHint::new("Esc", "Back"),
            ],
            Look::Mock(m) => m
                .keys()
                .into_iter()
                .map(|(k, a)| KeyHint::new(k, a))
                .collect(),
            Look::Generic => vec![
                KeyHint::new("J / K", "Move"),
                KeyHint::new("Enter", "Open"),
                KeyHint::new("Esc", "Back up"),
                KeyHint::new("Alt+F1", "Condensed or detailed"),
                KeyHint::new("Ctrl+E", "Export"),
                KeyHint::new("F2", "Period"),
            ],
        }
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx.api.as_ref());
        self.poll_export();
        self.follow_side(ctx);
        let mut action = ScreenAction::Stay;
        // Ctrl+Enter alters the ledger (index.html:1620); taken before the table reads Enter.
        if self.side.ledger().is_some()
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter))
        {
            self.alter_wanted = true;
        }
        if let ScreenAction::Push(screen) = self.draw(ui) {
            action = ScreenAction::Push(screen);
        }
        if std::mem::take(&mut self.alter_wanted)
            && let Some(ledger) = self.side.ledger()
        {
            action = ScreenAction::Push(Box::new(
                crate::screens::masters::MastersScreen::altering_ledger(ledger),
            ));
        }
        if let Some(key) = self.goto.take() {
            match crate::shell::navigate::open_target(
                &crate::palette::PaletteTarget::Report(key.to_owned()),
                ctx,
            ) {
                Ok(screen) => action = ScreenAction::Push(screen),
                Err(e) => self.notice = Some(Notice::Error(e.to_string())),
            }
        }
        if std::mem::take(&mut self.drill_wanted)
            && let ScreenAction::Push(screen) = self.drill()
        {
            action = ScreenAction::Push(screen);
        }
        if let Some(wanted) = self.open_wanted.take() {
            match crate::screens::ai_gate::open_voucher(wanted, ctx) {
                Ok(open) => action = open,
                Err(why) => self.notice = Some(Notice::Error(why)),
            }
        }
        if self.print.saved_path().is_some()
            && button(ui, "Open PDF", None, ButtonKind::Secondary).clicked()
            && let Some(note) = self.print.open_saved()
        {
            self.notice = Some(note);
        }
        self.run_export_request(ctx.api.as_ref());
        if let Some(note) = self.print.tick() {
            self.notice = Some(note);
        }
        self.run_print_request(ctx.api.as_ref());
        if self.is_exporting() || self.print.is_busy() {
            ui.ctx().request_repaint();
        }
        action
    }
}

/// The red note over a report the engine refused or could not run: the words the engine gave,
/// with the lead sentence in bold (the mock's `audit.unsupported`).
fn draw_error(ui: &mut Ui, text: &str) {
    let (lead, rest) = match text.split_once(": ") {
        Some((lead, rest)) => (format!("{lead}."), rest.to_owned()),
        None => (
            String::from("The report could not be shown."),
            text.to_owned(),
        ),
    };
    crate::ui::note::note_with_icon(ui, NoteKind::Bad, Icon::Alert, |ui, ink| {
        ui.label(
            egui::RichText::new(lead)
                .font(crate::ui::bold_font(ui, crate::theme::TEXT_META))
                .color(ink),
        );
        ui.label(para_text(&rest, crate::theme::TEXT_META, ink));
    });
    ui.add_space(8.0);
}

/// The placeholder a worker-mode screen shows until its first report arrives: no rows, no
/// figures.
fn pending_report(params: ViewParams) -> ViewReport {
    ViewReport {
        token: ReportToken("pending".to_owned()),
        title: "Report".to_owned(),
        params,
        columns: Vec::new(),
        rows: Vec::new(),
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod loading_card {
    //! The loading state is a card that says what is happening, with placeholder bars, never a
    //! figure (mock v2 `balance-sheet.loading`).
    //!
    //! provenance: index.html:1579: "Computing from all vouchers… Esc cancels. The previous
    //! result is not shown as current." over twelve skeleton rows.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use egui::Shape;
    use ledgerly_api::{ColumnSpec, Detail};
    use time::macros::date;

    fn texts(shape: &Shape, out: &mut Vec<String>) {
        if let Shape::Text(t) = shape {
            out.push(t.galley.text().to_owned());
        } else if let Shape::Vec(inner) = shape {
            for s in inner {
                texts(s, out);
            }
        }
    }

    #[test]
    fn a_pending_balance_sheet_says_it_is_computing() {
        let period = Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 21)).unwrap();
        let params = ReportParams {
            kind: ReportKind::BalanceSheet,
            period,
            as_of: date!(2026 - 11 - 21),
            scenario: ledgerly_model::Scenario::Actual,
            detail: Detail::Condensed,
            columns: ColumnSpec::Single,
            filter: ledgerly_api::ReportFilter::default(),
            exceptions_only: false,
        };
        let mut screen = ReportViewScreen::worker(ReportSource::worker(params)).unwrap();
        assert!(screen.is_loading());
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            screen.draw(ui);
        });
        let mut drawn = Vec::new();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut drawn);
        }
        out.drop_without_applying_deltas();
        assert!(
            drawn
                .iter()
                .any(|t| t.starts_with("Computing from all vouchers")),
            "texts drawn: {drawn:?}"
        );
        assert!(
            drawn.iter().all(|t| !t.contains("Loading the report")),
            "texts drawn: {drawn:?}"
        );
    }
}
