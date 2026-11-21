//! Item invoice entry. **Card U5** implements this module.
//!
//! Frozen interface (plan card U5):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `ItemInvoice`. The screen enters sales, purchase, credit-note and debit-note
//! vouchers as item invoices (party, place of supply, a sales or purchase ledger and stock lines).
//! Every other voucher kind answers [`Outcome::Unsupported`] from [`ItemInvoiceScreen::open`].
//!
//! # Where the numbers come from (rule C4: the UI never sums anything)
//!
//! * A line amount is quantity times rate, less the discount percent. It is arithmetic on what
//!   the user typed, done with checked decimals. The product is exact in rupees only when it has
//!   at most two decimals; otherwise the screen says so and asks for the amount to be typed
//!   (no rounding rule for a line amount has provenance yet, so none is applied silently).
//! * Tax, round-off, the invoice total and the accounting lines all come from the engine. This
//!   crate cannot call the GST engine (it depends on neither `ledgerly-gst` nor
//!   `ledgerly-engine`), so the host does: it takes [`ItemInvoiceScreen::take_compute_request`],
//!   runs the engine on that voucher and hands the answer back with
//!   [`ItemInvoiceScreen::set_gst`] as a [`GstResult`]. The screen shows exactly that answer.
//! * An answer belongs to the exact voucher it was computed for. Any edit that changes the
//!   voucher makes the answer stale: the block then says "not computed yet", never old figures,
//!   and Save refuses ([`GstStatus`]).
//! * The amount in words is [`ledgerly_core::format::amount_in_words`] of the engine's invoice
//!   total.
//!
//! # Not built here (reported, not guessed)
//!
//! Godown and batch allocation, bill references on a bill-wise party, units other than the item's
//! own, and inline creation of masters are `Unsupported` ([`InvoiceError::Unsupported`]).
//! The place of supply is never defaulted: the user sets it.

use std::collections::BTreeMap;
use std::str::FromStr;

use egui::{Id, Key, Modifiers, Ui};
use ledgerly_core::format::{amount_in_words, format_inr};
use ledgerly_core::{
    Date, Decimal, DrCr, FinancialYear, LedgerId, MoneyError, Outcome, Paise, Qty, Rate,
    RoundingRule, SignedAmount, StateCode, StockItemId, Unsupported, VoucherTypeId,
    round_rupees_to_paise,
};
use ledgerly_model::{
    BaseVoucherKind, Books, InventoryLine, LedgerLine, Masters, NumberingMethod, StoredGst,
    Voucher, VoucherMode, VoucherStatus,
};
use thiserror::Error;

use crate::screens::voucher_entry::{
    EntryDateError, LedgerHit, OpenError, PickerItem, UNASSIGNED_VOUCHER_ID, eval_amount,
    format_entry_date, parse_entry_date, search_ledgers,
};
use crate::{AppCtx, Screen, ScreenAction};

/// How many type-ahead entries are listed at most.
const PICKER_LIMIT: usize = 50;
/// Longest number text that is parsed.
const MAX_NUMBER_CHARS: usize = 30;

// ---------------------------------------------------------------------------------------------
// Voucher kinds
// ---------------------------------------------------------------------------------------------

/// The voucher kinds this screen enters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceKind {
    /// Sales invoice.
    Sales,
    /// Purchase invoice.
    Purchase,
    /// Credit note.
    CreditNote,
    /// Debit note.
    DebitNote,
}

impl InvoiceKind {
    /// The invoice kind of a voucher type's base, or `Unsupported` for kinds this screen does not
    /// enter.
    #[must_use]
    pub fn from_base(base: BaseVoucherKind) -> Outcome<InvoiceKind> {
        match base {
            BaseVoucherKind::Sales => Outcome::Value(InvoiceKind::Sales),
            BaseVoucherKind::Purchase => Outcome::Value(InvoiceKind::Purchase),
            BaseVoucherKind::CreditNote => Outcome::Value(InvoiceKind::CreditNote),
            BaseVoucherKind::DebitNote => Outcome::Value(InvoiceKind::DebitNote),
            BaseVoucherKind::Contra
            | BaseVoucherKind::Payment
            | BaseVoucherKind::Receipt
            | BaseVoucherKind::Journal
            | BaseVoucherKind::Memorandum
            | BaseVoucherKind::ReversingJournal => Outcome::unsupported(
                format!("{base:?} voucher on the item invoice screen"),
                "plain debit and credit vouchers are entered on the accounting voucher screen",
            ),
            BaseVoucherKind::ReceiptNote
            | BaseVoucherKind::DeliveryNote
            | BaseVoucherKind::RejectionIn
            | BaseVoucherKind::RejectionOut
            | BaseVoucherKind::StockJournal
            | BaseVoucherKind::PhysicalStock
            | BaseVoucherKind::SalesOrder
            | BaseVoucherKind::PurchaseOrder => Outcome::unsupported(
                format!("{base:?} voucher on the item invoice screen"),
                "stock-only and order vouchers have no invoice and no tax; not built yet",
            ),
        }
    }

