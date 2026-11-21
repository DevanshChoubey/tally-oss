//! Bank reconciliation. **Card U9** implements this module.
//!
//! Frozen interface (plan card U9):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Bank`: [`BankScreen`]. It shows one imported statement line by line next to
//! the matcher's suggestion for each line, and a second tab with the bank reconciliation
//! statement (spec 6.5, mock "Bank matching").
//!
//! What spec 6.5 asks of the screen, and where it lives:
//!
//! * Many-to-many matches: a [`SuggestionView`] names several statement lines and several
//!   vouchers, and accepting it matches them all ([`BankScreen::accept_focused`]).
//! * Match kinds Exact, Rule, `AiSuggested` and Manual ([`MatchKind`]). A suggestion is never
//!   [`MatchKind::Manual`]; that kind is only produced by [`BankScreen::confirm_manual`]. An
//!   AI suggestion is labelled "AI proposed".
//! * Only a user-confirmed match sets the bank date: nothing leaves the screen until the user
//!   accepts a suggestion (Enter, or the Match button) or confirms a manual match. The screen
//!   then records a [`BankMatch`], which the host collects with [`BankScreen::take_confirmed`].
//! * Keys: Enter accepts the focused suggestion, S skips (and un-skips), J/K or the arrow keys
//!   move, M opens a manual match, B switches to the statement, Esc closes a manual match and is
//!   otherwise left to the shell (spec 7: Esc goes back).
//!
//! Rule C4 (the UI never sums anything). The screen adds up nothing and compares no amounts: the
//! statement lines, the suggestions, the open vouchers and the reconciliation statement are all
//! handed in by the host, which computes them with `ledgerly-bank`, and every amount is shown as
//! given. A [`BankMatch`] is a request: the host checks that the lines and the vouchers agree
//! before it commits the event, so a manual match the engine rejects never sets a bank date. The
//! statement is [`Outcome::Unsupported`] until the host supplies it with
//! [`BankScreen::set_brs`], never a zero.
//!
//! # Worker mode (card C3)
//!
//! [`BankScreen::worker`] builds a screen that gets everything from the API worker: the
//! reconciliation statement (`Api::brs`), the newest stored statement with the matcher's
//! suggestions (`Api::stored_bank_statements`, `Api::bank_suggestions`), and each accepted
//! suggestion is sent to `Api::confirm_bank_match`. See the `worker` submodule for the rules
//! (pending line, error on a lost worker, stale answers dropped).
//!
//! This crate does not depend on `ledgerly-bank` (its manifest is frozen), so the screen defines
//! its own plain view types and the host converts. See `docs/interface-requests.md`.

use std::collections::BTreeSet;

use egui::{Key, Modifiers, RichText, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{BankStatementId, Date, Outcome, Paise, VoucherId};
use ledgerly_model::{BankMatch, MatchKind};
use thiserror::Error;

use crate::{AppCtx, Screen, ScreenAction};

mod worker;

// ---------------------------------------------------------------------------------------------
// View types handed in by the host
// ---------------------------------------------------------------------------------------------

/// Which way money moved on the statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// A deposit (a credit on the statement).
    In,
    /// A withdrawal (a debit on the statement).
    Out,
}

/// One line of the imported statement, as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementLineView {
    /// The line number inside the statement (1-based), as stored in [`BankMatch::statement_lines`].
    pub line_no: u32,
    /// The date the bank gives.
    pub date: Date,
    /// The bank's description of the line.
    pub description: String,
    /// The amount, never negative.
    pub amount: Paise,
    /// Money in or money out.
    pub flow: Flow,
}

/// How sure the matcher is, as a label. The screen shows it and never decides with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// High.
    High,
    /// Medium.
    Medium,
    /// Low.
    Low,
}

