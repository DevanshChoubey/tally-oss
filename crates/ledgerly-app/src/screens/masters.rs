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
//! nothing. The ledger list's closing balances are the engine's trial balance, asked for on the
//! API worker ([`balances`]). The item list's closing quantities and values, and its closing
//! stock total, are the engine's stock summary, asked for the same way ([`stock`]).
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

pub mod balances;
pub mod bills;
mod check;
mod command;
mod draft;
mod form_ui;
mod list;
mod look;
pub mod stock;
pub mod view;

use egui::{Align, Key, Layout, Modifiers, RichText, Ui};
use ledgerly_core::{Date, LedgerId, Outcome, Period, StockItemId, Unsupported};
use ledgerly_model::{Books, MasterSnapshot, Masters};

use command::command_for;
use list::snapshot_name;
use thiserror::Error;

use crate::screen::KeyHint;
use crate::shell::submit::{self, CommandReply};
use crate::theme::{SPACE_2, SPACE_3, TEXT_H2, TEXT_META};
use crate::ui::badge::BadgeKind;
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::card::card;
use crate::ui::empty::empty_state;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::prov::{self, ProvKind};
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, tokens};
use crate::worker::{ApiWorker, Ticket};
use crate::{AppCtx, Screen, ScreenAction};
use ledgerly_api::{Command, CreatedId};

pub use draft::{
    BankDraft, CostCentreDraft, GroupDraft, GstDraft, LedgerDraft, MasterDraft, PartyDraft,
    StockItemDraft, TaxDraft, TreeDraft, UNASSIGNED_ID, UnitDraft, creation_allowed, parse_rupees,
};
pub use form_ui::{address_id, line_ids};
pub use list::{MasterRow, list_rows, snapshot_of};
pub use look::filter_id;

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
    Open,
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
    /// Scroll the list to the cursor on the next frame (after J, K, Up or Down).
    scroll_to_selected: bool,
    /// The ledger list's Balance column, from the engine.
    balances: balances::LedgerBalances,
    /// The copy of the books and the date the balances were last asked for, so a change to
    /// either asks again.
    balances_for: Option<(crate::shell::BooksPrint, Result<Date, String>)>,
    /// The item list's Stock and value columns and its total line, from the engine.
    stock: stock::ItemStockFigures,
    /// The copy of the books and the period the stock figures were last asked for.
    stock_for: Option<(crate::shell::BooksPrint, Result<Period, String>)>,
    /// The Open bills card of the altered bill-wise party, from the engine.
    bills: bills::PartyBills,
    /// The copy of the books, the party and the period its bills were last asked for.
    bills_for: Option<(crate::shell::BooksPrint, LedgerId, Result<Period, String>)>,
    /// The ledger form waiting under a group form that Alt+C opened for its Under field.
    parent: Option<Box<FormState>>,
    /// A group created from a ledger's Under field is on its way to the engine: when it is
    /// created, its id goes into the ledger form's Under.
    awaiting_group: bool,
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
            scroll_to_selected: false,
            balances: balances::LedgerBalances::new(),
            balances_for: None,
            stock: stock::ItemStockFigures::new(),
            stock_for: None,
            bills: bills::PartyBills::new(),
            bills_for: None,
            parent: None,
            awaiting_group: false,
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
        if let Some(parent) = self.parent.take() {
            // The group made from the ledger's Under field: back to the ledger, whose Under
            // takes the group's id when the engine has created it.
            self.mode = Mode::Form(parent);
            self.awaiting_group = true;
            self.notice = Some(Notice::Info(format!(
                "'{name}' sent to be saved; it goes under this ledger once it is created."
            )));
            return Ok(());
        }
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
                    self.mode = self.parent.take().map_or(Mode::List, Mode::Form);
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

    /// Alt+C in a ledger form: opens a group form for the ledger's Under field, keeping the
    /// ledger form to come back to (TallyPrime: Alt+C on the Under field creates a group,
    /// help.tallysolutions.com/ledgers-in-tallyprime/). The ledger form's only master field is
    /// Under, so Alt+C anywhere in it means Under. Other forms have no such field here.
    fn begin_under_group(&mut self) {
        if self.parent.is_some() {
            return;
        }
        let Mode::Form(form) = &self.mode else {
            return;
        };
        if !matches!(form.draft, MasterDraft::Ledger(_)) {
            return;
        }
        let group = match MasterDraft::blank(MasterKind::Group) {
            Outcome::Value(d) => d,
            Outcome::Unsupported(u) => {
                self.notice = Some(Notice::Error(MasterError::from(u).to_string()));
                return;
            }
        };
        let Mode::Form(ledger) = std::mem::replace(&mut self.mode, Mode::List) else {
            return;
        };
        self.parent = Some(ledger);
        self.open_form(group, None);
        self.notice = Some(Notice::Info(
            "A new group for the ledger's Under. Ctrl+A saves it and goes back to the ledger; \
             Esc goes back without it."
                .to_owned(),
        ));
    }

    /// The engine created a master. A group made from a ledger's Under field goes into the
    /// ledger form's Under.
    fn master_created(&mut self, created: Option<CreatedId>) {
        if !self.awaiting_group {
            return;
        }
        let Some(CreatedId::Group(id)) = created else {
            return;
        };
        self.awaiting_group = false;
        let form = match &mut self.mode {
            Mode::Form(form) => Some(form),
            Mode::List | Mode::ConfirmDelete(_) => None,
        };
        if let Some(form) = form
            && let MasterDraft::Ledger(d) = &mut form.draft
        {
            d.group = Some(id);
        }
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
            KeyCmd::Open => return self.open_selected(books),
            KeyCmd::Save => drop(self.save(books)),
            KeyCmd::Create => {
                if matches!(self.mode, Mode::Form(_)) {
                    self.begin_under_group();
                } else {
                    drop(self.begin_create(books));
                }
            }
            KeyCmd::Alter => drop(self.begin_alter(books)),
            KeyCmd::Delete => drop(self.request_delete(books)),
            KeyCmd::Yes => drop(self.confirm_delete(books)),
            KeyCmd::No => self.mode = Mode::List,
            KeyCmd::Up => {
                self.move_selection(&books.masters, Step::Up);
                self.scroll_to_selected = true;
            }
            KeyCmd::Down => {
                self.move_selection(&books.masters, Step::Down);
                self.scroll_to_selected = true;
            }
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

fn read_keys(ui: &Ui, tag: ModeTag, kind: MasterKind) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    // J and K move only while no field has the keyboard, so they still type into the filter.
    let typing = ui.ctx().memory(|m| m.focused().is_some());
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
            let (modifiers, key) = bindings::CREATE;
            if consume_exact(i, key, modifiers) {
                out.push(KeyCmd::Create);
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
            if !typing {
                if consume_exact(i, Key::J, Modifiers::NONE) {
                    out.push(KeyCmd::Down);
                }
                if consume_exact(i, Key::K, Modifiers::NONE) {
                    out.push(KeyCmd::Up);
                }
            }
            // Enter opens a ledger's vouchers; the other kinds have nothing to open
            // (index.html:1797-1799).
            if kind == MasterKind::Ledger && consume_exact(i, Key::Enter, Modifiers::NONE) {
                out.push(KeyCmd::Open);
            }
        }
    });
    out
}