    fn account_label(self) -> &'static str {
        match self {
            InvoiceKind::Sales | InvoiceKind::CreditNote => "Sales ledger",
            InvoiceKind::Purchase | InvoiceKind::DebitNote => "Purchase ledger",
        }
    }

    fn party_label(self) -> &'static str {
        match self {
            InvoiceKind::Sales | InvoiceKind::CreditNote => "Customer",
            InvoiceKind::Purchase | InvoiceKind::DebitNote => "Supplier",
        }
    }

    fn reference_label(self) -> &'static str {
        match self {
            InvoiceKind::Sales => "Order or reference",
            InvoiceKind::Purchase => "Supplier invoice no.",
            InvoiceKind::CreditNote | InvoiceKind::DebitNote => "Original invoice no.",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Typed input
// ---------------------------------------------------------------------------------------------

/// Why the typed invoice cannot be turned into a voucher yet.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvoiceError {
    /// The date text is not a date.
    #[error(transparent)]
    Date(#[from] EntryDateError),
    /// A required header field is empty or names nothing.
    #[error("{field} is required")]
    Missing {
        /// The field's label.
        field: &'static str,
    },
    /// The place of supply text is not a state or union territory.
    #[error("place of supply: {0}")]
    Place(String),
    /// A line is incomplete or wrong.
    #[error("line {line}: {reason}")]
    Line {
        /// 1-based line number.
        line: usize,
        /// What is wrong and what to do.
        reason: String,
    },
    /// Quantity times rate has more than two decimals of a rupee.
    #[error(
        "line {line}: quantity x rate gives a fraction of a paisa; type the line amount yourself"
    )]
    NeedsAmount {
        /// 1-based line number.
        line: usize,
    },
    /// An amount does not fit.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// The input needs behaviour that is not built.
    #[error("not supported yet: {} ({})", .0.what, .0.reason)]
    Unsupported(Unsupported),
}

/// Reads a plain non-negative number: digits and at most one decimal point.
fn parse_decimal(text: &str) -> Result<Decimal, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("type a number".to_owned());
    }
    let plain = t.chars().all(|c| c.is_ascii_digit() || c == '.');
    if !plain || t == "." || t.matches('.').count() > 1 || t.chars().count() > MAX_NUMBER_CHARS {
        return Err(format!(
            "'{t}' is not a plain number; use digits and one decimal point"
        ));
    }
    let mut text = t.to_owned();
    if text.starts_with('.') {
        text.insert(0, '0');
    }
    if text.ends_with('.') {
        text.push('0');
    }
    Decimal::from_str(&text).map_err(|_| format!("'{t}' is not a number"))
}

/// Reads a place of supply: the state code (`27`), the name (`Maharashtra`) or a unique start of
/// the name, in any case; also the `27 Maharashtra` form that [`place_label`] writes.
///
/// # Errors
/// [`InvoiceError::Place`] when the text is empty, names no state or names several.
pub fn parse_place(text: &str) -> Result<StateCode, InvoiceError> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return Err(InvoiceError::Place("type a state code or name".to_owned()));
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        let code: u8 = t
            .parse()
            .map_err(|_| InvoiceError::Place(format!("'{t}' is not a state code")))?;
        return StateCode::new(code)
            .map_err(|_| InvoiceError::Place(format!("{code} is not a state code")));
    }
    // Codes 25 and 28 are pre-merger codes kept for old invoices; a name never selects them.
    let current = |s: &StateCode| !s.name().contains("(before");
    let exact: Vec<StateCode> = StateCode::all()
        .filter(|s| {
            place_label(*s).to_lowercase() == t || (current(s) && s.name().to_lowercase() == t)
        })
        .collect();
    if let [one] = exact.as_slice() {
        return Ok(*one);
    }
    let starts: Vec<StateCode> = StateCode::all()
        .filter(|s| current(s) && s.name().to_lowercase().starts_with(&t))
        .collect();
    match starts.as_slice() {
        [one] => Ok(*one),
        [] => Err(InvoiceError::Place(format!("'{t}' matches no state"))),
        [..] => Err(InvoiceError::Place(format!(
            "'{t}' matches several states; type more of the name or the code"
        ))),
    }
}

/// `27 Maharashtra`, the form the place field shows after Enter.
#[must_use]
pub fn place_label(state: StateCode) -> String {
    format!("{:02} {}", state.get(), state.name())
}

// ---------------------------------------------------------------------------------------------
// Item type-ahead
// ---------------------------------------------------------------------------------------------

/// A stock item offered by the type-ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemHit {
    /// The item.
    pub id: StockItemId,
    /// Its name.
    pub name: String,
    /// The alias that matched, when the name itself did not match as well.
    pub via_alias: Option<String>,
    /// Symbol of the item's unit, when the unit exists.
    pub unit: Option<String>,
}

/// 0 exact, 1 prefix, 2 a word starts with it, 3 contains it; `None` for no match.
fn item_rank(candidate: &str, query: &str) -> Option<u8> {
    if candidate == query {
        Some(0)
    } else if candidate.starts_with(query) {
        Some(1)
    } else if candidate.split_whitespace().any(|w| w.starts_with(query)) {
        Some(2)
    } else if candidate.contains(query) {
        Some(3)
    } else {
        None
    }
}

