//! The invoice as typed: stock rows or particulars, the draft, and the voucher built from it.

use super::{InvoiceError, InvoiceKind, InvoiceMode, parse_decimal, parse_place};
use crate::screens::voucher_entry::{UNASSIGNED_VOUCHER_ID, eval_amount, parse_entry_date};
use ledgerly_api::PartyBill;
use ledgerly_api::is_gst_tax_ledger;
use ledgerly_core::{
    Date, Decimal, DrCr, FinancialYear, GodownId, LedgerId, MoneyError, Paise, Qty, Rate,
    RoundingRule, SignedAmount, StockItemId, Unsupported, VoucherTypeId, round_rupees_to_paise,
};
use ledgerly_model::{
    BillRefKind, GodownAllocation, InventoryLine, LedgerLine, Masters, Voucher, VoucherMode,
    VoucherStatus,
};
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

/// One bill reference of the party line as typed: its type, its name and the part of the invoice
/// value it takes (TallyPrime's Bill-wise Details screen:
/// <https://help.tallysolutions.com/advances-received/>, read 2026-11-25).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BillRow {
    /// The type as typed: any start of New Ref, Agst Ref, Advance or On Account. Empty is a New
    /// Ref.
    pub kind_text: String,
    /// The bill's name. Empty on a New Ref means the voucher number, given when it is saved.
    pub name: String,
    /// The amount in rupees; empty takes what the other references leave.
    pub amount_text: String,
}

impl BillRow {
    pub(super) fn is_blank(&self) -> bool {
        self.kind_text.trim().is_empty()
            && self.name.trim().is_empty()
            && self.amount_text.trim().is_empty()
    }

    /// The words shown for `kind`: the product's own names for the four types.
    #[must_use]
    pub fn kind_label(kind: BillRefKind) -> &'static str {
        match kind {
            BillRefKind::New => "New Ref",
            BillRefKind::Against => "Agst Ref",
            BillRefKind::Advance => "Advance",
            BillRefKind::OnAccount => "On Account",
        }
    }

    /// Reads a typed type name: any start of the four labels, in any case; `None` when it is
    /// empty or names none of them.
    #[must_use]
    pub fn parse_kind(text: &str) -> Option<BillRefKind> {
        let t = text.trim().to_lowercase();
        if t.is_empty() {
            return None;
        }
        [
            BillRefKind::New,
            BillRefKind::Against,
            BillRefKind::Advance,
            BillRefKind::OnAccount,
        ]
        .into_iter()
        .find(|k| Self::kind_label(*k).to_lowercase().starts_with(&t))
    }
}

/// One particular of an accounting invoice as typed: a ledger and its amount.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticularRow {
    /// The ledger, once picked.
    pub ledger: Option<LedgerId>,
    /// The text in the ledger field (the query, or the picked ledger's name).
    pub ledger_text: String,
    /// The amount, in rupees.
    pub amount_text: String,
}

impl ParticularRow {
    pub(super) fn is_blank(&self) -> bool {
        self.ledger.is_none()
            && self.ledger_text.trim().is_empty()
            && self.amount_text.trim().is_empty()
    }
}

/// The side the particulars of `kind` are posted on: a sale credits its income ledger, a
/// purchase debits its expense ledger, a credit note (a sales return) debits and a debit note (a
/// purchase return) credits. The engine posts the tax lines on the same side.
fn particular_side(kind: InvoiceKind) -> DrCr {
    match kind {
        InvoiceKind::Sales | InvoiceKind::DebitNote => DrCr::Cr,
        InvoiceKind::Purchase | InvoiceKind::CreditNote => DrCr::Dr,
    }
}

/// Evaluates a line amount the user typed. A plain number with more than two decimals (for
/// example `100.005`) is refused: [`eval_amount`] would round it to paise with no notice, and no
/// rounding rule for a typed line amount has provenance. Expressions (for example `10/3`) go to
/// [`eval_amount`], which rounds half up to paise; that rounding is not refused here.
pub(super) fn typed_amount(text: &str) -> Result<Paise, String> {
    if let Ok(plain) = parse_decimal(text)
        && plain.normalize().scale() > 2
    {
        return Err(format!(
            "'{}' has more than two decimal places; type the amount in whole paise",
            text.trim()
        ));
    }
    eval_amount(text).map_err(|e| e.to_string())
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
        return typed_amount(&row.amount_text).map_err(|e| bad(format!("amount: {e}")));
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
    /// Supplier bill date or original invoice date, as typed (purchase, credit and debit notes).
    pub reference_date_text: String,
    /// How the invoice is entered.
    pub mode: InvoiceMode,
    /// The stock lines (item invoice).
    pub rows: Vec<InvoiceRow>,
    /// The particulars (accounting invoice).
    pub particulars: Vec<ParticularRow>,
    /// Narration.
    pub narration: String,
    /// The bill references typed for the party line. Empty means the default: for a bill-wise
    /// party one New Ref for the whole value, named by the voucher number.
    pub bills: Vec<BillRow>,
}

