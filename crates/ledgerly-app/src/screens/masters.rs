//! Masters screens. **Card U6** implements this module.
//!
//! Frozen interface (plan card U6):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Masters`: one screen that lists, creates, alters and deletes every master
//! kind in [`MasterKind`], one kind at a time (spec 7).
//!
//! What it does, and where it lives:
//!
//! * A list per kind, sorted by name, with search over name, alias and GSTIN ([`list_rows`]).
//! * Alt+C creates ([`MastersScreen::begin_create`]), Ctrl+Enter alters the master under the
//!   cursor ([`MastersScreen::begin_alter`]), Alt+D deletes after a Y/N question
//!   ([`MastersScreen::request_delete`], [`MastersScreen::confirm_delete`]), Ctrl+A and Ctrl+S
//!   save ([`MastersScreen::save`]), Esc goes back ([`MastersScreen::escape`]), Alt+Left and
//!   Alt+Right change the kind.
//! * The screen never writes to the store (rule D3 belongs to the host). A save, or a confirmed
//!   delete, is queued as a [`MasterRequest`] that the host takes with
//!   [`MastersScreen::take_requests`], runs as one audited command, and answers by refreshing the
//!   books, or by calling [`MastersScreen::report_host_error`].
//! * The rules a form enforces are in the `draft` module, and apply to a test exactly as to a
//!   click: unique names and aliases, group trees without loops, nature only on primary groups,
//!   GSTIN and PAN checks, no deleting reserved groups or ledgers (spec 5 invariant 5), no
//!   deleting a master the books still use, and the company's feature switches.
//!
//! Rule C4 (the UI never sums anything): this screen shows an opening balance as stored and adds
//! nothing. Balances and stock quantities are not shown here.
//!
//! Not built here, and said so rather than faked ([`MasterError::Unsupported`]):
//!
//! * Voucher types and batches: listed, not edited. Numbering has no verified behaviour yet, and
//!   batches belong to stock entry.
//! * Compound units, alternate units, per-date GST history, standard cost rates and opening stock:
//!   listed, kept as they are when the item is altered, not edited.
//! * UIN registrations: a UIN is not shaped like a GSTIN and the format is not sourced.
//! * The GST unit quantity code is checked for shape (three capital letters), not against the
//!   official list, which is not sourced.
//!
//! Known cost: the list is rebuilt each frame from the books. That is fine for thousands of
//! masters; a very large company needs the host to hand in a revision number so the list can be
//! cached (not asked for yet, nothing to key a cache on).

mod check;
mod command;
mod draft;
mod form_ui;
mod list;

use egui::{Key, Modifiers, Ui};
use ledgerly_core::{Outcome, Unsupported};
use ledgerly_model::{Books, MasterSnapshot, Masters};

use command::command_for;
use list::snapshot_name;
use thiserror::Error;

use crate::shell::submit::{self, CommandReply};
use crate::worker::Ticket;
use crate::{AppCtx, Screen, ScreenAction};
use ledgerly_api::Command;

pub use draft::{
    BankDraft, CostCentreDraft, GroupDraft, GstDraft, LedgerDraft, MasterDraft, PartyDraft,
    StockItemDraft, TaxDraft, TreeDraft, UNASSIGNED_ID, UnitDraft, creation_allowed, parse_rupees,
};
pub use list::{MasterRow, list_rows, snapshot_of};

// ---------------------------------------------------------------------------------------------
// Kinds, references, requests, errors
// ---------------------------------------------------------------------------------------------

/// A kind of master this screen lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MasterKind {
    /// Account group.
    Group,
    /// Ledger.
    Ledger,
    /// Voucher type (listed only).
    VoucherType,
    /// Unit of measure.
    Unit,
    /// Stock group.
    StockGroup,
    /// Stock item.
    StockItem,
    /// Godown.
    Godown,
    /// Batch (listed only).
    Batch,
    /// Cost category.
    CostCategory,
    /// Cost centre.
    CostCentre,
}