/// Stock items whose name or alias matches `query`, best match first then by name, at most 50.
/// An empty query lists every item.
#[must_use]
pub fn search_items(masters: &Masters, query: &str) -> Vec<ItemHit> {
    let q = query.trim().to_lowercase();
    let mut found: Vec<((u8, String, StockItemId), ItemHit)> = Vec::new();
    for item in masters.stock_items.values() {
        let name_lower = item.name.to_lowercase();
        let mut best: Option<(u8, Option<String>)> = if q.is_empty() {
            Some((4, None))
        } else {
            item_rank(&name_lower, &q).map(|r| (r, None))
        };
        for alias in &item.aliases {
            if let Some(r) = item_rank(&alias.to_lowercase(), &q)
                && best.as_ref().is_none_or(|(b, _)| r < *b)
            {
                best = Some((r, Some(alias.clone())));
            }
        }
        let Some((rank, via_alias)) = best else {
            continue;
        };
        let unit = masters.units.get(&item.unit).map(|u| u.symbol.clone());
        found.push((
            (rank, name_lower, item.id),
            ItemHit {
                id: item.id,
                name: item.name.clone(),
                via_alias,
                unit,
            },
        ));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.truncate(PICKER_LIMIT);
    found.into_iter().map(|(_, hit)| hit).collect()
}

/// The ledgers matching `query`, without the "create" entry (this screen creates no masters).
fn ledger_hits(
    masters: &Masters,
    query: &str,
    recents: &[LedgerId],
    balances: &BTreeMap<LedgerId, SignedAmount>,
) -> Vec<LedgerHit> {
    search_ledgers(masters, query, recents, balances)
        .into_iter()
        .filter_map(|item| match item {
            PickerItem::Ledger(hit) => Some(hit),
            PickerItem::Create { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The draft
// ---------------------------------------------------------------------------------------------

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
    fn is_blank(&self) -> bool {
        self.item.is_none()
            && self.item_text.trim().is_empty()
            && self.qty_text.trim().is_empty()
            && self.rate_text.trim().is_empty()
            && self.discount_text.trim().is_empty()
            && self.amount_text.trim().is_empty()
    }
}

/// The amount of one line: quantity x rate less discount, or the amount the user typed.
fn line_amount(masters: &Masters, row: &InvoiceRow, line: usize) -> Result<Paise, InvoiceError> {
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

// ---------------------------------------------------------------------------------------------
// The engine's answer
// ---------------------------------------------------------------------------------------------

/// Tax on one inventory line, as the engine computed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstBlockLine {
    /// Taxable value of the line.
    pub taxable: Paise,
    /// Total GST rate in percent.
    pub rate_percent: Decimal,
}

/// One tax head's voucher total, as the engine computed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstTotal {
    /// The head, for example `CGST`.
    pub head: String,
    /// The head's total for the voucher, rounded by the engine.
    pub amount: Paise,
}

/// What the tax engine answered for a voucher. The host fills it from the engine's output; the
/// screen shows it and adds nothing to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstBlock {
    /// Rule pack version the engine used.
    pub pack_version: String,
    /// One entry per inventory line, in order.
    pub lines: Vec<GstBlockLine>,
    /// Taxable value of the voucher.
    pub taxable_total: Paise,
    /// Tax heads, in display order.
    pub heads: Vec<GstTotal>,
    /// The explicit round-off line.
    pub round_off: Paise,
    /// The invoice value.
    pub invoice_total: Paise,
    /// The accounting lines the engine derived (party, sales or purchase, tax, round-off).
    pub ledger_lines: Vec<LedgerLine>,
    /// What is stored on the voucher.
    pub stored: StoredGst,
}

/// The engine's answer to a compute request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GstResult {
    /// Tax was computed.
    Computed(GstBlock),
    /// The engine does not compute this voucher; its reason is shown.
    Unsupported(Unsupported),
    /// The engine failed; the message says what and where.
    Failed(String),
}

/// What the GST block shows now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GstStatus<'a> {
    /// The invoice is not complete enough to compute; the text says what is missing.
    Incomplete(String),
    /// Complete, but no answer for this exact voucher yet (none asked for, or the one held is for
    /// an earlier version of the invoice).
    NotComputedYet,
    /// The engine's answer for exactly this voucher.
    Computed(&'a GstBlock),
    /// The engine does not compute this voucher.
    Unsupported(&'a Unsupported),
    /// The engine failed.
    Failed(&'a str),
}

/// Checks that a block describes `voucher` (same lines, party amount equals the invoice total).
fn check_block(voucher: &Voucher, block: &GstBlock) -> Result<(), String> {
    let same_lines = block.lines.len() == voucher.inventory_lines.len()
        && block
            .lines
            .iter()
            .zip(&voucher.inventory_lines)
            .all(|(b, v)| b.taxable == v.amount);
    if !same_lines {
        return Err(
            "the engine's lines do not match the invoice lines; nothing is shown".to_owned(),
        );
    }
    let party_ok = voucher.party.is_some_and(|party| {
        block
            .ledger_lines
            .iter()
            .any(|l| l.ledger == party && l.amount.amount == block.invoice_total)
    });
    if !party_ok {
        return Err(
            "the engine's party line does not equal its invoice total; nothing is shown".to_owned(),
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------------------------

/// A place the keyboard can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Voucher date.
    Date,
    /// Voucher number (manual numbering only).
    Number,
    /// Party.
    Party,
    /// Place of supply.
    Place,
    /// Sales or purchase ledger.
    Account,
    /// Reference.
    Reference,
    /// Item of a line.
    Item(usize),
    /// Quantity of a line.
    Qty(usize),
    /// Rate of a line.
    Rate(usize),
    /// Discount percent of a line.
    Discount(usize),
    /// Typed amount of a line.
    Amount(usize),
    /// Narration.
    Narration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Notice {
    Error(String),
    Info(String),
}

/// What Enter does next.
enum Step {
    Next,
    Stay,
    Goto(Field),
}

/// Which type-ahead list is open.
#[derive(Clone, Copy)]
enum Pick {
    Party,
    Account,
    Item(usize),
}

enum UiEvent {
    Focus(Field),
    Typed(Field),
    Submit { field: Field, back: bool },
    Pick(usize),
    Leave,
}

#[derive(Clone, Copy)]
enum KeyCmd {
    Save,
    PickUp,
    PickDown,
    Escape,
    DiscardYes,
    DiscardNo,
}

/// The item invoice entry screen (card U5).
pub struct ItemInvoiceScreen {
    type_name: String,
    fy: FinancialYear,
    draft: InvoiceDraft,
    balances: BTreeMap<LedgerId, SignedAmount>,
    recents: Vec<LedgerId>,
    focus: Field,
    focus_pending: bool,
    pick_index: usize,
    confirm_discard: bool,
    notice: Option<Notice>,
    requested: Option<Voucher>,
    answer: Option<(Voucher, GstResult)>,
    outbox: Option<Voucher>,
}

impl ItemInvoiceScreen {
    /// Opens an entry screen for voucher type `type_id`, dated `today` (in financial year `fy`).
    ///
    /// Returns `Unsupported` for voucher kinds this screen does not enter.
    ///
    /// # Errors
    /// [`OpenError`] when the voucher type is missing or inactive.
    pub fn open(
        books: &Books,
        type_id: VoucherTypeId,
        fy: FinancialYear,
        today: Date,
    ) -> Result<Outcome<Self>, OpenError> {
        let vt = books
            .masters
            .voucher_types
            .get(&type_id)
            .ok_or(OpenError::UnknownVoucherType(type_id))?;
        if !vt.active {
            return Err(OpenError::Inactive(vt.name.clone()));
        }
        let kind = match InvoiceKind::from_base(vt.base) {
            Outcome::Value(kind) => kind,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let manual = match vt.numbering.method {
            NumberingMethod::Manual => true,
            NumberingMethod::Automatic | NumberingMethod::None => false,
        };
        Ok(Outcome::Value(ItemInvoiceScreen {
            type_name: vt.name.clone(),
            fy,
            draft: InvoiceDraft::new(type_id, kind, manual, format_entry_date(today)),
            balances: BTreeMap::new(),
            recents: Vec::new(),
            focus: Field::Party,
            focus_pending: true,
            pick_index: 0,
            confirm_discard: false,
            notice: None,
            requested: None,
            answer: None,
            outbox: None,
        }))
    }

    /// The invoice being entered.
    #[must_use]
    pub fn draft(&self) -> &InvoiceDraft {
        &self.draft
    }

    /// The invoice being entered, for editing.
    pub fn draft_mut(&mut self) -> &mut InvoiceDraft {
        &mut self.draft
    }

    /// Where the keyboard is.
    #[must_use]
    pub fn focus(&self) -> Field {
        self.focus
    }

    /// The error under the form, if any.
    #[must_use]
    pub fn error_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Error(s)) => Some(s),
            Some(Notice::Info(_)) | None => None,
        }
    }

    /// The note under the form, if any.
    #[must_use]
    pub fn info_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Info(s)) => Some(s),
            Some(Notice::Error(_)) | None => None,
        }
    }

    /// Ledger balances the host wants shown in the type-ahead; a ledger without one shows none.
    pub fn set_balances(&mut self, balances: BTreeMap<LedgerId, SignedAmount>) {
        self.balances = balances;
    }

    /// Recently used ledgers, most recent first.
    pub fn set_recents(&mut self, recents: Vec<LedgerId>) {
        self.recents = recents;
    }

    /// The ledgers the party or sales/purchase type-ahead lists for `query`.
    #[must_use]
    pub fn ledger_choices(&self, masters: &Masters, query: &str) -> Vec<LedgerHit> {
        ledger_hits(masters, query, &self.recents, &self.balances)
    }

    // ---- typing ------------------------------------------------------------------------------

    /// Types into the party field; the party is chosen again with Enter.
    pub fn set_party_text(&mut self, text: &str) {
        text.clone_into(&mut self.draft.party_text);
        self.draft.party = None;
        self.pick_index = 0;
    }

    /// Types into the sales/purchase ledger field.
    pub fn set_account_text(&mut self, text: &str) {
        text.clone_into(&mut self.draft.account_text);
        self.draft.account = None;
        self.pick_index = 0;
    }

    /// Types into the item field of line `row`.
    pub fn set_item_text(&mut self, row: usize, text: &str) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            text.clone_into(&mut r.item_text);
            r.item = None;
        }
        self.pick_index = 0;
    }

    // ---- the engine's answer -----------------------------------------------------------------

    /// The voucher the host should run the tax engine on, once, when the invoice is complete and
    /// no answer or request for exactly this voucher exists.
    pub fn take_compute_request(&mut self, masters: &Masters) -> Option<Voucher> {
        let voucher = self.draft.build_voucher(masters, self.fy).ok()?;
        let answered = self.answer.as_ref().is_some_and(|(v, _)| *v == voucher);
        if answered || self.requested.as_ref() == Some(&voucher) {
            return None;
        }
        self.requested = Some(voucher.clone());
        Some(voucher)
    }

    /// Hands in the engine's answer for `voucher` (the one from [`Self::take_compute_request`]).
    /// A computed block that does not describe `voucher` is turned into a failure and not shown.
    pub fn set_gst(&mut self, voucher: Voucher, result: GstResult) {
        let result = match result {
            GstResult::Computed(block) => match check_block(&voucher, &block) {
                Ok(()) => GstResult::Computed(block),
                Err(message) => GstResult::Failed(message),
            },
            GstResult::Unsupported(u) => GstResult::Unsupported(u),
            GstResult::Failed(m) => GstResult::Failed(m),
        };
        self.answer = Some((voucher, result));
    }

    /// What the GST block shows for the invoice as it stands now.
    #[must_use]
    pub fn gst_status(&self, masters: &Masters) -> GstStatus<'_> {
        let voucher = match self.draft.build_voucher(masters, self.fy) {
            Ok(v) => v,
            Err(e) => return GstStatus::Incomplete(e.to_string()),
        };
        match &self.answer {
            Some((for_voucher, result)) if *for_voucher == voucher => match result {
                GstResult::Computed(block) => GstStatus::Computed(block),
                GstResult::Unsupported(u) => GstStatus::Unsupported(u),
                GstResult::Failed(m) => GstStatus::Failed(m),
            },
            Some(_) | None => GstStatus::NotComputedYet,
        }
    }

    /// The invoice total in words, from the engine's total; `None` unless the engine's answer for
    /// this exact invoice is on screen.
    #[must_use]
    pub fn invoice_total_in_words(&self, masters: &Masters) -> Option<String> {
        match self.gst_status(masters) {
            GstStatus::Computed(block) => Some(amount_in_words(block.invoice_total)),
            GstStatus::Incomplete(_)
            | GstStatus::NotComputedYet
            | GstStatus::Unsupported(_)
            | GstStatus::Failed(_) => None,
        }
    }

    // ---- save --------------------------------------------------------------------------------

    /// Ctrl+A: queues the voucher for the host when the engine's answer for this exact invoice is
    /// a computed block; otherwise says why not.
    pub fn save(&mut self, masters: &Masters) {
        let voucher = match self.draft.build_voucher(masters, self.fy) {
            Ok(v) => v,
            Err(e) => return self.fail(e.to_string()),
        };
        let block = match &self.answer {
            Some((for_voucher, GstResult::Computed(block))) if *for_voucher == voucher => block,
            Some((for_voucher, GstResult::Unsupported(u))) if *for_voucher == voucher => {
                let message = format!("Not supported yet: {} ({})", u.what, u.reason);
                return self.fail(message);
            }
            Some((for_voucher, GstResult::Failed(m))) if *for_voucher == voucher => {
                let message = format!("The tax engine failed: {m}");
                return self.fail(message);
            }
            Some(_) | None => {
                return self.fail("Tax has not been computed for this invoice yet.".to_owned());
            }
        };
        let mut final_voucher = voucher;
        final_voucher.ledger_lines.clone_from(&block.ledger_lines);
        final_voucher.gst = Some(block.stored.clone());
        self.outbox = Some(final_voucher);
        self.notice = Some(Notice::Info("Saving...".to_owned()));
    }

    /// The voucher queued by [`Self::save`], for the host to post.
    pub fn take_save_request(&mut self) -> Option<Voucher> {
        self.outbox.take()
    }

    /// The host posted the voucher: a fresh invoice of the same type starts, with the same date.
    pub fn confirm_saved(&mut self) {
        self.draft = InvoiceDraft::new(
            self.draft.type_id,
            self.draft.kind,
            self.draft.number_required,
            self.draft.date_text.clone(),
        );
        self.requested = None;
        self.answer = None;
        self.outbox = None;
        self.focus = Field::Party;
        self.focus_pending = true;
        self.pick_index = 0;
        self.notice = Some(Notice::Info("Saved.".to_owned()));
    }

    /// The host could not post the voucher.
    pub fn save_failed(&mut self, reason: &str) {
        self.outbox = None;
        self.fail(format!("Not saved: {reason}"));
    }

    fn fail(&mut self, message: String) {
        self.notice = Some(Notice::Error(message));
    }

    // ---- leaving -----------------------------------------------------------------------------

    /// Esc: returns `true` when the screen can close now; otherwise asks "discard?" first.
    pub fn request_leave(&mut self) -> bool {
        if self.draft.is_blank() {
            true
        } else {
            self.confirm_discard = true;
            false
        }
    }

    /// `true` while the screen asks "discard this invoice?".
    #[must_use]
    pub fn is_confirming_discard(&self) -> bool {
        self.confirm_discard
    }

    /// Answers the "discard?" question; `true` means leave.
    pub fn answer_discard(&mut self, discard: bool) -> bool {
        self.confirm_discard = false;
        discard
    }
}

