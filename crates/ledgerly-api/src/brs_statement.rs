//! The bank reconciliation statement with each entry the bank has not cleared, and its export.
//!
//! [`ledgerly_bank::brs`] works out the figures and keeps the entries as [`BrsItem`]s (voucher id,
//! date, instrument). The screen and the file need the words as well: the voucher number and the
//! other side of the entry. They are looked up here, in the same books the figures came from; no
//! amount is worked out in this module. The model is the one the accounting package's help page
//! gives for its reconciliation statement (cited in `ledgerly_bank::brs`).
//!
//! The export is the statement as the screen draws it, written by the report exporter
//! ([`ledgerly_reports::export`]) as CSV or XLSX, atomically. Amounts are plain signed rupee
//! amounts: the books balance is debit-positive and the statement balance is credit-positive, as
//! the engine gives them.

use std::path::Path;

use ledgerly_bank::brs::{Brs, BrsItem};
use ledgerly_core::{Date, LedgerId, Outcome, Paise, Period};
use ledgerly_model::{Books, Scenario};
use ledgerly_reports::{
    Cell, ColumnHeader, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams,
    ReportRow, RowKind,
};

use crate::error::ApiError;
use crate::facade::Api;
use crate::report::ExportFormat;

/// One entry the bank has not cleared, with its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrsLine {
    /// The voucher holding the entry.
    pub voucher: ledgerly_core::VoucherId,
    /// The voucher's number, when it has one.
    pub number: Option<String>,
    /// Voucher date.
    pub date: Date,
    /// The other side of the entry: the names of the voucher's other ledgers, or its narration
    /// when it has none.
    pub particulars: String,
    /// The cheque or reference number from the bank allocation, if any.
    pub instrument: Option<String>,
    /// The amount, never negative.
    pub amount: Paise,
}

/// The reconciliation statement and its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrsStatement {
    /// The engine's statement: balances, totals and the entries.
    pub brs: Brs,
    /// The uncleared deposits (debits to the bank ledger), by date.
    pub deposits: Vec<BrsLine>,
    /// The uncleared payments (credits to the bank ledger), by date.
    pub payments: Vec<BrsLine>,
}

/// The words of one entry.
fn line_of(books: &Books, bank: LedgerId, item: &BrsItem) -> BrsLine {
    let voucher = books.vouchers.get(&item.voucher);
    let number = voucher.and_then(|v| v.number.clone());
    let mut names: Vec<&str> = Vec::new();
    if let Some(v) = voucher {
        for line in v.ledger_lines.iter().filter(|l| l.ledger != bank) {
            if let Some(ledger) = books.masters.ledgers.get(&line.ledger) {
                if !names.contains(&ledger.name.as_str()) {
                    names.push(&ledger.name);
                }
            }
        }
    }
    let particulars = if names.is_empty() {
        voucher.map_or_else(String::new, |v| v.narration.clone())
    } else {
        names.join(", ")
    };
    BrsLine {
        voucher: item.voucher,
        number,
        date: item.date,
        particulars,
        instrument: item.instrument_number.clone(),
        amount: item.amount,
    }
}

fn text(s: &str) -> Cell {
    Cell::Text(s.to_owned())
}

fn row(label: &str, depth: u8, kind: RowKind, cells: Vec<Cell>) -> ReportRow {
    ReportRow {
        key: None,
        label: label.to_owned(),
        depth,
        kind,
        cells,
    }
}