impl MasterKind {
    /// Every kind, in the order the tabs show them.
    pub const ALL: [MasterKind; 10] = [
        MasterKind::Group,
        MasterKind::Ledger,
        MasterKind::VoucherType,
        MasterKind::Unit,
        MasterKind::StockGroup,
        MasterKind::StockItem,
        MasterKind::Godown,
        MasterKind::Batch,
        MasterKind::CostCategory,
        MasterKind::CostCentre,
    ];

    /// The singular name, for messages.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            MasterKind::Group => "group",
            MasterKind::Ledger => "ledger",
            MasterKind::VoucherType => "voucher type",
            MasterKind::Unit => "unit",
            MasterKind::StockGroup => "stock group",
            MasterKind::StockItem => "stock item",
            MasterKind::Godown => "godown",
            MasterKind::Batch => "batch",
            MasterKind::CostCategory => "cost category",
            MasterKind::CostCentre => "cost centre",
        }
    }

    /// The plural name, for tabs and titles.
    #[must_use]
    pub fn plural(self) -> &'static str {
        match self {
            MasterKind::Group => "Groups",
            MasterKind::Ledger => "Ledgers",
            MasterKind::VoucherType => "Voucher types",
            MasterKind::Unit => "Units",
            MasterKind::StockGroup => "Stock groups",
            MasterKind::StockItem => "Stock items",
            MasterKind::Godown => "Godowns",
            MasterKind::Batch => "Batches",
            MasterKind::CostCategory => "Cost categories",
            MasterKind::CostCentre => "Cost centres",
        }
    }

    fn step(self, forward: bool) -> MasterKind {
        let pos = MasterKind::ALL.iter().position(|k| *k == self).unwrap_or(0);
        let next = if forward {
            pos.saturating_add(1)
        } else {
            pos.saturating_sub(1)
        };
        MasterKind::ALL
            .get(next)
            .or_else(|| MasterKind::ALL.get(pos))
            .copied()
            .unwrap_or(self)
    }
}

/// One master, named by kind and raw id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MasterRef {
    /// The kind.
    pub kind: MasterKind,
    /// The id inside that kind (the number inside `GroupId`, `LedgerId` and so on).
    pub id: u64,
}

/// What the host should do for a confirmed form or delete.
///
/// A [`MasterRequest::Create`] carries [`UNASSIGNED_ID`]; the store assigns the real id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterRequest {
    /// Create this master.
    Create(MasterSnapshot),
    /// Replace the master with the same id by this one.
    Alter(MasterSnapshot),
    /// Delete this master.
    Delete(MasterRef),
}

/// Why a form, a delete or a key did not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MasterError {
    /// A name field is empty.
    #[error("{field} is empty; type one")]
    NameEmpty {
        /// The field.
        field: String,
    },
    /// The name or alias is used by another master of the same kind.
    #[error(
        "'{name}' is already used by another master of this kind; names and aliases must be unique"
    )]
    NameTaken {
        /// The repeated text.
        name: String,
    },
    /// A field is not acceptable.
    #[error("{field}: {reason}")]
    Invalid {
        /// The field.
        field: String,
        /// What is wrong and what to do.
        reason: String,
    },
    /// A reserved master may not change this way.
    #[error("'{name}' is a reserved master and cannot be {what}")]
    Reserved {
        /// The master.
        name: String,
        /// What was attempted.
        what: String,
    },
    /// Something in the books still refers to the master.
    #[error("'{name}' cannot be deleted: it is used by {by}")]
    InUse {
        /// The master.
        name: String,
        /// What uses it.
        by: String,
    },
    /// The company has switched the feature off.
    #[error("turn on the {feature} feature first (F11)")]
    FeatureOff {
        /// The feature's name.
        feature: &'static str,
    },
    /// The master no longer exists.
    #[error("that {kind} no longer exists")]
    NotFound {
        /// The kind, singular.
        kind: &'static str,
    },
    /// No behaviour is verified for this yet.
    #[error("{what} is not supported yet: {reason}")]
    Unsupported {
        /// What was asked.
        what: String,
        /// Why it is not available.
        reason: String,
    },
    /// Nothing is selected in the list.
    #[error("select a master in the list first")]
    NothingSelected,
    /// A form or a question is already open.
    #[error("finish or leave the open form first")]
    Busy,
    /// There is no form to save.
    #[error("no form is open")]
    NoForm,
    /// There is no delete to confirm.
    #[error("there is no delete to confirm")]
    NothingToConfirm,
    /// An amount does not fit.
    #[error(transparent)]
    Money(#[from] ledgerly_core::MoneyError),
}

