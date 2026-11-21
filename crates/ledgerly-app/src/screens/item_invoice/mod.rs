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
//!   A typed amount that is a plain number with more than two decimals is refused
//!   (`draft::typed_amount`). A typed expression such as `10/3` is still rounded half up to paise
//!   by `eval_amount` in `voucher_entry.rs`; that rounding is an open item, not refused here.
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
//!
//! # Keys are fixed here (deliberate exception)
//!
//! Ctrl+A and Ctrl+S (save), Enter, Esc, the arrow keys and the Y and N of the discard prompt are
//! hard-coded in the `keys` and `draw` submodules, not read from the keymap. The keymap (card U2, `keymap.rs`)
//! is a frozen stub in this build with no lookup this screen could call, and the other screens
//! read their keys the same way. When the keymap is merged the one key table in `keys` is the
//! place to switch over; until then remapping a key in the keymap does not change this screen.

mod compute;
mod draft;
mod draw;
mod keys;
mod nav;
mod picker;

use self::picker::ledger_hits;
use crate::screens::voucher_entry::{
    EntryDateError, LedgerHit, OpenError, format_entry_date, parse_entry_date,
};
use crate::shell::submit::CommandReply;
use crate::ui::format;
use crate::worker::Ticket;
use ledgerly_core::format::amount_in_words;
use ledgerly_core::{
    Date, Decimal, FinancialYear, LedgerId, MoneyError, Outcome, Paise, SignedAmount, StateCode,
    Unsupported, VoucherTypeId,
};
use ledgerly_model::{
    BaseVoucherKind, Books, LedgerLine, Masters, NumberingMethod, StoredGst, Voucher,
};
use std::collections::BTreeMap;
use std::str::FromStr;
use thiserror::Error;

pub use self::draft::{InvoiceDraft, InvoiceRow};
pub use self::draw::{LineShown, tax_rows};
pub use self::picker::{ItemHit, search_items};

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
    ConfirmSave,
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
    /// Set by Enter on the narration: the screen asks "Save this invoice?" and waits for a second
    /// Enter (or Yes), so a save is never one stray key away (mock `sales-invoice.save-confirm`).
    confirming_save: bool,
    /// The period lock date the host passes in each frame; `None` when none is known.
    lock_date: Option<Date>,
    notice: Option<Notice>,
    requested: Option<Voucher>,
    answer: Option<(Voucher, GstResult)>,
    outbox: Option<Voucher>,
    /// The post the worker is running for this screen, if any. While it runs the screen shows
    /// "Saving..." and takes no keys, so a second save cannot start.
    posting: Option<Ticket<CommandReply>>,
    /// The tax computation the worker is running, with the voucher it is for.
    computing: Option<(Voucher, Ticket<compute::ComputeReply>)>,
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
            posting: None,
            computing: None,
            confirming_save: false,
            lock_date: None,
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

    /// The period lock, when the invoice is dated on or before the lock date the host passed in:
    /// the invoice date and the lock date. `None` when no lock date is known or the date is not
    /// valid text yet.
    #[must_use]
    pub(super) fn locked_on(&self) -> Option<(Date, Date)> {
        let lock = self.lock_date?;
        let date = parse_entry_date(&self.draft.date_text, self.fy).ok()?;
        (date <= lock).then_some((date, lock))
    }

    /// The voucher a save would post, or the reason it cannot be saved yet. Every check a save
    /// makes is here, so Ctrl+A and the Enter confirmation refuse the same invoices.
    fn saveable(&self, masters: &Masters) -> Result<Voucher, String> {
        let voucher = self
            .draft
            .build_voucher(masters, self.fy)
            .map_err(|e| e.to_string())?;
        if let Some((date, lock)) = self.locked_on() {
            return Err(format!(
                "the period is locked: {} is on or before the lock date {}",
                format::date(date),
                format::date(lock)
            ));
        }
        let block = match &self.answer {
            Some((for_voucher, GstResult::Computed(block))) if *for_voucher == voucher => block,
            Some((for_voucher, GstResult::Unsupported(u))) if *for_voucher == voucher => {
                return Err(format!("Not supported yet: {} ({})", u.what, u.reason));
            }
            Some((for_voucher, GstResult::Failed(m))) if *for_voucher == voucher => {
                return Err(format!("The tax engine failed: {m}"));
            }
            Some(_) | None => {
                return Err("Tax has not been computed for this invoice yet.".to_owned());
            }
        };
        let mut final_voucher = voucher;
        final_voucher.ledger_lines.clone_from(&block.ledger_lines);
        final_voucher.gst = Some(block.stored.clone());
        Ok(final_voucher)
    }

    /// Ctrl+A: queues the voucher for the host when the engine's answer for this exact invoice is
    /// a computed block and the period is open; otherwise says why not. Saves at once: the
    /// Enter-on-narration path asks first through [`Self::ask_save`].
    pub fn save(&mut self, masters: &Masters) {
        self.confirming_save = false;
        match self.saveable(masters) {
            Ok(voucher) => {
                self.outbox = Some(voucher);
                self.notice = Some(Notice::Info("Saving...".to_owned()));
            }
            Err(message) => self.fail(message),
        }
    }

    /// Enter on the narration: when the invoice could be saved, asks "Save this invoice?" and
    /// waits for Enter or Yes; when it could not, says why, exactly as [`Self::save`] does.
    pub fn ask_save(&mut self, masters: &Masters) {
        match self.saveable(masters) {
            Ok(_) => {
                self.notice = None;
                self.confirming_save = true;
            }
            Err(message) => self.fail(message),
        }
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
        self.confirming_save = false;
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
