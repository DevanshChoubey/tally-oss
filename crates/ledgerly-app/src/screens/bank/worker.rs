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
//! With the assistant in the release, the load also asks `Api::bank_ai_suggestions` about the
//! lines the exact matcher leaves; the API's gate decides whether anything is asked. An AI
//! suggestion is shown with the engine's reason (same amount, days apart), never the provider's
//! words, and accepting it goes to `Api::accept_bank_ai_match`; skipping its line rejects it
//! (`Api::reject_ai_draft`). Both are logged in the AI activity log by the API.
//!
//! Limits, stated rather than guessed: a rule suggestion has no voucher yet, so it is not offered;
//! a manual match is not sent to the API (it only confirms a match the matcher lists), so the
//! manual list is empty in this mode; the statement shown is the newest stored one of the ledger.

use std::collections::{BTreeSet, VecDeque};

use ledgerly_api::{AiBankMatch, BankAiAnswer, BrsLine, BrsStatement, Candidate, ExportFormat};
use ledgerly_core::{BankStatementId, CancelToken, Date, EventSeq, LedgerId, Outcome, Paise};
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
    /// The assistant's checked suggestions; empty when the assistant was not asked.
    ai: Vec<AiBankMatch>,
}

/// What a queued write does.
#[derive(Debug, Clone)]
pub(super) enum Job {
    /// Confirm the matcher's candidate.
    Exact(Candidate),
    /// Accept the assistant's suggestion stored as this draft.
    AcceptAi(EventSeq),
    /// Reject the assistant's suggestion stored as this draft.
    RejectAi(EventSeq),
}

/// `None`: the ledger has no stored statement.
type LoadAnswer = Result<Outcome<Option<Loaded>>, String>;
type BrsAnswer = (Date, Result<Outcome<BrsStatement>, String>);
/// The file the statement was written to, or why it was not.
type ExportAnswer = (std::path::PathBuf, Result<Outcome<()>, String>);
type ConfirmAnswer = (BankMatch, bool, Result<Outcome<()>, String>);
/// The new statement's id, or why the file was refused.
type ImportAnswer = Result<Outcome<BankStatementId>, ImportFailure>;

/// Why an import was refused: the words, and the engine's working when the file did not add up.
#[derive(Debug)]
pub(super) struct ImportFailure {
    /// What went wrong, for the plain notice.
    text: String,
    /// Opening, lines, computed and printed closing balance, when that is the reason.
    working: Option<ledgerly_api::BalanceRefusal>,
}

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
    queue: VecDeque<(BankMatch, Job)>,
    in_flight: bool,
    /// A picked file that the next tick sends to the worker.
    import_pending: Option<PendingImport>,
    /// The import the worker is running.
    import: Latest<ImportAnswer>,
    /// A file the statement is to be written to; the next tick sends it to the worker.
    export_pending: Option<std::path::PathBuf>,
    /// The export the worker is running.
    export: Latest<ExportAnswer>,
    /// The matcher's candidates behind `BankScreen::suggestions`.
    candidates: Vec<Candidate>,
    /// The assistant's suggestions behind `BankScreen::suggestions`.
    ai: Vec<AiBankMatch>,
    /// Statement lines the user matched in this session; kept across a reload.
    matched: BTreeSet<u32>,
}

impl Run {
    /// The bank ledger this run reconciles.
    pub(super) fn ledger_id(&self) -> LedgerId {
        self.ledger
    }

    /// The date the reconciliation statement is as of.
    pub(super) fn as_of_date(&self) -> Date {
        self.as_of
    }
}

