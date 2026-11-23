//! Worker mode of the bank screen (card C3, spec 7 rule D1).
//!
//! In this mode every figure and every write goes through `ledgerly-api` on the API worker; the
//! UI thread never blocks and never sums anything (rule C4):
//!
//! * the reconciliation statement comes from `Api::brs`;
//! * the statement lines and the suggestions come from the newest stored statement of the ledger
//!   (`Api::stored_bank_statements`) and `Api::bank_suggestions`;
//! * accepting a suggestion sends the matcher's own candidate to `Api::confirm_bank_match`, which
//!   re-checks it and, only then, commits the bank date.
//!
//! Pending, errors and stale answers:
//!
//! * While a call runs the screen says so ([`BankScreen::is_loading`]). A worker that stopped
//!   before answering is an error line, never a figure.
//! * The reconciliation statement and the suggestions each use one [`Latest`]: a newer request
//!   drops the older ticket, so a slow earlier answer can never replace a newer one (for example
//!   after the as-of date changed).
//! * Confirmations are never dropped: they are queued and sent one at a time. A line shows as
//!   matched as soon as the user accepts it; if the API refuses or the worker is lost, the line
//!   goes back to waiting and the reason is shown.
//!
//! Limits, stated rather than guessed: a rule suggestion has no voucher yet, so it is not offered;
//! a manual match is not sent to the API (it only confirms a match the matcher lists), so the
//! manual list is empty in this mode; the statement shown is the newest stored one of the ledger.

use std::collections::{BTreeSet, VecDeque};

use ledgerly_api::{Brs, Candidate};
use ledgerly_core::{BankStatementId, Date, LedgerId, Outcome, Paise};
use ledgerly_model::{BankMatch, MatchKind};

use super::{
    BankScreen, BrsRow, BrsRowKind, BrsView, Confidence, Flow, LineState, LineStatus,
    StatementLineView, SuggestionView, import,
};
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// The statement lines and suggestions the worker found.
#[derive(Debug)]
pub(super) struct Loaded {
    statement: BankStatementId,
    lines: Vec<StatementLineView>,
    candidates: Vec<Candidate>,
}

/// `None`: the ledger has no stored statement.
type LoadAnswer = Result<Outcome<Option<Loaded>>, String>;
type BrsAnswer = (Date, Result<Outcome<Brs>, String>);
type ConfirmAnswer = (BankMatch, Result<Outcome<()>, String>);
/// The new statement's id, or the text of the refusal.
type ImportAnswer = Result<Outcome<BankStatementId>, String>;

/// A statement file the user picked, waiting for the worker to import it.
#[derive(Debug)]
pub(super) struct PendingImport {
    /// The bytes of the file, read on the UI thread when it was picked.
    bytes: Vec<u8>,
    /// The registry id of the layout to parse the file with.
    layout: &'static str,
}

/// What a worker-mode screen waits for.
#[derive(Debug)]
pub(super) struct Run {
    ledger: LedgerId,
    as_of: Date,
    /// The statement and suggestions, and the reconciliation statement, are to be (re)loaded.
    want_load: bool,
    load: Latest<LoadAnswer>,
    brs: Latest<BrsAnswer>,
    confirm: Latest<ConfirmAnswer>,
    queue: VecDeque<(BankMatch, Candidate)>,
    in_flight: bool,
    /// A picked file that the next tick sends to the worker.
    import_pending: Option<PendingImport>,
    /// The import the worker is running.
    import: Latest<ImportAnswer>,
    /// The matcher's candidates behind `BankScreen::suggestions`, in the same order.
    candidates: Vec<Candidate>,
    /// Statement lines the user matched in this session; kept across a reload.
    matched: BTreeSet<u32>,
}

impl BankScreen {
    /// A screen that gets its statement, suggestions and reconciliation statement from the API
    /// worker. The first frame asks for them; until they arrive the screen shows "Loading".
    #[must_use]
    pub fn worker(ledger_name: &str, ledger: LedgerId, as_of: Date) -> Self {
        BankScreen {
            ledger_name: ledger_name.to_owned(),
            statement: BankStatementId(0),
            lines: Vec::new(),
            suggestions: Vec::new(),
            open_vouchers: Vec::new(),
            used_vouchers: std::collections::BTreeSet::new(),
            focus: 0,
            tab: super::Tab::Matching,
            manual_open: false,
            manual_cursor: 0,
            manual_selected: std::collections::BTreeSet::new(),
            confirmed: Vec::new(),
            error: None,
            brs: Outcome::unsupported(
                "Bank reconciliation statement",
                "the engine has not answered yet",
            ),
            brs_note: None,
            notice: None,
            run: Some(Run {
                ledger,
                as_of,
                want_load: true,
                load: Latest::default(),
                brs: Latest::default(),
                confirm: Latest::default(),
                queue: VecDeque::new(),
                in_flight: false,
                import_pending: None,
                import: Latest::default(),
                candidates: Vec::new(),
                matched: BTreeSet::new(),
            }),
            import_layout: import::DEFAULT_LAYOUT,
            import_notice: None,
        }
    }

