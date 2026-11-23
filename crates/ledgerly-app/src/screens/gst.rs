//! GST workspace. **Card U8** implements this module.
//!
//! Frozen interface (plan card U8):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Gst`. The screen has three tabs numbered as the mock numbers them: GSTR-1 (key
//! `1`), GSTR-3B (key `2`) and 2B and IMS (key `3`). It opens on GSTR-1 when the
//! shell opens it ([`crate::shell::navigate::gst_screen`]).
//!
//! * **Exception buckets** (spec 6.3): Ready, Not relevant, Mismatch and Needs correction. Every
//!   item arrives with its bucket already decided by the engine; the screen never classifies a
//!   document itself. Counts are shown on the status tiles, which also filter the table. Enter
//!   opens the voucher to fix on top of the screen. [`GstScreen::take_reapply_request`] records a
//!   bulk re-apply for a host; no button is drawn while the shell does not take it
//!   (`design/PARITY-GAPS.md`). The drawing is in the `draw` module, laid out as the mock-v2
//!   GST screen.
//! * **2B and IMS table**: one row per supplier invoice. J and K (or the arrow keys) move the
//!   row, A accepts, R rejects, P marks pending, Enter opens the voucher. The engine's suggestion
//!   is shown beside the buttons but is never applied: an action exists only after the user
//!   presses a key or clicks a button on that row (spec 6.3 "user confirms every action"). The
//!   confirmed actions are handed to the host for the action file
//!   ([`GstScreen::confirmed_actions`], [`GstScreen::take_export_request`]); nothing is uploaded
//!   from here.
//!
//! Rule C4 (the UI never sums anything): every amount, including the difference column, is
//! supplied by the host from the engine. A missing amount shows as `none`, never as `0.00`. The
//! only numbers the screen computes are counts of rows. The reconciliation, GSTR-1 and GSTR-3B
//! data arrive through [`GstScreen::set_recon`], [`GstScreen::set_gstr1`] and
//! [`GstScreen::set_gstr3b`] as a [`Feed`], so a return the engine cannot build yet is shown as
//! "Not supported yet" and never as an empty table.
//!
//! The types here are the screen's own view model. The matching engine (card G11) and the IMS file
//! format (card F3) are not wired in yet (Wave 2); [`ConfirmedAction`] carries what the action
//! file needs. Credit notes that may stay pending for one period (spec 6.3) are not modelled: a
//! row's `Pending` action is the user's choice, and the period rule is the engine's.

use egui::{Key, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, Gstin, Paise, StateCode, Unsupported, VoucherId};
use thiserror::Error;

use crate::{AppCtx, Screen, ScreenAction};

mod draw;
pub mod look;
mod worker;

pub use worker::{FigureRow, Figures};

// ---------------------------------------------------------------------------------------------
// View model
// ---------------------------------------------------------------------------------------------

/// The three tabs of the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GstTab {
    /// GSTR-1 (outward supplies).
    Gstr1,
    /// GSTR-3B (summary return).
    Gstr3b,
    /// GSTR-2B and IMS reconciliation.
    Reconciliation,
}

impl GstTab {
    /// The tabs in display order (as in the mock).
    pub const ALL: [GstTab; 3] = [GstTab::Gstr1, GstTab::Gstr3b, GstTab::Reconciliation];

    /// Tab title.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            GstTab::Gstr1 => "GSTR-1",
            GstTab::Gstr3b => "GSTR-3B",
            GstTab::Reconciliation => "2B and IMS",
        }
    }

    /// The key that opens the tab (the mock's `1`, `2`, `3`).
    #[must_use]
    pub fn key(self) -> Key {
        match self {
            GstTab::Gstr1 => Key::Num1,
            GstTab::Gstr3b => Key::Num2,
            GstTab::Reconciliation => Key::Num3,
        }
    }

    /// The key as shown on the tab.
    #[must_use]
    pub fn key_hint(self) -> &'static str {
        match self {
            GstTab::Gstr1 => "1",
            GstTab::Gstr3b => "2",
            GstTab::Reconciliation => "3",
        }
    }
}

