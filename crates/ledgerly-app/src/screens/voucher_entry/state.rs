//! The entry screen's state: the draft, the focus and the editing keys' effects.

use std::collections::{BTreeMap, VecDeque};

use ledgerly_core::{Date, FinancialYear, GroupId, LedgerId, Outcome, SignedAmount, VoucherTypeId};
use ledgerly_model::{Books, Masters, Voucher};
use thiserror::Error;

use crate::shell::submit::CommandReply;
use crate::worker::Ticket;

use super::date::{format_entry_date, parse_entry_date};
use super::draft::{Balance, DraftRow, LedgerRef, VoucherDraft, entry_support};
use super::kind::EntryKind;
use super::search::{PickerItem, search_ledgers};

/// Which field has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The date.
    Date,
    /// The ledger of line `0` (0-based).
    Ledger(usize),
    /// The amount of line `0` (0-based).
    Amount(usize),
    /// The narration.
    Narration,
}

/// A ledger the user asked to create with Alt+C, for the host to create through the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLedgerRequest {
    /// The new ledger's name.
    pub name: String,
    /// The group to create it under.
    pub group: GroupId,
}

/// Why a screen could not be opened.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OpenError {
    /// The voucher type is not in the masters.
    #[error("voucher type {0:?} does not exist")]
    UnknownVoucherType(VoucherTypeId),
    /// The voucher type is switched off.
    #[error("voucher type '{0}' is not active; switch it on in its master first")]
    Inactive(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Notice {
    Error(String),
    Info(String),
}

pub(super) struct CreateForm {
    pub(super) name: String,
    pub(super) group: Option<GroupId>,
}

/// The accounting voucher entry screen (card U4).
pub struct VoucherEntryScreen {
    pub(super) type_name: String,
    pub(super) fy: FinancialYear,
    pub(super) books_begin: Date,
    pub(super) draft: VoucherDraft,
    pub(super) balances: BTreeMap<LedgerId, SignedAmount>,
    pub(super) recents: Vec<LedgerId>,
    pub(super) focus: Field,
    pub(super) focus_pending: bool,
    pub(super) pick_index: usize,
    pub(super) create: Option<CreateForm>,
    pub(super) confirm_discard: bool,
    pub(super) notice: Option<Notice>,
    pub(super) outbox: Option<Voucher>,
    pub(super) master_requests: Vec<CreateLedgerRequest>,
    /// The post the worker is running for this screen, if any. While it runs the screen shows
    /// "Saving..." and takes no keys, so a second save cannot start.
    pub(super) posting: Option<Ticket<CommandReply>>,
    /// Ledgers asked for with Alt+C that are waiting for the worker, oldest first.
    pub(super) create_queue: VecDeque<CreateLedgerRequest>,
    /// The ledger create the worker is running: the name asked for, and the ticket.
    pub(super) creating: Option<(String, Ticket<CommandReply>)>,
}
impl VoucherEntryScreen {
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
        let voucher_type = books
            .masters
            .voucher_types
            .get(&type_id)
            .ok_or(OpenError::UnknownVoucherType(type_id))?;
        if !voucher_type.active {
            return Err(OpenError::Inactive(voucher_type.name.clone()));
        }
        let kind = match EntryKind::from_base(voucher_type.base) {
            Outcome::Value(kind) => kind,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(Outcome::Value(VoucherEntryScreen {
            type_name: voucher_type.name.clone(),
            fy,
            books_begin: books.company.books_begin,
            draft: VoucherDraft::new(type_id, kind, format_entry_date(today)),
            balances: BTreeMap::new(),
            recents: Vec::new(),
            focus: Field::Ledger(0),
            focus_pending: true,
            pick_index: 0,
            create: None,
            confirm_discard: false,
            notice: None,
            outbox: None,
            master_requests: Vec::new(),
            posting: None,
            create_queue: VecDeque::new(),
            creating: None,
        }))
    }

    /// The voucher being entered.
    #[must_use]
    pub fn draft(&self) -> &VoucherDraft {
        &self.draft
    }

    /// The voucher being entered, for the host to prefill (for example from an AI proposal).
    pub fn draft_mut(&mut self) -> &mut VoucherDraft {
        &mut self.draft
    }

    /// The field that has the keyboard.
    #[must_use]
    pub fn focus(&self) -> Field {
        self.focus
    }

    /// The current error message, if any.
    #[must_use]
    pub fn error_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Error(s)) => Some(s),
            Some(Notice::Info(_)) | None => None,
        }
    }

    /// The current information message, if any.
    #[must_use]
    pub fn info_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Info(s)) => Some(s),
            Some(Notice::Error(_)) | None => None,
        }
    }

    /// Ledger balances to show in the type-ahead, from the engine through the API.
    pub fn set_balances(&mut self, balances: BTreeMap<LedgerId, SignedAmount>) {
        self.balances = balances;
    }

    /// Recently used ledgers, most recent first.
    pub fn set_recents(&mut self, recents: Vec<LedgerId>) {
        self.recents = recents;
    }

    pub(super) fn go(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
    }

    pub(super) fn fail(&mut self, message: impl Into<String>) {
        self.notice = Some(Notice::Error(message.into()));
    }

    pub(super) fn remember(&mut self, id: LedgerId) {
        self.recents.retain(|r| *r != id);
        self.recents.insert(0, id);
        self.recents.truncate(RECENTS_LIMIT);
    }

    /// The type-ahead list for the ledger field that has the keyboard (empty when another field
    /// has it, or its ledger is already picked).
    #[must_use]
    pub fn picker_items(&self, masters: &Masters) -> Vec<PickerItem> {
        let Field::Ledger(i) = self.focus else {
            return Vec::new();
        };
        match self.draft.rows.get(i) {
            Some(row) if row.ledger.is_none() => {
                search_ledgers(masters, &row.ledger_text, &self.recents, &self.balances)
            }
            Some(_) | None => Vec::new(),
        }
    }

    /// The highlighted line of the type-ahead list (0-based).
    #[must_use]
    pub fn pick_index(&self) -> usize {
        self.pick_index
    }

    /// Moves the highlight in the type-ahead list by `delta`, staying inside it.
    pub fn move_pick(&mut self, masters: &Masters, delta: isize) {
        let len = self.picker_items(masters).len();
        self.shift_pick(len, delta);
    }

    /// Moves the highlight by `delta` in a list of `len` items, staying inside it.
    pub(super) fn shift_pick(&mut self, len: usize, delta: isize) {
        let next = self.pick_index.saturating_add_signed(delta);
        self.pick_index = next.min(len.saturating_sub(1));
    }

    /// Types `text` into the ledger field of line `row` (0-based); a ledger already picked there
    /// is dropped, as the user is searching again.
    pub fn set_ledger_text(&mut self, row: usize, text: &str) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            text.clone_into(&mut r.ledger_text);
            r.ledger = None;
            self.pick_index = 0;
        }
    }

    /// Types `text` into the amount field of line `row` (0-based).
    pub fn set_amount_text(&mut self, row: usize, text: &str) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            text.clone_into(&mut r.amount_text);
        }
    }

    /// Enter: accepts the field that has the keyboard and moves on.
    pub fn submit(&mut self, masters: &Masters) {
        self.notice = None;
        match self.focus {
            Field::Date => self.submit_date(),
            Field::Ledger(i) => self.submit_ledger(masters, i),
            Field::Amount(i) => self.submit_amount(i),
            Field::Narration => self.save(masters),
        }
    }

    /// Shift+Enter: back to the previous field.
    pub fn step_back(&mut self) {
        let previous = match self.focus {
            Field::Date | Field::Ledger(0) => Field::Date,
            Field::Ledger(i) => Field::Amount(i.saturating_sub(1)),
            Field::Amount(i) => Field::Ledger(i),
            Field::Narration => Field::Amount(self.draft.rows.len().saturating_sub(1)),
        };
        self.go(previous);
    }

    pub(super) fn submit_date(&mut self) {
        match parse_entry_date(&self.draft.date_text, self.fy) {
            Ok(date) => {
                self.draft.date_text = format_entry_date(date);
                self.go(Field::Ledger(0));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    pub(super) fn submit_ledger(&mut self, masters: &Masters, i: usize) {
        let Some(row) = self.draft.rows.get(i) else {
            return;
        };
        if row.ledger.is_some() {
            self.go(Field::Amount(i));
            return;
        }
        if row.ledger_text.trim().is_empty() {
            self.end_of_list(i);
            return;
        }
        match self.picker_items(masters).get(self.pick_index).cloned() {
            Some(PickerItem::Ledger(hit)) => {
                if let Some(Outcome::Unsupported(u)) =
                    masters.ledgers.get(&hit.id).map(entry_support)
                {
                    return self.fail(format!("Not supported yet: {} ({})", u.what, u.reason));
                }
                if let Some(r) = self.draft.rows.get_mut(i) {
                    r.ledger = Some(LedgerRef::Chosen(hit.id));
                    r.ledger_text = hit.name;
                }
                self.remember(hit.id);
                self.go(Field::Amount(i));
            }
            Some(PickerItem::Create { name }) => {
                self.create = Some(CreateForm { name, group: None });
            }
            None => self.fail("No ledger matches; press Alt+C to create one."),
        }
    }

    /// Enter on an empty ledger field: ends the list when the lines balance.
    pub(super) fn end_of_list(&mut self, i: usize) {
        let row_empty = self
            .draft
            .rows
            .get(i)
            .is_some_and(|r| r.amount_text.trim().is_empty());
        match self.draft.balance() {
            Ok(Balance::Balanced) if i > 0 && row_empty => {
                self.draft.rows.truncate(i);
                self.go(Field::Narration);
            }
            Ok(Balance::Balanced | Balance::Needs { .. }) => {
                self.fail("Pick a ledger: type part of its name or alias.");
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    pub(super) fn submit_amount(&mut self, i: usize) {
        let amount = match self.draft.line_amount(i) {
            Ok(Some(amount)) => amount,
            Ok(None) => return self.fail("Type an amount."),
            Err(e) => return self.fail(e.to_string()),
        };
        if amount.is_zero() {
            return self.fail("An amount of zero cannot be posted.");
        }
        if let Some(r) = self.draft.rows.get_mut(i) {
            r.amount_text = amount.to_rupees().to_string();
        }
        let next = i.saturating_add(1);
        if next < self.draft.rows.len() {
            return self.go(Field::Ledger(next));
        }
        match self.draft.balance() {
            Ok(Balance::Balanced) => self.go(Field::Narration),
            Ok(Balance::Needs { .. }) => match self.draft.push_prefilled_row() {
                Ok(()) => self.go(Field::Ledger(next)),
                Err(e) => self.fail(e.to_string()),
            },
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// Switches line `row` between Dr and Cr.
    pub fn toggle_side(&mut self, row: usize) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            r.side = r.side.opposite();
        }
    }

    /// Deletes the line that has the keyboard (the only line is emptied instead). Bound to a
    /// provisional key, see `bindings::DELETE_LINE`.
    pub fn delete_line(&mut self) {
        let (Field::Ledger(i) | Field::Amount(i)) = self.focus else {
            return;
        };
        if self.draft.rows.len() <= 1 {
            self.draft.rows = vec![DraftRow::empty(self.draft.kind.first_side())];
            return self.go(Field::Ledger(0));
        }
        if i < self.draft.rows.len() {
            self.draft.rows.remove(i);
        }
        self.go(Field::Ledger(
            i.min(self.draft.rows.len().saturating_sub(1)),
        ));
    }
}
