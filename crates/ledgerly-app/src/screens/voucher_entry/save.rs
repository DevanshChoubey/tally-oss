//! Saving a voucher and leaving the screen.

use ledgerly_core::Outcome;
use ledgerly_model::{Masters, Voucher};

use super::draft::VoucherDraft;
use super::state::{Field, Notice, VoucherEntryScreen};

impl VoucherEntryScreen {
    // ---- saving and leaving ----

    /// Ctrl+A / Ctrl+S: checks the draft and, when it passes, leaves the voucher for the host in
    /// [`VoucherEntryScreen::take_save_request`]. The draft stays on screen until the host calls
    /// [`VoucherEntryScreen::confirm_saved`].
    pub fn save(&mut self, masters: &Masters) {
        match self.draft.build_voucher(masters, self.fy, self.books_begin) {
            Ok(Outcome::Value(voucher)) => {
                self.outbox = Some(voucher);
                self.notice = Some(Notice::Info(
                    "Checked. Waiting for the engine to post it.".to_owned(),
                ));
            }
            Ok(Outcome::Unsupported(u)) => {
                self.fail(format!("Not supported yet: {} ({})", u.what, u.reason));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// The voucher the user asked to save, once.
    pub fn take_save_request(&mut self) -> Option<Voucher> {
        self.outbox.take()
    }

    /// The host posted the voucher: starts a fresh one, keeping type and date.
    pub fn confirm_saved(&mut self) {
        self.draft = VoucherDraft::new(
            self.draft.type_id,
            self.draft.kind,
            self.draft.date_text.clone(),
        );
        self.outbox = None;
        self.notice = Some(Notice::Info("Saved. Next voucher.".to_owned()));
        self.go(Field::Ledger(0));
    }

    /// The host could not post the voucher: the draft stays and the reason is shown.
    pub fn save_failed(&mut self, reason: &str) {
        self.outbox = None;
        self.fail(format!("Not saved: {reason}"));
    }

    /// Esc: returns `true` when the screen can close now; otherwise asks "discard?" first.
    pub fn request_leave(&mut self) -> bool {
        if self.draft.is_blank() {
            true
        } else {
            self.confirm_discard = true;
            false
        }
    }

    /// `true` while the screen asks "discard this voucher?".
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
