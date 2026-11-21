//! Conversion from `ledgerly-api` reports to the screen's own report model, and the parameters
//! the screen asks the API for (card W7).
//!
//! Nothing here adds, rounds or re-derives a figure: every amount is copied from the engine's
//! report, cell for cell. The matches are exhaustive over the report model, so a new cell or row
//! kind fails to compile until it is shown (rule A2).

use ledgerly_api::{
    Api, ApiError, Cell, Detail, DrillKey, Report, ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_core::{Date, Outcome, Period};
use ledgerly_model::Scenario;

use crate::screens::audit::{AuditRow, AuditTable};

use super::text::cell_text;
use super::{
    ReportToken, ViewCell, ViewDetail, ViewError, ViewKey, ViewParams, ViewReport, ViewRow,
    ViewRowKind,
};

/// The engine's detail level, as the screen names it.
#[must_use]
pub fn view_detail(detail: Detail) -> ViewDetail {
    match detail {
        Detail::Condensed => ViewDetail::Condensed,
        Detail::Detailed => ViewDetail::Detailed,
    }
}

/// The engine's detail level for a screen level.
#[must_use]
pub fn engine_detail(detail: ViewDetail) -> Detail {
    match detail {
        ViewDetail::Condensed => Detail::Condensed,
        ViewDetail::Detailed => Detail::Detailed,
    }
}

/// The parameters for `kind` over `period`, with the defaults the screens use: the engine's
/// actual scenario, one column, no filter, and the period end as the balance date.
#[must_use]
pub fn default_params(kind: ReportKind, period: Period) -> ReportParams {
    ReportParams {
        kind,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ledgerly_api::ColumnSpec::Single,
        filter: ledgerly_api::ReportFilter::default(),
        exceptions_only: false,
    }
}

/// `base` with the period and detail the screen shows. The balance date follows the period end
/// when the period changes; `as_of` is kept otherwise.
#[must_use]
pub fn with_view(base: &ReportParams, p: ViewParams, as_of: Date) -> ReportParams {
    ReportParams {
        period: p.period,
        as_of,
        detail: engine_detail(p.detail),
        ..base.clone()
    }
}

/// The report a drill on `key` opens, or `None` when the key is not a report: a voucher is a
/// screen, and a stock item or a bill has no report yet. `base` supplies the period and the
/// scenario.
#[must_use]
pub fn drill_params(base: &ReportParams, key: &ViewKey) -> Option<ReportParams> {
    let kind = match key {
        ViewKey::Group(group) => ReportKind::GroupSummary(*group),
        ViewKey::Ledger(ledger) => ReportKind::LedgerVouchers(*ledger),
        ViewKey::Voucher(_) | ViewKey::Item(_) | ViewKey::Bill { .. } => return None,
    };
    Some(ReportParams {
        kind,
        ..base.clone()
    })
}

/// Runs `params` on the worker's API and shows the result as the screen's model. Runs on the
/// API thread: the result is plain data that can cross back to the UI thread.
///
/// # Errors
/// [`ViewError::Source`] with the API's message when the engine refuses the report.
pub fn run_report(api: &Api, params: &ReportParams) -> Result<Outcome<ViewReport>, ViewError> {
    match api.report(params) {
        Err(e) => Err(source_error(&e)),
        Ok(Outcome::Unsupported(u)) => Ok(Outcome::Unsupported(u)),
        Ok(Outcome::Value(report)) => Ok(Outcome::Value(view_report(&report))),
    }
}

/// The message the screen shows for an API error.
#[must_use]
pub fn source_error(e: &ApiError) -> ViewError {
    ViewError::Source {
        message: e.to_string(),
    }
}

/// A report in the screen's model. The token is the title; the screen never reads it, it only
/// keys the table's id.
#[must_use]
pub fn view_report(report: &Report) -> ViewReport {
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
pub fn view_cell(cell: &Cell) -> ViewCell {
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

/// An auditor report as the audit screen's table. Each row carries its label first and then one
/// text per cell, under a `Particulars` column, so a certificate's `Result` row reads as
/// `Result`, then its verdict. The cell text is the same text the report screen shows.
#[must_use]
pub fn audit_table_of(report: &Report) -> AuditTable {
    let mut columns = vec!["Particulars".to_owned()];
    columns.extend(report.columns.iter().map(|c| c.label.clone()));
    let rows = report
        .rows
        .iter()
        .map(|row| AuditRow {
            depth: row.depth,
            cells: std::iter::once(row.label.clone())
                .chain(row.cells.iter().map(|c| cell_text(&view_cell(c))))
                .collect(),
        })
        .collect();
    AuditTable {
        title: report.title.clone(),
        columns,
        rows,
        notes: report.notes.clone(),
    }
}