/// The subtitle under the page title (mock v2).
const SUBTITLE: &str = "Ledgers and stock items. Type to filter. Alt+C creates anywhere.";
/// The gap under the tabs' rule (`.tabs{margin-bottom:14px}`, index.html:139).
const TABS_GAP: f32 = 14.0;
/// The widest the create form grows (`style="max-width:880px"`, index.html:1808).
const FORM_MAX_WIDTH: f32 = 880.0;
/// What a cell shows where a figure is not available yet.
const NOT_AVAILABLE: &str = "\u{2014}";
/// The header tooltip of a column whose figures the engine does not supply to this screen yet.
const NOT_AVAILABLE_HINT: &str = "Not available yet: the engine does not give this screen this figure. Nothing is estimated here.";
/// What the form's foot says when it has a GSTIN field (index.html:1812).
const GSTIN_FOOT: &str =
    "The check character is verified on this computer. Ledgerly does not look the GSTIN up online.";
/// What the open bills card says before the engine's answer.
const OPEN_BILLS_LOADING: &str =
    "The open bills have not come from the engine yet. Nothing is estimated here.";
/// What the open bills card says when the party has nothing open.
const OPEN_BILLS_NONE: &str = "Nothing is open for this party.";

/// The ledger list's columns (index.html:1796), with the Balance heading's hover text and its
/// "engine" chip set from what the balances are now.
fn ledger_cols(balances: &balances::LedgerBalances) -> [look::Col; 4] {
    let mut cols = LEDGER_COLS;
    if let Some(balance) = cols.last_mut() {
        balance.hint = balances.heading_hint();
        balance.tag = balance
            .hint
            .is_none()
            .then_some(prov::label(ProvKind::Engine));
    }
    cols
}