/// The statement as a report: one amount column, the entries indented under their total.
fn report_of(ledger: LedgerId, as_of: Date, s: &BrsStatement) -> Result<Report, ApiError> {
    let period = Period::new(as_of, as_of).map_err(ledgerly_reports::ReportError::from)?;
    let mut rows = vec![row(
        "Balance as per company books",
        0,
        RowKind::Subtotal,
        vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Signed(s.brs.books_balance),
        ],
    )];
    let mut section = |label: &str, total: Paise, lines: &[BrsLine]| {
        rows.push(row(
            label,
            0,
            RowKind::Text,
            vec![Cell::Empty, Cell::Empty, Cell::Empty, Cell::Signed(total)],
        ));
        for l in lines {
            rows.push(row(
                &l.particulars,
                1,
                RowKind::Voucher,
                vec![
                    Cell::Date(l.date),
                    l.number.as_deref().map_or(Cell::Empty, text),
                    l.instrument.as_deref().map_or(Cell::Empty, text),
                    Cell::Signed(l.amount),
                ],
            ));
        }
    };
    section(
        "Amounts not reflected in bank: deposits",
        s.brs.uncleared_deposits_total,
        &s.deposits,
    );
    section(
        "Amounts not reflected in bank: payments",
        s.brs.uncleared_payments_total,
        &s.payments,
    );
    let amount_row = |label: &str, kind: RowKind, v: Paise| {
        row(
            label,
            0,
            kind,
            vec![Cell::Empty, Cell::Empty, Cell::Empty, Cell::Signed(v)],
        )
    };
    rows.push(amount_row(
        "Balance as per bank",
        RowKind::Subtotal,
        s.brs.balance_as_per_bank,
    ));
    // A figure the engine cannot give is said, never left blank or zero.
    let outcome_cell = |o: &Outcome<Paise>| match o {
        Outcome::Value(v) => Cell::Signed(*v),
        Outcome::Unsupported(u) => Cell::Unsupported(u.clone()),
    };
    rows.push(row(
        "Balance as per imported bank statement",
        0,
        RowKind::Text,
        vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            outcome_cell(&s.brs.statement_balance),
        ],
    ));
    rows.push(row(
        "Difference",
        0,
        RowKind::Difference,
        vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            outcome_cell(&s.brs.difference),
        ],
    ));
    let column = |label: &str| ColumnHeader {
        label: label.to_owned(),
        period: None,
    };
    Ok(Report {
        title: "Bank reconciliation statement".to_owned(),
        // The parameters describe what the report is about: the bank ledger on the as-of date.
        params: ReportParams {
            kind: ReportKind::LedgerVouchers(ledger),
            period,
            as_of,
            scenario: Scenario::Actual,
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        },
        columns: vec![
            column("Date"),
            column("Voucher"),
            column("Instrument"),
            column("Amount"),
        ],
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    })
}

impl Api {
    /// The bank reconciliation statement of `ledger` as of `as_of` with each entry the bank has
    /// not cleared (spec 6.5). The figures are [`Api::brs`]'s.
    ///
    /// # Errors
    /// The errors of [`Api::brs`].
    pub fn brs_statement(
        &self,
        ledger: LedgerId,
        as_of: Date,
    ) -> Result<Outcome<BrsStatement>, ApiError> {
        let brs = match self.brs(ledger, as_of)? {
            Outcome::Value(brs) => brs,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let deposits = brs
            .uncleared_deposits
            .iter()
            .map(|i| line_of(books, ledger, i))
            .collect();
        let payments = brs
            .uncleared_payments
            .iter()
            .map(|i| line_of(books, ledger, i))
            .collect();
        Ok(Outcome::Value(BrsStatement {
            brs,
            deposits,
            payments,
        }))
    }

    /// Writes the reconciliation statement of `ledger` as of `as_of` to `path` as CSV or XLSX,
    /// atomically. The file holds the figures and entries [`Api::brs_statement`] gives.
    ///
    /// # Errors
    /// The errors of [`Api::brs_statement`], and [`ApiError::Report`] when the file cannot be
    /// written.
    pub fn export_brs(
        &self,
        ledger: LedgerId,
        as_of: Date,
        format: ExportFormat,
        path: &Path,
    ) -> Result<Outcome<()>, ApiError> {
        let statement = match self.brs_statement(ledger, as_of)? {
            Outcome::Value(s) => s,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let report = report_of(ledger, as_of, &statement)?;
        match format {
            ExportFormat::Csv => ledgerly_reports::export::to_csv_file(&report, path)?,
            ExportFormat::Xlsx => ledgerly_reports::export::to_xlsx(&report, path)?,
        }
        Ok(Outcome::Value(()))
    }
}
