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
pub mod view;

use egui::{Align, Key, Layout, Modifiers, RichText, Sense, Ui};
use egui_extras::{Column, Size, StripBuilder, TableRow};
use ledgerly_core::{Outcome, Unsupported};
use ledgerly_model::{Books, MasterSnapshot, Masters};

use command::command_for;
use list::snapshot_name;
use thiserror::Error;

use crate::screen::KeyHint;
use crate::shell::submit::{self, CommandReply};
use crate::theme::{SPACE_2, SPACE_3, TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::empty::empty_state;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, table, tokens};
use crate::worker::{ApiWorker, Ticket};
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
    /// A ledger name to open the create form with, on the first frame that has books (the
    /// palette's `Create ledger "<name>"`).
    pending_create: Option<String>,
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
            pending_create: None,
        }
    }

    /// A ledger list that opens the create form with `name` filled in, as soon as the books are
    /// there (the palette's `Create ledger "<name>"`, `design/mock-v2/index.html:1129`). A form
    /// that cannot open says why, as Alt+C does.
    #[must_use]
    pub fn creating_ledger(name: impl Into<String>) -> Self {
        let mut screen = MastersScreen::with_kind(MasterKind::Ledger);
        screen.pending_create = Some(name.into());
        screen
    }

    /// Opens the create form the screen was made for, if any, now that `books` are there.
    fn start_pending_create(&mut self, books: &Books) {
        let Some(name) = self.pending_create.take() else {
            return;
        };
        if self.begin_create(books).is_ok()
            && let Some(MasterDraft::Ledger(draft)) = self.draft_mut()
        {
            draft.name = name;
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

/// The subtitle under the page title (mock v2).
const SUBTITLE: &str = "Ledgers and stock items. Type to filter. Alt+C creates anywhere.";
/// Width of the ledger detail panel beside the ledger table, in points.
const DETAIL_WIDTH: f32 = 300.0;
/// Height of a table body before it scrolls, in points.
const TABLE_MAX_HEIGHT: f32 = 460.0;
/// What a cell shows where a figure is not available yet.
const NOT_AVAILABLE: &str = "\u{2014}";
/// The header tooltip of a column whose figures the engine does not supply to this screen yet.
const NOT_AVAILABLE_HINT: &str = "Not available yet: the engine does not give this screen this figure. Nothing is estimated here.";

/// The search hint for a kind. It names only what the search really matches.
fn filter_hint(kind: MasterKind) -> String {
    match kind {
        MasterKind::Ledger => "Filter ledgers: name, GSTIN or alias".to_owned(),
        MasterKind::StockItem => "Filter stock items: name or alias".to_owned(),
        MasterKind::Group
        | MasterKind::VoucherType
        | MasterKind::Unit
        | MasterKind::StockGroup
        | MasterKind::Godown
        | MasterKind::Batch
        | MasterKind::CostCategory
        | MasterKind::CostCentre => {
            format!("Filter {}: name or alias", kind.plural().to_lowercase())
        }
    }
}

/// `text`, or the not-available dash when it is empty.
fn or_dash(text: &str) -> &str {
    if text.is_empty() { NOT_AVAILABLE } else { text }
}

impl MastersScreen {
    /// The primary tabs (Ledgers, Stock items). A kind behind "More master kinds" selects no tab.
    fn draw_tabs(&mut self, ui: &mut Ui) {
        let enabled = matches!(self.mode, Mode::List);
        let selected = view::primary_tab(self.kind).unwrap_or(view::PRIMARY_KINDS.len());
        let labels: Vec<&str> = view::PRIMARY_KINDS.iter().map(|k| k.plural()).collect();
        let mut chosen: Option<MasterKind> = None;
        ui.add_enabled_ui(enabled, |ui| {
            let response = tabs(ui, &labels, selected);
            chosen = response
                .clicked
                .and_then(|i| view::PRIMARY_KINDS.get(i).copied());
        });
        if let Some(kind) = chosen {
            self.select_kind(kind);
        }
    }

    /// The secondary control for the eight kinds that live under Settings (decision 8).
    fn draw_more_kinds(&mut self, ui: &mut Ui) {
        let enabled = matches!(self.mode, Mode::List);
        let shown = if view::primary_tab(self.kind).is_some() {
            "More master kinds".to_owned()
        } else {
            self.kind.plural().to_owned()
        };
        let current = self.kind;
        let mut chosen: Option<MasterKind> = None;
        ui.add_enabled_ui(enabled, |ui| {
            egui::ComboBox::from_id_salt("masters-more-kinds")
                .selected_text(shown.as_str())
                .show_ui(ui, |ui| {
                    for kind in view::MORE_KINDS {
                        if ui
                            .selectable_label(current == kind, kind.plural())
                            .clicked()
                        {
                            chosen = Some(kind);
                        }
                    }
                });
        });
        if let Some(kind) = chosen {
            self.select_kind(kind);
        }
    }

    /// The list for the kind being shown: the search box, then the table.
    fn draw_list(&mut self, ui: &mut Ui, books: &Books) {
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text(filter_hint(self.kind))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(SPACE_3);
        let kind = self.kind;
        match kind {
            MasterKind::Ledger => self.draw_ledgers(ui, books),
            MasterKind::StockItem => self.draw_items(ui, books),
            MasterKind::Group
            | MasterKind::VoucherType
            | MasterKind::Unit
            | MasterKind::StockGroup
            | MasterKind::Godown
            | MasterKind::Batch
            | MasterKind::CostCategory
            | MasterKind::CostCentre => self.draw_plain(ui, books),
        }
    }

    /// What a list with no rows shows: the empty state with its one action when the search is
    /// empty, or a line saying nothing matches.
    fn draw_no_rows(&mut self, ui: &mut Ui, books: &Books) {
        if self.filter.trim().is_empty() {
            let title = format!("No {} yet", self.kind.plural().to_lowercase());
            let sentence = format!("Create the first {} with Alt+C.", self.kind.label());
            let label = format!("Create {}", self.kind.label());
            if empty_state(ui, &title, &sentence, Some((label.as_str(), "Alt+C"))) {
                drop(self.begin_create(books));
            }
        } else {
            ui.weak(format!(
                "No {} matches '{}'.",
                self.kind.plural().to_lowercase(),
                self.filter
            ));
        }
    }

    /// The ledger table with the party detail panel beside it.
    fn draw_ledgers(&mut self, ui: &mut Ui, books: &Books) {
        let rows = view::ledger_rows(&books.masters, &self.filter);
        if rows.is_empty() {
            self.draw_no_rows(ui, books);
            return;
        }
        let selected = self.selected;
        let detail = selected.and_then(|s| view::ledger_detail(&books.masters, s.id));
        let mut clicked: Option<MasterRef> = None;
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(DETAIL_WIDTH))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    clicked = ledger_table(ui, &rows, selected);
                });
                strip.cell(|ui| {
                    ledger_detail_card(ui, detail.as_ref());
                });
            });
        self.list_footer(ui, books, clicked);
    }

    /// The stock item table.
    fn draw_items(&mut self, ui: &mut Ui, books: &Books) {
        let rows = view::item_rows(&books.masters, &self.filter);
        if rows.is_empty() {
            self.draw_no_rows(ui, books);
            return;
        }
        let clicked = item_table(ui, &rows, self.selected);
        self.list_footer(ui, books, clicked);
    }

    /// The table of a kind with no detail panel: the name and a short description.
    fn draw_plain(&mut self, ui: &mut Ui, books: &Books) {
        let rows = list_rows(&books.masters, self.kind, &self.filter);
        if rows.is_empty() {
            self.draw_no_rows(ui, books);
            return;
        }
        let clicked = plain_table(ui, &rows, self.selected);
        self.list_footer(ui, books, clicked);
    }

    /// Puts the cursor on a row the user clicked, then draws the Alter and Delete buttons under
    /// the list and runs the one that was clicked.
    fn list_footer(&mut self, ui: &mut Ui, books: &Books, clicked: Option<MasterRef>) {
        if let Some(target) = clicked {
            self.selected = Some(target);
        }
        let has = self.selected.is_some();
        let kind_label = self.kind.label();
        let alter_label = format!("Alter {kind_label}");
        let delete_label = format!("Delete {kind_label}");
        let (mut alter, mut delete) = (false, false);
        ui.add_space(SPACE_3);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(has, |ui| {
                alter =
                    button(ui, &alter_label, Some("Ctrl+Enter"), ButtonKind::Secondary).clicked();
                delete = button(ui, &delete_label, Some("Alt+D"), ButtonKind::Danger).clicked();
            });
        });
        // A failed command leaves its message in the notice.
        if alter {
            drop(self.begin_alter(books));
        }
        if delete {
            drop(self.request_delete(books));
        }
    }

    /// The create or alter form, in a card, with Save and Cancel.
    fn draw_form_panel(&mut self, ui: &mut Ui, books: &Books) {
        let (mut save, mut cancel) = (false, false);
        if let Mode::Form(form) = &mut self.mode {
            let verb = if form.editing.is_some() {
                "Alter"
            } else {
                "Create"
            };
            let editing = form.editing;
            let heading = format!("{verb} {}", form.draft.kind().label());
            card(ui, |ui| {
                let t = tokens(ui);
                let h2 = bold_font(ui, TEXT_H2);
                ui.label(RichText::new(heading.as_str()).font(h2).color(t.text));
                ui.add_space(SPACE_2);
                form_ui::draw_form(ui, &books.masters, &mut form.draft, editing);
                ui.add_space(SPACE_3);
                ui.horizontal(|ui| {
                    save = button(ui, "Save", Some("Ctrl+A"), ButtonKind::Primary).clicked();
                    cancel = button(ui, "Cancel", Some("Esc"), ButtonKind::Secondary).clicked();
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

    /// The delete question, in a card, with Delete and Keep.
    fn draw_confirm(&mut self, ui: &mut Ui, books: &Books) {
        let Mode::ConfirmDelete(target) = self.mode else {
            return;
        };
        let name = snapshot_of(&books.masters, target).map_or_else(
            || "this master".to_owned(),
            |s| snapshot_name(&s).to_owned(),
        );
        let (mut yes, mut no) = (false, false);
        card(ui, |ui| {
            note(
                ui,
                NoteKind::Warn,
                &format!("Delete '{name}'? Press Y to delete, N to keep it."),
            );
            ui.add_space(SPACE_2);
            ui.horizontal(|ui| {
                yes = button(ui, "Delete", Some("Y"), ButtonKind::Danger).clicked();
                no = button(ui, "Keep", Some("N"), ButtonKind::Secondary).clicked();
            });
        });
        if yes {
            drop(self.confirm_delete(books));
        }
        if no {
            self.mode = Mode::List;
        }
    }

    /// The message from the last command, as a note.
    fn draw_notice(&self, ui: &mut Ui) {
        match &self.notice {
            Some(Notice::Error(s)) => {
                note(ui, NoteKind::Bad, s);
            }
            Some(Notice::Warn(s)) => {
                note(ui, NoteKind::Warn, s);
            }
            Some(Notice::Info(s)) => {
                note(ui, NoteKind::Info, s);
            }
            None => {}
        }
    }

    /// The key bar hints for the mode now showing.
    fn key_hints(&self) -> Vec<KeyHint> {
        match self.mode_tag() {
            ModeTag::List => vec![
                KeyHint::new("Up / Down", "Move"),
                KeyHint::new("Alt+C", "Create"),
                KeyHint::new("Ctrl+Enter", "Alter"),
                KeyHint::new("Alt+D", "Delete"),
                KeyHint::new("Alt+Left / Right", "Change kind"),
                KeyHint::new("Esc", "Back"),
            ],
            ModeTag::Form => vec![
                KeyHint::new("Ctrl+A", "Save"),
                KeyHint::new("Esc", "Cancel"),
            ],
            ModeTag::Confirm => vec![
                KeyHint::new("Y", "Delete"),
                KeyHint::new("N", "Keep"),
                KeyHint::new("Esc", "Back"),
            ],
        }
    }

    fn draw(&mut self, ui: &mut Ui, books: &Books) -> ScreenAction {
        self.start_pending_create(books);
        let mut action = ScreenAction::Stay;
        let keys = read_keys(ui, self.mode_tag());
        for cmd in &keys {
            if matches!(self.handle_key(books, cmd), ScreenAction::Pop) {
                action = ScreenAction::Pop;
            }
        }
        let create_label = format!("Create {}", self.kind.label());
        let mut create = false;
        page_head(ui, "Masters", Some(SUBTITLE), |ui| {
            create = button(ui, &create_label, Some("Alt+C"), ButtonKind::Primary).clicked();
        });
        if create {
            drop(self.begin_create(books));
        }
        ui.add_space(SPACE_2);
        self.draw_tabs(ui);
        self.draw_more_kinds(ui);
        self.draw_notice(ui);
        match self.mode_tag() {
            ModeTag::List => self.draw_list(ui, books),
            ModeTag::Form => self.draw_form_panel(ui, books),
            ModeTag::Confirm => self.draw_confirm(ui, books),
        }
        if !keys.is_empty() {
            ui.ctx().request_repaint();
        }
        action
    }
}

/// The ledger table: name with its alias, group, GSTIN, and the balance column, which shows
/// "not available" until the engine supplies balances. Returns the row clicked this frame.
fn ledger_table(
    ui: &mut Ui,
    rows: &[view::LedgerRow],
    selected: Option<MasterRef>,
) -> Option<MasterRef> {
    let mut clicked = None;
    table::builder(ui)
        .sense(Sense::click())
        .max_scroll_height(TABLE_MAX_HEIGHT)
        .column(Column::remainder().at_least(160.0))
        .column(Column::initial(170.0).at_least(120.0))
        .column(Column::exact(150.0))
        .column(Column::exact(130.0))
        .header(table::HEADER_HEIGHT, |mut row| {
            table::header(&mut row, "Ledger", false);
            table::header(&mut row, "Group", false);
            table::header(&mut row, "GSTIN", false);
            hinted_header(&mut row, "Balance", true);
        })
        .body(|body| {
            body.rows(table::ROW_HEIGHT, rows.len(), |mut row| {
                let Some(item) = rows.get(row.index()) else {
                    return;
                };
                row.set_selected(selected == Some(item.master));
                row.col(|ui| name_cell(ui, &item.name, &item.alias));
                row.col(|ui| table::text(ui, &item.group));
                row.col(|ui| table::text(ui, or_dash(&item.gstin)));
                row.col(|ui| table::numeric(ui, NOT_AVAILABLE));
                if row.response().clicked() {
                    clicked = Some(item.master);
                }
            });
        });
    clicked
}

/// The stock item table: item, HSN (with a warning when it is missing), GST rate, and the stock
/// and value columns, which show "not available" until the engine supplies them.
fn item_table(
    ui: &mut Ui,
    rows: &[view::ItemRow],
    selected: Option<MasterRef>,
) -> Option<MasterRef> {
    let mut clicked = None;
    table::builder(ui)
        .sense(Sense::click())
        .max_scroll_height(TABLE_MAX_HEIGHT)
        .column(Column::remainder().at_least(200.0))
        .column(Column::exact(150.0))
        .column(Column::exact(90.0))
        .column(Column::exact(120.0))
        .column(Column::exact(170.0))
        .header(table::HEADER_HEIGHT, |mut row| {
            table::header(&mut row, "Item", false);
            table::header(&mut row, "HSN", false);
            table::header(&mut row, "GST", true);
            hinted_header(&mut row, "Stock", true);
            hinted_header(&mut row, "Value at average cost", true);
        })
        .body(|body| {
            body.rows(table::ROW_HEIGHT, rows.len(), |mut row| {
                let Some(item) = rows.get(row.index()) else {
                    return;
                };
                row.set_selected(selected == Some(item.master));
                row.col(|ui| table::text(ui, &item.name));
                row.col(|ui| {
                    if item.hsn.is_empty() {
                        badge(ui, BadgeKind::Bad, "Missing: add HSN");
                    } else {
                        table::text(ui, &item.hsn);
                    }
                });
                row.col(|ui| table::numeric(ui, or_dash(&item.gst_rate)));
                row.col(|ui| table::numeric(ui, NOT_AVAILABLE));
                row.col(|ui| table::numeric(ui, NOT_AVAILABLE));
                if row.response().clicked() {
                    clicked = Some(item.master);
                }
            });
        });
    clicked
}

/// The table of a kind with no detail panel: name and description.
fn plain_table(ui: &mut Ui, rows: &[MasterRow], selected: Option<MasterRef>) -> Option<MasterRef> {
    let mut clicked = None;
    table::builder(ui)
        .sense(Sense::click())
        .max_scroll_height(TABLE_MAX_HEIGHT)
        .column(Column::remainder().at_least(220.0))
        .column(Column::remainder())
        .header(table::HEADER_HEIGHT, |mut row| {
            table::header(&mut row, "Name", false);
            table::header(&mut row, "Details", false);
        })
        .body(|body| {
            body.rows(table::ROW_HEIGHT, rows.len(), |mut row| {
                let Some(item) = rows.get(row.index()) else {
                    return;
                };
                row.set_selected(selected == Some(item.master));
                row.col(|ui| table::text(ui, &item.name));
                row.col(|ui| table::text(ui, &item.detail));
                if row.response().clicked() {
                    clicked = Some(item.master);
                }
            });
        });
    clicked
}

/// A table header whose figures are not available yet: the label carries the tooltip that says
/// why.
fn hinted_header(row: &mut TableRow<'_, '_>, label: &str, numeric: bool) {
    row.col(|ui| {
        let t = tokens(ui);
        let text = RichText::new(label)
            .size(TEXT_META)
            .strong()
            .color(t.text_3);
        let layout = if numeric {
            Layout::right_to_left(Align::Center)
        } else {
            Layout::left_to_right(Align::Center)
        };
        ui.with_layout(layout, |ui| {
            ui.label(text).on_hover_text(NOT_AVAILABLE_HINT);
        });
    });
}

/// A name cell: the name, and the alias in small type after it.
fn name_cell(ui: &mut Ui, name: &str, alias: &str) {
    ui.horizontal(|ui| {
        table::text(ui, name);
        if !alias.is_empty() {
            let t = tokens(ui);
            ui.label(
                RichText::new(format!("alias {alias}"))
                    .size(TEXT_META)
                    .color(t.text_3),
            );
        }
    });
}

/// The party detail panel for the selected ledger. It shows the fields as stored, the GSTIN check,
/// and the balance and open bills as not available yet.
fn ledger_detail_card(ui: &mut Ui, detail: Option<&view::LedgerDetail>) {
    card(ui, |ui| {
        let Some(d) = detail else {
            ui.weak("Select a ledger to see its details.");
            return;
        };
        let t = tokens(ui);
        let h2 = bold_font(ui, TEXT_H2);
        ui.label(RichText::new(d.name.as_str()).font(h2).color(t.text));
        ui.add_space(SPACE_2);
        egui::Grid::new("masters-ledger-detail")
            .num_columns(2)
            .spacing([SPACE_3, SPACE_2])
            .show(ui, |ui| {
                detail_label(ui, "Under");
                detail_value(ui, &d.group);
                ui.end_row();

                detail_label(ui, "GSTIN");
                ui.vertical(|ui| {
                    ui.label(or_dash(&d.gstin));
                    form_ui::draw_gstin_check(ui, &d.gstin_check);
                });
                ui.end_row();

                detail_label(ui, "State");
                detail_value(ui, or_dash(&d.state));
                ui.end_row();

                detail_label(ui, "Credit days");
                let credit = d
                    .credit_days
                    .map_or_else(|| "Not set".to_owned(), |n| n.to_string());
                detail_value(ui, &credit);
                ui.end_row();

                detail_label(ui, "Bill-wise");
                if d.bill_wise {
                    badge(ui, BadgeKind::Ok, "On");
                } else {
                    badge(ui, BadgeKind::Neutral, "Off");
                }
                ui.end_row();

                detail_label(ui, "Address");
                detail_value(ui, or_dash(&d.address));
                ui.end_row();

                detail_label(ui, "Balance");
                ui.label(RichText::new(NOT_AVAILABLE).color(t.text))
                    .on_hover_text(NOT_AVAILABLE_HINT);
                ui.end_row();
            });
        ui.add_space(SPACE_3);
        let bills = bold_font(ui, TEXT_BODY);
        ui.label(RichText::new("Open bills").font(bills).color(t.text));
        ui.weak("Not available yet: open bills need bill-wise allocation, which the engine does not give this screen.");
    });
}

/// A label cell in the detail grid.
fn detail_label(ui: &mut Ui, label: &str) {
    let t = tokens(ui);
    ui.label(RichText::new(label).size(TEXT_META).color(t.text_3));
}

/// A value cell in the detail grid.
fn detail_value(ui: &mut Ui, value: &str) {
    let t = tokens(ui);
    ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
}

impl Screen for MastersScreen {
    fn title(&self) -> String {
        format!("Masters: {}", self.kind.plural())
    }

    fn page_title(&self) -> String {
        "Masters".to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        Some(SUBTITLE.to_owned())
    }

    fn keys(&self) -> Vec<KeyHint> {
        self.key_hints()
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
        // Without a worker the requests stay with the screen for the host to take.
        if let Some(worker) = ctx.api.as_ref() {
            self.queue_requests();
            self.send_next(worker);
        }
        action
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
    fn send_next(&mut self, worker: &ApiWorker) {
        if self.posting.is_some() {
            return;
        }
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
        if let Some(reply) = submit::settle(ticket) {
            self.posting = None;
            self.finish_post(reply);
            if !self.commands.is_empty() {
                ui.ctx().request_repaint();
            }
        } else {
            ui.label("Saving...");
            ui.ctx().request_repaint();
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