// ---------------------------------------------------------------------------------------------
// Moving between fields
// ---------------------------------------------------------------------------------------------

impl ItemInvoiceScreen {
    /// The fields in Enter order for the invoice as it stands.
    fn order(&self) -> Vec<Field> {
        let mut order = vec![Field::Date];
        if self.draft.number_required {
            order.push(Field::Number);
        }
        order.extend([Field::Party, Field::Place, Field::Account, Field::Reference]);
        for i in 0..self.draft.rows.len() {
            order.extend([
                Field::Item(i),
                Field::Qty(i),
                Field::Rate(i),
                Field::Discount(i),
            ]);
        }
        order.push(Field::Narration);
        order
    }

    /// Where `field` sits in the Enter order (a typed amount counts as its line's discount).
    fn position(&self, field: Field) -> Option<usize> {
        let key = match field {
            Field::Amount(i) => Field::Discount(i),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Narration => field,
        };
        self.order().iter().position(|f| *f == key)
    }

    fn move_to(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
    }

    fn advance(&mut self) {
        let last_row = self.draft.rows.len().saturating_sub(1);
        if let Field::Discount(i) | Field::Amount(i) = self.focus
            && i == last_row
        {
            self.draft.rows.push(InvoiceRow::default());
        }
        let order = self.order();
        let next = self
            .position(self.focus)
            .and_then(|p| order.get(p.saturating_add(1)));
        if let Some(next) = next {
            self.move_to(*next);
        }
    }

