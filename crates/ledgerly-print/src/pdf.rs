//! PDF rendering. **Card PR2** implements this module.
//!
//! Frozen interface (plan card PR2):
//!
//! ```text
//! pub fn render_invoice(t: &Template, v: &Voucher, books: &Books, gst: &StoredGst) -> Result<Vec<u8>, PrintError>;
//! ```
//!
//! The renderer prints a sales invoice, credit note or debit note on the page size and in the
//! block order of a [`Template`]. It adds no figure of its own: every number comes from the
//! voucher's stored GST and lines, and the totals are checked against the amount posted to the
//! party ledger before they are shown (rule A1). What it cannot print it refuses with
//! [`PrintError::Unsupported`], never a blank or a guess:
//!
//! * characters outside plain ASCII (the built-in Helvetica font covers nothing else);
//! * a voucher that is not a sales invoice, credit note or debit note, or is optional or
//!   cancelled;
//! * a company with several GSTINs or a composition registration;
//! * item or tax tables that cannot be tied to the stored GST (stored GST lines carry no HSN, and
//!   item lines carry no tax rate, so an HSN or rate column is printed only when one code or one
//!   rate covers the whole invoice);
//! * a `upi_qr` block when no payee is given (use [`render_invoice_with_upi`]).
//!
//! The PDF is written uncompressed so that the text stays extractable and the output is
//! byte-for-byte the same for the same input.

mod data;
mod page;

use ledgerly_bank::upi::upi_payload;
use ledgerly_core::Paise;
use ledgerly_model::{Books, StoredGst, Voucher};
use qrcode::{Color, QrCode};

use crate::template::{Block, BlockBody, ItemColumn, PrintError, Slot, TaxColumn, Template};
use data::{Invoice, unsupported};
use page::{Align, BODY, Canvas, Column};

/// Size of one QR module in points.
const QR_MODULE: i32 = 3;

/// Where a `upi_qr` block sends the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpiPayee<'a> {
    /// The payee's UPI address, such as `shop@bank`.
    pub vpa: &'a str,
    /// The payee's name.
    pub name: &'a str,
}

/// Renders the voucher on the template and returns the PDF bytes.
///
/// A template with a `upi_qr` block is `Unsupported` here, because the books hold no UPI address;
/// [`render_invoice_with_upi`] takes one.
///
/// # Errors
///
/// [`PrintError::Unsupported`] for input the renderer does not handle (see the module docs) and
/// [`PrintError::Render`] when the voucher or the books lack something the template prints, or
/// when the invoice totals disagree with what is posted.
pub fn render_invoice(
    t: &Template,
    v: &Voucher,
    books: &Books,
    gst: &StoredGst,
) -> Result<Vec<u8>, PrintError> {
    render_invoice_with_upi(t, v, books, gst, None)
}

/// Like [`render_invoice`], with the payee a `upi_qr` block pays to.
///
/// # Errors
///
/// As [`render_invoice`]; a `upi_qr` block with `upi` of `None` is `Unsupported`.
pub fn render_invoice_with_upi(
    t: &Template,
    v: &Voucher,
    books: &Books,
    gst: &StoredGst,
    upi: Option<&UpiPayee<'_>>,
) -> Result<Vec<u8>, PrintError> {
    let invoice = Invoice::new(books, v, gst)?;
    let (width_mm, height_mm) = t.page.millimetres();
    let mut canvas = Canvas::new(mm_to_points(width_mm), mm_to_points(height_mm));
    for block in &t.blocks {
        render_block(&mut canvas, &invoice, block, upi)?;
        canvas.skip(8);
    }
    canvas.finish()
}

/// Millimetres to whole points (72 per 25.4 mm), rounded to the nearest point.
fn mm_to_points(mm: u16) -> i32 {
    (i32::from(mm) * 720 + 127) / 254
}

fn render_block(
    canvas: &mut Canvas,
    invoice: &Invoice<'_>,
    block: &Block,
    upi: Option<&UpiPayee<'_>>,
) -> Result<(), PrintError> {
    if let Some(title) = &block.title {
        canvas.paragraph(title, BODY, true)?;
    }
    match &block.body {
        BlockBody::Header { slots } | BlockBody::Details { slots } => {
            for slot in slots {
                render_slot(canvas, invoice, *slot)?;
            }
            Ok(())
        }
        BlockBody::Totals { slots } => {
            for slot in slots {
                let (label, value) = invoice.slot(*slot)?;
                let bold = matches!(slot, Slot::GrandTotal);
                canvas.label_value(label, &value, BODY, bold)?;
            }
            Ok(())
        }
        BlockBody::ItemTable { columns } => {
            let rows = invoice.item_rows(columns)?;
            canvas.table(&item_columns(columns), &rows)
        }
        BlockBody::TaxTable { columns } => {
            let rows = invoice.tax_table_rows(columns)?;
            canvas.table(&tax_columns(columns), &rows)
        }
        BlockBody::AmountInWords => canvas.paragraph(&invoice.words()?, BODY, false),
        BlockBody::UpiQr => render_upi(canvas, invoice, upi),
        BlockBody::Text { text } => canvas.paragraph(text, BODY, false),
    }
}