impl BankScreen {
    /// A screen that gets its statement, suggestions and reconciliation statement from the API
    /// worker. The first frame asks for them; until they arrive the screen shows "Loading".
    #[must_use]
    pub fn worker(ledger_name: &str, ledger: LedgerId, as_of: Date) -> Self {
        BankScreen {
            tr: crate::i18n::Tr::default(),
            ledger_name: ledger_name.to_owned(),
            ai_tier: None,
            settings_requested: None,
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
                export_pending: None,
                export: Latest::default(),
                candidates: Vec::new(),
                ai: Vec::new(),
                matched: BTreeSet::new(),
            }),
            import_layout: import::DEFAULT_LAYOUT,
            import_notice: None,
            export_notice: None,
            import_working: None,
            account: None,
            can_switch: false,
            switch_requested: false,
            rejected: Vec::new(),
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
                || run.export.is_pending()
                || run.export_pending.is_some()
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
        self.queue_rejected();
        let Some(handle) = worker else {
            self.fail_without_worker();
            return;
        };
        self.send_confirm(handle);
        self.send_import(handle);
        self.send_export(handle);
        self.send_load(handle);
    }

    /// Queues a statement file the user picked. The next tick sends it to the worker. A file
    /// picked earlier and not yet sent is replaced.
    pub(super) fn queue_import(&mut self, bytes: Vec<u8>, layout: &'static str) {
        self.import_notice = None;
        self.import_working = None;
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
            Some(Done::Ready((matched, rejected, answer))) => {
                self.apply_confirm(&matched, rejected, answer);
            }
            Some(Done::Lost) => {
                self.notice = Some(LOST.to_owned());
                self.release_in_flight();
            }
            None => {}
        }
        let finished = self.run.as_mut().and_then(|run| run.export.poll());
        match finished {
            Some(Done::Ready((path, answer))) => self.apply_export(&path, answer),
            Some(Done::Lost) => self.export_notice = Some(LOST.to_owned()),
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
                self.import_working = None;
                self.notice = None;
                if let Some(run) = self.run.as_mut() {
                    run.want_load = true;
                }
            }
            Ok(Outcome::Unsupported(u)) => self.import_notice = Some(unsupported_text(&u)),
            Err(failure) => {
                self.import_notice = Some(failure.text);
                self.import_working = failure.working;
            }
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
                    run.ai.clear();
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
        let mut ai_kept = Vec::new();
        for m in loaded.ai {
            let waiting = self
                .lines
                .iter()
                .any(|l| l.view.line_no == m.line && l.status == LineStatus::Pending);
            let taken = self.suggestions.iter().any(|s| s.lines.contains(&m.line));
            if waiting && !taken {
                self.suggestions.push(ai_view_of(&m));
                ai_kept.push(m);
            }
        }
        if let Some(run) = self.run.as_mut() {
            run.candidates = kept;
            run.ai = ai_kept;
        }
    }

    fn apply_brs(&mut self, as_of: Date, answer: Result<Outcome<BrsStatement>, String>) {
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

    fn apply_confirm(
        &mut self,
        matched: &BankMatch,
        rejected: bool,
        answer: Result<Outcome<()>, String>,
    ) {
        self.release_in_flight();
        if rejected {
            // A rejection changes no line: the line stays as the person set it.
            match answer {
                Ok(Outcome::Value(())) => self.notice = None,
                Ok(Outcome::Unsupported(u)) => self.notice = Some(unsupported_text(&u)),
                Err(text) => self.notice = Some(text),
            }
            return;
        }
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
            if matched.kind == MatchKind::AiSuggested {
                let draft = self.run.as_ref().and_then(|run| {
                    run.ai
                        .iter()
                        .find(|m| {
                            matched.statement_lines == [m.line] && matched.vouchers == [m.voucher]
                        })
                        .map(|m| m.draft)
                });
                if let (Some(draft), Some(run)) = (draft, self.run.as_mut()) {
                    run.queue.push_back((matched, Job::AcceptAi(draft)));
                } else {
                    self.revert(&matched);
                    self.notice = Some(
                        "The assistant's suggestion is no longer open; nothing was saved."
                            .to_owned(),
                    );
                }
                continue;
            }
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
                        run.queue.push_back((matched, Job::Exact(candidate)));
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

    /// Queues a rejection for each line the person skipped while it showed the assistant's
    /// suggestion. The suggestion leaves the screen; the line stays skipped.
    fn queue_rejected(&mut self) {
        for line in self.take_rejected() {
            let Some(run) = self.run.as_mut() else {
                return;
            };
            let Some(at) = run.ai.iter().position(|m| m.line == line) else {
                continue;
            };
            let m = run.ai.remove(at);
            let matched = BankMatch {
                statement: self.statement,
                statement_lines: vec![m.line],
                vouchers: vec![m.voucher],
                kind: MatchKind::AiSuggested,
                bank_date: m.bank_date,
            };
            run.queue.push_back((matched, Job::RejectAi(m.draft)));
            self.suggestions
                .retain(|s| !(s.kind == MatchKind::AiSuggested && s.lines == [line]));
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
        let Some((matched, job)) = run.queue.pop_front() else {
            return;
        };
        let statement = matched.statement;
        run.in_flight = true;
        run.confirm.start(handle, move |api| {
            let rejected = matches!(job, Job::RejectAi(_));
            let answer = match job {
                Job::Exact(candidate) => api
                    .confirm_bank_match(statement, &candidate)
                    .map(|out| out.map(|_| ())),
                Job::AcceptAi(draft) => api
                    .accept_bank_ai_match(statement, draft)
                    .map(|out| out.map(|_| ())),
                Job::RejectAi(draft) => api.reject_ai_draft(draft, None).map(Outcome::Value),
            }
            .map_err(|e| e.to_string());
            (matched, rejected, answer)
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
                .map_err(|e| ImportFailure {
                    working: ledgerly_api::balance_refusal(&bytes, layout, &e),
                    text: e.to_string(),
                })
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
        let ask_ai = crate::release::ai_enabled();
        run.load
            .start(handle, move |api| load_statement(api, ledger, ask_ai));
        run.brs.start(handle, move |api| {
            let answer = api.brs_statement(ledger, as_of).map_err(|e| e.to_string());
            (as_of, answer)
        });
    }

    /// Writes the reconciliation statement on screen to `path`, as CSV or XLSX by the file's
    /// extension (the report export's rule). The next tick sends it to the worker; the screen
    /// then says where the file went or why it was not written ([`BankScreen::export_notice`]).
    pub fn export_statement_to(&mut self, path: std::path::PathBuf) {
        if ExportFormat::from_path(&path).is_none() {
            self.export_notice = Some(format!(
                "\"{}\" was not written: choose a file name ending in .csv or .xlsx.",
                path.display()
            ));
            return;
        }
        self.export_notice = None;
        if let Some(run) = self.run.as_mut() {
            run.export_pending = Some(path);
        }
    }

    fn send_export(&mut self, handle: &ApiWorker) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if run.export.is_pending() {
            return;
        }
        let Some(path) = run.export_pending.take() else {
            return;
        };
        let Some(format) = ExportFormat::from_path(&path) else {
            return;
        };
        let (ledger, as_of) = (run.ledger, run.as_of);
        run.export.start(handle, move |api| {
            let done = api
                .export_brs(ledger, as_of, format, &path)
                .map_err(|e| e.to_string());
            (path, done)
        });
    }

    fn apply_export(&mut self, path: &std::path::Path, answer: Result<Outcome<()>, String>) {
        self.export_notice = Some(match answer {
            Ok(Outcome::Value(())) => format!("Exported to {}.", path.display()),
            Ok(Outcome::Unsupported(u)) => unsupported_text(&u),
            Err(text) => format!("\"{}\" was not written: {text}", path.display()),
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

/// Reads the newest stored statement of `ledger`, the matcher's suggestions for it, and, when
/// `ask_ai`, the assistant's (the API's gate decides whether it is asked).
fn load_statement(api: &mut ledgerly_api::Api, ledger: LedgerId, ask_ai: bool) -> LoadAnswer {
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
    let candidates = match api.bank_suggestions(id, &[]).map_err(|e| e.to_string())? {
        Outcome::Value(candidates) => candidates,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let ai = if ask_ai {
        match api
            .bank_ai_suggestions(id, &CancelToken::new())
            .map_err(|e| e.to_string())?
        {
            Outcome::Value(BankAiAnswer::Suggested { matches, .. }) => matches,
            Outcome::Value(BankAiAnswer::Refused(_)) | Outcome::Unsupported(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    Ok(Outcome::Value(Some(Loaded {
        statement: id,
        lines,
        candidates,
        ai,
    })))
}

/// The view of one of the assistant's suggestions. The reason is the engine's check (the amounts
/// are equal, the days between), never the provider's words; the confidence is the provider's
/// stated per-mille as a label, which the screen shows and never decides with.
fn ai_view_of(m: &AiBankMatch) -> SuggestionView {
    let confidence = match m.confidence_permille {
        800..=u16::MAX => Confidence::High,
        500..=799 => Confidence::Medium,
        0..=499 => Confidence::Low,
    };
    SuggestionView {
        lines: vec![m.line],
        vouchers: vec![m.voucher],
        voucher_label: format!("voucher {}", m.voucher.0),
        reason: crate::i18n::tf("bank.ai.reason", &[("days", &m.days_apart.to_string())]),
        confidence,
        kind: MatchKind::AiSuggested,
        bank_date: m.bank_date,
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
            crate::i18n::lit(
                "Other suggestions compete for the same line or voucher, so choose carefully",
            ),
        )
    } else {
        (
            Confidence::High,
            crate::i18n::lit("The matcher found no competing suggestion for this line"),
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

/// The rows of the reconciliation statement, every amount as the engine computed it, with each
/// entry the bank has not cleared under its total. The note says why the statement side is
/// missing when it is.
fn brs_view(as_of: Date, statement: &BrsStatement) -> (BrsView, Option<String>) {
    let brs = &statement.brs;
    let mut rows = vec![row(
        "Balance as per company books",
        brs.books_balance,
        BrsRowKind::Books,
    )];
    rows.push(row(
        "Amounts not reflected in bank: deposits",
        brs.uncleared_deposits_total,
        BrsRowKind::Uncleared,
    ));
    rows.extend(statement.deposits.iter().map(item_row));
    rows.push(row(
        "Amounts not reflected in bank: payments",
        brs.uncleared_payments_total,
        BrsRowKind::Uncleared,
    ));
    rows.extend(statement.payments.iter().map(item_row));
    rows.push(row(
        "Balance as per bank",
        brs.balance_as_per_bank,
        BrsRowKind::Expected,
    ));
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

/// One entry under a total: "10 Nov 2026 · RC/0215 · Shree Ganesh Traders", with the cheque or
/// reference number after it when the entry has one.
fn item_row(line: &BrsLine) -> BrsRow {
    let mut label = crate::ui::format::date(line.date);
    if let Some(number) = &line.number {
        label.push_str(" \u{b7} ");
        label.push_str(number);
    }
    if !line.particulars.is_empty() {
        label.push_str(" \u{b7} ");
        label.push_str(&line.particulars);
    }
    if let Some(instrument) = &line.instrument {
        label.push_str(" \u{b7} cheque ");
        label.push_str(instrument);
    }
    row(&label, line.amount, BrsRowKind::Item)
}

fn row(label: &str, amount: Paise, kind: BrsRowKind) -> BrsRow {
    BrsRow {
        label: label.to_owned(),
        amount,
        kind,
    }
}
