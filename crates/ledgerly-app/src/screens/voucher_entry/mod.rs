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

use crate::i18n::Tr;
use egui::Ui;

use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::{AppCtx, Screen, ScreenAction};

impl Screen for VoucherEntryScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.tf("voucher.title", &[("kind", &self.draft.kind.label_in(tr))])
    }

    fn page_title(&self, tr: &Tr) -> String {
        self.draft.kind.label_in(tr)
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        let (action, text) = match self.draft.kind {
            EntryKind::Payment => (Action::NewPayment, tr.t("voucher.sub.single")),
            EntryKind::Receipt => (Action::NewReceipt, tr.t("voucher.sub.single")),
            EntryKind::Contra => (Action::NewContra, tr.t("voucher.sub.double")),
            EntryKind::Journal => (Action::NewJournal, tr.t("voucher.sub.double")),
            EntryKind::Memorandum | EntryKind::ReversingJournal => return None,
        };
        Some(match self.keymap.shortcut_label(action) {
            Some(key) => format!("{key} · {text}"),
            None => text,
        })
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        let mut hints = vec![KeyHint::new("Enter", tr.t("voucher.key.next_field"))];
        let bound = [
            (Action::CreateMaster, tr.t("voucher.key.create_ledger")),
            (Action::DeleteLine, tr.t("voucher.key.delete_line")),
            (Action::Save, tr.t("voucher.save")),
            (Action::Back, tr.t("voucher.discard")),
        ];
        for (action, label) in bound {
            if let Some(keys) = self.keymap.shortcut_label(action) {
                hints.push(KeyHint::new(keys, label));
            }
        }
        hints
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tr = ctx.tr.clone();
        self.poll_ledger_create(ui);
        if self.posting.is_some() {
            self.wait_for_post(ui);
            return ScreenAction::Stay;
        }
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters, &ctx.tr)
        } else {
            ui.label(no_books_text(ctx, &ctx.tr));
            ScreenAction::Stay
        };
        if ctx.api.is_some() {
            self.send_saves(ctx);
            self.send_ledger_creates(ctx);
        }
        action
    }
}

/// What the screen says when it has no books to draw from.
fn no_books_text(ctx: &AppCtx, tr: &Tr) -> String {
    if ctx.api.is_some() {
        tr.t("voucher.no_masters")
    } else {
        tr.t("voucher.no_company")
    }
}