/// Prints one slot of a header or details block: the company name and the document title large,
/// everything else as `label: value`.
fn render_slot(canvas: &mut Canvas, invoice: &Invoice<'_>, slot: Slot) -> Result<(), PrintError> {
    let (label, value) = invoice.slot(slot)?;
    let text = if label.is_empty() {
        value
    } else {
        format!("{label}: {value}")
    };
    match slot {
        Slot::CompanyName => canvas.paragraph(&text, 14, true),
        Slot::DocumentTitle => canvas.paragraph(&text, 16, true),
        Slot::CompanyAddress
        | Slot::CompanyGstin
        | Slot::VoucherNumber
        | Slot::VoucherDate
        | Slot::PartyName
        | Slot::PartyAddress
        | Slot::PartyGstin
        | Slot::PlaceOfSupply
        | Slot::TaxableTotal
        | Slot::TaxTotal
        | Slot::RoundOff
        | Slot::GrandTotal => canvas.paragraph(&text, BODY, false),
    }
}

fn render_upi(
    canvas: &mut Canvas,
    invoice: &Invoice<'_>,
    upi: Option<&UpiPayee<'_>>,
) -> Result<(), PrintError> {
    let payee = upi.ok_or_else(|| {
        unsupported("a upi_qr block needs the payee's UPI address, which the books do not hold")
    })?;
    let grand: Paise = invoice.grand_total()?;
    let payload = upi_payload(payee.vpa, payee.name, grand, invoice.number().unwrap_or(""))
        .map_err(|e| PrintError::Render {
            reason: e.to_string(),
        })?;
    let code = QrCode::new(payload.as_bytes()).map_err(|e| PrintError::Render {
        reason: format!("could not encode the UPI QR code: {e}"),
    })?;
    let side = code.width();
    let dark: Vec<bool> = code
        .into_colors()
        .into_iter()
        .map(|c| matches!(c, Color::Dark))
        .collect();
    canvas.paragraph(&format!("Pay to: {}", payee.vpa), BODY, false)?;
    canvas.modules(&dark, side, QR_MODULE)
}

fn item_columns(cols: &[ItemColumn]) -> Vec<Column> {
    cols.iter()
        .map(|c| match c {
            ItemColumn::Serial => column("#", 4, Align::Left),
            ItemColumn::Description => column("Description", 24, Align::Left),
            ItemColumn::Hsn => column("HSN/SAC", 9, Align::Left),
            ItemColumn::Quantity => column("Qty", 7, Align::Right),
            ItemColumn::Unit => column("Unit", 5, Align::Left),
            ItemColumn::Rate => column("Rate", 10, Align::Right),
            ItemColumn::Discount => column("Disc", 6, Align::Right),
            ItemColumn::TaxableValue => column("Taxable value", 12, Align::Right),
            ItemColumn::TaxRate => column("GST", 6, Align::Right),
            ItemColumn::Amount => column("Amount", 12, Align::Right),
        })
        .collect()
}

fn tax_columns(cols: &[TaxColumn]) -> Vec<Column> {
    cols.iter()
        .map(|c| match c {
            TaxColumn::Hsn => column("HSN/SAC", 9, Align::Left),
            TaxColumn::TaxableValue => column("Taxable value", 12, Align::Right),
            TaxColumn::CgstRate => column("CGST rate", 7, Align::Right),
            TaxColumn::CgstAmount => column("CGST", 10, Align::Right),
            TaxColumn::SgstRate => column("SGST rate", 7, Align::Right),
            TaxColumn::SgstAmount => column("SGST", 10, Align::Right),
            TaxColumn::IgstRate => column("IGST rate", 7, Align::Right),
            TaxColumn::IgstAmount => column("IGST", 10, Align::Right),
            TaxColumn::CessAmount => column("Cess", 8, Align::Right),
            TaxColumn::TotalTax => column("Total tax", 11, Align::Right),
        })
        .collect()
}

fn column(title: &'static str, weight: i32, align: Align) -> Column {
    Column {
        title,
        weight,
        align,
    }
}
