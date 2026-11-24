//! Printing a saved voucher or a report (card P1).
//!
//! The PDF bytes come from `ledgerly-print` only; no figure is drawn or computed here. A voucher
//! the print crate cannot print is `Outcome::Unsupported` with its reason, never a blank page.
//! Reports have no print layout yet (no crate renders them), so [`Api::print_report`] says so.

use ledgerly_core::{Outcome, VoucherId};
use ledgerly_print::TAX_INVOICE_A4;
use ledgerly_print::pdf::render_invoice;
use ledgerly_print::template::{PrintError, load_template};
use ledgerly_reports::ReportParams;

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

    /// Asks for a PDF of a report. No report layout exists yet, so this is always
    /// `Unsupported`, with the export formats the user can use instead.
    ///
    /// # Errors
    /// Never in this version; the signature leaves room for a report layout.
    pub fn print_report(&self, _p: &ReportParams) -> Result<Outcome<Vec<u8>>, ApiError> {
        Ok(Outcome::unsupported(
            "print report",
            "reports have no print layout yet; export the report to CSV or XLSX (Ctrl+E) instead",
        ))
    }
}