impl From<Unsupported> for MasterError {
    fn from(u: Unsupported) -> Self {
        MasterError::Unsupported {
            what: u.what,
            reason: u.reason,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------------------------

/// Keys. Ctrl is `Modifiers::COMMAND`. Provisional until the key map (card U2) is merged and the
/// screen can ask it for its bindings (spec 7: keys are rebindable).
mod bindings {
    use egui::{Key, Modifiers};

    /// Save: Ctrl+A and Ctrl+S (spec 7).
    pub(super) const SAVE: [(Modifiers, Key); 2] =
        [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];
    /// Create a master (spec 7: Alt+C).
    pub(super) const CREATE: (Modifiers, Key) = (Modifiers::ALT, Key::C);
    /// Alter the master under the cursor (spec 7: Ctrl+Enter).
    pub(super) const ALTER: (Modifiers, Key) = (Modifiers::COMMAND, Key::Enter);
    /// Delete the master under the cursor (spec 7: Alt+D).
    pub(super) const DELETE: (Modifiers, Key) = (Modifiers::ALT, Key::D);
    /// Previous master kind. Provisional: no source for this binding.
    pub(super) const PREV_KIND: (Modifiers, Key) = (Modifiers::ALT, Key::ArrowLeft);
    /// Next master kind. Provisional: no source for this binding.
    pub(super) const NEXT_KIND: (Modifiers, Key) = (Modifiers::ALT, Key::ArrowRight);
}

/// Which way the cursor moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Toward the top of the list.
    Up,
    /// Toward the bottom of the list.
    Down,
}

enum Notice {
    Error(String),
    Warn(String),
    Info(String),
}

struct FormState {
    draft: MasterDraft,
    initial: MasterDraft,
    editing: Option<u64>,
    /// The draft as it was when Esc first asked whether to discard it.
    armed: Option<MasterDraft>,
}

enum Mode {
    List,
    Form(Box<FormState>),
    ConfirmDelete(MasterRef),
}

enum EscState {
    List,
    Confirm,
    Form { dirty: bool, armed: bool },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModeTag {
    List,
    Form,
    Confirm,
}

enum KeyCmd {
    Save,
    Create,
    Alter,
    Delete,
    Escape,
    Up,
    Down,
    PrevKind,
    NextKind,
    Yes,
    No,
}

/// The masters screen (card U6).
pub struct MastersScreen {
    kind: MasterKind,
    filter: String,
    selected: Option<MasterRef>,
    mode: Mode,
    notice: Option<Notice>,
    requests: Vec<MasterRequest>,
    /// Commands waiting to be sent, oldest first. One runs at a time.
    commands: std::collections::VecDeque<Command>,
    /// The command the worker is running, if any. While it runs the screen takes no keys.
    posting: Option<Ticket<CommandReply>>,
}

impl Default for MastersScreen {
    fn default() -> Self {
        MastersScreen::new()
    }
}

impl MastersScreen {
    /// A screen showing the ledgers.
    #[must_use]
    pub fn new() -> Self {
        MastersScreen::with_kind(MasterKind::Ledger)
    }

    /// A screen showing `kind`.
    #[must_use]
    pub fn with_kind(kind: MasterKind) -> Self {
        MastersScreen {
            kind,
            filter: String::new(),
            selected: None,
            mode: Mode::List,
            notice: None,
            requests: Vec::new(),
            commands: std::collections::VecDeque::new(),
            posting: None,
        }
    }

    /// The kind being listed.
    #[must_use]
    pub fn kind(&self) -> MasterKind {
        self.kind
    }

    /// Switches the list to `kind`, clearing the cursor. Ignored while a form or a question is
    /// open.
    pub fn select_kind(&mut self, kind: MasterKind) {
        if matches!(self.mode, Mode::List) && self.kind != kind {
            self.kind = kind;
            self.selected = None;
            self.notice = None;
        }
    }

    /// The search text.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Sets the search text. Ignored while a form or a question is open.
    pub fn set_filter(&mut self, text: &str) {
        if matches!(self.mode, Mode::List) {
            text.clone_into(&mut self.filter);
        }
    }

    /// The master under the cursor.
    #[must_use]
    pub fn selected(&self) -> Option<MasterRef> {
        self.selected
    }

    /// Puts the cursor on `target` (and shows its kind). Ignored while a form or a question is
    /// open.
    pub fn select(&mut self, target: MasterRef) {
        if matches!(self.mode, Mode::List) {
            self.kind = target.kind;
            self.selected = Some(target);
        }
    }

    /// The rows the list shows now.
    #[must_use]
    pub fn rows(&self, masters: &Masters) -> Vec<MasterRow> {
        list_rows(masters, self.kind, &self.filter)
    }

    /// Moves the cursor one row. With nothing selected, either direction lands on the first row.
    pub fn move_selection(&mut self, masters: &Masters, step: Step) {
        let rows = self.rows(masters);
        let current = self
            .selected
            .and_then(|s| rows.iter().position(|r| r.master == s));
        let next = match (current, step) {
            (None, _) => 0,
            (Some(i), Step::Up) => i.saturating_sub(1),
            (Some(i), Step::Down) => i.saturating_add(1).min(rows.len().saturating_sub(1)),
        };
        self.selected = rows.get(next).map(|r| r.master);
    }

    /// `true` while a create or alter form is open.
    #[must_use]
    pub fn is_form_open(&self) -> bool {
        matches!(self.mode, Mode::Form(_))
    }

    /// The open form.
    #[must_use]
    pub fn draft(&self) -> Option<&MasterDraft> {
        match &self.mode {
            Mode::Form(f) => Some(&f.draft),
            Mode::List | Mode::ConfirmDelete(_) => None,
        }
    }

    /// The open form, for editing.
    pub fn draft_mut(&mut self) -> Option<&mut MasterDraft> {
        match &mut self.mode {
            Mode::Form(f) => Some(&mut f.draft),
            Mode::List | Mode::ConfirmDelete(_) => None,
        }
    }

    /// The id of the master the open form alters; `None` for a create form or no form.
    #[must_use]
    pub fn editing(&self) -> Option<u64> {
        match &self.mode {
            Mode::Form(f) => f.editing,
            Mode::List | Mode::ConfirmDelete(_) => None,
        }
    }

    /// The master a delete question is about.
    #[must_use]
    pub fn pending_delete(&self) -> Option<MasterRef> {
        match &self.mode {
            Mode::ConfirmDelete(t) => Some(*t),
            Mode::List | Mode::Form(_) => None,
        }
    }

    /// The error message shown now, if any.
    #[must_use]
    pub fn error_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Error(s)) => Some(s),
            Some(Notice::Warn(_) | Notice::Info(_)) | None => None,
        }
    }

