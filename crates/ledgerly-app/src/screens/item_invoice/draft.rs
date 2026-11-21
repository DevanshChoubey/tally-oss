//! The invoice as typed: stock rows, the draft, and the voucher built from it.

use super::{InvoiceError, InvoiceKind, parse_decimal, parse_place};
use crate::screens::voucher_entry::{UNASSIGNED_VOUCHER_ID, eval_amount, parse_entry_date};
use ledgerly_core::{
    Decimal, FinancialYear, LedgerId, MoneyError, Paise, Qty, Rate, RoundingRule, StockItemId,
    Unsupported, VoucherTypeId, round_rupees_to_paise,
};
use ledgerly_model::{InventoryLine, Masters, Voucher, VoucherMode, VoucherStatus};
use std::collections::BTreeMap;

/// One stock line as typed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvoiceRow {
    /// The item, once picked.
    pub item: Option<StockItemId>,
    /// The text in the item field (the query, or the picked item's name).
    pub item_text: String,
    /// Quantity in the item's unit.
    pub qty_text: String,
    /// Rate per unit, in rupees.
    pub rate_text: String,
    /// Discount in percent; empty for none.
    pub discount_text: String,
    /// The line amount typed by the user; empty to use quantity x rate less discount.
    pub amount_text: String,
}

impl InvoiceRow {
    pub(super) fn is_blank(&self) -> bool {
        self.item.is_none()
            && self.item_text.trim().is_empty()
            && self.qty_text.trim().is_empty()
            && self.rate_text.trim().is_empty()
            && self.discount_text.trim().is_empty()
            && self.amount_text.trim().is_empty()
    }
}

/// The amount of one line: quantity x rate less discount, or the amount the user typed.
pub(super) fn line_amount(
    masters: &Masters,
    row: &InvoiceRow,
    line: usize,
) -> Result<Paise, InvoiceError> {
    let bad = |reason: String| InvoiceError::Line { line, reason };
    let item_id = row.item.ok_or_else(|| bad("choose an item".to_owned()))?;
    let item = masters
        .stock_items
        .get(&item_id)
        .ok_or_else(|| bad("the item is not in the masters".to_owned()))?;
    let unit = masters
        .units
        .get(&item.unit)
        .ok_or_else(|| bad(format!("unit of '{}' is not in the masters", item.name)))?;
    let qty = parse_decimal(&row.qty_text).map_err(|e| bad(format!("quantity: {e}")))?;
    if qty.is_zero() {
        return Err(bad("quantity must be more than zero".to_owned()));
    }
    if qty.normalize().scale() > u32::from(unit.decimals) {
        return Err(bad(format!(
            "unit '{}' allows {} decimal places",
            unit.symbol, unit.decimals
        )));
    }
    let rate = parse_decimal(&row.rate_text).map_err(|e| bad(format!("rate: {e}")))?;
    let discount = if row.discount_text.trim().is_empty() {
        Decimal::ZERO
    } else {
        parse_decimal(&row.discount_text).map_err(|e| bad(format!("discount: {e}")))?
    };
    if discount > Decimal::ONE_HUNDRED {
        return Err(bad("discount cannot be more than 100%".to_owned()));
    }
    if !row.amount_text.trim().is_empty() {
        return eval_amount(&row.amount_text).map_err(|e| bad(format!("amount: {e}")));
    }
    let exact = Decimal::ONE_HUNDRED
        .checked_sub(discount)
        .and_then(|factor| qty.checked_mul(rate)?.checked_mul(factor))
        .and_then(|v| v.checked_div(Decimal::ONE_HUNDRED))
        .ok_or(InvoiceError::Money(MoneyError::Overflow))?;
    if exact.normalize().scale() > 2 {
        return Err(InvoiceError::NeedsAmount { line });
    }
    Ok(round_rupees_to_paise(exact, RoundingRule::HalfUpToPaise)?)
}

/// An invoice being entered, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceDraft {
    /// The voucher type.
    pub type_id: VoucherTypeId,
    /// The kind of invoice.
    pub kind: InvoiceKind,
    /// Whether the voucher type asks the user for the number.
    pub number_required: bool,
    /// Date text, `dd-mm-yyyy` after Enter.
    pub date_text: String,
    /// Voucher number text (manual numbering only).
    pub number_text: String,
    /// The party, once picked.
    pub party: Option<LedgerId>,
    /// Party field text.
    pub party_text: String,
    /// Place of supply text.
    pub place_text: String,
    /// The sales or purchase ledger, once picked.
    pub account: Option<LedgerId>,
    /// Sales or purchase ledger field text.
    pub account_text: String,
    /// Supplier invoice number, original invoice number or order reference.
    pub reference_text: String,
    /// The stock lines.
    pub rows: Vec<InvoiceRow>,
    /// Narration.
    pub narration: String,
}