impl InvoiceDraft {
    /// An empty item invoice of `kind` dated `date_text`, with one blank line.
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
            reference_date_text: String::new(),
            mode: InvoiceMode::Item,
            rows: vec![InvoiceRow::default()],
            particulars: vec![ParticularRow::default()],
            narration: String::new(),
            bills: Vec::new(),
        }
    }

    /// The same invoice entered in `mode`.
    #[must_use]
    pub fn in_mode(self, mode: InvoiceMode) -> Self {
        InvoiceDraft { mode, ..self }
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
            && self.particulars.iter().all(ParticularRow::is_blank)
            && self.bills.iter().all(BillRow::is_blank)
    }

    /// The bill references to hand the engine with the invoice; rows with nothing typed are
    /// skipped.
    ///
    /// # Errors
    /// [`InvoiceError::Bill`] when a type names none of the four or an amount is not a plain
    /// amount.
    pub fn party_bills(&self) -> Result<Vec<PartyBill>, InvoiceError> {
        let mut out = Vec::with_capacity(self.bills.len());
        for (index, row) in self.bills.iter().enumerate() {
            if row.is_blank() {
                continue;
            }
            let line = index.saturating_add(1);
            let kind = if row.kind_text.trim().is_empty() {
                BillRefKind::New
            } else {
                BillRow::parse_kind(&row.kind_text).ok_or_else(|| InvoiceError::Bill {
                    line,
                    reason: format!(
                        "'{}' is not New Ref, Agst Ref, Advance or On Account",
                        row.kind_text.trim()
                    ),
                })?
            };
            let amount = if row.amount_text.trim().is_empty() {
                None
            } else {
                Some(
                    typed_amount(&row.amount_text).map_err(|reason| InvoiceError::Bill {
                        line,
                        reason: format!("amount: {reason}"),
                    })?,
                )
            };
            out.push(PartyBill {
                kind,
                name: row.name.trim().to_owned(),
                amount,
                credit_days: None,
                due_date: None,
            });
        }
        Ok(out)
    }

    /// The typed reference date: `None` when nothing is typed.
    ///
    /// # Errors
    /// [`InvoiceError::Date`] when the text is not a date.
    pub fn reference_date(&self, fy: FinancialYear) -> Result<Option<Date>, InvoiceError> {
        if self.reference_date_text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(parse_entry_date(&self.reference_date_text, fy)?))
    }

    /// The voucher to hand to the tax engine: the header and the inventory lines (item invoice)
    /// or the particulars (accounting invoice), and nothing else. The party line, the tax lines
    /// and the stored GST are the engine's to produce.
    ///
    /// # Errors
    /// An [`InvoiceError`] naming the first thing that is missing or wrong.
    pub fn build_voucher(
        &self,
        masters: &Masters,
        fy: FinancialYear,
    ) -> Result<Voucher, InvoiceError> {
        let date = parse_entry_date(&self.date_text, fy)?;
        let reference_date = self.reference_date(fy)?;
        if self.number_required && self.number_text.trim().is_empty() {
            return Err(InvoiceError::Missing {
                field: "voucher number",
            });
        }
        let missing_party = InvoiceError::Missing {
            field: self.kind.party_label(),
        };
        let party = self.party.ok_or_else(|| missing_party.clone())?;
        if !masters.ledgers.contains_key(&party) {
            return Err(missing_party);
        }
        let place = parse_place(&self.place_text)?;
        let (mode, ledger_lines, inventory_lines) = match self.mode {
            InvoiceMode::Item => {
                let account = self.account.ok_or(InvoiceError::Missing {
                    field: self.kind.account_label(),
                })?;
                let lines = self.inventory_lines(masters, account)?;
                (VoucherMode::ItemInvoice, Vec::new(), lines)
            }
            InvoiceMode::Accounting => (
                VoucherMode::AccountingInvoice,
                self.particular_lines(masters)?,
                Vec::new(),
            ),
            InvoiceMode::AsVoucher => (
                VoucherMode::AsVoucher,
                self.particular_lines(masters)?,
                Vec::new(),
            ),
        };
        Ok(Voucher {
            id: UNASSIGNED_VOUCHER_ID,
            type_id: self.type_id,
            number: nonblank(&self.number_text),
            series: None,
            date,
            supply_date: None,
            payment_date: None,
            status: VoucherStatus::Regular,
            mode,
            party: Some(party),
            place_of_supply: Some(place),
            reverse_charge: false,
            reference: nonblank(&self.reference_text),
            reference_date,
            narration: self.narration.trim().to_owned(),
            ledger_lines,
            inventory_lines,
            gst: None,
            tally: None,
            extra: BTreeMap::new(),
        })
    }

    /// The non-blank particulars as ledger lines on the kind's side, in typed order.
    fn particular_lines(&self, masters: &Masters) -> Result<Vec<LedgerLine>, InvoiceError> {
        let side = particular_side(self.kind);
        let mut lines = Vec::new();
        for (index, row) in self.particulars.iter().enumerate() {
            if row.is_blank() {
                continue;
            }
            let line = index.saturating_add(1);
            let bad = |reason: String| InvoiceError::Line { line, reason };
            let ledger = row
                .ledger
                .filter(|id| masters.ledgers.contains_key(id))
                .ok_or_else(|| bad("choose a ledger".to_owned()))?;
            // As voucher: a GST ledger needs no amount, the engine fills it (a typed one is
            // replaced).
            let engine_fills = self.mode == InvoiceMode::AsVoucher
                && row.amount_text.trim().is_empty()
                && is_gst_tax_ledger(masters, ledger);
            let amount = if engine_fills {
                Paise::ZERO
            } else {
                typed_amount(&row.amount_text).map_err(|e| bad(format!("amount: {e}")))?
            };
            if !engine_fills && (amount.is_zero() || amount.is_negative()) {
                return Err(bad("the amount must be more than zero".to_owned()));
            }
            lines.push(LedgerLine {
                ledger,
                amount: SignedAmount { amount, side },
                bills: Vec::new(),
                cost: Vec::new(),
                bank: None,
                ineligible_itc: None,
            });
        }
        if lines.is_empty() {
            return Err(InvoiceError::Missing { field: "a ledger" });
        }
        Ok(lines)
    }

    fn inventory_lines(
        &self,
        masters: &Masters,
        account: LedgerId,
    ) -> Result<Vec<InventoryLine>, InvoiceError> {
        let godown = only_godown(masters)?;
        let mut lines = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            if !row.is_blank() {
                lines.push(inventory_line(
                    masters,
                    row,
                    index.saturating_add(1),
                    account,
                    godown,
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
    godown: Option<GodownId>,
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
        godowns: godown
            .map(|godown| GodownAllocation {
                godown,
                batch: None,
                qty: Qty(qty),
                amount,
            })
            .into_iter()
            .collect(),
    })
}

/// The godown every line goes to: the company's only godown, or `None` when it has none.
///
/// The TallyPrime godowns FAQ
/// (<https://help.tallysolutions.com/tally-prime/godowns-locations/godowns-locations-faq-tally/>,
/// accessed 2026-11-21) says that without multiple godowns all stock sits in Main Location, and
/// voucher entry shows no godown list. Every company file starts with Main Location, so while it
/// is the only godown the whole line goes there. With more godowns the user must choose, and the
/// screen has no picker yet, so it refuses. Masters with no godown at all (not a company file,
/// which always holds Main Location) leave the line unallocated, and the store refuses it on save.
fn only_godown(masters: &Masters) -> Result<Option<GodownId>, InvoiceError> {
    let mut ids = masters.godowns.keys();
    match (ids.next(), ids.next()) {
        (Some(id), None) => Ok(Some(*id)),
        (Some(_), Some(_)) => Err(InvoiceError::Unsupported(Unsupported {
            what: "godown allocation on an item invoice".to_owned(),
            reason: "this company has more than one godown and the screen cannot pick one yet"
                .to_owned(),
        })),
        (None, _) => Ok(None),
    }
}

fn nonblank(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_owned())
}
