//! Printing a saved voucher or a report (card P1).
//!
//! The PDF bytes come from `ledgerly-print` only; no figure is drawn or computed here. A voucher
//! the print crate cannot print is `Outcome::Unsupported` with its reason, never a blank page.
//! Reports have no print layout yet (no crate renders them), so [`Api::print_report`] says so.

use ledgerly_core::{Outcome, VoucherId};
use ledgerly_print::TAX_INVOICE_A4;
use ledgerly_print::pdf::{ReportPage, render_invoice, render_report};
use ledgerly_print::template::{PrintError, load_template};
use ledgerly_reports::export::report_text;
use ledgerly_reports::{ReportKind, ReportParams};

use crate::error::ApiError;
use crate::facade::Api;

impl Api {
    /// Renders saved voucher `id` as a PDF on the built-in A4 tax invoice template.
    ///
    /// Only sales invoices, credit notes and debit notes print. Any other voucher, and any voucher
    /// without a GST block, is `Unsupported` with the reason. Nothing is written to disk here.
    ///
    /// # Errors
    /// [`ApiError::UnknownVoucher`] when the voucher is not in the books, [`ApiError::Print`]
    /// when the built-in template fails to load or the renderer fails on a voucher it should
    /// print, and [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn print_voucher(&self, id: VoucherId) -> Result<Outcome<Vec<u8>>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let voucher = books
            .vouchers
            .get(&id)
            .ok_or(ApiError::UnknownVoucher { id: id.0 })?;
        let Some(gst) = voucher.gst.as_ref() else {
            return Ok(Outcome::unsupported(
                format!("print voucher {}", id.0),
                "only sales invoices, credit notes and debit notes print, and this voucher has no GST block",
            ));
        };
        let template = load_template(TAX_INVOICE_A4)?;
        match render_invoice(&template, voucher, books, gst) {
            Ok(bytes) => Ok(Outcome::Value(bytes)),
            Err(PrintError::Unsupported { reason }) => Ok(Outcome::unsupported(
                format!("print voucher {}", id.0),
                reason,
            )),
            Err(other) => Err(ApiError::Print(other)),
        }
    }

    /// Renders report `p` as a PDF: the company, the title and period, and the engine's table,
    /// with the heading repeated on each page and page numbers at the foot.
    ///
    /// The reports with a print layout are the trial balance, balance sheet, profit and loss,
    /// day book and ledger vouchers. Any other report is `Unsupported` with the reason and the
    /// export the user can use instead.
    ///
    /// # Errors
    /// The errors of [`Api::report`], [`ApiError::Report`] when the report cannot be turned into
    /// print text, [`ApiError::Print`] for a renderer failure, and [`ApiError::Stale`].
    pub fn print_report(&self, p: &ReportParams) -> Result<Outcome<Vec<u8>>, ApiError> {
        let printable = match &p.kind {
            ReportKind::TrialBalance
            | ReportKind::BalanceSheet
            | ReportKind::ProfitAndLoss
            | ReportKind::DayBook
            | ReportKind::LedgerVouchers(_) => true,
            ReportKind::GroupSummary(_)
            | ReportKind::GroupLedgers(_)
            | ReportKind::Register(_)
            | ReportKind::Outstanding { .. }
            | ReportKind::StockSummary
            | ReportKind::Audit(_) => false,
        };
        if !printable {
            return Ok(Outcome::unsupported(
                "print report",
                "reports have no print layout yet; export the report to CSV or XLSX (Ctrl+E) instead",
            ));
        }
        let report = match self.report(p)? {
            Outcome::Value(report) => report,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let company = match self.cached_books()? {
            Outcome::Value(books) => books.company.name.clone(),
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let text = report_text(&report)?;
        let page = ReportPage {
            company,
            title: report.title.clone(),
            period: format!("{} to {}", dmy(p.period.from()), dmy(p.period.to())),
            headings: text.headings,
            rows: text.rows,
            notes: text.notes,
        };
        match render_report(&page) {
            Ok(bytes) => Ok(Outcome::Value(bytes)),
            Err(PrintError::Unsupported { reason }) => Ok(Outcome::unsupported(
                format!("print {}", report.title),
                reason,
            )),
            Err(other) => Err(ApiError::Print(other)),
        }
    }
}

/// A date as day-month-year, the form the screens print.
fn dmy(d: ledgerly_core::Date) -> String {
    format!("{:02}-{:02}-{}", d.day(), u8::from(d.month()), d.year())
}