    /// The warning or information message shown now, if any.
    #[must_use]
    pub fn note_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Warn(s) | Notice::Info(s)) => Some(s),
            Some(Notice::Error(_)) | None => None,
        }
    }

    /// Takes the requests queued for the host, oldest first.
    pub fn take_requests(&mut self) -> Vec<MasterRequest> {
        std::mem::take(&mut self.requests)
    }

    /// The host could not run a request; shows why. The books are unchanged, so the user can try
    /// again.
    pub fn report_host_error(&mut self, message: &str) {
        self.notice = Some(Notice::Error(message.to_owned()));
    }

    fn record<T>(&mut self, result: Result<T, MasterError>) -> Result<T, MasterError> {
        if let Err(e) = &result {
            self.notice = Some(Notice::Error(e.to_string()));
        }
        result
    }

    fn require_list(&self) -> Result<(), MasterError> {
        if matches!(self.mode, Mode::List) {
            Ok(())
        } else {
            Err(MasterError::Busy)
        }
    }

    fn open_form(&mut self, draft: MasterDraft, editing: Option<u64>) {
        self.notice = None;
        self.mode = Mode::Form(Box::new(FormState {
            initial: draft.clone(),
            draft,
            editing,
            armed: None,
        }));
    }

    /// Opens an empty form for the kind being listed.
    ///
    /// # Errors
    /// [`MasterError::Busy`] with a form open, [`MasterError::FeatureOff`] when the company has
    /// switched the kind's feature off, [`MasterError::Unsupported`] for voucher types and
    /// batches.
    pub fn begin_create(&mut self, books: &Books) -> Result<(), MasterError> {
        let result = self.begin_create_inner(books);
        self.record(result)
    }

    fn begin_create_inner(&mut self, books: &Books) -> Result<(), MasterError> {
        self.require_list()?;
        creation_allowed(self.kind, &books.company.features)?;
        match MasterDraft::blank(self.kind) {
            Outcome::Value(d) => {
                self.open_form(d, None);
                Ok(())
            }
            Outcome::Unsupported(u) => Err(u.into()),
        }
    }

    /// Opens the form for the master under the cursor.
    ///
    /// # Errors
    /// [`MasterError::Busy`], [`MasterError::NothingSelected`], [`MasterError::NotFound`] when the
    /// master has gone, [`MasterError::Unsupported`] for kinds and shapes this screen does not
    /// edit.
    pub fn begin_alter(&mut self, books: &Books) -> Result<(), MasterError> {
        let result = self.begin_alter_inner(books);
        self.record(result)
    }

    fn begin_alter_inner(&mut self, books: &Books) -> Result<(), MasterError> {
        self.require_list()?;
        let target = self.selected.ok_or(MasterError::NothingSelected)?;
        let snapshot = snapshot_of(&books.masters, target).ok_or(MasterError::NotFound {
            kind: target.kind.label(),
        })?;
        match MasterDraft::from_existing(&snapshot) {
            Outcome::Value(d) => {
                self.open_form(d, Some(target.id));
                Ok(())
            }
            Outcome::Unsupported(u) => Err(u.into()),
        }
    }

    /// Checks the open form and queues a [`MasterRequest::Create`] or [`MasterRequest::Alter`].
    ///
    /// # Errors
    /// [`MasterError::NoForm`], or whatever [`MasterDraft::build`] finds wrong; the form stays
    /// open so the user can fix it.
    pub fn save(&mut self, books: &Books) -> Result<(), MasterError> {
        let result = self.save_inner(books);
        self.record(result)
    }

    fn save_inner(&mut self, books: &Books) -> Result<(), MasterError> {
        let Mode::Form(form) = &self.mode else {
            return Err(MasterError::NoForm);
        };
        let editing = form.editing;
        let snapshot = form.draft.build(books, editing)?;
        let name = snapshot_name(&snapshot).to_owned();
        let request = if editing.is_some() {
            MasterRequest::Alter(snapshot)
        } else {
            MasterRequest::Create(snapshot)
        };
        self.requests.push(request);
        self.mode = Mode::List;
        if editing.is_none() {
            self.selected = None;
        }
        self.notice = Some(Notice::Info(format!("'{name}' sent to be saved.")));
        Ok(())
    }

    /// Asks whether to delete the master under the cursor, after checking nothing blocks it.
    ///
    /// # Errors
    /// [`MasterError::Busy`], [`MasterError::NothingSelected`], [`MasterError::Reserved`] for a
    /// reserved group or ledger (spec 5 invariant 5), [`MasterError::InUse`] when the books still
    /// refer to it, [`MasterError::Unsupported`] for voucher types and batches.
    pub fn request_delete(&mut self, books: &Books) -> Result<(), MasterError> {
        let result = self.request_delete_inner(books);
        self.record(result)
    }

    fn request_delete_inner(&mut self, books: &Books) -> Result<(), MasterError> {
        self.require_list()?;
        let target = self.selected.ok_or(MasterError::NothingSelected)?;
        check::delete_blockers(books, target)?;
        self.notice = None;
        self.mode = Mode::ConfirmDelete(target);
        Ok(())
    }

    /// The user answered yes: queues a [`MasterRequest::Delete`], after checking again.
    ///
    /// # Errors
    /// [`MasterError::NothingToConfirm`], or the reason the delete is no longer allowed.
    pub fn confirm_delete(&mut self, books: &Books) -> Result<(), MasterError> {
        let result = self.confirm_delete_inner(books);
        self.record(result)
    }

    fn confirm_delete_inner(&mut self, books: &Books) -> Result<(), MasterError> {
        let Mode::ConfirmDelete(target) = self.mode else {
            return Err(MasterError::NothingToConfirm);
        };
        self.mode = Mode::List;
        check::delete_blockers(books, target)?;
        self.requests.push(MasterRequest::Delete(target));
        self.selected = None;
        self.notice = Some(Notice::Info("Delete sent.".to_owned()));
        Ok(())
    }

    /// Esc: closes a question, leaves a form (asking once if it holds changes), clears the
    /// search, and finally leaves the screen.
    pub fn escape(&mut self) -> ScreenAction {
        let state = match &self.mode {
            Mode::List => EscState::List,
            Mode::ConfirmDelete(_) => EscState::Confirm,
            Mode::Form(f) => EscState::Form {
                dirty: f.draft != f.initial,
                armed: f.armed.as_ref() == Some(&f.draft),
            },
        };
        match state {
            EscState::List => {
                if self.filter.is_empty() {
                    return ScreenAction::Pop;
                }
                self.filter.clear();
            }
            EscState::Confirm => self.mode = Mode::List,
            EscState::Form { dirty, armed } => {
                if !dirty || armed {
                    self.mode = Mode::List;
                    self.notice = None;
                } else {
                    if let Mode::Form(f) = &mut self.mode {
                        f.armed = Some(f.draft.clone());
                    }
                    self.notice = Some(Notice::Warn(
                        "This form has changes. Press Esc again to discard them.".to_owned(),
                    ));
                }
            }
        }
        ScreenAction::Stay
    }

    fn mode_tag(&self) -> ModeTag {
        match self.mode {
            Mode::List => ModeTag::List,
            Mode::Form(_) => ModeTag::Form,
            Mode::ConfirmDelete(_) => ModeTag::Confirm,
        }
    }

    fn handle_key(&mut self, books: &Books, cmd: &KeyCmd) -> ScreenAction {
        // A failed command leaves its message in the notice; there is nothing more to do here.
        match cmd {
            KeyCmd::Save => drop(self.save(books)),
            KeyCmd::Create => drop(self.begin_create(books)),
            KeyCmd::Alter => drop(self.begin_alter(books)),
            KeyCmd::Delete => drop(self.request_delete(books)),
            KeyCmd::Yes => drop(self.confirm_delete(books)),
            KeyCmd::No => self.mode = Mode::List,
            KeyCmd::Up => self.move_selection(&books.masters, Step::Up),
            KeyCmd::Down => self.move_selection(&books.masters, Step::Down),
            KeyCmd::PrevKind => self.select_kind(self.kind.step(false)),
            KeyCmd::NextKind => self.select_kind(self.kind.step(true)),
            KeyCmd::Escape => return self.escape(),
        }
        ScreenAction::Stay
    }
}