/// A match the matcher proposes. It is never confirmed until the user accepts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionView {
    /// The statement lines in the match (at least one). Accepting needs all of them pending.
    pub lines: Vec<u32>,
    /// The vouchers in the match (at least one). Accepting needs all of them unused.
    pub vouchers: Vec<VoucherId>,
    /// What to show for the vouchers, for example `Payment/0318, Kaveri Packaging Pvt Ltd`.
    pub voucher_label: String,
    /// Why the matcher proposes it, as text.
    pub reason: String,
    /// The matcher's confidence label.
    pub confidence: Confidence,
    /// How the match was found. Never [`MatchKind::Manual`].
    pub kind: MatchKind,
    /// The bank date the vouchers get when the user accepts.
    pub bank_date: Date,
}

/// A voucher on the bank ledger that has no bank date yet, offered for a manual match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenVoucherView {
    /// The voucher.
    pub voucher: VoucherId,
    /// Voucher date.
    pub date: Date,
    /// What to show, for example `Receipt/0219, Kisan Seva Kendra`.
    pub label: String,
    /// The amount on the bank line, never negative.
    pub amount: Paise,
    /// Money in (a receipt) or out (a payment).
    pub flow: Flow,
}

/// One row of the reconciliation statement as the engine computed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrsRow {
    /// The row's label.
    pub label: String,
    /// The amount, as computed.
    pub amount: Paise,
}

/// The bank reconciliation statement as the engine computed it (card K3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrsView {
    /// The date it is as of.
    pub as_of: Date,
    /// The rows, top to bottom.
    pub rows: Vec<BrsRow>,
}

// ---------------------------------------------------------------------------------------------
// Errors and state
// ---------------------------------------------------------------------------------------------

/// Why the screen could not be built, or an action was refused. Nothing changes on an error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BankScreenError {
    /// Two statement lines carry the same number.
    #[error("statement line {line_no} appears twice")]
    DuplicateLine {
        /// The line number.
        line_no: u32,
    },
    /// A suggestion names a statement line the statement does not have.
    #[error("a suggestion names statement line {line_no}, which is not in the statement")]
    UnknownLine {
        /// The line number.
        line_no: u32,
    },
    /// A suggestion names no statement line, no voucher, or the same line or voucher twice.
    #[error("a suggestion must name at least one statement line and one voucher, each once")]
    MalformedSuggestion,
    /// A suggestion claims to be a manual match. Only the user makes those.
    #[error("a suggestion cannot be a manual match; only the user confirms those")]
    ManualSuggestion,
    /// Two open vouchers carry the same id.
    #[error("voucher {voucher:?} is offered twice")]
    DuplicateVoucher {
        /// The voucher.
        voucher: VoucherId,
    },
    /// The statement has no lines.
    #[error("the statement has no lines")]
    NoLines,
    /// The focused line is already matched or skipped.
    #[error("statement line {line_no} is not waiting for a match")]
    LineNotPending {
        /// The line number.
        line_no: u32,
    },
    /// There is nothing to accept on this line.
    #[error("there is no suggestion for statement line {line_no}; press M to match it by hand")]
    NoSuggestion {
        /// The line number.
        line_no: u32,
    },
    /// A manual match needs at least one voucher chosen.
    #[error("choose at least one voucher for the manual match")]
    NoVoucherSelected,
    /// The voucher is not among the open vouchers.
    #[error("voucher {voucher:?} is not offered for a manual match")]
    UnknownVoucher {
        /// The voucher.
        voucher: VoucherId,
    },
    /// The voucher already belongs to a confirmed match.
    #[error("voucher {voucher:?} is already matched")]
    VoucherAlreadyMatched {
        /// The voucher.
        voucher: VoucherId,
    },
    /// A matched line cannot be skipped.
    #[error("statement line {line_no} is matched; it cannot be skipped")]
    CannotSkipMatched {
        /// The line number.
        line_no: u32,
    },
    /// A statement line carries a negative amount. The direction is the flow, never a sign, so
    /// a negative amount would show the wrong direction of money.
    #[error(
        "statement line {line_no} has a negative amount; the amount must be zero or more and the direction comes from the flow"
    )]
    NegativeLineAmount {
        /// The line number.
        line_no: u32,
    },
    /// An open voucher carries a negative amount.
    #[error(
        "voucher {voucher:?} has a negative amount; the amount must be zero or more and the direction comes from the flow"
    )]
    NegativeVoucherAmount {
        /// The voucher.
        voucher: VoucherId,
    },
    /// A suggestion names a voucher that is not among the open vouchers, so accepting it would
    /// match a voucher the bank ledger does not offer.
    #[error("a suggestion names voucher {voucher:?}, which is not an open voucher")]
    UnknownSuggestionVoucher {
        /// The voucher.
        voucher: VoucherId,
    },
}

