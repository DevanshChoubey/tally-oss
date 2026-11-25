//! The entry screen's state: the draft, the focus and the editing keys' effects.

use std::collections::{BTreeMap, VecDeque};

use ledgerly_core::{Date, FinancialYear, GroupId, LedgerId, Outcome, SignedAmount, VoucherTypeId};
use ledgerly_model::{Books, Masters, Voucher};
use thiserror::Error;

use crate::keymap::{self, Keymap};
use crate::screens::item_invoice::extras::Extras;
use crate::shell::submit::CommandReply;
use crate::worker::Ticket;

use super::amount::eval_amount;
use super::date::{format_entry_date, parse_entry_date};
use super::draft::{
    Balance, BillDraft, DraftRow, LedgerRef, VoucherDraft, entry_support, mode_label, parse_mode,
};
use super::kind::EntryKind;
use super::search::{PickerItem, search_ledgers};

/// How many recently used ledgers are remembered.
const RECENTS_LIMIT: usize = 20;

/// Which field has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The date.
    Date,
    /// The ledger of line `0` (0-based).
    Ledger(usize),
    /// The amount of line `0` (0-based).
    Amount(usize),
    /// The mode of a single-entry voucher (NEFT, UPI, ...), after a bank account.
    Mode,
    /// The amount settled against open bill `0` (0-based) in the bill-wise details.
    Bill(usize),
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

/// The create-ledger form Alt+C opens: the name and the group typed so far.
pub(crate) struct CreateForm {
    /// The new ledger's name.
    pub(crate) name: String,
    /// The group to create it under, once chosen.
    pub(crate) group: Option<GroupId>,
}

/// The two notes of a contra with no account and no amount (index.html:2150).
pub(super) const CONTRA_EMPTY: &str =
    "Pick the cash or bank account first.\nAn amount of zero cannot be posted.";

/// The accounting voucher entry screen (card U4).
pub struct VoucherEntryScreen {
    pub(super) type_name: String,
    /// The keymap the screen reads its keys from (the classic preset until the shell passes its own).
    pub(super) keymap: Keymap,
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
    /// Where parts of the screen were drawn in the last frame, for layout tests.
    pub(super) shown: super::draw::Shown,
    /// The number the voucher will get and the account's balance after it, from the engine.
    pub(super) extras: Extras,
    /// The line whose bill-wise details are open, and whether they were filled in already.
    pub(super) panel: Option<BillPanel>,
    /// Whether the account is a bank account, so Enter on it goes to the mode.
    pub(super) mode_stop: bool,
}

