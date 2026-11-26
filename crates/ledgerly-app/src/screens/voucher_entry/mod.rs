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
mod bills;
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
pub use draft::{
    Balance, BillDraft, DraftError, DraftRow, LedgerRef, MODES, UNASSIGNED_VOUCHER_ID,
    VoucherDraft, mode_label, parse_mode,
};
pub use kind::EntryKind;
pub(crate) use post::{created_ledger, new_ledger};
pub use search::{LedgerHit, PickerItem, search_ledgers};
pub(crate) use state::CreateForm;
pub use state::{CreateLedgerRequest, Field, OpenError, VoucherEntryScreen};

use crate::i18n::Tr;
use egui::Ui;

use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::screens::entry_print::PrintKey;
use crate::{AppCtx, Screen, ScreenAction};
use state::Notice;

impl Screen for VoucherEntryScreen {
    fn title(&self, tr: &Tr) -> String {
        self.shown_type_name(tr)
    }

    fn page_title(&self, tr: &Tr) -> String {
        match self.entry_print.page_title() {
            Some(title) => title.to_owned(),
            None => self.shown_type_name(tr),
        }
    }

    /// The sidebar row of the voucher type stays selected, as the mock marks the open screen's
    /// row (index.html line 904): Payment and Receipt have rows; Contra and Journal have none.
    fn sidebar_title(&self, tr: &Tr) -> String {
        self.shown_type_name(tr)
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        if let Some(saved) = self.entry_print.subtitle() {
            return Some(saved.to_owned());
        }
        let (action, text) = match self.draft.kind {
            EntryKind::Payment => (
                Action::NewPayment,
                tr.lit("single entry: pick the cash or bank account once, then the ledgers"),
            ),
            EntryKind::Receipt => (
                Action::NewReceipt,
                tr.lit("single entry: pick the cash or bank account once, then the ledgers"),
            ),
            // The mock's contra subtitle (index.html:2075) says "pick the account once", which is
            // not how this screen works (each line has its own account), so only its first half
            // is kept.
            EntryKind::Contra => (
                Action::NewContra,
                tr.lit("cash and bank transfers \u{b7} every line has its own Dr or Cr"),
            ),
            EntryKind::Journal => (
                Action::NewJournal,
                tr.lit("debit and credit lines, no cash or bank account set once \u{b7} the entry must balance before it saves"),
            ),
            EntryKind::Memorandum | EntryKind::ReversingJournal => return None,
        };
        Some(match self.keymap.shortcut_label(action) {
            Some(key) => format!("{key} · {text}"),
            None => text,
        })
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        if self.entry_print.preview().is_some() {
            // index.html:2272: the preview's key bar.
            return vec![
                KeyHint::new("Ctrl+P", tr.lit("Save as PDF")),
                KeyHint::new("Esc", tr.lit("Close")),
            ];
        }
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
        if self.draw_print(ui, ctx) {
            return ScreenAction::Stay;
        }
        self.poll_ledger_create(ui);
        // A save that is out is polled first; the sheet is drawn either way, taking no input
        // while it waits.
        self.wait_for_post(ui);
        self.drive_extras(ui, ctx);
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
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

impl VoucherEntryScreen {
    /// The voucher type's name for the title and the sidebar: a type still called by its built-in
    /// English name is shown in the language of `tr`; a name the user chose is shown as typed.
    fn shown_type_name(&self, tr: &Tr) -> String {
        if self.type_name == self.draft.kind.label_in(&Tr::default()) {
            self.draft.kind.label_in(tr)
        } else {
            self.type_name.clone()
        }
    }

    /// Ctrl+P and the print preview. `true` while the preview (or its busy note) takes the
    /// screen, so the entry sheet is not drawn and takes no key.
    fn draw_print(&mut self, ui: &mut Ui, ctx: &AppCtx) -> bool {
        if let Some(note) = self.entry_print.tick(ctx) {
            self.notice = Some(match note {
                crate::screens::report_view::Notice::Info(s) => Notice::Info(s),
                crate::screens::report_view::Notice::Error(s) => Notice::Error(s),
            });
        }
        let busy = self.posting.is_some() || self.creating.is_some() || self.create.is_some();
        if !busy {
            let title = self.shown_type_name(&ctx.tr);
            match self
                .entry_print
                .on_ctrl_p(ui, ctx, &title, !self.draft.is_blank())
            {
                PrintKey::None | PrintKey::Started => {}
                PrintKey::Said(text) => self.notice = Some(Notice::Info(text)),
            }
        }
        self.entry_print.show(ui)
    }

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
        let open = self.open_bills_request();
        let open_busy = self.extras.drive_open_bills(worker, open);
        self.offer_bills();
        if number_busy || balance_busy || open_busy {
            ui.ctx().request_repaint();
        }
    }

    /// The ledger and date the open bills are asked for: the line whose bill-wise details are
    /// open.
    fn open_bills_request(&self) -> Option<(ledgerly_core::LedgerId, ledgerly_core::Date)> {
        let panel = self.panel?;
        let LedgerRef::Chosen(ledger) = self.draft.rows.get(panel.row)?.ledger.as_ref()? else {
            return None;
        };
        let date = parse_entry_date(&self.draft.date_text, self.fy).ok()?;
        Some((*ledger, date))
    }

    /// The account and the voucher the balance box asks about: for a single-entry voucher the
    /// account line, for a contra the credited account (the cash or bank the money leaves), once
    /// the lines build.
    pub(super) fn balance_request(
        &self,
        masters: &ledgerly_model::Masters,
    ) -> Option<(ledgerly_core::LedgerId, ledgerly_model::Voucher)> {
        let account_row = if self.draft.is_single_entry() {
            self.draft.rows.first()?
        } else if self.draft.kind == EntryKind::Contra {
            self.draft
                .rows
                .iter()
                .find(|r| r.side == ledgerly_core::DrCr::Cr && r.ledger.is_some())?
        } else {
            return None;
        };
        let account = match account_row.ledger {
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
fn no_books_text(ctx: &AppCtx, tr: &Tr) -> String {
    if ctx.api.is_some() {
        tr.t("voucher.no_masters")
    } else {
        tr.t("voucher.no_company")
    }
}