/// The ledger list's columns before the balances are known. Widths are read off the mock's
/// capture at 1280 by 800; the ledger name takes what is left.
const LEDGER_COLS: [look::Col; 4] = [
    look::Col {
        label: "Ledger",
        width: None,
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "Group",
        width: Some(276.0),
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "GSTIN",
        width: Some(200.0),
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "Balance",
        width: Some(197.0),
        numeric: true,
        hint: Some(NOT_AVAILABLE_HINT),
        tag: None,
    },
];

/// The stock item list's columns (index.html:1798), with the Stock and value headings' hover
/// text and the value's "stock engine" chip set from what the figures are now.
fn item_cols(figures: &stock::ItemStockFigures) -> [look::Col; 5] {
    let mut cols = ITEM_COLS;
    let hint = figures.heading_hint();
    let [.., qty, value] = &mut cols;
    qty.hint = hint;
    value.hint = hint;
    value.tag = hint.is_none().then_some(STOCK_ENGINE_TAG);
    cols
}

/// The value heading's provenance chip (`prov('eng','stock engine')`, index.html:1798).
const STOCK_ENGINE_TAG: &str = "stock engine";

/// The stock item list's columns before the figures are known, widths read off the mock's
/// capture. The mock's value heading is "Value at average cost"; the engine values each item by
/// its own method (average cost, FIFO, last purchase or standard cost), so the heading is the
/// engine's own "Closing value" (design/PARITY-GAPS.md).
const ITEM_COLS: [look::Col; 5] = [
    look::Col {
        label: "Item",
        width: Some(296.0),
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "HSN",
        width: Some(213.0),
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "GST",
        width: Some(72.0),
        numeric: true,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "Stock",
        width: Some(116.0),
        numeric: true,
        hint: None,
        tag: None,
    },
    look::Col {
        label: stock::VALUE_HEADING,
        width: None,
        numeric: true,
        hint: None,
        tag: None,
    },
];

/// The columns of a kind the mock does not draw: its name and a short description.
const PLAIN_COLS: [look::Col; 2] = [
    look::Col {
        label: "Name",
        width: None,
        numeric: false,
        hint: None,
        tag: None,
    },
    look::Col {
        label: "Details",
        width: None,
        numeric: false,
        hint: None,
        tag: None,
    },
];

