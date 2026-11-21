//! Background work: posting a voucher and creating ledgers through the worker.

use egui::Ui;
use ledgerly_api::{Command, CreatedId};
use ledgerly_core::{LedgerId, Paise, SignedAmount};
use ledgerly_model::Ledger;

use super::state::{Notice, VoucherEntryScreen};
use crate::AppCtx;
use crate::shell::submit::{self, CommandReply};

impl VoucherEntryScreen {
    /// Sends the voucher the user asked to save to the worker. Without a worker nothing can be
    /// posted, and the screen says so.
    pub(super) fn send_saves(&mut self, ctx: &AppCtx) {
        // Without a worker the host takes the request itself, so it stays in the outbox.
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        if let Some(voucher) = self.take_save_request() {
            self.posting = Some(submit::send(worker, Command::PostVoucher(voucher)));
            self.notice = Some(Notice::Info("Saving...".to_owned()));
        }
    }

    /// Polls the post in progress and shows "Saving..." until the worker answers.
    pub(super) fn wait_for_post(&mut self, ui: &mut Ui) {
        let Some(ticket) = self.posting.as_mut() else {
            return;
        };
        if let Some(reply) = submit::settle(ticket) {
            self.posting = None;
            self.finish_post(reply);
        } else {
            ui.label("Saving...");
            ui.ctx().request_repaint();
        }
    }

    /// Shows the engine's answer. A booked voucher starts a fresh one; anything else keeps the
    /// draft and says why nothing was saved.
    pub(super) fn finish_post(&mut self, reply: CommandReply) {
        match reply {
            CommandReply::Booked(_) => self.confirm_saved(),
            CommandReply::Unsupported(u) => {
                self.save_failed(&format!("not supported yet: {} ({})", u.what, u.reason));
            }
            CommandReply::Failed(reason) => self.save_failed(&reason),
        }
    }

    /// Inline ledger creation (Alt+C): queues each request the user confirmed and sends the oldest
    /// to the worker when none is running. The ledger goes out with id 0; the API assigns the real
    /// id and reports it in `CommandResult::created`.
    pub(super) fn send_ledger_creates(&mut self, ctx: &AppCtx) {
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        let requests = self.take_master_requests();
        self.create_queue.extend(requests);
        if self.creating.is_some() {
            return;
        }
        if let Some(request) = self.create_queue.pop_front() {
            let ledger = Ledger {
                ineligible_itc: None,
                id: LedgerId(0),
                name: request.name.clone(),
                aliases: Vec::new(),
                group: request.group,
                opening: SignedAmount::dr(Paise::ZERO),
                bill_wise: None,
                cost_centres_applicable: false,
                party: None,
                bank: None,
                tax: None,
                gst: None,
                reserved: None,
                tally: None,
            };
            let ticket = submit::send(worker, Command::CreateLedger(ledger));
            self.creating = Some((request.name, ticket));
        }
    }

    /// Takes the answer of the ledger create in progress, if it has come. A created ledger fills
    /// the lines waiting for it; a refusal clears them and says why.
    pub(super) fn poll_ledger_create(&mut self, ui: &Ui) {
        let Some((name, ticket)) = self.creating.as_mut() else {
            return;
        };
        let Some(reply) = submit::settle(ticket) else {
            ui.ctx().request_repaint();
            return;
        };
        let name = std::mem::take(name);
        self.creating = None;
        match reply {
            CommandReply::Booked(result) => match result.created {
                Some(CreatedId::Ledger(id)) => self.resolve_created_ledger(&name, id),
                Some(
                    other @ (CreatedId::Group(_)
                    | CreatedId::Unit(_)
                    | CreatedId::StockGroup(_)
                    | CreatedId::StockItem(_)
                    | CreatedId::Godown(_)
                    | CreatedId::Batch(_)
                    | CreatedId::CostCategory(_)
                    | CreatedId::CostCentre(_)),
                ) => self.reject_created_ledger(
                    &name,
                    &format!("the engine reported a different master ({other:?})"),
                ),
                None => {
                    self.reject_created_ledger(&name, "the engine did not report the new ledger");
                }
            },
            CommandReply::Unsupported(u) => {
                let why = format!("not supported yet: {} ({})", u.what, u.reason);
                self.reject_created_ledger(&name, &why);
            }
            CommandReply::Failed(reason) => self.reject_created_ledger(&name, &reason),
        }
        ui.ctx().request_repaint();
    }
}