    /// Shift+Enter: back one field.
    pub fn step_back(&mut self) {
        let order = self.order();
        let prev = self
            .position(self.focus)
            .and_then(|p| p.checked_sub(1))
            .and_then(|p| order.get(p));
        if let Some(prev) = prev {
            self.move_to(*prev);
        }
    }

    /// Enter: checks the field that has the keyboard and moves on.
    pub fn submit(&mut self, masters: &Masters) {
        self.notice = None;
        let step = match self.focus {
            Field::Date => self.submit_date(),
            Field::Party => self.submit_party(masters),
            Field::Account => self.submit_account(masters),
            Field::Place => self.submit_place(),
            Field::Item(i) => self.submit_item(masters, i),
            Field::Qty(i) | Field::Rate(i) | Field::Discount(i) | Field::Amount(i) => {
                self.submit_cell(i)
            }
            Field::Number | Field::Reference => Step::Next,
            Field::Narration => {
                self.save(masters);
                Step::Stay
            }
        };
        match step {
            Step::Next => self.advance(),
            Step::Goto(field) => self.move_to(field),
            Step::Stay => {}
        }
    }

    fn refuse(&mut self, message: String) -> Step {
        self.fail(message);
        Step::Stay
    }

    fn submit_date(&mut self) -> Step {
        match parse_entry_date(&self.draft.date_text, self.fy) {
            Ok(date) => {
                self.draft.date_text = format_entry_date(date);
                Step::Next
            }
            Err(e) => self.refuse(e.to_string()),
        }
    }

