//! Worker-mode report requests: the conversion from `ledgerly-api` reports to the screen's model,
//! and the parameters a screen asks the API for (card W7).
//!
//! Nothing here adds, rounds or re-derives a figure: every amount is copied from the engine's
//! report, cell for cell. The matches are exhaustive over the report model, so a new cell or row
//! kind fails to compile until it is shown (rule A2).

use ledgerly_api::{
    Api, Cell, Detail, DrillKey, Report, ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_core::{Date, Outcome};

use super::{
    Notice, ReportToken, ViewCell, ViewDetail, ViewError, ViewKey, ViewParams, ViewReport, ViewRow,
    ViewRowKind,
};
use crate::screens::wire::unsupported_message;

/// The engine's detail level, as the screen names it.
#[must_use]
pub(super) fn view_detail(detail: Detail) -> ViewDetail {
    match detail {
        Detail::Condensed => ViewDetail::Condensed,
        Detail::Detailed => ViewDetail::Detailed,
    }
}

/// The engine's detail level for a screen level.
#[must_use]
pub(super) fn engine_detail(detail: ViewDetail) -> Detail {
    match detail {
        ViewDetail::Condensed => Detail::Condensed,
        ViewDetail::Detailed => Detail::Detailed,
    }
}

/// `base` with the period and detail the screen shows. `as_of` is the balance date the request
/// uses; the caller decides it (the period end when the period changed, otherwise the old date).
#[must_use]
pub(super) fn with_view(base: &ReportParams, p: ViewParams, as_of: Date) -> ReportParams {
    ReportParams {
        period: p.period,
        as_of,
        detail: engine_detail(p.detail),
        ..base.clone()
    }
}

/// The report a drill on `key` opens. `base` supplies the period, the detail and the scenario.
///
/// # Errors
/// The notice to show instead when `key` does not open a report: a voucher is a screen (not
/// wired in worker mode yet), and a stock item or a bill has no report yet.
pub(super) fn drill_params(base: &ReportParams, key: &ViewKey) -> Result<ReportParams, Notice> {
    let kind = match key {
        ViewKey::Group(group) => ReportKind::GroupSummary(*group),
        ViewKey::Ledger(ledger) => ReportKind::LedgerVouchers(*ledger),
        ViewKey::Voucher(_) => {
            return Err(Notice::Info(
                "Opening a voucher from a report is not wired yet.".to_owned(),
            ));
        }
        ViewKey::Item(_) | ViewKey::Bill { .. } => {
            return Err(Notice::Error(unsupported_message(
                "drill into stock items and bills",
                "no report exists for them yet",
            )));
        }
    };
    Ok(ReportParams {
        kind,
        ..base.clone()
    })
}

/// Runs `params` on the worker's API and shows the result as the screen's model. Runs on the
/// API thread: the result is plain data that can cross back to the UI thread.
///
/// # Errors
/// [`ViewError::Source`] with the API's message when the engine refuses the report.
pub(super) fn run_report(
    api: &Api,
    params: &ReportParams,
) -> Result<Outcome<ViewReport>, ViewError> {
    match api.report(params) {
        Err(e) => Err(ViewError::Source {
            message: e.to_string(),
        }),
        Ok(Outcome::Unsupported(u)) => Ok(Outcome::Unsupported(u)),
        Ok(Outcome::Value(report)) => Ok(Outcome::Value(view_report(&report))),
    }
}

/// A report in the screen's model. The token is the title; the screen never reads it, it only
/// keys the table's id.
#[must_use]
pub(super) fn view_report(report: &Report) -> ViewReport {
    ViewReport {
        token: ReportToken(report.title.clone()),
        title: report.title.clone(),
        params: ViewParams {
            period: report.params.period,
            detail: view_detail(report.params.detail),
        },
        columns: report.columns.iter().map(|c| c.label.clone()).collect(),
        rows: report.rows.iter().map(view_row).collect(),
        totals: report.totals.iter().map(view_cell).collect(),
        notes: report.notes.clone(),
    }
}

fn view_row(row: &ReportRow) -> ViewRow {
    ViewRow {
        key: row.key.as_ref().map(view_key),
        label: row.label.clone(),
        depth: row.depth,
        kind: view_row_kind(row.kind),
        cells: row.cells.iter().map(view_cell).collect(),
    }
}

fn view_key(key: &DrillKey) -> ViewKey {
    match key {
        DrillKey::Group(g) => ViewKey::Group(*g),
        DrillKey::Ledger(l) => ViewKey::Ledger(*l),
        DrillKey::Voucher(v) => ViewKey::Voucher(*v),
        DrillKey::Item(i) => ViewKey::Item(*i),
        DrillKey::Bill { ledger, name } => ViewKey::Bill {
            ledger: *ledger,
            name: name.clone(),
        },
    }
}

fn view_row_kind(kind: RowKind) -> ViewRowKind {
    match kind {
        RowKind::Group => ViewRowKind::Group,
        RowKind::Ledger => ViewRowKind::Ledger,
        RowKind::Voucher => ViewRowKind::Voucher,
        RowKind::Item => ViewRowKind::Item,
        RowKind::Bill => ViewRowKind::Bill,
        RowKind::Difference => ViewRowKind::Difference,
        RowKind::Subtotal => ViewRowKind::Subtotal,
        RowKind::Text => ViewRowKind::Text,
    }
}

/// A cell, copied as the engine gave it.
#[must_use]
pub(super) fn view_cell(cell: &Cell) -> ViewCell {
    match cell {
        Cell::Amount(a) => ViewCell::Amount(*a),
        Cell::Signed(p) => ViewCell::Signed(*p),
        Cell::Qty(q) => ViewCell::Qty(*q),
        Cell::Text(s) => ViewCell::Text(s.clone()),
        Cell::Date(d) => ViewCell::Date(*d),
        Cell::Empty => ViewCell::Empty,
        Cell::Provisional { amount, reason } => ViewCell::Provisional {
            amount: *amount,
            reason: reason.clone(),
        },
        Cell::Unsupported(u) => ViewCell::Unsupported(u.clone()),
    }
}
