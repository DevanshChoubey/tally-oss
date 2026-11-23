//! Creating a ledger on the fly (Alt+C) from the party, the sales/purchase ledger field or a
//! particular's ledger field, as voucher entry does. TallyPrime documents it: "In the Voucher Creation screen, when your cursor
//! is on the ledger selection field, press Alt+C" (help.tallysolutions.com/ledgers-in-tallyprime/).
//!
//! The screen creates nothing itself: it records a [`CreateLedgerRequest`]. With a worker the
//! request goes to the engine here; without one the host takes it with
//! [`ItemInvoiceScreen::take_master_requests`]. The field shows the name and holds no ledger until
//! the engine names the new one ([`ItemInvoiceScreen::resolve_created_ledger`]), so an invoice can
//! never be saved on a ledger that does not exist yet.

use egui::Ui;
use ledgerly_api::Command;
use ledgerly_core::{GroupId, LedgerId};
use ledgerly_model::Masters;

use super::{Field, ItemInvoiceScreen, Notice};
use crate::AppCtx;
use crate::screens::voucher_entry::{
    CreateForm, CreateLedgerRequest, FormAnswer, check_new_ledger, create_form_ui, created_ledger,
    new_ledger,
};
use crate::shell::submit;

/// The field a new ledger is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CreateFor {
    /// The party.
    Party,
    /// The sales or purchase ledger.
    Account,
    /// The ledger of particular `i` (accounting invoice).
    Particular(usize),
}

impl CreateFor {
    fn of(field: Field) -> Option<CreateFor> {
        match field {
            Field::Party => Some(CreateFor::Party),
            Field::Account => Some(CreateFor::Account),
            Field::Ledger(i) => Some(CreateFor::Particular(i)),
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::LedgerAmount(_)
            | Field::Narration => None,
        }
    }

    fn field(self) -> Field {
        match self {
            CreateFor::Party => Field::Party,
            CreateFor::Account => Field::Account,
            CreateFor::Particular(i) => Field::Ledger(i),
        }
    }
}

impl ItemInvoiceScreen {
    /// Alt+C: opens the "create ledger" form when the keyboard is on the party, the
    /// sales/purchase ledger or a particular's ledger field, with the typed text as the name. Elsewhere it does nothing:
    /// the pages read do not document Alt+C on an item field.
    pub fn begin_create(&mut self) {
        let Some(target) = CreateFor::of(self.focus) else {
            return;
        };
        let name = match target {
            CreateFor::Party => self.draft.party_text.trim().to_owned(),
            CreateFor::Account => self.draft.account_text.trim().to_owned(),
            CreateFor::Particular(i) => self
                .draft
                .particulars
                .get(i)
                .map_or_else(String::new, |r| r.ledger_text.trim().to_owned()),
        };
        self.create = Some((target, CreateForm { name, group: None }));
    }

    /// `true` while the create-ledger form is open.
    #[must_use]
    pub fn is_creating(&self) -> bool {
        self.create.is_some()
    }

    /// The name in the open create-ledger form, if one is open.
    #[must_use]
    pub fn creating_name(&self) -> Option<&str> {
        self.create.as_ref().map(|(_, form)| form.name.as_str())
    }

    /// Closes the create-ledger form without creating anything.
    pub fn cancel_create(&mut self) {
        self.create = None;
    }

    /// Confirms the create form: records a [`CreateLedgerRequest`], puts the name in the field
    /// with no ledger behind it yet, and moves on.
    pub fn confirm_create(&mut self, masters: &Masters, name: &str, group: Option<GroupId>) {
        let Some(target) = self.create.as_ref().map(|(t, _)| *t) else {
            return self.fail("Press Alt+C on the party or ledger field first.".to_owned());
        };
        let queued: Vec<CreateLedgerRequest> = self
            .master_requests
            .iter()
            .chain(&self.create_queue)
            .cloned()
            .collect();
        let (name, group) = match check_new_ledger(masters, name, group, &queued) {
            Ok(checked) => checked,
            Err(message) => return self.fail(message),
        };
        match target {
            CreateFor::Party => {
                self.draft.party = None;
                name.clone_into(&mut self.draft.party_text);
            }
            CreateFor::Account => {
                self.draft.account = None;
                name.clone_into(&mut self.draft.account_text);
            }
            CreateFor::Particular(i) => {
                if let Some(row) = self.draft.particulars.get_mut(i) {
                    row.ledger = None;
                    name.clone_into(&mut row.ledger_text);
                }
            }
        }
        self.master_requests.push(CreateLedgerRequest {
            name: name.clone(),
            group,
        });
        self.waiting.push((target, name.clone()));
        self.create = None;
        self.focus = target.field();
        self.advance();
        self.notice = Some(Notice::Info(format!("Creating ledger '{name}'.")));
    }

    /// Ledgers the user asked to create since the last call, for the host to create.
    pub fn take_master_requests(&mut self) -> Vec<CreateLedgerRequest> {
        std::mem::take(&mut self.master_requests)
    }

    /// `true` when the ledger `name` was asked for and the engine has not answered yet.
    pub(super) fn is_waiting_for(&self, name: &str) -> bool {
        let name = name.trim().to_lowercase();
        self.waiting
            .iter()
            .any(|(_, waiting)| waiting.to_lowercase() == name)
    }

    /// The host created the requested ledger `name` as `id`: a field waiting for it, still
    /// showing that name, now holds it.
    pub fn resolve_created_ledger(&mut self, name: &str, id: LedgerId) {
        for (target, _) in self.take_waiting(name) {
            let (text, ledger) = match target {
                CreateFor::Party => (&self.draft.party_text, &mut self.draft.party),
                CreateFor::Account => (&self.draft.account_text, &mut self.draft.account),
                CreateFor::Particular(i) => match self.draft.particulars.get_mut(i) {
                    Some(row) => (&row.ledger_text, &mut row.ledger),
                    None => continue,
                },
            };
            if ledger.is_none() && text.trim() == name {
                *ledger = Some(id);
            }
        }
    }

    /// The host could not create the requested ledger `name`: the field stays without a ledger
    /// and the screen says why.
    pub fn reject_created_ledger(&mut self, name: &str, reason: &str) {
        let _cleared = self.take_waiting(name);
        self.fail(format!("Ledger '{name}' was not created: {reason}"));
    }

    fn take_waiting(&mut self, name: &str) -> Vec<(CreateFor, String)> {
        let (done, rest) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|(_, waiting)| waiting == name);
        self.waiting = rest;
        done
    }

    /// Draws the open create-ledger form and acts on its buttons.
    pub(super) fn draw_create_form(&mut self, ui: &mut Ui, masters: &Masters) {
        let Some((_, form)) = self.create.as_mut() else {
            return;
        };
        match create_form_ui(ui, form, masters, "u5-create-group") {
            FormAnswer::Open => {}
            FormAnswer::Confirm => {
                let (name, group) = (form.name.clone(), form.group);
                self.confirm_create(masters, &name, group);
            }
            FormAnswer::Cancel => self.create = None,
        }
    }

    /// Queues each request the user confirmed and sends the oldest to the worker when none is
    /// running. Without a worker the requests stay for the host.
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
            let ticket = submit::send(worker, Command::CreateLedger(new_ledger(&request)));
            self.creating = Some((request.name, ticket));
        }
    }

    /// Takes the answer of the ledger create in progress, if it has come.
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
        match created_ledger(reply) {
            Ok(id) => self.resolve_created_ledger(&name, id),
            Err(why) => self.reject_created_ledger(&name, &why),
        }
        ui.ctx().request_repaint();
    }
}