/// Where a statement line stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStatus {
    /// Waiting for the user.
    Pending,
    /// In a match the user confirmed.
    Matched,
    /// Set aside by the user.
    Skipped,
}

/// The two tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Lines and suggestions.
    Matching,
    /// The reconciliation statement.
    Statement,
}

#[derive(Debug, Clone)]
struct LineState {
    view: StatementLineView,
    status: LineStatus,
}

/// The bank reconciliation screen.
#[derive(Debug)]
pub struct BankScreen {
    ledger_name: String,
    statement: BankStatementId,
    lines: Vec<LineState>,
    suggestions: Vec<SuggestionView>,
    open_vouchers: Vec<OpenVoucherView>,
    used_vouchers: BTreeSet<VoucherId>,
    focus: usize,
    tab: Tab,
    manual_open: bool,
    manual_cursor: usize,
    manual_selected: BTreeSet<VoucherId>,
    confirmed: Vec<BankMatch>,
    error: Option<BankScreenError>,
    brs: Outcome<BrsView>,
    /// Text shown under the reconciliation statement (why the statement side is missing).
    brs_note: Option<String>,
    /// A failure reported by the worker or the API, shown in red until the next success.
    notice: Option<String>,
    /// Present only in worker mode.
    run: Option<worker::Run>,
}

impl BankScreen {
    /// Builds the screen for one statement of one bank ledger.
    ///
    /// The reconciliation statement starts as [`Outcome::Unsupported`] until
    /// [`BankScreen::set_brs`] is called.
    ///
    /// # Errors
    /// [`BankScreenError`] when the statement has no lines, a line number or an open voucher
    /// repeats, or a suggestion is malformed, names an unknown line, or claims to be manual.
    pub fn new(
        ledger_name: &str,
        statement: BankStatementId,
        lines: Vec<StatementLineView>,
        suggestions: Vec<SuggestionView>,
        open_vouchers: Vec<OpenVoucherView>,
    ) -> Result<Self, BankScreenError> {
        if lines.is_empty() {
            return Err(BankScreenError::NoLines);
        }
        let mut numbers = BTreeSet::new();
        for line in &lines {
            if !numbers.insert(line.line_no) {
                return Err(BankScreenError::DuplicateLine {
                    line_no: line.line_no,
                });
            }
            if line.amount.is_negative() {
                return Err(BankScreenError::NegativeLineAmount {
                    line_no: line.line_no,
                });
            }
        }
        let mut seen = BTreeSet::new();
        for v in &open_vouchers {
            if !seen.insert(v.voucher) {
                return Err(BankScreenError::DuplicateVoucher { voucher: v.voucher });
            }
            if v.amount.is_negative() {
                return Err(BankScreenError::NegativeVoucherAmount { voucher: v.voucher });
            }
        }
        for s in &suggestions {
            check_suggestion(s, &numbers, &seen)?;
        }
        Ok(BankScreen {
            ledger_name: ledger_name.to_owned(),
            statement,
            lines: lines
                .into_iter()
                .map(|view| LineState {
                    view,
                    status: LineStatus::Pending,
                })
                .collect(),
            suggestions,
            open_vouchers,
            used_vouchers: BTreeSet::new(),
            focus: 0,
            tab: Tab::Matching,
            manual_open: false,
            manual_cursor: 0,
            manual_selected: BTreeSet::new(),
            confirmed: Vec::new(),
            error: None,
            brs: Outcome::unsupported(
                "Bank reconciliation statement",
                "the engine has not supplied it to this screen yet",
            ),
            brs_note: None,
            notice: None,
            run: None,
        })
    }