/// Exception bucket of a document on a return (spec 6.3). The engine decides the bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bucket {
    /// Ready to file.
    Ready,
    /// Does not belong on this return.
    NotRelevant,
    /// Books and portal disagree.
    Mismatch,
    /// The books need a fix before the document can be filed.
    NeedsCorrection,
}

impl Bucket {
    /// The buckets in display order.
    pub const ALL: [Bucket; 4] = [
        Bucket::Ready,
        Bucket::NotRelevant,
        Bucket::Mismatch,
        Bucket::NeedsCorrection,
    ];

    /// Chip label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Bucket::Ready => "Ready",
            Bucket::NotRelevant => "Not relevant",
            Bucket::Mismatch => "Mismatch",
            Bucket::NeedsCorrection => "Needs correction",
        }
    }
}

/// Data the host hands to the screen: not loaded, not supported, or loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed<T> {
    /// The host has not supplied it yet.
    NotLoaded,
    /// The engine cannot produce it (rule A1); shown as such.
    Unsupported(Unsupported),
    /// The data.
    Loaded(T),
}

/// One exception on GSTR-1 or GSTR-3B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionItem {
    /// The bucket the engine put it in.
    pub bucket: Bucket,
    /// Short title, for example a document number.
    pub title: String,
    /// What is wrong, or why it is fine.
    pub detail: String,
    /// The voucher to open to fix it, when there is one.
    pub voucher: Option<VoucherId>,
}

/// The user's (or the engine's suggested) IMS action on a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImsAction {
    /// Accept the document: its ITC counts this month.
    Accept,
    /// Reject the document.
    Reject,
    /// Leave it pending.
    Pending,
}

impl ImsAction {
    /// The actions in display order.
    pub const ALL: [ImsAction; 3] = [ImsAction::Accept, ImsAction::Reject, ImsAction::Pending];

    /// Button label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ImsAction::Accept => "Accept",
            ImsAction::Reject => "Reject",
            ImsAction::Pending => "Pending",
        }
    }

    /// Past tense, for the notice line.
    #[must_use]
    pub fn past(self) -> &'static str {
        match self {
            ImsAction::Accept => "accepted",
            ImsAction::Reject => "rejected",
            ImsAction::Pending => "left pending",
        }
    }

    /// The key that sets the action on the selected row (the mock's A, R, P).
    #[must_use]
    pub fn key(self) -> Key {
        match self {
            ImsAction::Accept => Key::A,
            ImsAction::Reject => Key::R,
            ImsAction::Pending => Key::P,
        }
    }
}

/// How a books invoice and a 2B line compare. The engine decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchState {
    /// Same supplier, number, date and values.
    Matched,
    /// The taxable value or tax differs.
    ValueDiffers,
    /// The date differs.
    DateDiffers,
    /// In 2B but not in the books.
    NotInBooks,
    /// In 2B and looks like a second copy of a booked invoice.
    PossibleDuplicate,
    /// In the books but the supplier has not filed it yet.
    NotIn2b,
}

impl MatchState {
    /// Label of the match column.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            MatchState::Matched => "Matched",
            MatchState::ValueDiffers => "Value differs",
            MatchState::DateDiffers => "Date differs",
            MatchState::NotInBooks => "Not in books",
            MatchState::PossibleDuplicate => "Possible duplicate",
            MatchState::NotIn2b => "Not in 2B yet",
        }
    }

    /// `true` when the document is in GSTR-2B, so an IMS action can be set on it.
    #[must_use]
    pub fn in_2b(self) -> bool {
        match self {
            MatchState::Matched
            | MatchState::ValueDiffers
            | MatchState::DateDiffers
            | MatchState::NotInBooks
            | MatchState::PossibleDuplicate => true,
            MatchState::NotIn2b => false,
        }
    }
}