    fn submit_place(&mut self) -> Step {
        match parse_place(&self.draft.place_text) {
            Ok(state) => {
                self.draft.place_text = place_label(state);
                Step::Next
            }
            Err(e) => self.refuse(e.to_string()),
        }
    }

    fn picked_ledger(&self, masters: &Masters, query: &str) -> Option<LedgerHit> {
        ledger_hits(masters, query, &self.recents, &self.balances)
            .into_iter()
            .nth(self.pick_index)
    }

    fn submit_party(&mut self, masters: &Masters) -> Step {
        if self.draft.party.is_some() {
            return Step::Next;
        }
        match self.picked_ledger(masters, &self.draft.party_text) {
            Some(hit) => {
                self.draft.party = Some(hit.id);
                self.draft.party_text = hit.name;
                Step::Next
            }
            None => self.refuse("No ledger matches; create it on the masters screen.".to_owned()),
        }
    }

    fn submit_account(&mut self, masters: &Masters) -> Step {
        if self.draft.account.is_some() {
            return Step::Next;
        }
        match self.picked_ledger(masters, &self.draft.account_text) {
            Some(hit) => {
                self.draft.account = Some(hit.id);
                self.draft.account_text = hit.name;
                Step::Next
            }
            None => self.refuse("No ledger matches; create it on the masters screen.".to_owned()),
        }
    }

    fn submit_item(&mut self, masters: &Masters, i: usize) -> Step {
        let Some(row) = self.draft.rows.get(i) else {
            return Step::Stay;
        };
        if row.item.is_some() {
            return Step::Next;
        }
        if row.is_blank() {
            if i == 0 {
                return self.refuse("Enter at least one item.".to_owned());
            }
            if i.saturating_add(1) == self.draft.rows.len() {
                self.draft.rows.pop();
            }
            return Step::Goto(Field::Narration);
        }
        let hit = search_items(masters, &row.item_text)
            .into_iter()
            .nth(self.pick_index);
        let Some(hit) = hit else {
            return self.refuse("No item matches; create it on the masters screen.".to_owned());
        };
        if let Some(row) = self.draft.rows.get_mut(i) {
            row.item = Some(hit.id);
            row.item_text = hit.name;
        }
        Step::Next
    }

