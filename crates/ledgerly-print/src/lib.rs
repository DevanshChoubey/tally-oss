//! Print templates and PDF rendering.

pub mod pdf;
pub mod template;

/// The built-in tax invoice template (A4): the header, bill-to details, item and tax tables,
/// totals, amount in words and the terms line. It names no `upi_qr` block, because the books do
/// not hold a UPI address to pay to (see `design/PARITY-GAPS.md`).
pub const TAX_INVOICE_A4: &str = include_str!("../templates/tax-invoice-a4.toml");