/// One row of the 2B and IMS table. Every figure comes from the engine (rule C4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconRow {
    /// Supplier name.
    pub supplier: String,
    /// Supplier GSTIN.
    pub gstin: Gstin,
    /// Supplier's invoice number as printed.
    pub invoice_no: String,
    /// Date in the books, when the invoice is booked.
    pub books_date: Option<Date>,
    /// Date in GSTR-2B, when the invoice is in 2B.
    pub portal_date: Option<Date>,
    /// Taxable value in the books.
    pub taxable_books: Option<Paise>,
    /// Taxable value in 2B.
    pub taxable_2b: Option<Paise>,
    /// Tax in the books.
    pub tax_books: Option<Paise>,
    /// Tax in 2B.
    pub tax_2b: Option<Paise>,
    /// Tax in 2B less tax in the books, computed by the engine.
    pub difference: Option<Paise>,
    /// How the books and 2B compare.
    pub state: MatchState,
    /// The exception bucket.
    pub bucket: Bucket,
    /// The engine's suggested action. Never applied without the user.
    pub suggested: ImsAction,
    /// The purchase voucher in the books, when there is one.
    pub voucher: Option<VoucherId>,
}

impl ReconRow {
    /// `true` when an IMS action can be set on this row.
    #[must_use]
    pub fn in_2b(&self) -> bool {
        self.state.in_2b()
    }

    /// The invoice dates as shown: the books date, and the 2B date when it differs.
    #[must_use]
    pub fn dates_text(&self) -> String {
        match (self.books_date, self.portal_date) {
            (Some(books), Some(portal)) if books != portal => format!(
                "{} (in 2B: {})",
                date_cell(Some(books)),
                date_cell(Some(portal))
            ),
            (Some(books), Some(_) | None) => date_cell(Some(books)),
            (None, Some(portal)) => format!("in 2B: {}", date_cell(Some(portal))),
            (None, None) => date_cell(None),
        }
    }
}

/// `dd-mm-yyyy`, or `none` when there is no date.
#[must_use]
pub fn date_cell(date: Option<Date>) -> String {
    match date {
        Some(d) => format!("{:02}-{:02}-{}", d.day(), u8::from(d.month()), d.year()),
        None => "none".to_owned(),
    }
}

/// An amount with Indian grouping, or `none` when the engine gave no amount. A missing amount is
/// never shown as `0.00`.
#[must_use]
pub fn amount_cell(amount: Option<Paise>) -> String {
    match amount {
        Some(p) => format_inr(p),
        None => "none".to_owned(),
    }
}

/// The difference column: the same convention as every amount cell, so a missing difference
/// reads `none` beside the three amounts it is computed from.
#[must_use]
pub fn difference_cell(difference: Option<Paise>) -> String {
    amount_cell(difference)
}

/// An action the user confirmed, for the IMS action file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedAction {
    /// Supplier GSTIN.
    pub supplier_gstin: Gstin,
    /// Supplier's invoice number.
    pub invoice_no: String,
    /// The invoice date in 2B.
    pub portal_date: Option<Date>,
    /// What the user chose.
    pub action: ImsAction,
}

/// Why an IMS action could not be set.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActionError {
    /// No reconciliation is loaded.
    #[error("the 2B and IMS reconciliation is not available, so there is nothing to act on")]
    NoReconciliation,
    /// The row does not exist.
    #[error("row {row} does not exist")]
    NoSuchRow {
        /// The row index asked for.
        row: usize,
    },
    /// The document is not in GSTR-2B.
    #[error(
        "{invoice_no} from {supplier} is not in 2B yet, so there is nothing to act on in IMS; ask the supplier to file"
    )]
    NotIn2b {
        /// Supplier name.
        supplier: String,
        /// Invoice number.
        invoice_no: String,
    },
}

/// A command from the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GstCmd {
    /// Show a tab.
    SwitchTab(GstTab),
    /// Next row (J, Down).
    Next,
    /// Previous row (K, Up).
    Prev,
    /// Set an IMS action on the selected row (A, R, P).
    Decide(ImsAction),
    /// Open the voucher of the selected row (Enter).
    Open,
    /// Leave the screen (Esc).
    Close,
    /// Export the GSTR-1 JSON (Ctrl+E).
    ExportJson,
}

// ---------------------------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------------------------

/// A confirmed action keyed by supplier GSTIN and invoice number, so it can follow the row
/// across a refresh.
type RememberedAction = (Gstin, String, ImsAction);

