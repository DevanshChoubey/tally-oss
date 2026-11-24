//! Accounting voucher entry. **Card U4** implements this module.
//!
//! Frozen interface (plan card U4):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `VoucherEntry`. The screen enters the vouchers that are plain debit and credit
//! lines: Contra, Payment, Receipt, Journal, Memorandum and Reversing journal. Other voucher kinds
//! answer [`Outcome::Unsupported`](ledgerly_core::Outcome::Unsupported) from [`VoucherEntryScreen::open`] (sales, purchase and notes
//! belong to the invoice screen, card U5; stock and order kinds hold no accounting lines).
//!
//! What spec 7 asks of an entry form, and where it lives:
//!
//! * Enter advances, Shift+Enter goes back ([`VoucherEntryScreen::submit`],
//!   [`VoucherEntryScreen::step_back`]).
//! * Type-ahead with fuzzy and alias matching, recents first, balance and GSTIN shown, and a
//!   "create new" item ([`search_ledgers`], [`PickerItem`]).
//! * Inline arithmetic such as `1200*18%` ([`eval_amount`]).
//! * The balancing line is prefilled ([`VoucherDraft::push_prefilled_row`]).
//! * Alt+C creates a ledger inline ([`VoucherEntryScreen::begin_create`]). The screen never writes
//!   to the store: it records a [`CreateLedgerRequest`] for the host and keeps the line
//!   [`LedgerRef::Pending`] until the host calls [`VoucherEntryScreen::resolve_created_ledger`].
//! * A fresh voucher after save ([`VoucherEntryScreen::confirm_saved`]).
//!
//! Rule C4 (the UI never sums anything): the only arithmetic here is on what the user typed. The
//! difference shown under the lines and the prefilled balancing line are an entry aid; the engine
//! recomputes and checks every voucher when it posts it (spec 5 invariant 1, spec 6.1). Ledger
//! balances shown in the type-ahead are handed in by the host through
//! [`VoucherEntryScreen::set_balances`]; a ledger without one shows no balance, never zero.
mod amount;
mod create;
mod date;
mod display;
mod draft;
mod draw;
mod keys;
mod kind;
mod post;
mod save;
mod search;
mod state;

pub use amount::{AmountError, eval_amount};
pub(crate) use create::{FormAnswer, check_new_ledger, create_form_ui};
pub use date::{EntryDateError, format_entry_date, parse_entry_date};
pub use draft::{Balance, DraftError, DraftRow, LedgerRef, UNASSIGNED_VOUCHER_ID, VoucherDraft};
pub use kind::EntryKind;
pub(crate) use post::{created_ledger, new_ledger};
pub use search::{LedgerHit, PickerItem, search_ledgers};
pub(crate) use state::CreateForm;
pub use state::{CreateLedgerRequest, Field, OpenError, VoucherEntryScreen};

use egui::Ui;

use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::{AppCtx, Screen, ScreenAction};

impl Screen for VoucherEntryScreen {
    fn title(&self) -> String {
        self.type_name.clone()
    }

    fn page_title(&self) -> String {
        self.type_name.clone()
    }

    /// The sidebar row of the voucher type stays selected, as the mock marks the open screen's
    /// row (index.html line 904): Payment and Receipt have rows; Contra and Journal have none.
    fn sidebar_title(&self) -> String {
        self.type_name.clone()
    }

    fn subtitle(&self) -> Option<String> {
        let (action, text) = match self.draft.kind {
            EntryKind::Payment => (
                Action::NewPayment,
                "single entry: pick the cash or bank account once, then the ledgers",
            ),
            EntryKind::Receipt => (
                Action::NewReceipt,
                "single entry: pick the cash or bank account once, then the ledgers",
            ),
            EntryKind::Contra => (
                Action::NewContra,
                "double entry: every line has its own Dr or Cr",
            ),
            EntryKind::Journal => (
                Action::NewJournal,
                "double entry: every line has its own Dr or Cr",
            ),
            EntryKind::Memorandum | EntryKind::ReversingJournal => return None,
        };
        Some(match self.keymap.shortcut_label(action) {
            Some(key) => format!("{key} · {text}"),
            None => text.to_owned(),
        })
    }

    fn keys(&self) -> Vec<KeyHint> {
        let mut hints = vec![KeyHint::new("Enter", "Next field")];
        let bound = [
            (Action::CreateMaster, "Create ledger"),
            (Action::DeleteLine, "Delete line"),
            (Action::Save, "Save"),
            (Action::Back, "Discard"),
        ];
        for (action, label) in bound {
            if let Some(keys) = self.keymap.shortcut_label(action) {
                hints.push(KeyHint::new(keys, label));
            }
        }
        hints
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.poll_ledger_create(ui);
        // A save that is out is polled first; the sheet is drawn either way, taking no input
        // while it waits.
        self.wait_for_post(ui);
        self.drive_extras(ui, ctx);
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
        } else {
            ui.label(no_books_text(ctx));
            ScreenAction::Stay
        };
        if ctx.api.is_some() {
            self.send_saves(ctx);
            self.send_ledger_creates(ctx);
        }
        action
    }
}

impl VoucherEntryScreen {
    /// Asks the engine for the number the voucher will get and, for a single-entry voucher, the
    /// account's balance now and after it. The screen shows the answers and adds nothing to them.
    fn drive_extras(&mut self, ui: &Ui, ctx: &AppCtx) {
        let (Some(worker), Some(books)) = (ctx.api.as_ref(), ctx.books.as_ref()) else {
            return;
        };
        let date = parse_entry_date(&self.draft.date_text, self.fy).ok();
        let number_busy = self
            .extras
            .drive_number(worker, date.map(|d| (self.draft.type_id, d)));
        let want = self.balance_request(&books.masters);
        let balance_busy = self.extras.drive_balance(worker, want);
        if number_busy || balance_busy {
            ui.ctx().request_repaint();
        }
    }

    /// The account and the voucher the balance box asks about: a single-entry voucher whose
    /// account is picked and whose lines build.
    pub(super) fn balance_request(
        &self,
        masters: &ledgerly_model::Masters,
    ) -> Option<(ledgerly_core::LedgerId, ledgerly_model::Voucher)> {
        if !self.draft.is_single_entry() {
            return None;
        }
        let account = match self.draft.rows.first()?.ledger {
            Some(LedgerRef::Chosen(id)) => id,
            Some(LedgerRef::Pending(_)) | None => return None,
        };
        match self
            .draft
            .build_voucher(masters, self.fy, self.books_begin)
            .ok()?
        {
            ledgerly_core::Outcome::Value(v) => Some((account, v)),
            ledgerly_core::Outcome::Unsupported(_) => None,
        }
    }
}

/// What the screen says when it has no books to draw from.
fn no_books_text(ctx: &AppCtx) -> &'static str {
    if ctx.api.is_some() {
        "The company is open. Its masters are being read, or could not be read."
    } else {
        "No company is open."
    }
}