/// `kind`'s singular name with a capital, for the key bar ("Ledger", index.html:1831).
fn capitalised(kind: MasterKind) -> String {
    let label = kind.label();
    let mut chars = label.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
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

    /// The control for the eight kinds that live under Settings (decision 8). The mock has no
    /// such control; it sits in the page head beside Create so the list keeps the mock's place
    /// (design/PARITY-GAPS.md).
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
            // Drawn as the kit's secondary button.
            crate::ui::button::style_combo(ui);
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

    /// The list for the kind being shown: the filter box, then the list in a card. The cursor is
    /// kept on a row the list shows, the first one when it is not (index.html:1797, `sel` on the
    /// first row). Returns what a double click asked for.
    fn draw_list(&mut self, ui: &mut Ui, books: &Books) -> Option<KeyCmd> {
        let hint = view::filter_hint(self.kind);
        look::filter_box(ui, &mut self.filter, &hint);
        ui.add_space(look::FILTER_GAP);
        let refs = self.visible_refs(&books.masters);
        if refs.is_empty() {
            self.draw_no_rows(ui, books);
            return None;
        }
        if !self.selected.is_some_and(|s| refs.contains(&s)) {
            self.selected = refs.first().copied();
        }
        let selected = self
            .selected
            .and_then(|s| refs.iter().position(|r| *r == s));
        let scroll = std::mem::take(&mut self.scroll_to_selected);
        let kind = self.kind;
        let events = match kind {
            MasterKind::Ledger => {
                if let Some(text) = self.balance_note() {
                    note(ui, NoteKind::Warn, &text);
                    ui.add_space(look::FILTER_GAP);
                }
                let rows = ledger_cells(
                    &view::ledger_rows(&books.masters, &self.filter),
                    &self.balances,
                );
                let cols = ledger_cols(&self.balances);
                look::list_card(ui, "ledgers", &cols, &rows, selected, scroll, None)
            }
            MasterKind::StockItem => {
                if let Some(text) = self.stock_note() {
                    note(ui, NoteKind::Warn, &text);
                    ui.add_space(look::FILTER_GAP);
                }
                let rows = item_cells(&view::item_rows(&books.masters, &self.filter), &self.stock);
                let cols = item_cols(&self.stock);
                let total = self.stock.total_text().map(|value| look::TotalLine {
                    label: CLOSING_STOCK,
                    value,
                    why: self.stock.total_why(),
                });
                look::list_card(ui, "items", &cols, &rows, selected, scroll, total.as_ref())
            }
            MasterKind::Group
            | MasterKind::VoucherType
            | MasterKind::Unit
            | MasterKind::StockGroup
            | MasterKind::Godown
            | MasterKind::Batch
            | MasterKind::CostCategory
            | MasterKind::CostCentre => {
                let rows = plain_cells(&list_rows(&books.masters, kind, &self.filter));
                look::list_card(ui, "plain", &PLAIN_COLS, &rows, selected, scroll, None)
            }
        };
        if let Some(i) = events.clicked.or(events.opened) {
            self.selected = refs.get(i).copied();
        }
        events.opened.map(|_| {
            if kind == MasterKind::Ledger {
                KeyCmd::Open
            } else {
                KeyCmd::Alter
            }
        })
    }

    /// The masters the list shows now, in its order.
    fn visible_refs(&self, masters: &Masters) -> Vec<MasterRef> {
        list_rows(masters, self.kind, &self.filter)
            .into_iter()
            .map(|r| r.master)
            .collect()
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

    /// The create or alter form as the mock's sheet (index.html:1802-1812): the fields in a grid
    /// of four columns, then a foot with a note and Cancel and Save. Altering a bill-wise ledger
    /// puts the open bills card beside it (index.html:1806).
    fn draw_form_panel(&mut self, ui: &mut Ui, books: &Books) {
        let mut clicks = FormClicks::default();
        if let Mode::Form(form) = &mut self.mode {
            let editing = form.editing;
            let bills =
                editing.is_some() && matches!(&form.draft, MasterDraft::Ledger(d) if d.bill_wise);
            let foot = match &form.draft {
                MasterDraft::Ledger(d) if d.party.is_some() => Some(GSTIN_FOOT),
                MasterDraft::Ledger(_)
                | MasterDraft::Group(_)
                | MasterDraft::Unit(_)
                | MasterDraft::StockGroup(_)
                | MasterDraft::Godown(_)
                | MasterDraft::CostCategory(_)
                | MasterDraft::CostCentre(_)
                | MasterDraft::StockItem(_) => None,
            };
            // An altered ledger shows its closing balance as the engine gave it (index.html:1805).
            let balance = form_balance(form, &self.balances);
            let balance = balance.as_deref();
            let party_bills = &self.bills;
            let draft = &mut form.draft;
            // A form with every section on is taller than the window, so it scrolls and Save
            // stays in reach.
            egui::ScrollArea::vertical()
                .id_salt("masters-form")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if bills {
                        crate::screens::home::draw::split(
                            ui,
                            |ui| {
                                clicks = sheet(ui, &books.masters, draft, editing, foot, balance);
                            },
                            |ui| open_bills_card(ui, party_bills),
                        );
                    } else {
                        let width = ui.available_width().min(FORM_MAX_WIDTH);
                        ui.allocate_ui_with_layout(
                            egui::vec2(width, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(width);
                                clicks = sheet(ui, &books.masters, draft, editing, foot, balance);
                            },
                        );
                    }
                });
        }
        if clicks.save {
            drop(self.save(books));
        }
        if clicks.cancel {
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
        let shown = match &self.notice {
            Some(Notice::Error(s)) => Some((NoteKind::Bad, s)),
            Some(Notice::Warn(s)) => Some((NoteKind::Warn, s)),
            Some(Notice::Info(s)) => Some((NoteKind::Info, s)),
            None => None,
        };
        if let Some((kind_of, text)) = shown {
            note(ui, kind_of, text);
            ui.add_space(SPACE_3);
        }
    }

    /// The key bar hints for the mode now showing (index.html:1831). Enter is offered only on the
    /// ledgers, where it opens the ledger's vouchers; Alt+D and Alt+Left / Right are this
    /// screen's own keys and come before Esc.
    fn key_hints(&self) -> Vec<KeyHint> {
        match self.mode_tag() {
            ModeTag::List => {
                let mut keys = vec![KeyHint::new("J / K", "Move")];
                if self.kind == MasterKind::Ledger {
                    keys.push(KeyHint::new("Enter", "Open"));
                }
                keys.extend([
                    KeyHint::new("Alt+C", "Create"),
                    KeyHint::new("Ctrl+Enter", "Alter"),
                    KeyHint::new("Alt+D", "Delete"),
                    KeyHint::new("Alt+Left / Right", "Change kind"),
                    KeyHint::new("Esc", "Back"),
                ]);
                keys
            }
            ModeTag::Form => vec![
                KeyHint::new("Enter", "Next field"),
                KeyHint::new("Ctrl+A", "Save"),
                KeyHint::new("Esc", "Back"),
            ],
            ModeTag::Confirm => vec![
                KeyHint::new("Y", "Delete"),
                KeyHint::new("N", "Keep"),
                KeyHint::new("Esc", "Back"),
            ],
        }
    }

    /// The ledger vouchers report of the ledger under the cursor (index.html:1797), or a notice
    /// saying why it cannot open.
    fn open_selected(&mut self, books: &Books) -> ScreenAction {
        let Some(target) = self.selected.filter(|s| s.kind == MasterKind::Ledger) else {
            self.notice = Some(Notice::Error(MasterError::NothingSelected.to_string()));
            return ScreenAction::Stay;
        };
        let kind = ledgerly_api::ReportKind::LedgerVouchers(ledgerly_core::LedgerId(target.id));
        match crate::shell::navigate::report_screen(kind, books) {
            Ok(screen) => ScreenAction::Push(Box::new(screen)),
            Err(e) => {
                self.notice = Some(Notice::Error(e.to_string()));
                ScreenAction::Stay
            }
        }
    }

    fn draw(&mut self, ui: &mut Ui, books: &Books) -> ScreenAction {
        self.start_pending_create(books);
        let mut action = ScreenAction::Stay;
        let mut keys = read_keys(ui, self.mode_tag(), self.kind);
        let create_label = format!("Create {}", self.kind.label());
        let mut create = false;
        page_head(ui, "Masters", Some(SUBTITLE), |ui| {
            create = button_icon(
                ui,
                Some(Icon::Plus),
                &create_label,
                Some("Alt+C"),
                ButtonKind::Primary,
            )
            .clicked();
            self.draw_more_kinds(ui);
        });
        if create {
            drop(self.begin_create(books));
        }
        self.draw_tabs(ui);
        ui.add_space(TABS_GAP);
        self.draw_notice(ui);
        match self.mode_tag() {
            ModeTag::List => keys.extend(self.draw_list(ui, books)),
            ModeTag::Form => self.draw_form_panel(ui, books),
            ModeTag::Confirm => self.draw_confirm(ui, books),
        }
        for cmd in &keys {
            let next = self.handle_key(books, cmd);
            if matches!(action, ScreenAction::Stay) {
                action = next;
            }
        }
        if !keys.is_empty() {
            ui.ctx().request_repaint();
        }
        action
    }
}

