//! The original invoice of a credit or debit note: its date and the quantity of each item.
//!
//! A credit note names the tax invoice it corrects (CGST Act s.34, Rule 53(1A)); a debit note of a
//! purchase return goes against the supplier's bill. The note screen shows the invoice's date and
//! quantities beside the lines; they come from here, never from the screen.

use ledgerly_core::{Date, LedgerId, MoneyError, Outcome, Qty, StockItemId, UnitId};
use ledgerly_model::{BaseVoucherKind, Books, VoucherStatus};

use ledgerly_gst::GstError;

use crate::error::ApiError;
use crate::facade::Api;

/// One item of the original invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicedLine {
    /// The stock item.
    pub item: StockItemId,
    /// Its name.
    pub name: String,
    /// The symbol of the unit the line was in.
    pub unit_symbol: String,
    /// The quantity invoiced, summed over the lines of the item.
    pub qty: Qty,
}

/// The original invoice of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicedAgainst {
    /// The invoice number (a sales voucher's number, or the supplier's bill number).
    pub number: String,
    /// The invoice date (a purchase's bill date when it has one, else the voucher date).
    pub date: Date,
    /// The items, in the order of the invoice's lines.
    pub lines: Vec<InvoicedLine>,
}

/// The kind of voucher a note of `kind` goes against, or `None` for a kind that is not a note.
fn original_kind(kind: BaseVoucherKind) -> Option<BaseVoucherKind> {
    match kind {
        BaseVoucherKind::CreditNote => Some(BaseVoucherKind::Sales),
        BaseVoucherKind::DebitNote => Some(BaseVoucherKind::Purchase),
        BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => None,
    }
}

impl Api {
    /// The invoice a note of `kind` (credit or debit) against `party` refers to by `reference`:
    /// for a credit note a sales voucher of that party with that number; for a debit note a
    /// purchase of that party whose supplier bill number is `reference`. Cancelled vouchers are
    /// not found. The date is the bill date of a purchase when it has one.
    ///
    /// `Unsupported` for a kind that is not a note, a reference that matches nothing, or an
    /// invoice without stock lines (there is no quantity to show).
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed; [`ApiError::Gst`] when
    /// quantities overflow.
    pub fn invoiced_against(
        &self,
        kind: BaseVoucherKind,
        party: LedgerId,
        reference: &str,
    ) -> Result<Outcome<InvoicedAgainst>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let Some(original) = original_kind(kind) else {
            return Ok(Outcome::unsupported(
                "original invoice",
                "only a credit note or a debit note has an original invoice",
            ));
        };
        let wanted = reference.trim();
        let found = books.vouchers.values().find(|v| {
            let is_original = books
                .masters
                .voucher_types
                .get(&v.type_id)
                .is_some_and(|t| t.base == original);
            let named = match original {
                BaseVoucherKind::Purchase => v.reference.as_deref(),
                _ => v.number.as_deref(),
            };
            is_original
                && v.party == Some(party)
                && v.status != VoucherStatus::Cancelled
                && !wanted.is_empty()
                && named.is_some_and(|n| n.trim().eq_ignore_ascii_case(wanted))
        });
        let Some(invoice) = found else {
            return Ok(Outcome::unsupported(
                "original invoice",
                format!("no invoice of this party is numbered {wanted}"),
            ));
        };
        let mut lines: Vec<InvoicedLine> = Vec::new();
        for l in &invoice.inventory_lines {
            if let Some(existing) = lines
                .iter_mut()
                .find(|x| x.item == l.item && x.unit_symbol == unit_symbol(books, l.unit))
            {
                existing.qty = Qty(existing
                    .qty
                    .0
                    .checked_add(l.qty.0)
                    .ok_or_else(|| ApiError::Gst(GstError::from(MoneyError::Overflow)))?);
            } else {
                lines.push(InvoicedLine {
                    item: l.item,
                    name: books
                        .masters
                        .stock_items
                        .get(&l.item)
                        .map(|i| i.name.clone())
                        .unwrap_or_default(),
                    unit_symbol: unit_symbol(books, l.unit),
                    qty: l.qty,
                });
            }
        }
        if lines.is_empty() {
            return Ok(Outcome::unsupported(
                "original invoice quantities",
                format!("{wanted} has no stock lines, so there is no quantity to show"),
            ));
        }
        Ok(Outcome::Value(InvoicedAgainst {
            number: wanted.to_owned(),
            date: invoice.reference_date.unwrap_or(invoice.date),
            lines,
        }))
    }
}

fn unit_symbol(books: &Books, unit: UnitId) -> String {
    books
        .masters
        .units
        .get(&unit)
        .map(|u| u.symbol.clone())
        .unwrap_or_default()
}