/// Consumes the press of exactly `key` with exactly `modifiers` held, and reports whether there was
/// one. egui's `consume_key` also accepts extra Alt or Shift, so a plain-Y question would take
/// Shift+Y as a yes.
fn consume_exact(input: &mut egui::InputState, key: Key, modifiers: Modifiers) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            egui::Event::Key { key: pressed_key, modifiers: held, pressed: true, .. }
                if *pressed_key == key && held.matches_exact(modifiers)
        )
    });
    input.events.len() != before
}

fn read_keys(ui: &Ui, tag: ModeTag) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.ctx().input_mut(|i| match tag {
        ModeTag::Form => {
            let mut save = false;
            for (modifiers, key) in bindings::SAVE {
                // Consume every binding, so one key press never reaches a widget.
                save |= i.consume_key(modifiers, key);
            }
            if save {
                out.push(KeyCmd::Save);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                out.push(KeyCmd::Escape);
            }
        }
        ModeTag::Confirm => {
            // Plain keys only: a Shift+Y or Alt+Y must not answer a permanent delete.
            if consume_exact(i, Key::Y, Modifiers::NONE) {
                out.push(KeyCmd::Yes);
            }
            if consume_exact(i, Key::N, Modifiers::NONE) {
                out.push(KeyCmd::No);
            }
            if consume_exact(i, Key::Escape, Modifiers::NONE) {
                out.push(KeyCmd::Escape);
            }
        }
        ModeTag::List => {
            let table = [
                (bindings::CREATE, KeyCmd::Create),
                (bindings::ALTER, KeyCmd::Alter),
                (bindings::DELETE, KeyCmd::Delete),
                (bindings::PREV_KIND, KeyCmd::PrevKind),
                (bindings::NEXT_KIND, KeyCmd::NextKind),
                ((Modifiers::NONE, Key::ArrowUp), KeyCmd::Up),
                ((Modifiers::NONE, Key::ArrowDown), KeyCmd::Down),
                ((Modifiers::NONE, Key::Escape), KeyCmd::Escape),
            ];
            for ((modifiers, key), cmd) in table {
                if i.consume_key(modifiers, key) {
                    out.push(cmd);
                }
            }
        }
    });
    out
}