    /// Gives the screen the reconciliation statement the engine computed, or the engine's
    /// `Unsupported` answer.
    pub fn set_brs(&mut self, brs: Outcome<BrsView>) {
        self.brs = brs;
    }

    /// The reconciliation statement as last supplied.
    pub fn brs(&self) -> &Outcome<BrsView> {
        &self.brs
    }

    /// Which tab is showing.
    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Shows a tab.
    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// The number of the focused statement line.
    pub fn focused_line_no(&self) -> Option<u32> {
        self.lines.get(self.focus).map(|l| l.view.line_no)
    }

    /// Where a statement line stands, `None` for a number the statement does not have.
    pub fn status(&self, line_no: u32) -> Option<LineStatus> {
        self.lines
            .iter()
            .find(|l| l.view.line_no == line_no)
            .map(|l| l.status)
    }

    /// How many lines still wait for the user.
    pub fn pending_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.status == LineStatus::Pending)
            .count()
    }

    /// The error from the last refused action, until the next action succeeds.
    pub fn last_error(&self) -> Option<&BankScreenError> {
        self.error.as_ref()
    }

    /// The suggestion that Enter would accept on a statement line: the first one that names the
    /// line, whose lines are all pending and whose vouchers are all unused.
    pub fn suggestion_for(&self, line_no: u32) -> Option<&SuggestionView> {
        self.suggestions.iter().find(|s| {
            s.lines.contains(&line_no)
                && s.lines
                    .iter()
                    .all(|n| self.status(*n) == Some(LineStatus::Pending))
                && s.vouchers.iter().all(|v| !self.used_vouchers.contains(v))
        })
    }

    /// Hands the matches the user confirmed since the last call to the host, oldest first.
    pub fn take_confirmed(&mut self) -> Vec<BankMatch> {
        std::mem::take(&mut self.confirmed)
    }

    /// Moves to the next statement line, if there is one.
    pub fn focus_next(&mut self) {
        if self.focus.saturating_add(1) < self.lines.len() {
            self.focus = self.focus.saturating_add(1);
        }
    }

    /// Moves to the previous statement line, if there is one.
    pub fn focus_prev(&mut self) {
        self.focus = self.focus.saturating_sub(1);
    }

    /// Accepts the suggestion on the focused line: the user confirms it, so it is recorded as a
    /// [`BankMatch`] with the suggestion's kind and bank date, and focus moves to the next line
    /// that still waits.
    ///
    /// # Errors
    /// [`BankScreenError::LineNotPending`] or [`BankScreenError::NoSuggestion`]. Nothing is
    /// recorded.
    pub fn accept_focused(&mut self) -> Result<(), BankScreenError> {
        let result = self.try_accept();
        self.note(result)
    }

    fn try_accept(&mut self) -> Result<(), BankScreenError> {
        let line_no = self.require_pending()?;
        let Some(s) = self.suggestion_for(line_no).cloned() else {
            return Err(BankScreenError::NoSuggestion { line_no });
        };
        let mut lines = s.lines;
        lines.sort_unstable();
        self.record(lines, s.vouchers, s.kind, s.bank_date);
        Ok(())
    }

    /// Sets the focused line aside, or puts a skipped line back to waiting.
    ///
    /// # Errors
    /// [`BankScreenError::CannotSkipMatched`] on a matched line.
    pub fn skip_focused(&mut self) -> Result<(), BankScreenError> {
        let result = self.try_skip();
        self.note(result)
    }

    fn try_skip(&mut self) -> Result<(), BankScreenError> {
        let Some(line) = self.lines.get_mut(self.focus) else {
            return Err(BankScreenError::NoLines);
        };
        match line.status {
            LineStatus::Pending => {
                line.status = LineStatus::Skipped;
                self.advance();
                Ok(())
            }
            LineStatus::Skipped => {
                line.status = LineStatus::Pending;
                Ok(())
            }
            LineStatus::Matched => Err(BankScreenError::CannotSkipMatched {
                line_no: line.view.line_no,
            }),
        }
    }

    /// Opens or closes the manual match panel. Choices are kept until the match is confirmed.
    pub fn toggle_manual(&mut self) {
        self.manual_open = !self.manual_open;
        self.manual_cursor = 0;
    }

    /// Whether the manual match panel is open.
    pub fn is_manual_open(&self) -> bool {
        self.manual_open
    }

    /// The vouchers the manual panel offers: the open vouchers not in a confirmed match.
    pub fn manual_candidates(&self) -> Vec<&OpenVoucherView> {
        self.open_vouchers
            .iter()
            .filter(|v| !self.used_vouchers.contains(&v.voucher))
            .collect()
    }

    /// Chooses a voucher for the manual match, or takes it back.
    ///
    /// # Errors
    /// [`BankScreenError::UnknownVoucher`] or [`BankScreenError::VoucherAlreadyMatched`].
    pub fn toggle_voucher(&mut self, voucher: VoucherId) -> Result<(), BankScreenError> {
        let result = self.try_toggle_voucher(voucher);
        self.note(result)
    }

    fn try_toggle_voucher(&mut self, voucher: VoucherId) -> Result<(), BankScreenError> {
        if !self.open_vouchers.iter().any(|v| v.voucher == voucher) {
            return Err(BankScreenError::UnknownVoucher { voucher });
        }
        if self.used_vouchers.contains(&voucher) {
            return Err(BankScreenError::VoucherAlreadyMatched { voucher });
        }
        if !self.manual_selected.remove(&voucher) {
            self.manual_selected.insert(voucher);
        }
        Ok(())
    }

    /// The vouchers chosen for the manual match, in id order.
    pub fn selected_vouchers(&self) -> Vec<VoucherId> {
        self.manual_selected.iter().copied().collect()
    }

    /// Confirms a manual match of the focused line with the chosen vouchers. The bank date is
    /// the date of the statement line. The screen does not compare amounts (rule C4); the host's
    /// engine rejects a match whose lines and vouchers disagree before it commits.
    ///
    /// # Errors
    /// [`BankScreenError::LineNotPending`] or [`BankScreenError::NoVoucherSelected`]. Nothing is
    /// recorded.
    pub fn confirm_manual(&mut self) -> Result<(), BankScreenError> {
        let result = self.try_confirm_manual();
        self.note(result)
    }

    fn try_confirm_manual(&mut self) -> Result<(), BankScreenError> {
        let line_no = self.require_pending()?;
        if self.manual_selected.is_empty() {
            return Err(BankScreenError::NoVoucherSelected);
        }
        let Some(date) = self.lines.get(self.focus).map(|l| l.view.date) else {
            return Err(BankScreenError::NoLines);
        };
        let vouchers: Vec<VoucherId> = std::mem::take(&mut self.manual_selected)
            .into_iter()
            .collect();
        self.manual_open = false;
        self.record(vec![line_no], vouchers, MatchKind::Manual, date);
        Ok(())
    }

    fn require_pending(&self) -> Result<u32, BankScreenError> {
        let Some(line) = self.lines.get(self.focus) else {
            return Err(BankScreenError::NoLines);
        };
        match line.status {
            LineStatus::Pending => Ok(line.view.line_no),
            LineStatus::Matched | LineStatus::Skipped => Err(BankScreenError::LineNotPending {
                line_no: line.view.line_no,
            }),
        }
    }

    fn record(&mut self, lines: Vec<u32>, vouchers: Vec<VoucherId>, kind: MatchKind, date: Date) {
        for line in &mut self.lines {
            if lines.contains(&line.view.line_no) {
                line.status = LineStatus::Matched;
            }
        }
        self.used_vouchers.extend(vouchers.iter().copied());
        for v in &vouchers {
            self.manual_selected.remove(v);
        }
        self.confirmed.push(BankMatch {
            statement: self.statement,
            statement_lines: lines,
            vouchers,
            kind,
            bank_date: date,
        });
        self.advance();
    }

    /// Moves focus to the next waiting line after the focused one, else the first waiting line.
    fn advance(&mut self) {
        let next = self
            .lines
            .iter()
            .enumerate()
            .skip(self.focus.saturating_add(1))
            .find(|(_, l)| l.status == LineStatus::Pending)
            .or_else(|| {
                self.lines
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.status == LineStatus::Pending)
            })
            .map(|(i, _)| i);
        if let Some(i) = next {
            self.focus = i;
        }
    }

    fn note(&mut self, result: Result<(), BankScreenError>) -> Result<(), BankScreenError> {
        self.error = result.clone().err();
        result
    }
}

