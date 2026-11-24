//! A ledger's balance now and after a draft voucher is saved (the "balance now ... after saving"
//! lines of the invoice, payment and receipt screens).
//!
//! Both figures are the trial balance's: the first over the books as they are, the second over a
//! copy of the books with the draft added. A screen never adds a voucher's amount to a balance
//! itself (rule C4). A ledger the trial balance has no row for (a nil balance is left out of it)
//! gives `Unsupported` rather than an assumed zero.

use ledgerly_core::{Date, LedgerId, Outcome, Period, SignedAmount, Unsupported, VoucherId};
use ledgerly_model::{Books, Posting, Scenario, Voucher};
use ledgerly_reports::trial_balance::trial_balance;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, RowKind,
};

use crate::error::ApiError;
use crate::facade::Api;

/// A ledger's balance before and after a draft voucher, as the trial balance gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceChange {
    /// The balance on the voucher's date, over the books as they are.
    pub now: SignedAmount,
    /// The balance on the voucher's date once the voucher is saved.
    pub after: SignedAmount,
}

impl Api {
    /// `ledger`'s balance on `voucher`'s date now, and after `voucher` is saved.
    ///
    /// The draft must carry its ledger lines (an invoice gets them from [`Api::invoice_gst`]).
    ///
    /// # Errors
    /// [`ApiError::Report`] when the trial balance rejects the date; [`ApiError::Stale`] after a
    /// saved change whose reload failed.
    pub fn balance_change(
        &self,
        ledger: LedgerId,
        voucher: &Voucher,
    ) -> Result<Outcome<BalanceChange>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let on = voucher.date;
        let now = match ledger_balance(books, ledger, on)? {
            Outcome::Value(amount) => amount,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let mut with = books.clone();
        let next = with
            .vouchers
            .keys()
            .map(|id| id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut draft = voucher.clone();
        draft.id = VoucherId(next);
        for line in &draft.ledger_lines {
            with.postings.push(Posting {
                voucher: draft.id,
                date: draft.date,
                ledger: line.ledger,
                amount: line.amount,
                status: draft.status,
                type_id: draft.type_id,
            });
        }
        with.postings.sort_by_key(|p| (p.date, p.voucher));
        with.vouchers.insert(draft.id, draft);
        let after = match ledger_balance(&with, ledger, on)? {
            Outcome::Value(amount) => amount,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(Outcome::Value(BalanceChange { now, after }))
    }
}

/// The trial balance's closing balance of `ledger` on `on`.
fn ledger_balance(
    books: &Books,
    ledger: LedgerId,
    on: Date,
) -> Result<Outcome<SignedAmount>, ApiError> {
    let period = Period::new(on, on).map_err(|e| ReportError::Invalid {
        field: "date".to_owned(),
        reason: e.to_string(),
    })?;
    let params = ReportParams {
        kind: ReportKind::TrialBalance,
        period,
        as_of: on,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    let report = match trial_balance(books, &params)? {
        Outcome::Value(report) => report,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    Ok(row_amount(&report, ledger))
}

/// The one amount on `ledger`'s row of the report.
fn row_amount(report: &Report, ledger: LedgerId) -> Outcome<SignedAmount> {
    let missing = |why: &str| {
        Outcome::Unsupported(Unsupported {
            what: "ledger balance".to_owned(),
            reason: format!(
                "the trial balance {why} for ledger {}, so no balance is shown",
                ledger.0
            ),
        })
    };
    let Some(row) = report
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Ledger && r.key == Some(DrillKey::Ledger(ledger)))
    else {
        return missing("has no row");
    };
    let mut found: Option<SignedAmount> = None;
    for cell in &row.cells {
        match cell {
            Cell::Empty => {}
            Cell::Amount(amount) => {
                if found.is_some() {
                    return missing("has amounts on both sides");
                }
                found = Some(*amount);
            }
            Cell::Unsupported(_)
            | Cell::Provisional { .. }
            | Cell::Signed(_)
            | Cell::Qty(_)
            | Cell::Text(_)
            | Cell::Date(_) => return missing("has no plain amount"),
        }
    }
    found.map_or_else(|| missing("has an empty row"), Outcome::Value)
}