impl MastersScreen {
    fn draw_tabs(&mut self, ui: &mut Ui) {
        let enabled = matches!(self.mode, Mode::List);
        let mut chosen = None;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                for k in MasterKind::ALL {
                    if ui.selectable_label(self.kind == k, k.plural()).clicked() {
                        chosen = Some(k);
                    }
                }
            });
        });
        if let Some(k) = chosen {
            self.select_kind(k);
        }
    }

    fn draw_list(&mut self, ui: &mut Ui, books: &Books) {
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add(egui::TextEdit::singleline(&mut self.filter).desired_width(280.0));
        });
        let rows = self.rows(&books.masters);
        let selected = self.selected;
        let mut clicked = None;
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        egui::ScrollArea::vertical().max_height(360.0).show_rows(
            ui,
            row_height,
            rows.len(),
            |ui, range| {
                for row in rows.iter().skip(range.start).take(range.len()) {
                    let text = if row.detail.is_empty() {
                        row.name.clone()
                    } else {
                        format!("{}    {}", row.name, row.detail)
                    };
                    if ui
                        .selectable_label(selected == Some(row.master), text)
                        .clicked()
                    {
                        clicked = Some(row.master);
                    }
                }
            },
        );
        if rows.is_empty() {
            ui.weak("Nothing to list.");
        }
        if let Some(target) = clicked {
            self.selected = Some(target);
        }
        let has = self.selected.is_some();
        let (mut create, mut alter, mut delete, mut close) = (false, false, false, false);
        ui.horizontal(|ui| {
            create = ui.button("Create (Alt+C)").clicked();
            alter = ui
                .add_enabled(has, egui::Button::new("Alter (Ctrl+Enter)"))
                .clicked();
            delete = ui
                .add_enabled(has, egui::Button::new("Delete (Alt+D)"))
                .clicked();
            close = ui.button("Close (Esc)").clicked();
        });
        // A failed command leaves its message in the notice.
        if create {
            drop(self.begin_create(books));
        }
        if alter {
            drop(self.begin_alter(books));
        }
        if delete {
            drop(self.request_delete(books));
        }
        if close {
            self.filter.clear();
        }
    }

    fn draw_form_panel(&mut self, ui: &mut Ui, books: &Books) {
        let (mut save, mut cancel) = (false, false);
        if let Mode::Form(form) = &mut self.mode {
            let verb = if form.editing.is_some() {
                "Alter"
            } else {
                "Create"
            };
            let editing = form.editing;
            let kind = form.draft.kind();
            ui.group(|ui| {
                ui.label(format!("{verb} {}", kind.label()));
                form_ui::draw_form(ui, &books.masters, &mut form.draft, editing);
                ui.horizontal(|ui| {
                    save = ui.button("Save (Ctrl+A)").clicked();
                    cancel = ui.button("Cancel (Esc)").clicked();
                });
            });
        }
        if save {
            drop(self.save(books));
        }
        if cancel {
            drop(self.escape());
        }
    }

    fn draw_confirm(&mut self, ui: &mut Ui, books: &Books) {
        let Mode::ConfirmDelete(target) = self.mode else {
            return;
        };
        let name = snapshot_of(&books.masters, target).map_or_else(
            || "this master".to_owned(),
            |s| snapshot_name(&s).to_owned(),
        );
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("Warning: delete '{name}'? Press Y to delete, N to keep it."),
        );
        let (mut yes, mut no) = (false, false);
        ui.horizontal(|ui| {
            yes = ui.button("Delete (Y)").clicked();
            no = ui.button("Keep (N)").clicked();
        });
        if yes {
            drop(self.confirm_delete(books));
        }
        if no {
            self.mode = Mode::List;
        }
    }

    fn draw_notice(&self, ui: &mut Ui) {
        match &self.notice {
            Some(Notice::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {s}"));
            }
            Some(Notice::Warn(s)) => {
                ui.colored_label(ui.visuals().warn_fg_color, format!("Warning: {s}"));
            }
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
    }

    fn draw(&mut self, ui: &mut Ui, books: &Books) -> ScreenAction {
        let mut action = ScreenAction::Stay;
        let keys = read_keys(ui, self.mode_tag());
        for cmd in &keys {
            if matches!(self.handle_key(books, cmd), ScreenAction::Pop) {
                action = ScreenAction::Pop;
            }
        }
        ui.heading(self.title());
        self.draw_tabs(ui);
        match self.mode_tag() {
            ModeTag::List => self.draw_list(ui, books),
            ModeTag::Form => self.draw_form_panel(ui, books),
            ModeTag::Confirm => self.draw_confirm(ui, books),
        }
        self.draw_notice(ui);
        if !keys.is_empty() {
            ui.ctx().request_repaint();
        }
        action
    }
}