/// What the form's buttons were asked for this frame.
#[derive(Default)]
struct FormClicks {
    save: bool,
    cancel: bool,
}

/// The form sheet: the fields, a rule, then the foot with `foot` (when given) and the buttons.
fn sheet(
    ui: &mut Ui,
    masters: &Masters,
    draft: &mut MasterDraft,
    editing: Option<u64>,
    foot: Option<&str>,
    balance: Option<&str>,
) -> FormClicks {
    let t = tokens(ui);
    let k = crate::ui::kind(ui);
    let mut clicks = FormClicks::default();
    egui::Frame::new()
        .fill(t.surface)
        .stroke(egui::Stroke::new(crate::theme::border_width(k), t.border))
        .corner_radius(crate::theme::RADIUS_M)
        .shadow(crate::theme::shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::Frame::new()
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    form_ui::draw_form(ui, masters, draft, editing, balance);
                });
            let rule = ui.available_rect_before_wrap();
            ui.painter().hline(
                rule.x_range(),
                rule.top() + 0.5,
                egui::Stroke::new(crate::theme::border_width(k), t.border),
            );
            ui.add_space(1.0);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_2;
                        let buttons_w = 340.0_f32.min(ui.available_width() / 2.0);
                        let note_w = (ui.available_width() - buttons_w - 16.0).max(0.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(note_w, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(note_w);
                                if let Some(text) = foot {
                                    ui.label(RichText::new(text).size(TEXT_META).color(t.text_2));
                                }
                            },
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            clicks.save = button_icon(
                                ui,
                                Some(Icon::Save),
                                "Save",
                                Some("Ctrl+A"),
                                ButtonKind::Primary,
                            )
                            .clicked();
                            clicks.cancel =
                                button(ui, "Cancel", Some("Esc"), ButtonKind::Secondary).clicked();
                        });
                    });
                });
        });
    clicks
}

