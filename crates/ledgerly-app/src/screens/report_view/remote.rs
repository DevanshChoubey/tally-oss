//! Worker-mode report requests: the conversion from `ledgerly-api` reports to the screen's model,
//! and the parameters a screen asks the API for (card W7).
//!
//! Nothing here adds, rounds or re-derives a figure: every amount is copied from the engine's
//! report, cell for cell. The matches are exhaustive over the report model, so a new cell or row
//! kind fails to compile until it is shown (rule A2).

use ledgerly_api::{
    Api, Cell, Detail, DrillKey, Report, ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_core::{Date, Outcome, StockItemId};

use super::{
    Notice, ReportToken, ViewCell, ViewDetail, ViewError, ViewKey, ViewParams, ViewReport, ViewRow,
    ViewRowKind, mock,
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
/// The notice to show instead when `key` does not open a report: a voucher is a screen (the
/// caller opens it, see `ReportViewScreen::drill`), and a stock item or a bill has no report yet.
pub(super) fn drill_params(base: &ReportParams, key: &ViewKey) -> Result<ReportParams, Notice> {
    let kind = match key {
        ViewKey::Group(group) => ReportKind::GroupLedgers(*group),
        ViewKey::Ledger(ledger) => ReportKind::LedgerVouchers(*ledger),
        ViewKey::Voucher(_) => {
            return Err(Notice::Info(
                "A voucher opens as its own screen, not as a report.".to_owned(),
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
    // The balance sheet's default view lists each group with its sub-groups (the mock's
    // `balanceSheet()` view), which is neither of the engine's levels: the engine's `Condensed` is
    // primary groups only. So the screen asks for `Detailed` and drops the ledger rows below.
    let groups_only = params.kind == ReportKind::BalanceSheet && params.detail == Detail::Condensed;
    let asked = if groups_only {
        ReportParams {
            detail: Detail::Detailed,
            ..params.clone()
        }
    } else {
        params.clone()
    };
    match api.report(&asked) {
        Err(e) => Err(ViewError::Source {
            message: e.to_string(),
        }),
        Ok(Outcome::Unsupported(u)) => Ok(Outcome::Unsupported(u)),
        Ok(Outcome::Value(report)) => {
            let mut view = view_report(&report);
            if groups_only {
                view.rows = groups_and_sub_groups(view.rows);
                view.params.detail = ViewDetail::Condensed;
            }
            Ok(Outcome::Value(view))
        }
    }
}

/// The lines the mock draws for the trial balance, the outstanding bills and the stock summary,
/// made from what the engine returned (see `mock.rs`); `None` for any other report, or when the
/// engine's detailed report cannot be had. The screen keeps the engine's own report as it is and
/// draws these lines.
pub(super) fn mock_lines(
    api: &Api,
    params: &ReportParams,
    view: &ViewReport,
) -> Option<ViewReport> {
    match &params.kind {
        ReportKind::TrialBalance | ReportKind::Outstanding { .. } => {
            // The mock lists ledgers and bills, which only the detailed report holds.
            let detailed = if view.params.detail == ViewDetail::Detailed {
                view.clone()
            } else {
                let asked = ReportParams {
                    detail: Detail::Detailed,
                    ..params.clone()
                };
                match api.report(&asked) {
                    Ok(Outcome::Value(report)) => view_report(&report),
                    Ok(Outcome::Unsupported(_)) | Err(_) => return None,
                }
            };
            Some(match &params.kind {
                ReportKind::Outstanding { receivable } => mock::outstanding(detailed, *receivable),
                ReportKind::TrialBalance
                | ReportKind::BalanceSheet
                | ReportKind::ProfitAndLoss
                | ReportKind::GroupSummary(_)
                | ReportKind::GroupLedgers(_)
                | ReportKind::LedgerVouchers(_)
                | ReportKind::DayBook
                | ReportKind::Register(_)
                | ReportKind::StockSummary
                | ReportKind::Audit(_) => mock::trial_balance(detailed),
            })
        }
        ReportKind::StockSummary => {
            let books = match api.books() {
                Ok(Outcome::Value(books)) => Some(books),
                Ok(Outcome::Unsupported(_)) | Err(_) => None,
            };
            let facts = |id: StockItemId| {
                let books = books.as_ref()?;
                let item = books.masters.stock_items.get(&id)?;
                let unit = books.masters.units.get(&item.unit)?;
                Some(mock::ItemFacts {
                    hsn: item
                        .hsn
                        .as_ref()
                        .map_or_else(|| "none".to_owned(), |h| h.as_str().to_owned()),
                    unit: unit.symbol.clone(),
                })
            };
            Some(mock::stock_summary(view.clone(), &facts))
        }
        ReportKind::ProfitAndLoss => Some(mock::profit_loss(view.clone())),
        ReportKind::BalanceSheet
        | ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Audit(_) => None,
    }
}

/// The rows of a detailed balance sheet that a group-level view keeps: every row down to the
/// first sub-group level, ledgers left out. The stock row stays (it is an item, not a ledger).
fn groups_and_sub_groups(rows: Vec<ViewRow>) -> Vec<ViewRow> {
    rows.into_iter()
        .filter(|r| r.depth <= 1 && r.kind != ViewRowKind::Ledger)
        .collect()
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