    fn submit_cell(&mut self, i: usize) -> Step {
        let Some(row) = self.draft.rows.get(i) else {
            return Step::Stay;
        };
        let check = match self.focus {
            Field::Qty(_) => parse_decimal(&row.qty_text).map(|_| ()),
            Field::Rate(_) => parse_decimal(&row.rate_text).map(|_| ()),
            Field::Discount(_) if !row.discount_text.trim().is_empty() => {
                parse_decimal(&row.discount_text).map(|_| ())
            }
            Field::Amount(_) if !row.amount_text.trim().is_empty() => eval_amount(&row.amount_text)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::Item(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Narration => Ok(()),
        };
        match check {
            Ok(()) => Step::Next,
            Err(e) => self.refuse(e),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type-ahead lists
// ---------------------------------------------------------------------------------------------

fn ledger_label(hit: &LedgerHit) -> String {
    let mut parts = vec![hit.name.clone()];
    if let Some(alias) = &hit.via_alias {
        parts.push(format!("(alias {alias})"));
    }
    if let Some(balance) = hit.balance {
        let side = match balance.side {
            DrCr::Dr => "Dr",
            DrCr::Cr => "Cr",
        };
        parts.push(format!("{} {side}", format_inr(balance.amount)));
    }
    if let Some(gstin) = &hit.gstin {
        parts.push(gstin.clone());
    }
    parts.join("   ")
}

fn item_label(hit: &ItemHit) -> String {
    let mut parts = vec![hit.name.clone()];
    if let Some(alias) = &hit.via_alias {
        parts.push(format!("(alias {alias})"));
    }
    if let Some(unit) = &hit.unit {
        parts.push(format!("per {unit}"));
    }
    parts.join("   ")
}

impl ItemInvoiceScreen {
    fn pick_kind(&self) -> Option<Pick> {
        match self.focus {
            Field::Party => self.draft.party.is_none().then_some(Pick::Party),
            Field::Account => self.draft.account.is_none().then_some(Pick::Account),
            Field::Item(i) => self
                .draft
                .rows
                .get(i)
                .filter(|r| r.item.is_none())
                .map(|_| Pick::Item(i)),
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Narration => None,
        }
    }

    /// The lines of the type-ahead list that is open, as shown.
    fn pick_labels(&self, masters: &Masters) -> Vec<String> {
        let Some(kind) = self.pick_kind() else {
            return Vec::new();
        };
        match kind {
            Pick::Party => {
                let hits = self.ledger_choices(masters, &self.draft.party_text);
                hits.iter().map(ledger_label).collect()
            }
            Pick::Account => {
                let hits = self.ledger_choices(masters, &self.draft.account_text);
                hits.iter().map(ledger_label).collect()
            }
            Pick::Item(i) => {
                let text = self.draft.rows.get(i).map_or("", |r| r.item_text.as_str());
                search_items(masters, text).iter().map(item_label).collect()
            }
        }
    }

    /// `true` while the field that has the keyboard holds an unfinished search.
    fn searching(&self) -> bool {
        let text = match self.pick_kind() {
            Some(Pick::Party) => self.draft.party_text.as_str(),
            Some(Pick::Account) => self.draft.account_text.as_str(),
            Some(Pick::Item(i)) => self.draft.rows.get(i).map_or("", |r| r.item_text.as_str()),
            None => return false,
        };
        !text.trim().is_empty()
    }

    fn clear_search(&mut self) {
        match self.pick_kind() {
            Some(Pick::Party) => self.set_party_text(""),
            Some(Pick::Account) => self.set_account_text(""),
            Some(Pick::Item(i)) => self.set_item_text(i, ""),
            None => {}
        }
    }

    fn row_summary(&self, masters: &Masters, i: usize) -> String {
        let Some(row) = self.draft.rows.get(i) else {
            return String::new();
        };
        if row.is_blank() {
            return String::new();
        }
        let unit = row
            .item
            .and_then(|id| masters.stock_items.get(&id))
            .and_then(|item| masters.units.get(&item.unit))
            .map_or(String::new(), |u| format!("{}  ", u.symbol));
        match line_amount(masters, row, i.saturating_add(1)) {
            Ok(amount) => format!("{unit}Amount {}", format_inr(amount)),
            Err(e) => format!("{unit}{e}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// Save: Ctrl+A and Ctrl+S (spec 7 names Ctrl+A). Ctrl+Enter is not here: spec 7 gives it to
/// "alter master". Hard-coded until the keymap card U2 is wired (see `docs/interface-requests.md`).
const SAVE_KEYS: [(Modifiers, Key); 2] =
    [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];

struct FieldSpec {
    id: Id,
    field: Field,
    label: &'static str,
    width: f32,
    hint: &'static str,
}

fn read_keys(ui: &Ui, picker_open: bool, escape_wanted: bool, confirming: bool) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.ctx().input_mut(|i| {
        if confirming {
            if i.consume_key(Modifiers::NONE, Key::Y) {
                out.push(KeyCmd::DiscardYes);
            }
            if i.consume_key(Modifiers::NONE, Key::N) {
                out.push(KeyCmd::DiscardNo);
            }
        }
        let mut save = false;
        for (modifiers, key) in SAVE_KEYS {
            save |= i.consume_key(modifiers, key);
        }
        if save {
            out.push(KeyCmd::Save);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            out.push(KeyCmd::PickDown);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            out.push(KeyCmd::PickUp);
        }
        if escape_wanted && i.consume_key(Modifiers::NONE, Key::Escape) {
            out.push(KeyCmd::Escape);
        }
    });
    out
}

/// A single-line field that can take the keyboard on request. Reports focus, typing and Enter as
/// events.
fn text_field(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) {
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .id(spec.id)
            .desired_width(spec.width)
            .hint_text(spec.hint),
    );
    if want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        events.push(UiEvent::Focus(spec.field));
    }
    if response.changed() {
        events.push(UiEvent::Typed(spec.field));
    }
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
}

fn labelled(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) {
    ui.horizontal(|ui| {
        ui.label(spec.label);
        text_field(ui, spec, text, want, events);
    });
}

fn draw_picker(ui: &mut Ui, labels: &[String], pick: usize, events: &mut Vec<UiEvent>) {
    if labels.is_empty() {
        return;
    }
    ui.group(|ui| {
        for (j, label) in labels.iter().enumerate() {
            if ui.selectable_label(j == pick, label.as_str()).clicked() {
                events.push(UiEvent::Pick(j));
            }
        }
    });
}

/// Shows the engine's figures exactly as handed in, and the total in words.
fn draw_block(ui: &mut Ui, block: &GstBlock) {
    ui.weak(format!("Rule pack {}", block.pack_version));
    for (i, line) in block.lines.iter().enumerate() {
        ui.label(format!(
            "Line {}: taxable {} at {}%",
            i.saturating_add(1),
            format_inr(line.taxable),
            line.rate_percent.normalize()
        ));
    }
    ui.label(format!("Taxable value {}", format_inr(block.taxable_total)));
    for head in &block.heads {
        ui.label(format!("{} {}", head.head, format_inr(head.amount)));
    }
    ui.label(format!("Round off {}", format_inr(block.round_off)));
    ui.strong(format!("Invoice total {}", format_inr(block.invoice_total)));
    ui.label(amount_in_words(block.invoice_total));
}

impl ItemInvoiceScreen {
    fn handle_key(&mut self, masters: &Masters, cmd: KeyCmd, list_len: usize) -> ScreenAction {
        match cmd {
            KeyCmd::Save => self.save(masters),
            KeyCmd::PickUp => self.pick_index = self.pick_index.saturating_sub(1),
            KeyCmd::PickDown => {
                let last = list_len.saturating_sub(1);
                self.pick_index = self.pick_index.saturating_add(1).min(last);
            }
            KeyCmd::DiscardYes => {
                if self.answer_discard(true) {
                    return ScreenAction::Pop;
                }
            }
            KeyCmd::DiscardNo => {
                self.answer_discard(false);
            }
            KeyCmd::Escape => {
                if self.confirm_discard {
                    self.confirm_discard = false;
                } else if self.searching() {
                    self.clear_search();
                } else if self.request_leave() {
                    return ScreenAction::Pop;
                }
            }
        }
        ScreenAction::Stay
    }

    fn apply_events(&mut self, masters: &Masters, events: Vec<UiEvent>) -> ScreenAction {
        let mut action = ScreenAction::Stay;
        for event in events {
            match event {
                UiEvent::Focus(field) => {
                    if self.focus != field {
                        self.focus = field;
                        self.pick_index = 0;
                    }
                }
                UiEvent::Typed(field) => self.typed(field),
                UiEvent::Submit { field, back } => {
                    self.focus = field;
                    if back {
                        self.step_back();
                    } else {
                        self.submit(masters);
                    }
                }
                UiEvent::Pick(j) => {
                    self.pick_index = j;
                    self.submit(masters);
                }
                UiEvent::Leave => {
                    if self.request_leave() {
                        action = ScreenAction::Pop;
                    }
                }
            }
        }
        action
    }

    /// Typing into a picked field un-picks it.
    fn typed(&mut self, field: Field) {
        self.pick_index = 0;
        match field {
            Field::Party => self.draft.party = None,
            Field::Account => self.draft.account = None,
            Field::Item(i) => {
                if let Some(row) = self.draft.rows.get_mut(i) {
                    row.item = None;
                }
            }
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Narration => {}
        }
    }

    fn draw_header(&mut self, ui: &mut Ui, base: Id, labels: &[String], ev: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let kind = self.draft.kind;
        let spec =
            |name: &str, field: Field, label: &'static str, width: f32, hint: &'static str| {
                FieldSpec {
                    id: base.with(name),
                    field,
                    label,
                    width,
                    hint,
                }
            };
        let date = spec("date", Field::Date, "Date", 110.0, "dd-mm-yyyy");
        labelled(ui, &date, &mut self.draft.date_text, want, ev);
        if self.draft.number_required {
            let s = spec("number", Field::Number, "Voucher no.", 140.0, "Required");
            labelled(ui, &s, &mut self.draft.number_text, want, ev);
        }
        let party = kind.party_label();
        let s = spec("party", Field::Party, party, 320.0, "type to search");
        labelled(ui, &s, &mut self.draft.party_text, want, ev);
        if self.focus == Field::Party {
            draw_picker(ui, labels, self.pick_index, ev);
        }
        let s = spec(
            "place",
            Field::Place,
            "Place of supply",
            220.0,
            "code or name",
        );
        labelled(ui, &s, &mut self.draft.place_text, want, ev);
        let account = kind.account_label();
        let s = spec("account", Field::Account, account, 320.0, "type to search");
        labelled(ui, &s, &mut self.draft.account_text, want, ev);
        if self.focus == Field::Account {
            draw_picker(ui, labels, self.pick_index, ev);
        }
        let reference = kind.reference_label();
        let s = spec("ref", Field::Reference, reference, 220.0, "Optional");
        labelled(ui, &s, &mut self.draft.reference_text, want, ev);
    }

    fn draw_rows(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let (focus, pick) = (self.focus, self.pick_index);
        let summaries: Vec<String> = (0..self.draft.rows.len())
            .map(|i| self.row_summary(masters, i))
            .collect();
        for (i, row) in self.draft.rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let cells: [(&str, Field, f32, &'static str); 5] = [
                    ("item", Field::Item(i), 260.0, "Item: type to search"),
                    ("qty", Field::Qty(i), 70.0, "Qty"),
                    ("rate", Field::Rate(i), 90.0, "Rate"),
                    ("disc", Field::Discount(i), 60.0, "Disc %"),
                    ("amt", Field::Amount(i), 110.0, "Amount (optional)"),
                ];
                for (name, field, width, hint) in cells {
                    let text = match field {
                        Field::Item(_) => &mut row.item_text,
                        Field::Qty(_) => &mut row.qty_text,
                        Field::Rate(_) => &mut row.rate_text,
                        Field::Discount(_) => &mut row.discount_text,
                        Field::Amount(_) => &mut row.amount_text,
                        Field::Date
                        | Field::Number
                        | Field::Party
                        | Field::Place
                        | Field::Account
                        | Field::Reference
                        | Field::Narration => continue,
                    };
                    let spec = FieldSpec {
                        id: base.with((name, i)),
                        field,
                        label: "",
                        width,
                        hint,
                    };
                    text_field(ui, &spec, text, want, ev);
                }
                ui.weak(summaries.get(i).map_or("", String::as_str));
            });
            if focus == Field::Item(i) {
                draw_picker(ui, labels, pick, ev);
            }
        }
    }

    fn draw_gst(&self, ui: &mut Ui, masters: &Masters) {
        ui.strong("Tax (computed by the engine)");
        match self.gst_status(masters) {
            GstStatus::Incomplete(why) => {
                ui.weak(format!("Complete the invoice to see tax ({why})."));
            }
            GstStatus::NotComputedYet => {
                ui.weak("Tax is not computed yet for this invoice.");
            }
            GstStatus::Unsupported(u) => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("Warning: not supported yet: {} ({})", u.what, u.reason),
                );
            }
            GstStatus::Failed(m) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {m}"));
            }
            GstStatus::Computed(block) => draw_block(ui, block),
        }
    }