/// The action remembered for a supplier GSTIN and invoice number, if any.
fn remembered_for(
    actions: &[RememberedAction],
    gstin: &Gstin,
    invoice_no: &str,
) -> Option<ImsAction> {
    actions
        .iter()
        .find(|(g, n, _)| g == gstin && n.as_str() == invoice_no)
        .map(|(_, _, action)| *action)
}

/// The GST workspace screen.
#[derive(Debug)]
pub struct GstScreen {
    period_label: String,
    own_gstin: Option<Gstin>,
    /// The state of the registration, named in the subtitle.
    state: Option<StateCode>,
    tab: GstTab,
    gstr1: Feed<Vec<ExceptionItem>>,
    gstr3b: Feed<Vec<ExceptionItem>>,
    recon: Feed<Vec<ReconRow>>,
    /// The user's confirmed action per reconciliation row, parallel to `recon`.
    decisions: Vec<Option<ImsAction>>,
    /// Confirmed actions kept while the reconciliation feed is not loaded, so that a refresh
    /// which fails does not erase them. Restored by the next loaded feed.
    held: Vec<RememberedAction>,
    bucket_filter: Option<Bucket>,
    open_only: bool,
    /// Position, in the visible list, of the selected row.
    focus: usize,
    scroll_pending: bool,
    notice: Option<String>,
    drill: Option<VoucherId>,
    reapply: Option<GstTab>,
    export: Option<Vec<ConfirmedAction>>,
    /// The GSTR-1 JSON built by the worker, with its file name, until the user saves it.
    export_json: Option<(String, String)>,
    revision: u64,
    /// The load from the API worker (see the `worker` module).
    run: worker::GstRun,
    /// Figures of the GSTR-1 tab, from the worker.
    figures1: Figures,
    /// Figures of the GSTR-3B tab, from the worker.
    figures3b: Figures,
    /// The GSTR-3B table, from the worker.
    lines3b: Vec<look::Line3b>,
}

/// What a click in the table asks for, applied after the frame is drawn.
#[derive(Debug, Clone, Copy)]
enum UiEvent {
    Tab(GstTab),
    Filter(Option<Bucket>),
    OpenOnly(bool),
    Focus(usize),
    Drill(usize),
    Decide(usize, ImsAction),
    ExportJson,
}

impl GstScreen {
    /// A screen for one return period. The reconciliation tab is shown first, as in the mock.
    #[must_use]
    pub fn new(period_label: impl Into<String>, own_gstin: Option<Gstin>) -> Self {
        GstScreen {
            period_label: period_label.into(),
            own_gstin,
            state: None,
            tab: GstTab::Reconciliation,
            gstr1: Feed::NotLoaded,
            gstr3b: Feed::NotLoaded,
            recon: Feed::NotLoaded,
            decisions: Vec::new(),
            held: Vec::new(),
            bucket_filter: None,
            open_only: false,
            focus: 0,
            scroll_pending: false,
            notice: None,
            drill: None,
            reapply: None,
            export: None,
            export_json: None,
            revision: 0,
            run: worker::GstRun::default(),
            figures1: Figures::default(),
            figures3b: Figures::default(),
            lines3b: Vec::new(),
        }
    }

    /// The same screen naming `state`, the state of the registration, in its subtitle.
    #[must_use]
    pub fn with_state(mut self, state: StateCode) -> Self {
        self.state = Some(state);
        self
    }

    /// The tab on show.
    #[must_use]
    pub fn tab(&self) -> GstTab {
        self.tab
    }

    #[must_use]
    /// The line under the heading: the GSTIN, the state, the period and "monthly", joined by
    /// middle dots as in the mock (index.html:1670). A part with no source is left out. Returns
    /// are monthly because the shell opens one calendar month
    /// ([`crate::shell::navigate::gst_return_period`]).
    pub fn subtitle(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(g) = &self.own_gstin {
            parts.push(g.to_string());
        }
        if let Some(state) = self.state {
            parts.push(state.name().to_owned());
        }
        parts.push(self.period_label.clone());
        parts.push("monthly".to_owned());
        parts.join(" \u{00B7} ")
    }