/// The Balance field of an alter form of a ledger: its closing balance as the engine gave it
/// ([`balances::LedgerBalances::text`]). A create form has none.
fn form_balance(form: &FormState, balances: &balances::LedgerBalances) -> Option<String> {
    form.editing
        .filter(|_| matches!(&form.draft, MasterDraft::Ledger(_)))
        .map(|id| balances.text(LedgerId(id)))
}

/// The open bills card beside an altered bill-wise ledger (index.html:1806): each open line's
/// name, date, open amount and age, as the engine's outstanding report gives them
/// ([`bills::PartyBills`]).
fn open_bills_card(ui: &mut Ui, party: &bills::PartyBills) {
    card(ui, |ui| {
        let t = tokens(ui);
        let h2 = bold_font(ui, TEXT_H2);
        ui.label(RichText::new("Open bills").font(h2).color(t.text));
        ui.add_space(SPACE_2);
        let quiet = |ui: &mut Ui, text: &str| {
            ui.label(RichText::new(text).size(TEXT_META).color(t.text_2));
        };
        match party.state() {
            bills::BillsState::NotAsked => quiet(ui, OPEN_BILLS_LOADING),
            bills::BillsState::Unsupported(why) => quiet(
                ui,
                &format!("Open bills are not shown: {why}. Nothing is estimated here."),
            ),
            bills::BillsState::Loaded(lines) if lines.is_empty() => quiet(ui, OPEN_BILLS_NONE),
            bills::BillsState::Loaded(lines) => bills_table(ui, lines),
        }
    });
}

/// The open lines in the `table.t` rules: name, date (secondary ink), amount, age badge.
fn bills_table(ui: &mut Ui, lines: &[bills::BillLine]) {
    use crate::ui::table::{self, CellPlace, RowState, T_ROW_HEIGHT};
    let t = tokens(ui);
    let font = egui::FontId::proportional(crate::theme::TEXT_BODY);
    ui.spacing_mut().item_spacing.y = 0.0;
    let width = ui.available_width();
    let texts: Vec<(String, String)> = lines
        .iter()
        .map(|line| {
            let date = line.date.map_or_else(String::new, crate::ui::format::date);
            let amount = line.pending.as_ref().map_or_else(
                |_| NOT_AVAILABLE.to_owned(),
                |a| crate::screens::report_view::amount_text(*a),
            );
            (date, amount)
        })
        .collect();
    // Each text column is as wide as its widest text, so nothing overlaps in a narrow card; the
    // badges take what is left.
    let widest = |each: &mut dyn Iterator<Item = &str>| {
        each.map(|text| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font.clone(), t.text)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max)
            + 2.0 * table::CELL_PAD_X
    };
    let name_w = widest(&mut lines.iter().map(|l| l.name.as_str()));
    let date_w = widest(&mut texts.iter().map(|(d, _)| d.as_str()));
    let amount_w = widest(&mut texts.iter().map(|(_, a)| a.as_str()));
    for (line, (date, amount)) in lines.iter().zip(&texts) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, T_ROW_HEIGHT), egui::Sense::hover());
        table::paint_row(
            ui,
            rect,
            RowState {
                selected: false,
                hovered: false,
                sub: false,
            },
        );
        let col = |from: f32, to: f32| {
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + from, rect.top()),
                egui::pos2(rect.left() + to, rect.bottom()),
            )
        };
        let left = CellPlace::Left(table::CELL_PAD_X);
        let date_at = name_w;
        let amount_at = date_at + date_w;
        let badge_at = amount_at + amount_w;
        let cells = [
            (0.0, date_at, line.name.as_str(), t.text, left),
            (date_at, amount_at, date.as_str(), t.text_2, left),
            (
                amount_at,
                badge_at,
                amount.as_str(),
                t.text,
                CellPlace::Right,
            ),
        ];
        for (from, to, text, color, place) in cells {
            table::paint_cell_text(ui, col(from, to), 1.0, text, font.clone(), color, place);
        }
        if let Some(badge) = bills::age_badge(&line.age) {
            age_badge(ui, col(badge_at, rect.width().max(badge_at)), &badge);
        }
    }
}