    fn draw_footer(&mut self, ui: &mut Ui, base: Id, masters: &Masters, ev: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            label: "Narration",
            width: 480.0,
            hint: "Optional",
        };
        labelled(ui, &spec, &mut self.draft.narration, want, ev);
        match &self.notice {
            Some(Notice::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {s}"));
            }
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui.button("Save (Ctrl+A)").clicked() {
                self.save(masters);
            }
            if ui.button("Discard (Esc)").clicked() {
                ev.push(UiEvent::Leave);
            }
        });
    }

    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let base = ui.scope_id().with("item-invoice");
        let escape_wanted = self.confirm_discard || self.searching() || !self.draft.is_blank();
        let mut labels = self.pick_labels(masters);
        let keys = read_keys(ui, !labels.is_empty(), escape_wanted, self.confirm_discard);
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if matches!(
                self.handle_key(masters, *cmd, labels.len()),
                ScreenAction::Pop
            ) {
                action = ScreenAction::Pop;
            }
        }
        if !keys.is_empty() {
            labels = self.pick_labels(masters);
        }
        ui.heading(self.title());
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this invoice? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let enabled = !self.confirm_discard;
        ui.add_enabled_ui(enabled, |ui| {
            self.draw_header(ui, base, &labels, &mut events);
            ui.separator();
            self.draw_rows(ui, base, masters, &labels, &mut events);
            ui.separator();
            self.draw_gst(ui, masters);
            ui.separator();
            self.draw_footer(ui, base, masters, &mut events);
        });
        self.focus_pending = false;
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
        }
        if !keys.is_empty() || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}

impl Screen for ItemInvoiceScreen {
    fn title(&self) -> String {
        format!("{} invoice", self.type_name)
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
        } else {
            ui.label("No company is open.");
            ScreenAction::Stay
        }
    }
}
