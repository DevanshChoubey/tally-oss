//! Creating a ledger inline (Alt+C): the form and the host request.

use ledgerly_core::{GroupId, LedgerId};
use ledgerly_model::Masters;

use super::draft::LedgerRef;
use super::state::{CreateForm, CreateLedgerRequest, Field, Notice, VoucherEntryScreen};

impl VoucherEntryScreen {
    // ---- creating a ledger inline (Alt+C) ----

    /// Alt+C: opens the "create ledger" form for the ledger field that has the keyboard, with the
    /// typed text as the name.
    pub fn begin_create(&mut self) {
        if let Field::Ledger(i) = self.focus {
            let name = self
                .draft
                .rows
                .get(i)
                .map_or_default(|r| r.ledger_text.trim().to_owned());
            self.create = Some(CreateForm { name, group: None });
        }
    }

    /// `true` while the create-ledger form is open.
    #[must_use]
    pub fn is_creating(&self) -> bool {
        self.create.is_some()
    }

    /// Confirms the create form: records a [`CreateLedgerRequest`] and marks the line pending.
    pub fn confirm_create(&mut self, masters: &Masters, name: &str, group: Option<GroupId>) {
        let name = name.trim();
        let Some(group) = group else {
            return self.fail("Choose a group for the new ledger.");
        };
        if name.is_empty() {
            return self.fail("Type a name for the new ledger.");
        }
        if !masters.groups.contains_key(&group) {
            return self.fail("That group does not exist.");
        }
        let lower = name.to_lowercase();
        let taken = masters
            .ledgers
            .values()
            .any(|l| l.name.to_lowercase() == lower)
            || self
                .master_requests
                .iter()
                .any(|r| r.name.to_lowercase() == lower);
        if taken {
            return self.fail(format!(
                "A ledger called '{name}' already exists; pick it from the list."
            ));
        }
        let Field::Ledger(i) = self.focus else {
            return self.fail("Put the cursor on a ledger field first.");
        };
        if let Some(r) = self.draft.rows.get_mut(i) {
            r.ledger = Some(LedgerRef::Pending(name.to_owned()));
            name.clone_into(&mut r.ledger_text);
        }
        self.master_requests.push(CreateLedgerRequest {
            name: name.to_owned(),
            group,
        });
        self.create = None;
        self.notice = Some(Notice::Info(format!("Creating ledger '{name}'.")));
        self.go(Field::Amount(i));
    }

    /// Ledgers the user asked to create since the last call, for the host to create.
    pub fn take_master_requests(&mut self) -> Vec<CreateLedgerRequest> {
        std::mem::take(&mut self.master_requests)
    }

    /// The host created the requested ledger `name` as `id`: lines waiting for it now use it.
    pub fn resolve_created_ledger(&mut self, name: &str, id: LedgerId) {
        for row in &mut self.draft.rows {
            if row.ledger == Some(LedgerRef::Pending(name.to_owned())) {
                row.ledger = Some(LedgerRef::Chosen(id));
            }
        }
        self.remember(id);
    }

    /// The host could not create the requested ledger `name`: lines waiting for it are cleared.
    pub fn reject_created_ledger(&mut self, name: &str, reason: &str) {
        for row in &mut self.draft.rows {
            if row.ledger == Some(LedgerRef::Pending(name.to_owned())) {
                row.ledger = None;
            }
        }
        self.fail(format!("Ledger '{name}' was not created: {reason}"));
    }
}