fn check_suggestion(
    s: &SuggestionView,
    numbers: &BTreeSet<u32>,
    open: &BTreeSet<VoucherId>,
) -> Result<(), BankScreenError> {
    match s.kind {
        MatchKind::Manual => return Err(BankScreenError::ManualSuggestion),
        MatchKind::Exact | MatchKind::Rule | MatchKind::AiSuggested => {}
    }
    let distinct_lines: BTreeSet<u32> = s.lines.iter().copied().collect();
    let distinct_vouchers: BTreeSet<VoucherId> = s.vouchers.iter().copied().collect();
    if s.lines.is_empty()
        || s.vouchers.is_empty()
        || distinct_lines.len() != s.lines.len()
        || distinct_vouchers.len() != s.vouchers.len()
    {
        return Err(BankScreenError::MalformedSuggestion);
    }
    if let Some(line_no) = s.lines.iter().find(|n| !numbers.contains(*n)) {
        return Err(BankScreenError::UnknownLine { line_no: *line_no });
    }
    if let Some(voucher) = s.vouchers.iter().find(|v| !open.contains(*v)) {
        return Err(BankScreenError::UnknownSuggestionVoucher { voucher: *voucher });
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------------------------

/// `20-11-2026`.
pub fn format_line_date(date: Date) -> String {
    format!(
        "{:02}-{:02}-{}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// The amount of a statement line with its direction: `+ 1,47,384.50` in, `- 50,000.00` out.
pub fn format_flow(amount: Paise, flow: Flow) -> String {
    let sign = match flow {
        Flow::In => '+',
        Flow::Out => '-',
    };
    format!("{sign} {}", format_inr(amount))
}

/// The label of a match kind. An assistant suggestion is marked as the assistant's (spec 6.8).
pub fn kind_label(kind: MatchKind) -> &'static str {
    match kind {
        MatchKind::Exact => "Exact",
        MatchKind::Rule => "Rule",
        MatchKind::AiSuggested => "AI proposed",
        MatchKind::Manual => "Manual",
    }
}

fn confidence_label(c: Confidence) -> &'static str {
    match c {
        Confidence::High => "high",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    }
}

fn status_label(s: LineStatus) -> &'static str {
    match s {
        LineStatus::Pending => "Not matched",
        LineStatus::Matched => "Matched",
        LineStatus::Skipped => "Skipped",
    }
}

// ---------------------------------------------------------------------------------------------
// Keys and drawing
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyCmd {
    Accept,
    Skip,
    Down,
    Up,
    Manual,
    Statement,
    Toggle,
    CloseManual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiEvent {
    Focus(usize),
    Accept(usize),
    Skip(usize),
    Tab(Tab),
    Choose(VoucherId),
    ConfirmManual,
}

fn read_keys(ui: &Ui, manual_open: bool) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.input_mut(|i| {
        let none = Modifiers::NONE;
        if i.consume_key(none, Key::Enter) {
            out.push(KeyCmd::Accept);
        }
        if i.consume_key(none, Key::S) {
            out.push(KeyCmd::Skip);
        }
        if i.consume_key(none, Key::J) || i.consume_key(none, Key::ArrowDown) {
            out.push(KeyCmd::Down);
        }
        if i.consume_key(none, Key::K) || i.consume_key(none, Key::ArrowUp) {
            out.push(KeyCmd::Up);
        }
        if i.consume_key(none, Key::M) {
            out.push(KeyCmd::Manual);
        }
        if i.consume_key(none, Key::B) {
            out.push(KeyCmd::Statement);
        }
        if manual_open && i.consume_key(none, Key::Space) {
            out.push(KeyCmd::Toggle);
        }
        // Esc is the shell's "back" unless the manual panel is open.
        if manual_open && i.consume_key(none, Key::Escape) {
            out.push(KeyCmd::CloseManual);
        }
    });
    out
}

impl BankScreen {
    /// Applies a key. A refused action leaves its error in [`BankScreen::last_error`], which the
    /// screen shows, so the `Result` is not needed here.
    fn run_key(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Accept => {
                if self.tab == Tab::Matching {
                    let _ = if self.manual_open {
                        self.confirm_manual()
                    } else {
                        self.accept_focused()
                    };
                }
            }
            KeyCmd::Skip => {
                if self.tab == Tab::Matching && !self.manual_open {
                    let _ = self.skip_focused();
                }
            }
            // Moving the focus is for the Matching tab only: the Statement tab shows no focus, so
            // a move there would change which line Enter accepts without the user seeing it.
            KeyCmd::Down => {
                if self.tab == Tab::Matching {
                    if self.manual_open {
                        let last = self.manual_candidates().len().saturating_sub(1);
                        self.manual_cursor = self.manual_cursor.saturating_add(1).min(last);
                    } else {
                        self.focus_next();
                    }
                }
            }
            KeyCmd::Up => {
                if self.tab == Tab::Matching {
                    if self.manual_open {
                        self.manual_cursor = self.manual_cursor.saturating_sub(1);
                    } else {
                        self.focus_prev();
                    }
                }
            }
            KeyCmd::Manual => {
                if self.tab == Tab::Matching {
                    self.toggle_manual();
                }
            }
            KeyCmd::Statement => {
                self.tab = match self.tab {
                    Tab::Matching => Tab::Statement,
                    Tab::Statement => Tab::Matching,
                };
            }
            KeyCmd::Toggle => {
                let id = self
                    .manual_candidates()
                    .get(self.manual_cursor)
                    .map(|v| v.voucher);
                if let Some(id) = id {
                    let _ = self.toggle_voucher(id);
                }
            }
            KeyCmd::CloseManual => self.manual_open = false,
        }
    }

    fn run_event(&mut self, event: UiEvent) {
        let last = self.lines.len().saturating_sub(1);
        match event {
            UiEvent::Focus(i) => self.focus = i.min(last),
            UiEvent::Accept(i) => {
                self.focus = i.min(last);
                let _ = self.accept_focused();
            }
            UiEvent::Skip(i) => {
                self.focus = i.min(last);
                let _ = self.skip_focused();
            }
            UiEvent::Tab(tab) => self.tab = tab,
            UiEvent::Choose(id) => {
                let _ = self.toggle_voucher(id);
            }
            UiEvent::ConfirmManual => {
                let _ = self.confirm_manual();
            }
        }
    }

    fn draw(&mut self, ui: &mut Ui) {
        for cmd in read_keys(ui, self.manual_open) {
            self.run_key(cmd);
        }
        let mut events = Vec::new();
        ui.heading(self.title());
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.tab == Tab::Matching, "Matching")
                .clicked()
            {
                events.push(UiEvent::Tab(Tab::Matching));
            }
            if ui
                .selectable_label(self.tab == Tab::Statement, "Statement (B)")
                .clicked()
            {
                events.push(UiEvent::Tab(Tab::Statement));
            }
        });
        ui.separator();
        match self.tab {
            Tab::Matching => self.draw_matching(ui, &mut events),
            Tab::Statement => self.draw_statement(ui),
        }
        if let Some(err) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {err}"));
        }
        if let Some(text) = &self.notice {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {text}"));
        }
        for event in events {
            self.run_event(event);
        }
    }

    fn draw_matching(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        if self.is_loading() {
            ui.weak("Loading the statement and its suggestions.");
        } else if self.lines.is_empty() {
            ui.weak("No bank statement has been imported for this ledger.");
        }
        ui.weak(format!(
            "{} of {} statement lines are not matched. Enter accepts the suggestion, S skips, \
             M matches by hand, J and K move.",
            self.pending_count(),
            self.lines.len()
        ));
        if self.manual_open {
            self.draw_manual(ui, events);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, line) in self.lines.iter().enumerate() {
                self.draw_line(ui, i, line, events);
            }
        });
    }

    fn draw_line(&self, ui: &mut Ui, i: usize, line: &LineState, events: &mut Vec<UiEvent>) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let date = format_line_date(line.view.date);
                if ui.selectable_label(i == self.focus, date).clicked() {
                    events.push(UiEvent::Focus(i));
                }
                ui.label(RichText::new(format_flow(line.view.amount, line.view.flow)).strong());
                ui.weak(status_label(line.status));
            });
            ui.weak(line.view.description.as_str());
            if line.status != LineStatus::Pending {
                return;
            }
            let Some(s) = self.suggestion_for(line.view.line_no) else {
                ui.weak("No suggestion. Press M to match it by hand.");
                if ui.button("Skip").clicked() {
                    events.push(UiEvent::Skip(i));
                }
                return;
            };
            let together = s.lines.len() > 1 || s.vouchers.len() > 1;
            ui.label(format!(
                "{}: {} ({})",
                if together { "Match together" } else { "Match" },
                s.voucher_label,
                kind_label(s.kind)
            ));
            ui.weak(format!(
                "{}. Confidence {}.",
                s.reason,
                confidence_label(s.confidence)
            ));
            ui.horizontal(|ui| {
                if ui.button("Match").clicked() {
                    events.push(UiEvent::Accept(i));
                }
                if ui.button("Skip").clicked() {
                    events.push(UiEvent::Skip(i));
                }
            });
        });
    }

    fn draw_manual(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.group(|ui| {
            ui.strong("Match by hand");
            ui.weak(
                "Choose the vouchers for the focused statement line (Space), then Enter. The \
                 engine checks the amounts before the bank date is set. Esc closes.",
            );
            let candidates = self.manual_candidates();
            if candidates.is_empty() {
                ui.label("Every open voucher is matched.");
            }
            for (j, v) in candidates.iter().enumerate() {
                let mut chosen = self.manual_selected.contains(&v.voucher);
                let text = format!(
                    "{}{}  {}  {}",
                    if j == self.manual_cursor { "> " } else { "  " },
                    format_line_date(v.date),
                    v.label,
                    format_flow(v.amount, v.flow),
                );
                if ui.checkbox(&mut chosen, text).changed() {
                    events.push(UiEvent::Choose(v.voucher));
                }
            }
            if ui.button("Match by hand").clicked() {
                events.push(UiEvent::ConfirmManual);
            }
        });
    }

    fn draw_statement(&self, ui: &mut Ui) {
        match &self.brs {
            Outcome::Value(view) => {
                ui.strong(format!(
                    "Reconciliation statement as of {}",
                    format_line_date(view.as_of)
                ));
                egui::Grid::new("bank-brs").show(ui, |ui| {
                    for row in &view.rows {
                        ui.label(row.label.as_str());
                        ui.label(format_inr(row.amount));
                        ui.end_row();
                    }
                });
                if let Some(note) = &self.brs_note {
                    ui.weak(note.as_str());
                }
            }
            Outcome::Unsupported(u) => {
                ui.strong("Reconciliation statement");
                ui.label(format!("Unsupported: {}. {}", u.what, u.reason));
            }
        }
    }
}

impl Screen for BankScreen {
    fn title(&self) -> String {
        format!("Bank reconciliation: {}", self.ledger_name)
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx.api.as_ref());
        if self.is_loading() {
            ui.ctx().request_repaint();
        }
        self.draw(ui);
        ScreenAction::Stay
    }
}
