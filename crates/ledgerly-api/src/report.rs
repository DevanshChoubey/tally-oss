//! Report dispatch (card W1): `ReportParams` to the report engines over the cached books.
//!
//! [`run`] names every `ReportKind` and every `AuditReportKind`, so a new kind fails to compile
//! until it is dispatched. The report engines do not value stock themselves; they take the
//! valuations as input. This module values each item with its own method (average cost, FIFO,
//! last purchase cost or standard cost) from `Books::stock_moves`, dated as each report needs.
//! When an item cannot be valued, the report is `Unsupported` and names the item.

use ledgerly_core::{Date, Outcome, Period, PeriodError, Unsupported};
use ledgerly_model::{Books, StockItem, StockMove, ValuationMethod};
use ledgerly_reports::audit::{audit_report, integrity_certificate};
use ledgerly_reports::balance_sheet::balance_sheet;
use ledgerly_reports::ledger::{group_summary, ledger_vouchers};
use ledgerly_reports::outstanding::outstanding;
use ledgerly_reports::profit_loss::profit_loss;
use ledgerly_reports::registers::{day_book, register};
use ledgerly_reports::stock_summary::stock_summary;
use ledgerly_reports::trial_balance::trial_balance;
use ledgerly_reports::{AuditReportKind, Report, ReportKind, ReportParams};
use ledgerly_stock::avg::average_cost;
use ledgerly_stock::fifo::fifo;
use ledgerly_stock::other_methods::{last_purchase_cost, standard_cost};
use ledgerly_stock::{StockError, StockValuation};
use ledgerly_store::SqliteStore;
use ledgerly_store::verify::verify_chain;

use crate::ApiError;
use crate::events::read_all;

/// Runs `p` over `books`. `store` is read only by the audit reports.
///
/// # Errors
/// [`ApiError::Report`] when a report engine rejects `p`; [`ApiError::Store`] when the audit log
/// cannot be read for an audit report.
pub(crate) fn run(
    store: &SqliteStore,
    books: &Books,
    p: &ReportParams,
) -> Result<Outcome<Report>, ApiError> {
    let out = match &p.kind {
        ReportKind::TrialBalance => trial_balance(books, p)?,
        ReportKind::BalanceSheet => balance_sheet(books, p, &valuations_at(books, p.as_of))?,
        ReportKind::ProfitAndLoss => {
            let opening = opening_valuations(books, p.period.from());
            let closing = valuations_at(books, p.period.to());
            profit_loss(books, p, &opening, &closing)?
        }
        ReportKind::GroupSummary(group) => group_summary(books, p, *group)?,
        ReportKind::LedgerVouchers(ledger) => ledger_vouchers(books, p, *ledger)?,
        ReportKind::DayBook => day_book(books, p)?,
        ReportKind::Register(kind) => register(books, p, *kind)?,
        ReportKind::Outstanding { receivable } => outstanding(books, p, *receivable)?,
        ReportKind::StockSummary => {
            stock_summary(books, p, &stock_summary_valuations(books, p.period))?
        }
        ReportKind::Audit(kind) => audit(store, books, kind, p)?,
    };
    Ok(out)
}

/// Audit reports (spec 6.6). The integrity certificate needs the chain verification; the others
/// need the audit records.
fn audit(
    store: &SqliteStore,
    books: &Books,
    kind: &AuditReportKind,
    p: &ReportParams,
) -> Result<Outcome<Report>, ApiError> {
    match kind {
        AuditReportKind::IntegrityCertificate => {
            let verify = verify_chain(store)?;
            Ok(Outcome::Value(integrity_certificate(&verify)?))
        }
        AuditReportKind::ObjectHistory
        | AuditReportKind::EditLog
        | AuditReportKind::DeletedAltered
        | AuditReportKind::BackDated
        | AuditReportKind::EditsAfterLock => {
            let records = read_all(store)?;
            Ok(audit_report(&records, books, kind, p)?)
        }
    }
}

/// Valuations of every stock item at the end of `as_of`'s day (the balance sheet's closing
/// stock). `Unsupported` names the first item that cannot be valued.
fn valuations_at(books: &Books, as_of: Date) -> Outcome<Vec<StockValuation>> {
    let period = match valuation_period(books.company.books_begin, as_of) {
        Ok(period) => period,
        Err(e) => {
            return Outcome::unsupported(format!("stock valuation at {as_of}"), e.to_string());
        }
    };
    let mut out = Vec::with_capacity(books.masters.stock_items.len());
    for item in books.masters.stock_items.values() {
        match value_item(item, &books.stock_moves, period) {
            Ok(valuation) => out.push(valuation),
            Err(e) => {
                return Outcome::Unsupported(Unsupported {
                    what: format!("stock item '{}'", item.name),
                    reason: e.to_string(),
                });
            }
        }
    }
    Outcome::Value(out)
}

/// Valuations at the day before `from` (the opening stock of a period starting on `from`).
fn opening_valuations(books: &Books, from: Date) -> Outcome<Vec<StockValuation>> {
    match from.previous_day() {
        Some(day) => valuations_at(books, day),
        None => Outcome::unsupported(
            "opening stock",
            "the period starts on the first date the calendar holds, so there is no day before it",
        ),
    }
}

/// The valuations the stock summary takes: the opening (day before the period) and the closing
/// (the period end) values together, told apart by their dates.
fn stock_summary_valuations(books: &Books, period: Period) -> Outcome<Vec<StockValuation>> {
    let closing = valuations_at(books, period.to());
    opening_valuations(books, period.from()).and_then(|mut opening| {
        closing.map(|mut closing| {
            opening.append(&mut closing);
            opening
        })
    })
}

/// The period a valuation is computed over. Movements start at the books' first day, so the
/// period starts there; a date before the books (the opening of a period that starts on the
/// first day) is a single-day period, which holds no movements.
fn valuation_period(books_begin: Date, as_of: Date) -> Result<Period, PeriodError> {
    if as_of >= books_begin {
        Period::new(books_begin, as_of)
    } else {
        Period::new(as_of, as_of)
    }
}

/// Values one item with its valuation method.
fn value_item(
    item: &StockItem,
    moves: &[StockMove],
    period: Period,
) -> Result<StockValuation, StockError> {
    match item.valuation {
        ValuationMethod::AverageCost => average_cost(item, moves, period),
        ValuationMethod::Fifo => fifo(item, moves, period),
        ValuationMethod::LastPurchaseCost => last_purchase_cost(item, moves, period),
        ValuationMethod::StandardCost => standard_cost(item, moves, period),
    }
}