    /// Changes the date the reconciliation statement is as of and asks for it again. An answer
    /// still on its way for the old date is dropped.
    pub fn set_as_of(&mut self, as_of: Date) {
        if let Some(run) = self.run.as_mut() {
            run.as_of = as_of;
            run.want_load = true;
        }
    }

    /// The last failure the worker or the API reported, until the next success.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// `true` while a worker-mode screen waits for the worker.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.run.as_ref().is_some_and(|run| {
            run.want_load
                || run.load.is_pending()
                || run.brs.is_pending()
                || run.confirm.is_pending()
                || run.import.is_pending()
                || run.import_pending.is_some()
                || !run.queue.is_empty()
        })
    }

    /// Advances a worker-mode screen by one step: applies finished answers, queues the matches
    /// the user accepted, and sends waiting requests to `worker`. Does nothing in host mode.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        if self.run.is_none() {
            return;
        }
        self.apply_answers();
        self.queue_accepted();
        let Some(handle) = worker else {
            self.fail_without_worker();
            return;
        };
        self.send_confirm(handle);
        self.send_import(handle);
        self.send_load(handle);
    }

    /// Queues a statement file the user picked. The next tick sends it to the worker. A file
    /// picked earlier and not yet sent is replaced.
    pub(super) fn queue_import(&mut self, bytes: Vec<u8>, layout: &'static str) {
        self.import_notice = None;
        if let Some(run) = self.run.as_mut() {
            run.import_pending = Some(PendingImport { bytes, layout });
        }
    }

    fn fail_without_worker(&mut self) {
        let waiting = self.run.as_ref().is_some_and(|run| {
            run.want_load || !run.queue.is_empty() || run.import_pending.is_some()
        });
        if waiting {
            self.notice = Some("No company is open, so the bank request cannot run.".to_owned());
            let had_import = self
                .run
                .as_mut()
                .is_some_and(|run| run.import_pending.take().is_some());
            if had_import {
                self.import_notice =
                    Some("No company is open, so the statement cannot be imported.".to_owned());
            }
            if let Some(run) = self.run.as_mut() {
                run.want_load = false;
            }
            let queued: Vec<BankMatch> = self.run.as_mut().map_or_else(Vec::new, |run| {
                run.queue.drain(..).map(|(m, _)| m).collect()
            });
            for matched in &queued {
                self.revert(matched);
            }
        }
    }

    fn apply_answers(&mut self) {
        let finished = self.run.as_mut().and_then(|run| run.load.poll());
        match finished {
            Some(Done::Ready(answer)) => self.apply_load(answer),
            Some(Done::Lost) => self.notice = Some(LOST.to_owned()),
            None => {}
        }
        let finished = self.run.as_mut().and_then(|run| run.brs.poll());
        match finished {
            Some(Done::Ready((as_of, answer))) => self.apply_brs(as_of, answer),
            Some(Done::Lost) => self.notice = Some(LOST.to_owned()),
            None => {}
        }
        let finished = self.run.as_mut().and_then(|run| run.confirm.poll());
        match finished {
            Some(Done::Ready((matched, answer))) => self.apply_confirm(&matched, answer),
            Some(Done::Lost) => {
                self.notice = Some(LOST.to_owned());
                self.release_in_flight();
            }
            None => {}
        }
        let finished = self.run.as_mut().and_then(|run| run.import.poll());
        match finished {
            Some(Done::Ready(answer)) => self.apply_import(answer),
            Some(Done::Lost) => self.notice = Some(LOST.to_owned()),
            None => {}
        }
    }

    /// Shows why the import was refused, or asks for the statement again when it worked.
    fn apply_import(&mut self, answer: ImportAnswer) {
        match answer {
            Ok(Outcome::Value(_)) => {
                self.import_notice = None;
                self.notice = None;
                if let Some(run) = self.run.as_mut() {
                    run.want_load = true;
                }
            }
            Ok(Outcome::Unsupported(u)) => self.import_notice = Some(unsupported_text(&u)),
            Err(text) => self.import_notice = Some(text),
        }
    }

    fn apply_load(&mut self, answer: LoadAnswer) {
        match answer {
            Err(text) => self.notice = Some(text),
            Ok(Outcome::Unsupported(u)) => self.notice = Some(unsupported_text(&u)),
            Ok(Outcome::Value(None)) => {
                self.lines.clear();
                self.suggestions.clear();
                self.used_vouchers.clear();
                if let Some(run) = self.run.as_mut() {
                    run.candidates.clear();
                }
            }
            Ok(Outcome::Value(Some(loaded))) => self.show_loaded(loaded),
        }
    }

    fn show_loaded(&mut self, loaded: Loaded) {
        if self.statement != loaded.statement
            && let Some(run) = self.run.as_mut()
        {
            run.matched.clear();
        }
        self.statement = loaded.statement;
        let matched = self
            .run
            .as_ref()
            .map_or_else(Default::default, |run| run.matched.clone());
        self.lines = loaded
            .lines
            .into_iter()
            .map(|view| LineState {
                status: if matched.contains(&view.line_no) {
                    LineStatus::Matched
                } else {
                    LineStatus::Pending
                },
                view,
            })
            .collect();
        self.focus = 0;
        self.used_vouchers.clear();
        self.suggestions.clear();
        self.notice = None;
        let mut kept = Vec::new();
        for candidate in loaded.candidates {
            // A rule suggestion has no voucher to match yet; only Exact candidates are offered.
            if candidate.kind != MatchKind::Exact || candidate.vouchers.is_empty() {
                continue;
            }
            let known = candidate
                .statement_lines
                .iter()
                .all(|n| self.lines.iter().any(|l| l.view.line_no == *n));
            if !known {
                self.notice = Some(
                    "A suggestion names a statement line the statement does not have; it was left out."
                        .to_owned(),
                );
                continue;
            }
            self.suggestions.push(view_of(&candidate));
            kept.push(candidate);
        }
        if let Some(run) = self.run.as_mut() {
            run.candidates = kept;
        }
    }

    fn apply_brs(&mut self, as_of: Date, answer: Result<Outcome<Brs>, String>) {
        match answer {
            Err(text) => self.notice = Some(text),
            Ok(Outcome::Unsupported(u)) => {
                self.brs = Outcome::Unsupported(u);
                self.brs_note = None;
            }
            Ok(Outcome::Value(brs)) => {
                let (view, note) = brs_view(as_of, &brs);
                self.brs = Outcome::Value(view);
                self.brs_note = note;
            }
        }
    }

    fn apply_confirm(&mut self, matched: &BankMatch, answer: Result<Outcome<()>, String>) {
        self.release_in_flight();
        match answer {
            Ok(Outcome::Value(())) => {
                // The confirmed match set a bank date, so the shell reads the books again.
                crate::shell::submit::note_changed();
                self.notice = None;
                if let Some(run) = self.run.as_mut() {
                    run.matched.extend(matched.statement_lines.iter().copied());
                    run.want_load = true;
                }
            }
            Ok(Outcome::Unsupported(u)) => {
                self.revert(matched);
                self.notice = Some(unsupported_text(&u));
            }
            Err(text) => {
                self.revert(matched);
                self.notice = Some(text);
            }
        }
    }

    fn release_in_flight(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.in_flight = false;
        }
    }

    /// Takes the matches the user accepted and queues each with the candidate behind it. A match
    /// the matcher did not suggest (a manual one) is refused here and put back.
    fn queue_accepted(&mut self) {
        for matched in self.take_confirmed() {
            let candidate = self.run.as_ref().and_then(|run| {
                run.candidates
                    .iter()
                    .find(|c| {
                        c.statement_lines == matched.statement_lines
                            && c.vouchers == matched.vouchers
                    })
                    .cloned()
            });
            match (candidate, matched.kind) {
                (Some(candidate), MatchKind::Exact) => {
                    if let Some(run) = self.run.as_mut() {
                        run.queue.push_back((matched, candidate));
                    }
                }
                (None, MatchKind::Exact)
                | (_, MatchKind::Rule | MatchKind::AiSuggested | MatchKind::Manual) => {
                    self.revert(&matched);
                    self.notice = Some(
                        "Only a match the matcher suggests can be confirmed here; nothing was saved."
                            .to_owned(),
                    );
                }
            }
        }
    }

    /// Sends the next queued confirmation when none is running.
    fn send_confirm(&mut self, handle: &ApiWorker) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if run.in_flight {
            return;
        }
        let Some((matched, candidate)) = run.queue.pop_front() else {
            return;
        };
        let statement = matched.statement;
        run.in_flight = true;
        run.confirm.start(handle, move |api| {
            let answer = api
                .confirm_bank_match(statement, &candidate)
                .map(|out| out.map(|_| ()))
                .map_err(|e| e.to_string());
            (matched, answer)
        });
    }

    /// Sends the picked statement file to the worker, once no confirmation is running or waiting
    /// and no other import is running.
    fn send_import(&mut self, handle: &ApiWorker) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if run.in_flight || !run.queue.is_empty() || run.import.is_pending() {
            return;
        }
        let Some(PendingImport { bytes, layout }) = run.import_pending.take() else {
            return;
        };
        let ledger = run.ledger;
        run.import.start(handle, move |api| {
            api.import_bank_statement(&bytes, layout, ledger)
                .map_err(|e| e.to_string())
        });
    }

    /// Sends the statement, suggestions and reconciliation statement requests, once the
    /// confirmations ahead of them have finished (so they see the new bank dates) and no
    /// statement file is waiting to be imported.
    fn send_load(&mut self, handle: &ApiWorker) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if !run.want_load
            || run.in_flight
            || !run.queue.is_empty()
            || run.import.is_pending()
            || run.import_pending.is_some()
        {
            return;
        }
        run.want_load = false;
        let (ledger, as_of) = (run.ledger, run.as_of);
        run.load
            .start(handle, move |api| load_statement(api, ledger));
        run.brs.start(handle, move |api| {
            let answer = api.brs(ledger, as_of).map_err(|e| e.to_string());
            (as_of, answer)
        });
    }

    /// Puts the lines and vouchers of a refused match back to waiting.
    fn revert(&mut self, matched: &BankMatch) {
        for line in &mut self.lines {
            if matched.statement_lines.contains(&line.view.line_no) {
                line.status = LineStatus::Pending;
            }
        }
        for voucher in &matched.vouchers {
            self.used_vouchers.remove(voucher);
        }
    }
}