    /// The message under the table, when there is one.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Counts the changes to the confirmed actions. The host recomputes GSTR-3B when it changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // ---- data from the host ------------------------------------------------------------------

    /// Supplies the GSTR-1 exceptions.
    pub fn set_gstr1(&mut self, feed: Feed<Vec<ExceptionItem>>) {
        self.gstr1 = feed;
        self.clamp_focus();
    }

    /// Supplies the GSTR-3B exceptions.
    pub fn set_gstr3b(&mut self, feed: Feed<Vec<ExceptionItem>>) {
        self.gstr3b = feed;
        self.clamp_focus();
    }

    /// Supplies the reconciliation rows. Actions the user already confirmed are kept for rows
    /// with the same supplier GSTIN and invoice number that are still in 2B. A confirmed action
    /// with no such row any more is dropped and the notice says so.
    ///
    /// A feed that is not loaded ([`Feed::NotLoaded`] or [`Feed::Unsupported`]) drops nothing: the
    /// confirmed actions are held and the next loaded feed restores the ones whose rows are
    /// still in 2B. The notice names only the real cause (a row that left 2B), never a feed that
    /// merely failed to load.
    pub fn set_recon(&mut self, feed: Feed<Vec<ReconRow>>) {
        let before = self.remembered_actions();
        match &feed {
            Feed::Loaded(rows) => {
                self.decisions = rows
                    .iter()
                    .map(|row| {
                        if row.in_2b() {
                            remembered_for(&before, &row.gstin, &row.invoice_no)
                        } else {
                            None
                        }
                    })
                    .collect();
                self.held.clear();
            }
            Feed::NotLoaded | Feed::Unsupported(_) => {
                self.decisions.clear();
                self.held.clone_from(&before);
            }
        }
        self.recon = feed;
        let after = self.remembered_actions();
        if before.len() > after.len() {
            self.notice = Some(format!(
                "{} confirmed IMS action(s) were dropped because the invoice is no longer in 2B.",
                before.len().saturating_sub(after.len())
            ));
        }
        if before != after {
            self.bump();
        }
        self.clamp_focus();
    }

    /// Every action the user has confirmed, by supplier GSTIN and invoice number. While the
    /// reconciliation is loaded these come from its rows; otherwise they are the held actions.
    fn remembered_actions(&self) -> Vec<RememberedAction> {
        match &self.recon {
            Feed::Loaded(rows) => rows
                .iter()
                .zip(self.decisions.iter())
                .filter_map(|(row, decision)| {
                    decision.map(|action| (row.gstin.clone(), row.invoice_no.clone(), action))
                })
                .collect(),
            Feed::NotLoaded | Feed::Unsupported(_) => self.held.clone(),
        }
    }

    // ---- counts ------------------------------------------------------------------------------

    fn buckets(&self, tab: GstTab) -> Option<Vec<Bucket>> {
        match tab {
            GstTab::Gstr1 => exception_buckets(&self.gstr1),
            GstTab::Gstr3b => exception_buckets(&self.gstr3b),
            GstTab::Reconciliation => match &self.recon {
                Feed::Loaded(rows) => Some(rows.iter().map(|r| r.bucket).collect()),
                Feed::NotLoaded | Feed::Unsupported(_) => None,
            },
        }
    }

    /// Number of documents on a tab, or `None` when the tab has no data.
    #[must_use]
    pub fn item_count(&self, tab: GstTab) -> Option<usize> {
        self.buckets(tab).map(|b| b.len())
    }

    /// Number of documents in a bucket on a tab, or `None` when the tab has no data.
    #[must_use]
    pub fn bucket_count(&self, tab: GstTab, bucket: Bucket) -> Option<usize> {
        self.buckets(tab)
            .map(|b| b.iter().filter(|x| **x == bucket).count())
    }

    /// Reconciliation rows in 2B that still need the user: no action yet, or Pending. This is the
    /// "IMS actions still open" count of the mock.
    #[must_use]
    pub fn open_count(&self) -> usize {
        let Feed::Loaded(rows) = &self.recon else {
            return 0;
        };
        rows.iter()
            .enumerate()
            .filter(|(i, row)| is_open(row, self.decision(*i)))
            .count()
    }

