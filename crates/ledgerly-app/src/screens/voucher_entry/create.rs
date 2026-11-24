//! Creating a ledger inline (Alt+C): the form and the host request.

use egui::Ui;
use ledgerly_core::{GroupId, LedgerId};
use ledgerly_model::Masters;

use super::draft::LedgerRef;
use super::state::{CreateForm, CreateLedgerRequest, Field, Notice, VoucherEntryScreen};

/// Checks a ledger the user asked to create with Alt+C: a group that exists, a name, and no
/// ledger (or request still waiting in `queued`) with the same name in any case. Returns the
/// trimmed name and the group, or what to tell the user.
pub(crate) fn check_new_ledger(
    tr: &crate::i18n::Tr,
    masters: &Masters,
    name: &str,
    group: Option<GroupId>,
    queued: &[CreateLedgerRequest],
) -> Result<(String, GroupId), String> {
    let name = name.trim();
    let Some(group) = group else {
        return Err(tr.t("voucher.new_ledger.no_group"));
    };
    if name.is_empty() {
        return Err(tr.t("voucher.new_ledger.no_name"));
    }
    if !masters.groups.contains_key(&group) {
        return Err(tr.t("voucher.new_ledger.no_such_group"));
    }
    let lower = name.to_lowercase();
    let taken = masters
        .ledgers
        .values()
        .any(|l| l.name.to_lowercase() == lower)
        || queued.iter().any(|r| r.name.to_lowercase() == lower);
    if taken {
        return Err(tr.tf("voucher.new_ledger.taken", &[("name", name)]));
    }
    Ok((name.to_owned(), group))
}

/// What the user did in the create-ledger form this frame.
pub(crate) enum FormAnswer {
    /// Nothing yet: the form stays open.
    Open,
    /// Pressed Create.
    Confirm,
    /// Pressed Cancel.
    Cancel,
}

/// Draws the create-ledger form: the name, a group list (its id salted with `salt`), Create and
/// Cancel.
pub(crate) fn create_form_ui(
    ui: &mut Ui,
    tr: &crate::i18n::Tr,
    form: &mut CreateForm,
    masters: &Masters,
    salt: &str,
) -> FormAnswer {
    let mut answer = FormAnswer::Open;
    ui.group(|ui| {
        ui.label(tr.t("voucher.create_form.title"));
        ui.horizontal(|ui| {
            ui.label(tr.t("voucher.create_form.name"));
            ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(280.0));
        });
        let selected = form.group.and_then(|g| masters.groups.get(&g)).map_or_else(
            || tr.t("voucher.create_form.choose_group"),
            |g| g.name.clone(),
        );
        egui::ComboBox::from_id_salt(salt)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (id, group) in &masters.groups {
                    ui.selectable_value(&mut form.group, Some(*id), group.name.as_str());
                }
            });
        ui.horizontal(|ui| {
            if ui.button(tr.t("voucher.create_form.create")).clicked() {
                answer = FormAnswer::Confirm;
            }
            if ui.button(tr.t("voucher.create_form.cancel")).clicked() {
                answer = FormAnswer::Cancel;
            }
        });
    });
    answer
}

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
        let (name, group) =
            match check_new_ledger(&self.tr, masters, name, group, &self.master_requests) {
                Ok(checked) => checked,
                Err(message) => return self.fail(message),
            };
        let name = name.as_str();
        let Field::Ledger(i) = self.focus else {
            return self.fail(self.tr.t("voucher.put_cursor_on_ledger"));
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
        self.notice = Some(Notice::Info(
            self.tr.tf("voucher.creating_ledger", &[("name", name)]),
        ));
        self.go(self.after_ledger(i));
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
        self.fail(self.tr.tf(
            "voucher.ledger_not_created",
            &[("name", name), ("reason", reason)],
        ));
    }
}