/// The bill-wise details that are open: the line they belong to and whether the open bills have
/// been offered as a default yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BillPanel {
    /// The line (0-based).
    pub(super) row: usize,
    /// Whether the first open bill was given the line's amount (once the engine answered).
    pub(super) offered: bool,
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
            keymap: keymap::classic(),
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
            shown: super::draw::Shown::default(),
            extras: Extras::default(),
            panel: None,
            mode_stop: false,
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

    /// Where the field captioned `name` was drawn in the last frame.
    #[must_use]
    pub fn field_rect(&self, name: &str) -> Option<egui::Rect> {
        self.shown
            .fields
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, r)| *r)
    }

    /// The line table's headings and where each was drawn in the last frame, left to right.
    #[must_use]
    pub fn heading_rects(&self) -> Vec<(&'static str, egui::Rect)> {
        self.shown.heads.clone()
    }

    /// Where the open type-ahead list was drawn in the last frame; `None` when none was open.
    #[must_use]
    pub fn picker_rect(&self) -> Option<egui::Rect> {
        self.shown.picker
    }

    /// Whether the contra was just refused for having no account and no amount, which the screen
    /// draws with both lines listed and what each needs (index.html `SCREENS.contra`,
    /// validation-error).
    #[must_use]
    pub(super) fn contra_refused(&self) -> bool {
        self.draft.kind == EntryKind::Contra && self.error_text() == Some(CONTRA_EMPTY)
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

    /// The field that follows the ledger of line `i`: the amount, or for the single-entry account
    /// (line 0) the first particulars ledger, since the account has no amount of its own.
    pub(super) fn after_ledger(&self, i: usize) -> Field {
        if self.draft.is_single_entry() && i == 0 {
            Field::Ledger(1)
        } else {
            Field::Amount(i)
        }
    }

    /// As [`Self::after_ledger`], but a single-entry account that is a bank account is followed by
    /// its mode, and the answer is remembered for Shift+Enter.
    fn after_ledger_in(&mut self, masters: &Masters, i: usize) -> Field {
        if self.draft.is_single_entry() && i == 0 {
            self.mode_stop = match self.draft.rows.first().and_then(|r| r.ledger.as_ref()) {
                Some(LedgerRef::Chosen(id)) => {
                    masters.ledgers.get(id).is_some_and(|l| l.bank.is_some())
                }
                Some(LedgerRef::Pending(_)) | None => false,
            };
            if self.mode_stop {
                return Field::Mode;
            }
        }
        self.after_ledger(i)
    }

    /// Enter on the mode: accepts any start of an instrument's name, or nothing.
    pub(super) fn submit_mode(&mut self) {
        let text = self.draft.mode_text.trim();
        if !text.is_empty() {
            match parse_mode(text) {
                Some(kind) => mode_label(kind).clone_into(&mut self.draft.mode_text),
                None => {
                    return self.fail(format!(
                        "The mode '{text}' is none of Cheque, NEFT, RTGS, IMPS, UPI, Card or Other."
                    ));
                }
            }
        }
        self.go(Field::Ledger(1));
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

    /// Picks the mode from the chooser; the draft keeps the instrument's label.
    pub fn choose_mode(&mut self, kind: ledgerly_model::InstrumentKind) {
        mode_label(kind).clone_into(&mut self.draft.mode_text);
    }

    /// Enter: accepts the field that has the keyboard and moves on.
    pub fn submit(&mut self, masters: &Masters) {
        self.notice = None;
        match self.focus {
            Field::Date => self.submit_date(),
            Field::Ledger(i) => self.submit_ledger(masters, i),
            Field::Amount(i) => self.submit_amount(masters, i),
            Field::Mode => self.submit_mode(),
            Field::Bill(k) => self.submit_bill(k),
            Field::Narration => self.save(masters),
        }
    }

    /// Shift+Enter: back to the previous field.
    pub fn step_back(&mut self) {
        let previous = match self.focus {
            Field::Date | Field::Ledger(0) => Field::Date,
            Field::Ledger(1) if self.draft.is_single_entry() && self.mode_stop => Field::Mode,
            Field::Ledger(1) if self.draft.is_single_entry() => Field::Ledger(0),
            Field::Mode => Field::Ledger(0),
            Field::Bill(0) => self
                .panel
                .map_or(Field::Ledger(0), |p| Field::Amount(p.row)),
            Field::Bill(k) => Field::Bill(k.saturating_sub(1)),
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
            {
                let next = self.after_ledger_in(masters, i);
                self.go(next);
            }
            return;
        }
        if row.ledger_text.trim().is_empty() {
            self.end_of_list(i);
            return;
        }
        match self.picker_items(masters).get(self.pick_index).cloned() {
            Some(PickerItem::Ledger(hit)) => {
                if let Some(Outcome::Unsupported(u)) = masters
                    .ledgers
                    .get(&hit.id)
                    .map(|l| entry_support(l, self.draft.is_single_entry()))
                {
                    return self.fail(format!("Not supported yet: {} ({})", u.what, u.reason));
                }
                if let Some(r) = self.draft.rows.get_mut(i) {
                    r.ledger = Some(LedgerRef::Chosen(hit.id));
                    r.ledger_text = hit.name;
                }
                self.remember(hit.id);
                let next = self.after_ledger_in(masters, i);
                self.go(next);
            }
            Some(PickerItem::Create { name }) => {
                self.create = Some(CreateForm { name, group: None });
            }
            None => self.fail("No ledger matches; press Alt+C to create one."),
        }
    }

    /// Enter on an empty ledger field: ends the list when the lines balance.
    pub(super) fn end_of_list(&mut self, i: usize) {
        if self.draft.is_single_entry() {
            return self.end_of_single_list(i);
        }
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

    /// Enter on an empty ledger field of a single-entry voucher: the list ends after at least one
    /// particulars line. The account is never a list end; it must be picked first.
    pub(super) fn end_of_single_list(&mut self, i: usize) {
        if i == 0 {
            return self.fail("Pick the cash or bank account first.");
        }
        let row_empty = self
            .draft
            .rows
            .get(i)
            .is_some_and(|r| r.amount_text.trim().is_empty());
        if !row_empty {
            return self.fail("Pick a ledger: type part of its name or alias.");
        }
        if i == 1 {
            return self.fail("Pick the ledger that is paid or received first.");
        }
        self.draft.rows.truncate(i);
        self.go(Field::Narration);
    }

    pub(super) fn submit_amount(&mut self, masters: &Masters, i: usize) {
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
        if self.draft.is_single_entry() && self.line_tracks_bills(masters, i) {
            // The ledger tracks bills: its bill-wise details open before the next line.
            if let Some(r) = self.draft.rows.get_mut(i) {
                r.bills_done = false;
            }
            self.panel = Some(BillPanel {
                row: i,
                offered: false,
            });
            return self.go(Field::Bill(0));
        }
        self.after_amount(i);
    }

    /// What follows an accepted amount: the next line, or the narration.
    fn after_amount(&mut self, i: usize) {
        let next = i.saturating_add(1);
        if self.draft.is_single_entry() {
            // Single entry: each amount is followed by the next particulars line, which is always
            // there to be filled or finished with an empty Enter.
            if next >= self.draft.rows.len() {
                self.draft.push_particular();
            }
            return self.go(Field::Ledger(next));
        }
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

    /// Whether the ledger of line `i` tracks bills.
    pub(super) fn line_tracks_bills(&self, masters: &Masters, i: usize) -> bool {
        match self.draft.rows.get(i).and_then(|r| r.ledger.as_ref()) {
            Some(LedgerRef::Chosen(id)) => masters
                .ledgers
                .get(id)
                .is_some_and(|l| l.bill_wise.is_some()),
            Some(LedgerRef::Pending(_)) | None => false,
        }
    }

    /// The open bills offered in the bill-wise details that are open, once the engine answered.
    #[must_use]
    pub fn open_bills(&self) -> Option<&[ledgerly_api::OpenBill]> {
        let panel = self.panel?;
        let row = self.draft.rows.get(panel.row)?;
        let LedgerRef::Chosen(ledger) = row.ledger.as_ref()? else {
            return None;
        };
        let date = parse_entry_date(&self.draft.date_text, self.fy).ok()?;
        self.extras.open_bills_for(*ledger, date)
    }

    /// Hands in the open bills of `ledger` on the voucher's date, as the engine would answer.
    pub fn set_open_bills(&mut self, ledger: LedgerId, bills: Vec<ledgerly_api::OpenBill>) {
        if let Ok(date) = parse_entry_date(&self.draft.date_text, self.fy) {
            self.extras.set_open_bills(ledger, date, bills);
            self.offer_bills();
        }
    }

    /// Gives the first open bill the line's amount (or the bill's whole open amount when that is
    /// less), once, when the engine's list has arrived and nothing was typed. A default the user
    /// can overtype: the figures are the line's and the bill's own, nothing is added.
    pub(super) fn offer_bills(&mut self) {
        let Some(panel) = self.panel else {
            return;
        };
        if panel.offered {
            return;
        }
        let Some(open) = self.open_bills().map(<[_]>::to_vec) else {
            return;
        };
        let amount = self.draft.line_amount(panel.row).ok().flatten();
        let side = self.draft.rows.get(panel.row).map(|r| r.side);
        let Some(row) = self.draft.rows.get_mut(panel.row) else {
            return;
        };
        if row.bills.is_empty()
            && let (Some(first), Some(amount), Some(side)) = (open.first(), amount, side)
            && first.open.side != side
        {
            let part = first.open.amount.min(amount);
            row.bills.push(BillDraft {
                name: first.name.clone(),
                amount_text: part.to_rupees().to_string(),
            });
        }
        self.panel = Some(BillPanel {
            offered: true,
            ..panel
        });
    }

    /// Enter on an open bill's amount: the next bill, or, after the last, accepts the details.
    pub(super) fn submit_bill(&mut self, k: usize) {
        let count = self.open_bills().map_or(0, <[_]>::len);
        if k.saturating_add(1) < count {
            return self.go(Field::Bill(k.saturating_add(1)));
        }
        self.accept_bills();
    }

    /// Accepts the bill-wise details of the open line: what is settled must add up to the line's
    /// amount, or nothing is typed and the whole amount is On Account.
    pub(super) fn accept_bills(&mut self) {
        let Some(panel) = self.panel else {
            return;
        };
        let i = panel.row;
        let amount = match self.draft.line_amount(i) {
            Ok(Some(amount)) => amount,
            Ok(None) => return self.fail("Type an amount."),
            Err(e) => return self.fail(e.to_string()),
        };
        let mut settled = ledgerly_core::Paise::ZERO;
        let typed: Vec<BillDraft> = self.draft.rows.get(i).map_or_else(Vec::new, |r| {
            r.bills
                .iter()
                .filter(|b| !b.amount_text.trim().is_empty())
                .cloned()
                .collect()
        });
        for bill in &typed {
            let part = match eval_amount(&bill.amount_text) {
                Ok(part) => part,
                Err(e) => return self.fail(format!("{}: {e}", bill.name)),
            };
            if part.is_zero() {
                return self.fail(format!("{}: an amount of zero settles nothing.", bill.name));
            }
            settled = match settled.checked_add(part) {
                Ok(sum) => sum,
                Err(e) => return self.fail(e.to_string()),
            };
        }
        if !typed.is_empty() && settled != amount {
            return self.fail(format!(
                "The bills settle {} of {}. Make them add up to the line, or clear them to leave it On Account.",
                crate::ui::format::amount(settled),
                crate::ui::format::amount(amount),
            ));
        }
        if let Some(row) = self.draft.rows.get_mut(i) {
            row.bills = typed;
            row.bills_done = true;
        }
        self.panel = None;
        self.after_amount(i);
    }

    /// Esc in the bill-wise details: back to the amount, with the details not accepted.
    pub(super) fn close_bills(&mut self) {
        if let Some(panel) = self.panel.take() {
            self.go(Field::Amount(panel.row));
        }
    }

    /// Switches line `row` between Dr and Cr.
    pub fn toggle_side(&mut self, row: usize) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            r.side = r.side.opposite();
        }
    }

    /// Deletes the line that has the keyboard (the only line is emptied instead). Bound to the
    /// keymap's `delete-line` action.
    pub fn delete_line(&mut self) {
        let (Field::Ledger(i) | Field::Amount(i)) = self.focus else {
            return;
        };
        if self.draft.is_single_entry() {
            return self.delete_single_line(i);
        }
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

    /// Deletes a particulars line of a single-entry voucher. The account (line 0) stays, and one
    /// particulars line is always left to fill.
    pub(super) fn delete_single_line(&mut self, i: usize) {
        if i == 0 {
            return self.fail("The cash or bank account is set once for the whole voucher.");
        }
        if i < self.draft.rows.len() {
            self.draft.rows.remove(i);
        }
        if self.draft.rows.len() < 2 {
            self.draft.push_particular();
        }
        let last = self.draft.rows.len().saturating_sub(1);
        self.go(Field::Ledger(i.min(last)));
    }
}
