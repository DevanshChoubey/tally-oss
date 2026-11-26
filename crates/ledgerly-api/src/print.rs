//! Printing a saved voucher or a report (card P1).
//!
//! The PDF bytes come from `ledgerly-print` only; no figure is drawn or computed here. A voucher
//! the print crate cannot print is `Outcome::Unsupported` with its reason, never a blank page.
//! Reports have no print layout yet (no crate renders them), so [`Api::print_report`] says so.

use ledgerly_core::format::format_inr;
use ledgerly_core::{DrCr, Outcome, Paise, VoucherId};
use ledgerly_model::{BaseVoucherKind, Books, Voucher, VoucherStatus};
use ledgerly_print::TAX_INVOICE_A4;
use ledgerly_print::pdf::{
    ColumnAlign, PrintedDocument, ReportPage, RowStyle, render_invoice_pages_with,
    render_report_pages_with,
};
use ledgerly_print::setup::PrintSetup;
use ledgerly_print::template::{PrintError, load_template};
use ledgerly_reports::print_text::{PrintAlign, PrintRowStyle, print_text};
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
        Ok(self.print_voucher_document(id)?.map(|doc| doc.pdf))
    }

    /// As [`Api::print_voucher`], with the pages the PDF was written from, so a preview shows
    /// the engine's own layout.
    ///
    /// # Errors
    /// As [`Api::print_voucher`].
    pub fn print_voucher_document(
        &self,
        id: VoucherId,
    ) -> Result<Outcome<PrintedDocument>, ApiError> {
        self.print_voucher_document_with(id, &PrintSetup::default())
    }

    /// As [`Api::print_voucher_document`], on the paper, copies, pages and first page number
    /// `setup` names.
    ///
    /// # Errors
    /// As [`Api::print_voucher`]; [`ApiError::Print`] with a setup error when the settings
    /// cannot be used for this document (for example a page range past its last page).
    pub fn print_voucher_document_with(
        &self,
        id: VoucherId,
        setup: &PrintSetup,
    ) -> Result<Outcome<PrintedDocument>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let voucher = books
            .vouchers
            .get(&id)
            .ok_or(ApiError::UnknownVoucher { id: id.0 })?;
        let voucher_type = books.masters.voucher_types.get(&voucher.type_id);
        let kind = match voucher_type {
            Some(t) => t.base,
            None => {
                return Ok(Outcome::unsupported(
                    format!("print voucher {}", id.0),
                    "the voucher's type is not in the books",
                ));
            }
        };
        let rendered = match voucher_layout(kind) {
            Layout::TaxDocument => {
                let Some(gst) = voucher.gst.as_ref() else {
                    return Ok(Outcome::unsupported(
                        format!("print voucher {}", id.0),
                        "only sales invoices, credit notes and debit notes print, and this voucher has no GST block",
                    ));
                };
                let template = load_template(TAX_INVOICE_A4)?;
                render_invoice_pages_with(&template, voucher, books, gst, None, setup)
            }
            Layout::Accounts(title) => match voucher_page(books, voucher, title) {
                Ok(page) => render_report_pages_with(&page, setup),
                Err(e) => Err(e),
            },
            Layout::None => {
                let name = voucher_type.map_or("", |t| t.name.as_str());
                return Ok(Outcome::unsupported(
                    format!("print voucher {}", id.0),
                    format!("a {name} voucher has no print layout"),
                ));
            }
        };
        match rendered {
            Ok(doc) => Ok(Outcome::Value(doc)),
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
        Ok(self.print_report_document(p)?.map(|doc| doc.pdf))
    }

    /// As [`Api::print_report`], with the pages the PDF was written from, so a preview shows
    /// the engine's own layout.
    ///
    /// # Errors
    /// As [`Api::print_report`].
    pub fn print_report_document(
        &self,
        p: &ReportParams,
    ) -> Result<Outcome<PrintedDocument>, ApiError> {
        self.print_report_document_with(p, &PrintSetup::default())
    }

    /// As [`Api::print_report_document`], on the paper, copies, pages and first page number
    /// `setup` names.
    ///
    /// # Errors
    /// As [`Api::print_report`]; [`ApiError::Print`] with a setup error when the settings
    /// cannot be used for this document.
    pub fn print_report_document_with(
        &self,
        p: &ReportParams,
        setup: &PrintSetup,
    ) -> Result<Outcome<PrintedDocument>, ApiError> {
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
        let text = print_text(&report)?;
        let page = ReportPage {
            company,
            title: report.title.clone(),
            period: format!("{} to {}", dmy(p.period.from()), dmy(p.period.to())),
            headings: text.headings,
            rows: text.rows.iter().map(|r| r.cells.clone()).collect(),
            notes: text.notes,
            aligns: text
                .aligns
                .iter()
                .map(|a| match a {
                    PrintAlign::Left => ColumnAlign::Left,
                    PrintAlign::Right => ColumnAlign::Right,
                })
                .collect(),
            styles: text
                .rows
                .iter()
                .map(|r| match r.style {
                    PrintRowStyle::Plain => RowStyle::Plain,
                    PrintRowStyle::Band => RowStyle::Band,
                    PrintRowStyle::Total => RowStyle::Total,
                })
                .collect(),
        };
        match render_report_pages_with(&page, setup) {
            Ok(doc) => Ok(Outcome::Value(doc)),
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

/// How a voucher of a kind prints.
enum Layout {
    /// A tax invoice, credit note or debit note, on the tax invoice template.
    TaxDocument,
    /// Particulars with debit and credit, under this title.
    Accounts(&'static str),
    /// No print layout.
    None,
}

/// The layout of each voucher kind. Every kind is named: a new kind must be decided here.
fn voucher_layout(kind: BaseVoucherKind) -> Layout {
    match kind {
        BaseVoucherKind::Sales | BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => {
            Layout::TaxDocument
        }
        BaseVoucherKind::Payment => Layout::Accounts("Payment Voucher"),
        BaseVoucherKind::Receipt => Layout::Accounts("Receipt Voucher"),
        BaseVoucherKind::Journal => Layout::Accounts("Journal Voucher"),
        BaseVoucherKind::Contra => Layout::Accounts("Contra Voucher"),
        BaseVoucherKind::Purchase => Layout::Accounts("Purchase Voucher"),
        BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => Layout::None,
    }
}

fn render_error(reason: impl Into<String>) -> PrintError {
    PrintError::Render {
        reason: reason.into(),
    }
}

/// The rows of a voucher page, with how each is drawn.
#[derive(Default)]
struct VoucherRows {
    rows: Vec<Vec<String>>,
    styles: Vec<RowStyle>,
}

impl VoucherRows {
    fn push(&mut self, label: String, debit: String, credit: String, style: RowStyle) {
        self.rows.push(vec![label, debit, credit]);
        self.styles.push(style);
    }

    fn note(&mut self, label: String) {
        self.push(label, String::new(), String::new(), RowStyle::Plain);
    }
}

/// The page of a payment, receipt, journal, contra or purchase voucher: the company, the voucher
/// title, its number and date, then each ledger line under Debit or Credit as stored, the items
/// of a purchase, the totals, and the reference and narration. Nothing is computed beyond the sum
/// of the stored lines, which must agree (a posted voucher has equal debits and credits).
fn voucher_page(books: &Books, voucher: &Voucher, title: &str) -> Result<ReportPage, PrintError> {
    match voucher.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => {}
        VoucherStatus::Optional => {
            return Err(PrintError::Unsupported {
                reason: "an optional voucher is not in the books and does not print".to_owned(),
            });
        }
        VoucherStatus::Cancelled => {
            return Err(PrintError::Unsupported {
                reason: "a cancelled voucher has no amounts to print".to_owned(),
            });
        }
    }
    let mut out = VoucherRows::default();
    let (debit, credit) = ledger_rows(books, voucher, &mut out)?;
    item_rows(books, voucher, &mut out)?;
    out.push(
        "Total".to_owned(),
        format_inr(debit),
        format_inr(credit),
        RowStyle::Total,
    );
    if let Some(reference) = &voucher.reference {
        let on = voucher
            .reference_date
            .map_or_else(String::new, |d| format!(" dated {}", dmy(d)));
        out.note(format!("Reference: {reference}{on}"));
    }
    if !voucher.narration.is_empty() {
        out.note(format!("Narration: {}", voucher.narration));
    }
    let number = voucher
        .number
        .as_deref()
        .map_or("not allocated".to_owned(), str::to_owned);
    Ok(ReportPage {
        company: books.company.name.clone(),
        title: title.to_owned(),
        period: format!("No. {number}, dated {}", dmy(voucher.date)),
        headings: vec![
            "Particulars".to_owned(),
            "Debit".to_owned(),
            "Credit".to_owned(),
        ],
        rows: out.rows,
        notes: Vec::new(),
        aligns: vec![ColumnAlign::Left, ColumnAlign::Right, ColumnAlign::Right],
        styles: out.styles,
    })
}

/// The ledger lines as rows, and the debit and credit sums (which must agree).
fn ledger_rows(
    books: &Books,
    voucher: &Voucher,
    out: &mut VoucherRows,
) -> Result<(Paise, Paise), PrintError> {
    let (mut debit, mut credit) = (Paise::ZERO, Paise::ZERO);
    for line in &voucher.ledger_lines {
        let ledger =
            books.masters.ledgers.get(&line.ledger).ok_or_else(|| {
                render_error(format!("ledger {} is not in the books", line.ledger.0))
            })?;
        let amount = format_inr(line.amount.amount);
        match line.amount.side {
            DrCr::Dr => {
                debit = debit.checked_add(line.amount.amount).map_err(overflow)?;
                out.push(ledger.name.clone(), amount, String::new(), RowStyle::Plain);
            }
            DrCr::Cr => {
                credit = credit.checked_add(line.amount.amount).map_err(overflow)?;
                out.push(ledger.name.clone(), String::new(), amount, RowStyle::Plain);
            }
        }
    }
    if debit != credit {
        return Err(render_error(
            "the voucher's debits and credits differ, so it is not printed",
        ));
    }
    Ok((debit, credit))
}

/// The stock lines of the voucher, under an "Items" heading, as stored.
fn item_rows(books: &Books, voucher: &Voucher, out: &mut VoucherRows) -> Result<(), PrintError> {
    if voucher.inventory_lines.is_empty() {
        return Ok(());
    }
    out.push(
        "Items".to_owned(),
        String::new(),
        String::new(),
        RowStyle::Band,
    );
    for line in &voucher.inventory_lines {
        let item = books.masters.stock_items.get(&line.item).ok_or_else(|| {
            render_error(format!("stock item {} is not in the books", line.item.0))
        })?;
        let unit = books
            .masters
            .units
            .get(&line.unit)
            .ok_or_else(|| render_error(format!("unit {} is not in the books", line.unit.0)))?;
        out.note(format!(
            "{}: {} {} at {} = {}",
            item.name,
            line.qty.0.normalize(),
            unit.symbol,
            line.rate.0.normalize(),
            format_inr(line.amount)
        ));
    }
    Ok(())
}

fn overflow(_: ledgerly_core::MoneyError) -> PrintError {
    render_error("an amount on the voucher is too large to add")
}