    // ---- rows on show ------------------------------------------------------------------------

    fn visible_exceptions(&self, feed: &Feed<Vec<ExceptionItem>>) -> Vec<usize> {
        let Feed::Loaded(items) = feed else {
            return Vec::new();
        };
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.bucket_filter.is_none_or(|b| item.bucket == b))
            .map(|(i, _)| i)
            .collect()
    }

    fn visible_recon(&self) -> Vec<usize> {
        let Feed::Loaded(rows) = &self.recon else {
            return Vec::new();
        };
        rows.iter()
            .enumerate()
            .filter(|(i, row)| {
                self.bucket_filter.is_none_or(|b| row.bucket == b)
                    && (!self.open_only || is_open(row, self.decision(*i)))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Indices (into the tab's data) of the rows the filters leave on show, in order.
    #[must_use]
    pub fn visible_indices(&self) -> Vec<usize> {
        match self.tab {
            GstTab::Gstr1 => self.visible_exceptions(&self.gstr1),
            GstTab::Gstr3b => self.visible_exceptions(&self.gstr3b),
            GstTab::Reconciliation => self.visible_recon(),
        }
    }

    /// Index (into the tab's data) of the selected row.
    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.visible_indices().get(self.focus).copied()
    }

    fn clamp_focus(&mut self) {
        let last = self.visible_indices().len().saturating_sub(1);
        self.focus = self.focus.min(last);
    }

    /// Selects the next row (J). Stops at the last row.
    pub fn focus_next(&mut self) {
        let last = self.visible_indices().len().saturating_sub(1);
        self.focus = self.focus.saturating_add(1).min(last);
        self.scroll_pending = true;
    }

    /// Selects the previous row (K). Stops at the first row.
    pub fn focus_prev(&mut self) {
        self.focus = self.focus.saturating_sub(1);
        self.scroll_pending = true;
    }

    /// Shows a tab. The selection returns to the first row.
    pub fn switch_tab(&mut self, tab: GstTab) {
        if self.tab != tab {
            self.tab = tab;
            self.focus = 0;
            self.scroll_pending = true;
        }
    }

    /// Filters the tab by bucket (`None` shows every bucket).
    pub fn set_bucket_filter(&mut self, bucket: Option<Bucket>) {
        self.bucket_filter = bucket;
        self.focus = 0;
    }

    /// The bucket filter on show.
    #[must_use]
    pub fn bucket_filter(&self) -> Option<Bucket> {
        self.bucket_filter
    }

    /// Shows only reconciliation rows that still need an action.
    pub fn set_open_only(&mut self, open_only: bool) {
        self.open_only = open_only;
        self.focus = 0;
    }

    // ---- IMS actions -------------------------------------------------------------------------

    /// The action the user confirmed on a reconciliation row, if any.
    #[must_use]
    pub fn decision(&self, row: usize) -> Option<ImsAction> {
        self.decisions.get(row).copied().flatten()
    }

    fn bump(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    /// Records the user's action on a reconciliation row.
    ///
    /// # Errors
    /// [`ActionError::NoReconciliation`] when no reconciliation is loaded,
    /// [`ActionError::NoSuchRow`] for a row that does not exist and [`ActionError::NotIn2b`] for a
    /// document the supplier has not filed (nothing is recorded in any of these cases).
    pub fn decide(&mut self, row: usize, action: ImsAction) -> Result<(), ActionError> {
        let Feed::Loaded(rows) = &self.recon else {
            return Err(ActionError::NoReconciliation);
        };
        let Some(r) = rows.get(row) else {
            return Err(ActionError::NoSuchRow { row });
        };
        if !r.in_2b() {
            return Err(ActionError::NotIn2b {
                supplier: r.supplier.clone(),
                invoice_no: r.invoice_no.clone(),
            });
        }
        let Some(slot) = self.decisions.get_mut(row) else {
            return Err(ActionError::NoSuchRow { row });
        };
        if *slot != Some(action) {
            *slot = Some(action);
            self.bump();
        }
        self.clamp_focus();
        Ok(())
    }

    /// The actions the user confirmed, in row order. Rows without a confirmed action are left out:
    /// a suggestion alone is never exported.
    #[must_use]
    pub fn confirmed_actions(&self) -> Vec<ConfirmedAction> {
        let Feed::Loaded(rows) = &self.recon else {
            return Vec::new();
        };
        rows.iter()
            .zip(self.decisions.iter())
            .filter_map(|(row, decision)| {
                decision.map(|action| ConfirmedAction {
                    supplier_gstin: row.gstin.clone(),
                    invoice_no: row.invoice_no.clone(),
                    portal_date: row.portal_date,
                    action,
                })
            })
            .collect()
    }

    // ---- requests to the host ----------------------------------------------------------------

    /// The voucher the user asked to open (Enter or double click), once.
    pub fn take_drill_request(&mut self) -> Option<VoucherId> {
        self.drill.take()
    }

    /// The tab on which the user asked to re-apply masters in bulk, once.
    pub fn take_reapply_request(&mut self) -> Option<GstTab> {
        self.reapply.take()
    }

    /// The confirmed actions at the moment the user asked for the action file, once.
    pub fn take_export_request(&mut self) -> Option<Vec<ConfirmedAction>> {
        self.export.take()
    }

    /// Records a bulk "re-apply masters" request for the tab on show (the button does the same).
    pub fn request_reapply(&mut self) {
        self.reapply = Some(self.tab);
    }

    /// Asks for the IMS action file (the button does the same). With no confirmed action there is
    /// nothing to export: no request is made and the notice says so.
    pub fn request_export(&mut self) {
        let actions = self.confirmed_actions();
        if actions.is_empty() {
            self.notice = Some(
                "Nothing to export: no IMS action is confirmed yet. Press A, R or P on a row."
                    .to_owned(),
            );
        } else {
            self.export = Some(actions);
        }
    }

    /// Asks for the GSTR-1 JSON, or says why it cannot be asked for yet (the reason is in
    /// [`GstScreen::export_blocked_reason`]).
    fn request_json_export(&mut self) {
        match self.export_blocked_reason() {
            Some(reason) => self.notice = Some(reason),
            None => self.request_json(),
        }
    }

    // ---- commands ----------------------------------------------------------------------------

    fn decide_with_notice(&mut self, row: usize, action: ImsAction) {
        match self.decide(row, action) {
            Ok(()) => {
                let invoice = match &self.recon {
                    Feed::Loaded(rows) => rows.get(row).map(|r| r.invoice_no.clone()),
                    Feed::NotLoaded | Feed::Unsupported(_) => None,
                };
                self.notice = Some(format!(
                    "{}: {}. Saved here only; nothing is uploaded to IMS.",
                    invoice.unwrap_or_default(),
                    action.past()
                ));
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    fn decide_focused(&mut self, action: ImsAction) {
        if self.tab != GstTab::Reconciliation {
            self.notice = Some(
                "IMS actions are set on the 2B and IMS reconciliation tab (press 2).".to_owned(),
            );
            return;
        }
        match self.focused_index() {
            Some(row) => self.decide_with_notice(row, action),
            None => self.notice = Some("No row is selected.".to_owned()),
        }
    }

    fn voucher_at(&self, index: usize) -> Option<VoucherId> {
        match self.tab {
            GstTab::Gstr1 => exception_voucher(&self.gstr1, index),
            GstTab::Gstr3b => exception_voucher(&self.gstr3b, index),
            GstTab::Reconciliation => match &self.recon {
                Feed::Loaded(rows) => rows.get(index).and_then(|r| r.voucher),
                Feed::NotLoaded | Feed::Unsupported(_) => None,
            },
        }
    }

    fn open_focused(&mut self) {
        let Some(index) = self.focused_index() else {
            self.notice = Some("No row is selected.".to_owned());
            return;
        };
        match self.voucher_at(index) {
            Some(v) => self.drill = Some(v),
            None => {
                self.notice = Some(
                    "This row has no voucher in your books, so there is nothing to open."
                        .to_owned(),
                );
            }
        }
    }

    /// Runs one command and says what the stack should do.
    pub fn apply(&mut self, cmd: GstCmd) -> ScreenAction {
        self.notice = None;
        match cmd {
            GstCmd::SwitchTab(tab) => self.switch_tab(tab),
            GstCmd::Next => self.focus_next(),
            GstCmd::Prev => self.focus_prev(),
            GstCmd::Decide(action) => self.decide_focused(action),
            GstCmd::Open => self.open_focused(),
            GstCmd::Close => return ScreenAction::Pop,
            GstCmd::ExportJson => self.request_json_export(),
        }
        ScreenAction::Stay
    }

    fn apply_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Tab(tab) => self.switch_tab(tab),
            UiEvent::Filter(bucket) => self.set_bucket_filter(bucket),
            UiEvent::OpenOnly(on) => self.set_open_only(on),
            UiEvent::Focus(pos) => self.focus = pos,
            UiEvent::Drill(pos) => {
                self.focus = pos;
                self.open_focused();
            }
            UiEvent::Decide(row, action) => self.decide_with_notice(row, action),
            UiEvent::ExportJson => self.request_json_export(),
        }
        self.clamp_focus();
    }
}

fn exception_buckets(feed: &Feed<Vec<ExceptionItem>>) -> Option<Vec<Bucket>> {
    match feed {
        Feed::Loaded(items) => Some(items.iter().map(|i| i.bucket).collect()),
        Feed::NotLoaded | Feed::Unsupported(_) => None,
    }
}

fn exception_voucher(feed: &Feed<Vec<ExceptionItem>>, index: usize) -> Option<VoucherId> {
    match feed {
        Feed::Loaded(items) => items.get(index).and_then(|i| i.voucher),
        Feed::NotLoaded | Feed::Unsupported(_) => None,
    }
}

/// A row needs the user when it is in 2B and has no action yet, or is pending.
fn is_open(row: &ReconRow, decision: Option<ImsAction>) -> bool {
    row.in_2b() && matches!(decision, None | Some(ImsAction::Pending))
}

impl Screen for GstScreen {
    fn title(&self) -> String {
        "GST returns".to_owned()
    }

    fn page_title(&self) -> String {
        "GST returns".to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        Some(GstScreen::subtitle(self))
    }

    fn keys(&self) -> Vec<crate::screen::KeyHint> {
        use crate::screen::KeyHint;
        // The mock's key bars (index.html:1721), less keys with nothing to act on: GSTR-3B has
        // no rows and exports nothing, and on the 2B tab the IMS keys need rows; Y and X need
        // the engine's match (engine item E-3).
        match (self.tab, &self.recon) {
            (GstTab::Gstr3b, _) => vec![KeyHint::new("1 2 3", "Tabs"), KeyHint::new("Esc", "Back")],
            (GstTab::Gstr1, _) => vec![
                KeyHint::new("1 2 3", "Tabs"),
                KeyHint::new("J / K", "Move"),
                KeyHint::new("Enter", "Open"),
                KeyHint::new("Ctrl+E", "Export JSON"),
                KeyHint::new("Esc", "Back"),
            ],
            (GstTab::Reconciliation, Feed::Loaded(_)) => vec![
                KeyHint::new("J / K", "Move"),
                KeyHint::new("A", "Accept"),
                KeyHint::new("R", "Reject"),
                KeyHint::new("P", "Pending"),
                KeyHint::new("Esc", "Back"),
            ],
            (GstTab::Reconciliation, Feed::NotLoaded | Feed::Unsupported(_)) => {
                vec![KeyHint::new("1 2 3", "Tabs"), KeyHint::new("Esc", "Back")]
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx.api.as_ref());
        if self.is_loading() {
            ui.ctx().request_repaint();
        }
        let action = self.draw(ui);
        // Enter or a double click asked for a voucher: open it on top, as the day book does.
        match (action, self.take_drill_request()) {
            (ScreenAction::Stay, Some(id)) => ScreenAction::Push(Box::new(
                crate::screens::report_view::VoucherViewScreen::new(id),
            )),
            (action, _) => action,
        }
    }
}