impl Screen for MastersScreen {
    fn title(&self) -> String {
        format!("Masters: {}", self.kind.plural())
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.posting.is_some() {
            self.wait_for_post(ui);
            return ScreenAction::Stay;
        }
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, books)
        } else {
            ui.label(no_books_text(ctx));
            ScreenAction::Stay
        };
        self.queue_requests();
        self.send_next(ctx);
        action
    }
}

/// What the screen says when it has no books to draw from.
fn no_books_text(ctx: &AppCtx) -> &'static str {
    if ctx.api.is_some() {
        "The company is open, but this screen cannot read its masters yet."
    } else {
        "No company is open."
    }
}

impl MastersScreen {
    /// Turns the requests a confirmed form or delete queued into commands. A request with no
    /// command in this build is refused with its reason.
    fn queue_requests(&mut self) {
        for request in self.take_requests() {
            match command_for(request) {
                Ok(cmd) => self.commands.push_back(cmd),
                Err(reason) => self.notice = Some(Notice::Error(reason)),
            }
        }
    }

    /// Sends the oldest queued command to the worker, when none is running.
    fn send_next(&mut self, ctx: &AppCtx) {
        if self.commands.is_empty() {
            return;
        }
        let Some(worker) = ctx.api.as_ref() else {
            self.commands.clear();
            self.notice = Some(Notice::Error(
                "No company is open; nothing was saved.".to_owned(),
            ));
            return;
        };
        if let Some(cmd) = self.commands.pop_front() {
            self.posting = Some(submit::send(worker, cmd));
            self.notice = Some(Notice::Info("Saving...".to_owned()));
        }
    }

    /// Polls the command in progress and shows "Saving..." until the worker answers.
    fn wait_for_post(&mut self, ui: &mut Ui) {
        let Some(ticket) = self.posting.as_mut() else {
            return;
        };
        match submit::settle(ticket) {
            Some(reply) => {
                self.posting = None;
                self.finish_post(reply);
                if !self.commands.is_empty() {
                    ui.ctx().request_repaint();
                }
            }
            None => {
                ui.label("Saving...");
                ui.ctx().request_repaint();
            }
        }
    }

    /// Shows the engine's answer for the command that ran.
    fn finish_post(&mut self, reply: CommandReply) {
        self.notice = Some(match reply {
            CommandReply::Booked(_) => Notice::Info("Saved.".to_owned()),
            CommandReply::Unsupported(u) => {
                Notice::Warn(format!("Not supported yet: {} ({})", u.what, u.reason))
            }
            CommandReply::Failed(reason) => Notice::Error(format!("Not saved: {reason}")),
        });
    }
}