impl InvoiceDraft {
    /// An empty invoice of `kind` dated `date_text`, with one blank line.
    #[must_use]
    pub fn new(
        type_id: VoucherTypeId,
        kind: InvoiceKind,
        number_required: bool,
        date_text: String,
    ) -> Self {
        InvoiceDraft {
            type_id,
            kind,
            number_required,
            date_text,
            number_text: String::new(),
            party: None,
            party_text: String::new(),
            place_text: String::new(),
            account: None,
            account_text: String::new(),
            reference_text: String::new(),
            rows: vec![InvoiceRow::default()],
            narration: String::new(),
        }
    }

    /// `true` when nothing but the date has been entered.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.party.is_none()
            && self.account.is_none()
            && self.party_text.trim().is_empty()
            && self.account_text.trim().is_empty()
            && self.place_text.trim().is_empty()
            && self.number_text.trim().is_empty()
            && self.reference_text.trim().is_empty()
            && self.narration.trim().is_empty()
            && self.rows.iter().all(InvoiceRow::is_blank)
    }

    /// The voucher to hand to the tax engine: the header, the inventory lines and nothing else.
    /// The accounting lines and the stored GST are the engine's to produce.
    ///
    /// # Errors
    /// An [`InvoiceError`] naming the first thing that is missing or wrong.
    pub fn build_voucher(
        &self,
        masters: &Masters,
        fy: FinancialYear,
    ) -> Result<Voucher, InvoiceError> {
        let date = parse_entry_date(&self.date_text, fy)?;
        if self.number_required && self.number_text.trim().is_empty() {
            return Err(InvoiceError::Missing {
                field: "voucher number",
            });
        }
        let missing_party = InvoiceError::Missing {
            field: self.kind.party_label(),
        };
        let party = self.party.ok_or_else(|| missing_party.clone())?;
        let party_ledger = masters.ledgers.get(&party).ok_or(missing_party)?;
        if party_ledger.bill_wise.is_some() {
            return Err(InvoiceError::Unsupported(Unsupported {
                what: "bill reference on a bill-wise party".to_owned(),
                reason: "this screen does not enter bill references yet; the party ledger must not be bill-wise"
                    .to_owned(),
            }));
        }
        let place = parse_place(&self.place_text)?;
        let account = self.account.ok_or(InvoiceError::Missing {
            field: self.kind.account_label(),
        })?;
        let inventory_lines = self.inventory_lines(masters, account)?;
        Ok(Voucher {
            id: UNASSIGNED_VOUCHER_ID,
            type_id: self.type_id,
            number: nonblank(&self.number_text),
            series: None,
            date,
            supply_date: None,
            payment_date: None,
            status: VoucherStatus::Regular,
            mode: VoucherMode::ItemInvoice,
            party: Some(party),
            place_of_supply: Some(place),
            reverse_charge: false,
            reference: nonblank(&self.reference_text),
            reference_date: None,
            narration: self.narration.trim().to_owned(),
            ledger_lines: Vec::new(),
            inventory_lines,
            gst: None,
            tally: None,
            extra: BTreeMap::new(),
        })
    }

    fn inventory_lines(
        &self,
        masters: &Masters,
        account: LedgerId,
    ) -> Result<Vec<InventoryLine>, InvoiceError> {
        if !masters.godowns.is_empty() {
            return Err(InvoiceError::Unsupported(Unsupported {
                what: "godown allocation on an item invoice".to_owned(),
                reason: "this company has godowns and the screen cannot pick one yet".to_owned(),
            }));
        }
        let mut lines = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            if !row.is_blank() {
                lines.push(inventory_line(
                    masters,
                    row,
                    index.saturating_add(1),
                    account,
                )?);
            }
        }
        if lines.is_empty() {
            return Err(InvoiceError::Missing { field: "an item" });
        }
        Ok(lines)
    }
}

/// One typed row as an inventory line posted to `account`.
fn inventory_line(
    masters: &Masters,
    row: &InvoiceRow,
    line: usize,
    account: LedgerId,
) -> Result<InventoryLine, InvoiceError> {
    let amount = line_amount(masters, row, line)?;
    let Some(item) = row.item.and_then(|id| masters.stock_items.get(&id)) else {
        return Err(InvoiceError::Line {
            line,
            reason: "choose an item".to_owned(),
        });
    };
    if item.batches_enabled {
        return Err(InvoiceError::Unsupported(Unsupported {
            what: format!("batch on item '{}'", item.name),
            reason: "the screen cannot pick a batch yet".to_owned(),
        }));
    }
    let number = |label: &str, text: &str| {
        parse_decimal(text).map_err(|reason| InvoiceError::Line {
            line,
            reason: format!("{label}: {reason}"),
        })
    };
    let qty = number("quantity", &row.qty_text)?;
    let rate = number("rate", &row.rate_text)?;
    let discount_percent = if row.discount_text.trim().is_empty() {
        None
    } else {
        Some(Rate(number("discount", &row.discount_text)?))
    };
    Ok(InventoryLine {
        item: item.id,
        qty: Qty(qty),
        unit: item.unit,
        rate: Rate(rate),
        discount_percent,
        amount,
        ledger: Some(account),
        godowns: Vec::new(),
    })
}

fn nonblank(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_owned())
}