const LOST: &str =
    "the engine stopped before it answered; check the books and open the bank screen again";

/// Reads the newest stored statement of `ledger` and the matcher's suggestions for it.
fn load_statement(api: &ledgerly_api::Api, ledger: LedgerId) -> LoadAnswer {
    let stored = api
        .stored_bank_statements(ledger)
        .map_err(|e| e.to_string())?;
    let Some((id, statement)) = stored.last() else {
        return Ok(Outcome::Value(None));
    };
    let lines = statement
        .lines()
        .iter()
        .map(|l| {
            let (flow, amount) = if l.credit.is_zero() {
                (Flow::Out, l.debit)
            } else {
                (Flow::In, l.credit)
            };
            StatementLineView {
                line_no: l.row,
                date: l.date,
                description: l.description.clone(),
                amount,
                flow,
            }
        })
        .collect();
    let id = *id;
    match api.bank_suggestions(id, &[]).map_err(|e| e.to_string())? {
        Outcome::Value(candidates) => Ok(Outcome::Value(Some(Loaded {
            statement: id,
            lines,
            candidates,
        }))),
        Outcome::Unsupported(u) => Ok(Outcome::Unsupported(u)),
    }
}

/// The view of one of the matcher's candidates. The text says only what the matcher reported.
fn view_of(candidate: &Candidate) -> SuggestionView {
    let voucher_label = candidate
        .vouchers
        .iter()
        .map(|v| format!("voucher {}", v.0))
        .collect::<Vec<_>>()
        .join(", ");
    let (confidence, reason) = if candidate.ambiguous {
        (
            Confidence::Low,
            "Other suggestions compete for the same line or voucher, so choose carefully"
                .to_owned(),
        )
    } else {
        (
            Confidence::High,
            "The matcher found no competing suggestion for this line".to_owned(),
        )
    };
    SuggestionView {
        lines: candidate.statement_lines.clone(),
        vouchers: candidate.vouchers.clone(),
        voucher_label,
        reason,
        confidence,
        kind: candidate.kind,
        bank_date: candidate.proposed_bank_date,
    }
}