/// Draws an open bill's age badge in `cell`.
fn age_badge(ui: &mut Ui, cell: egui::Rect, (text, warn): &(String, bool)) {
    let inner = egui::Rect::from_min_max(
        egui::pos2(cell.left() + crate::ui::table::CELL_PAD_X, cell.top()),
        egui::pos2(
            cell.right() - crate::ui::table::CELL_PAD_X,
            cell.bottom() - 1.0,
        ),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let kind = if *warn {
        BadgeKind::Warn
    } else {
        BadgeKind::Neutral
    };
    crate::ui::badge::badge(&mut child, kind, text);
}

/// The ledger list's cells: the name with its alias, the group and GSTIN in the secondary ink
/// (blank for none), and the balance as the engine gave it ([`balances::LedgerBalances::text`]).
fn ledger_cells(
    rows: &[view::LedgerRow],
    balances: &balances::LedgerBalances,
) -> Vec<Vec<look::Cell>> {
    rows.iter()
        .map(|r| {
            vec![
                look::Cell::Named {
                    name: r.name.clone(),
                    alias: r.alias.clone(),
                },
                look::Cell::Quiet(r.group.clone()),
                look::Cell::Quiet(r.gstin.clone()),
                look::Cell::text_why(
                    balances.text(LedgerId(r.master.id)),
                    balances.why(LedgerId(r.master.id)).map(str::to_owned),
                ),
            ]
        })
        .collect()
}

/// The label of the item list's total line (index.html:1800).
const CLOSING_STOCK: &str = "Closing stock";

/// The stock item list's cells: the name, the HSN (or the missing-HSN badge), the GST rate, and
/// the closing quantity and value as the engine gave them ([`stock::ItemStockFigures`]).
fn item_cells(rows: &[view::ItemRow], figures: &stock::ItemStockFigures) -> Vec<Vec<look::Cell>> {
    rows.iter()
        .map(|r| {
            let hsn = if r.hsn.is_empty() {
                look::Cell::Badge(BadgeKind::Bad, "Missing: add HSN")
            } else {
                look::Cell::Text(r.hsn.clone())
            };
            let rate = if r.gst_rate.is_empty() {
                NOT_AVAILABLE.to_owned()
            } else {
                r.gst_rate.clone()
            };
            vec![
                look::Cell::Text(r.name.clone()),
                hsn,
                look::Cell::Text(rate),
                look::Cell::text_why(
                    figures.qty_text(StockItemId(r.master.id), &r.unit),
                    figures.qty_why(StockItemId(r.master.id)),
                ),
                look::Cell::text_why(
                    figures.value_text(StockItemId(r.master.id)),
                    figures.value_why(StockItemId(r.master.id)),
                ),
            ]
        })
        .collect()
}

/// The cells of a kind with no list of its own in the mock: the name and the description.
fn plain_cells(rows: &[MasterRow]) -> Vec<Vec<look::Cell>> {
    rows.iter()
        .map(|r| {
            vec![
                look::Cell::Text(r.name.clone()),
                look::Cell::Quiet(r.detail.clone()),
            ]
        })
        .collect()
}

impl Screen for MastersScreen {
    fn title(&self) -> String {
        // The breadcrumb reads "Masters" on every kind (index.html:1794).
        "Masters".to_owned()
    }

    fn key_bar_name(&self) -> String {
        // The form's key bar is named after what it makes, "Ledger" (index.html:1831).
        match self.mode_tag() {
            ModeTag::Form => self
                .draft()
                .map_or_else(|| "Masters".to_owned(), |d| capitalised(d.kind())),
            ModeTag::List | ModeTag::Confirm => "Masters".to_owned(),
        }
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
            self.follow_balances(books);
            self.balances.tick(ctx.api.as_ref());
            self.follow_stock(books);
            self.stock.tick(ctx.api.as_ref());
            self.follow_bills(books);
            self.bills.tick(ctx.api.as_ref());
            let loading =
                self.balances.is_loading() || self.stock.is_loading() || self.bills.is_loading();
            if loading && ctx.api.is_some() {
                ui.ctx().request_repaint();
            }
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

impl MastersScreen {
    /// Asks for the balances again when `books` is a copy, or the date is one, they were not
    /// asked for: on the first frame, after anything is saved, and when the books arrive with a
    /// later last voucher date. They are as of the last voucher date, the date the balance sheet
    /// opens on (`navigate::default_period`).
    fn follow_balances(&mut self, books: &Books) {
        let print = crate::shell::books_print(books);
        let as_of = crate::shell::navigate::default_period(books)
            .map(ledgerly_core::Period::to)
            .map_err(|e| e.to_string());
        let key = (print, as_of);
        if self.balances_for.as_ref() == Some(&key) {
            return;
        }
        match &key.1 {
            Ok(date) => self.balances.request(*date),
            Err(why) => self
                .balances
                .set(balances::BalanceState::Unsupported(why.clone())),
        }
        self.balances_for = Some(key);
    }

    /// Asks for the stock figures again when `books` is a copy, or the period is one, they were
    /// not asked for, as [`MastersScreen::follow_balances`] does. The period is the one the stock
    /// summary opens with (`navigate::default_period`): the financial year to its last voucher.
    fn follow_stock(&mut self, books: &Books) {
        let print = crate::shell::books_print(books);
        let period = crate::shell::navigate::default_period(books).map_err(|e| e.to_string());
        let key = (print, period);
        if self.stock_for.as_ref() == Some(&key) {
            return;
        }
        match &key.1 {
            Ok(period) => self.stock.request(*period),
            Err(why) => self.stock.set(stock::StockState::Unsupported(why.clone())),
        }
        self.stock_for = Some(key);
    }

    /// The Balance field of the open alter form of a ledger (index.html:1805), or `None` when no
    /// ledger is being altered.
    #[must_use]
    pub fn form_balance_text(&self) -> Option<String> {
        match &self.mode {
            Mode::Form(form) => form_balance(form, &self.balances),
            Mode::List | Mode::ConfirmDelete(_) => None,
        }
    }

    /// The bill-wise party the open form alters, if any: the one whose Open bills card shows.
    fn bills_party(&self) -> Option<LedgerId> {
        match &self.mode {
            Mode::Form(form) => form
                .editing
                .filter(|_| matches!(&form.draft, MasterDraft::Ledger(d) if d.bill_wise))
                .map(LedgerId),
            Mode::List | Mode::ConfirmDelete(_) => None,
        }
    }

    /// Asks for the open bills when a bill-wise party's alter form is open and its bills were not
    /// asked for with this copy of the books and this period. They are as of the last voucher
    /// date, as the balances are.
    fn follow_bills(&mut self, books: &Books) {
        let Some(party) = self.bills_party() else {
            return;
        };
        let print = crate::shell::books_print(books);
        let period = crate::shell::navigate::default_period(books).map_err(|e| e.to_string());
        let key = (print, party, period);
        if self.bills_for.as_ref() == Some(&key) {
            return;
        }
        match &key.2 {
            Ok(period) => self.bills.request(party, *period),
            Err(why) => self.bills.set(bills::BillsState::Unsupported(why.clone())),
        }
        self.bills_for = Some(key);
    }

    /// The Open bills card's bills, as the screen holds them.
    #[must_use]
    pub fn bills(&self) -> &bills::PartyBills {
        &self.bills
    }

    /// The Open bills card's bills, to set them as an answer from the worker would.
    pub fn bills_mut(&mut self) -> &mut bills::PartyBills {
        &mut self.bills
    }

    /// The item list's stock figures, as the screen holds them.
    #[must_use]
    pub fn stock(&self) -> &stock::ItemStockFigures {
        &self.stock
    }

    /// The item list's stock figures, to set them as an answer from the worker would.
    pub fn stock_mut(&mut self) -> &mut stock::ItemStockFigures {
        &mut self.stock
    }

    /// The note drawn above the item list when the engine could not give the stock figures,
    /// with the engine's own reason (rule A1).
    #[must_use]
    pub fn stock_note(&self) -> Option<String> {
        self.stock
            .notice()
            .map(|why| format!("Stock figures are not shown: {why}. Nothing is estimated here."))
    }

    /// The ledger list's balances, as the screen holds them.
    #[must_use]
    pub fn balances(&self) -> &balances::LedgerBalances {
        &self.balances
    }

    /// The ledger list's balances, to set them as an answer from the worker would.
    pub fn balances_mut(&mut self) -> &mut balances::LedgerBalances {
        &mut self.balances
    }

    /// The note drawn above the ledger list when the engine could not give the balances, with
    /// the engine's own reason (rule A1).
    #[must_use]
    pub fn balance_note(&self) -> Option<String> {
        self.balances
            .notice()
            .map(|why| format!("Balances are not shown: {why}. Nothing is estimated here."))
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
        if !matches!(reply, CommandReply::Booked(_)) {
            self.awaiting_group = false;
        }
        self.notice = Some(match reply {
            CommandReply::Booked(result) => {
                self.master_created(result.created);
                Notice::Info("Saved.".to_owned())
            }
            CommandReply::Unsupported(u) => {
                Notice::Warn(format!("Not supported yet: {} ({})", u.what, u.reason))
            }
            CommandReply::Failed(reason) => Notice::Error(format!("Not saved: {reason}")),
        });
    }
}