/// The rows of the reconciliation statement, every amount as the engine computed it. The note
/// says why the statement side is missing when it is.
fn brs_view(as_of: Date, brs: &Brs) -> (BrsView, Option<String>) {
    let mut rows = vec![
        row(
            "Balance as per company books",
            brs.books_balance,
            BrsRowKind::Books,
        ),
        row(
            "Amounts not reflected in bank: deposits",
            brs.uncleared_deposits_total,
            BrsRowKind::Uncleared,
        ),
        row(
            "Amounts not reflected in bank: payments",
            brs.uncleared_payments_total,
            BrsRowKind::Uncleared,
        ),
        row(
            "Balance as per bank",
            brs.balance_as_per_bank,
            BrsRowKind::Expected,
        ),
    ];
    let mut note = None;
    match &brs.statement_balance {
        Outcome::Value(paise) => rows.push(row(
            "Balance as per imported bank statement",
            *paise,
            BrsRowKind::Statement,
        )),
        Outcome::Unsupported(u) => {
            note = Some(format!(
                "Balance as per imported bank statement: {}",
                unsupported_text(u)
            ));
        }
    }
    if let Outcome::Value(paise) = &brs.difference {
        rows.push(row("Difference", *paise, BrsRowKind::Difference));
    }
    (BrsView { as_of, rows }, note)
}

fn row(label: &str, amount: Paise, kind: BrsRowKind) -> BrsRow {
    BrsRow {
        label: label.to_owned(),
        amount,
        kind,
    }
}
