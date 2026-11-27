//! The assistant's bank match suggestions (round s21, plan round F; spec 6.5 and 6.8).
//!
//! The exact matcher (`ledgerly_bank::matcher::suggest_matches`) runs first and is not the
//! assistant's. A statement line it leaves unclaimed, and that no confirmed match covers, goes
//! through the gate for [`AiFeature::BankSuggestion`] to the provider, with the vouchers the
//! matcher leaves open in the line's direction. The provider names one voucher. The engine then
//! checks that the answer is for this row, names exactly one voucher, that voucher is one it
//! offered, and its amount on the bank ledger equals the line's. Only then is the suggestion
//! stored as a draft (`DraftStored`); every refusal is a logged `ReadFailed`. A suggestion that
//! would post a new voucher from the line (`ledger_name`) is not built and is refused.
//!
//! A line with an open suggestion is not asked again (the open draft is returned), nor is a line
//! whose suggestion was accepted or rejected, nor a line whose read already failed with the same
//! provider and model version (its logged reason is returned; a read cancelled or failed on the
//! network or the key is asked again). A line is known by the hash of its statement id,
//! row, date, amount and description.
//!
//! Accepting ([`Api::accept_bank_ai_match`]) checks again against the books as they are now and
//! confirms the match as [`MatchKind::AiSuggested`], with the line's date as the bank date and the
//! accept in the same commit. The person decides each line; nothing is accepted in bulk.

use ledgerly_assist::provider::ProviderInfo;
use ledgerly_assist::{AssistError, BankLineQuestion, MatchSuggestion, OpenVoucher, Proposal};
use ledgerly_bank::matcher::{LineDirection, OpenEntry, line_flow, open_entries, suggest_matches};
use ledgerly_bankfmt::RawLine;
use ledgerly_core::{
    BankStatementId, CancelToken, Date, EventSeq, LedgerId, Outcome, Paise, VoucherId,
};
use ledgerly_model::{AiActivity, AiActivityKind, AiFeature, BankMatch, Books, MatchKind};
use ledgerly_store::Event;
use sha2::{Digest, Sha256};

use crate::ai_drafts::{NewAiDraft, draft_activity};
use crate::ai_features::{AiGate, hex};
use crate::command::{Command, CommandResult};
use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The version recorded in place of a prompt template for a bank line.
const BANK_TEMPLATE: &str = "bank-match/1";

/// One suggestion the engine checked, stored as an open draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBankMatch {
    /// The draft (the sequence number of its `DraftStored` event).
    pub draft: EventSeq,
    /// The statement row.
    pub line: u32,
    /// The voucher the assistant names.
    pub voucher: VoucherId,
    /// The line's amount, equal to the voucher's on the bank ledger (the engine checked).
    pub amount: Paise,
    /// Days between the line and the voucher, as the engine counts them.
    pub days_apart: i64,
    /// The bank date the voucher gets on accept: the line's date.
    pub bank_date: Date,
    /// The provider's confidence in per-mille, as it stated it.
    pub confidence_permille: u16,
}

/// What asking for bank suggestions gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BankAiAnswer {
    /// The gate was ready: the checked suggestions, and the rows the provider could not answer
    /// or answered with something the engine refused, each with the reason (logged).
    Suggested {
        /// The open suggestions, by row.
        matches: Vec<AiBankMatch>,
        /// Rows not suggested, with why.
        not_read: Vec<(u32, String)>,
    },
    /// The gate refused; nothing was asked and nothing was logged.
    Refused(AiGate),
}

impl Api {
    /// The confirmed matches of `statement`, oldest first, from the audit log.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn bank_matches(&self, statement: BankStatementId) -> Result<Vec<BankMatch>, ApiError> {
        Ok(read_all(self.store())?
            .into_iter()
            .filter_map(|record| match record.event {
                Event::MatchConfirmed(m) if m.statement == statement => Some(m),
                Event::MatchConfirmed(_)
                | Event::MasterCreated(_)
                | Event::MasterAltered { .. }
                | Event::MasterDeleted(_)
                | Event::VoucherPosted(_)
                | Event::VoucherAltered { .. }
                | Event::VoucherCancelled { .. }
                | Event::VoucherDeleted { .. }
                | Event::SettingChanged { .. }
                | Event::PeriodLocked { .. }
                | Event::StatementImported(_)
                | Event::AiProposalRecorded(_)
                | Event::BankStatementStored { .. }
                | Event::Gstr2bImported { .. }
                | Event::ImsActionRecorded { .. }
                | Event::Gstr3bFiled { .. }
                | Event::AiActivityRecorded(_) => None,
            })
            .collect())
    }

    /// Asks the assistant about the lines of `statement` the exact matcher leaves (see the
    /// module documentation).
    ///
    /// # Errors
    /// - [`ApiError::UnknownBankStatement`] when this session does not hold `statement`.
    /// - [`ApiError::Store`] when the audit log cannot be read or an entry cannot be committed.
    pub fn bank_ai_suggestions(
        &mut self,
        statement: BankStatementId,
        cancel: &CancelToken,
    ) -> Result<Outcome<BankAiAnswer>, ApiError> {
        let info = match self.ai_gate(AiFeature::BankSuggestion)? {
            AiGate::Ready(info) => info,
            refused @ (AiGate::NotChosen
            | AiGate::Off
            | AiGate::FeatureOff
            | AiGate::NotInstalled(_)
            | AiGate::NotConfigured(_)) => {
                return Ok(Outcome::Value(BankAiAnswer::Refused(refused)));
            }
        };
        let asks = match self.bank_asks(statement)? {
            Outcome::Value(asks) => asks,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let earlier = self.earlier_bank_drafts()?;
        let failed = self.earlier_bank_failures(&info)?;
        let mut matches = Vec::new();
        let mut not_read = Vec::new();
        for ask in asks {
            match earlier.iter().find(|e| e.hash == ask.hash) {
                Some(Earlier {
                    open: Some(found), ..
                }) => {
                    if let Some(m) = ask.checked(found.0, &found.1) {
                        matches.push(m);
                    }
                    continue;
                }
                Some(Earlier { open: None, .. }) => continue,
                None => {}
            }
            if let Some((_, reason)) = failed.iter().find(|(hash, _)| *hash == ask.hash) {
                not_read.push((ask.question.row, reason.clone()));
                continue;
            }
            let answer = match self.assistant() {
                Some(provider) => provider.match_bank_line(&ask.question, &ask.offered, cancel),
                None => Err(AssistError::Output {
                    reason: "no provider is attached".to_owned(),
                }),
            };
            let checked = answer
                .map_err(|e| e.to_string())
                .and_then(|s| ask.check(&s).map(|()| s));
            match checked {
                Ok(suggestion) => {
                    let draft = self.record_bank_draft(&info, &ask, &suggestion)?;
                    if let Some(m) = ask.checked(draft, &suggestion) {
                        matches.push(m);
                    }
                }
                Err(reason) => {
                    self.record_ai_activity(bank_activity(
                        &info,
                        &ask,
                        AiActivityKind::ReadFailed {
                            reason: reason.clone(),
                        },
                    ))?;
                    not_read.push((ask.question.row, reason));
                }
            }
        }
        Ok(Outcome::Value(BankAiAnswer::Suggested {
            matches,
            not_read,
        }))
    }

    /// Accepts the bank suggestion stored as `draft` for `statement`: the engine checks it again
    /// against the books as they are now and confirms the match as [`MatchKind::AiSuggested`],
    /// with the accept in the same commit. `Unsupported` when the line or the voucher no longer
    /// fits (the voucher cleared, changed amount or direction, or the line was matched).
    ///
    /// # Errors
    /// - [`ApiError::UnknownAiDraft`] when no open draft has that id.
    /// - [`ApiError::UnknownBankStatement`] when this session does not hold `statement`.
    /// - [`ApiError::Engine`] when the period lock covers the voucher. Nothing is written.
    /// - [`ApiError::Store`] when the commit fails.
    pub fn accept_bank_ai_match(
        &mut self,
        statement: BankStatementId,
        draft: EventSeq,
    ) -> Result<Outcome<CommandResult>, ApiError> {
        let stored = self.ai_draft(draft)?;
        let suggestion = match &stored.proposal {
            Proposal::MatchSuggestion(s) if stored.feature == AiFeature::BankSuggestion => {
                s.clone()
            }
            Proposal::MatchSuggestion(_)
            | Proposal::VoucherDraft(_)
            | Proposal::ReportQuery(_)
            | Proposal::MappingSuggestion(_) => {
                return Ok(Outcome::unsupported(
                    "accepting a bank suggestion",
                    "this draft is not a bank match suggestion",
                ));
            }
        };
        let confirmed = self.bank_matches(statement)?;
        if confirmed
            .iter()
            .any(|m| m.statement_lines.contains(&suggestion.statement_row))
        {
            return Ok(Outcome::unsupported(
                "accepting a bank suggestion",
                "this statement line is already matched",
            ));
        }
        let held = self.held_statement(statement)?;
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let open = open_entries(books, held.ledger());
        let ask = held
            .lines()
            .iter()
            .find(|line| line.row == suggestion.statement_row)
            .and_then(|line| Ask::new(statement, held.ledger(), line, &open, books));
        let Some(ask) = ask else {
            return Ok(Outcome::unsupported(
                "accepting a bank suggestion",
                "the statement has no such line, or it moves no money",
            ));
        };
        if let Err(reason) = ask.check(&suggestion) {
            return Ok(Outcome::unsupported("accepting a bank suggestion", reason));
        }
        let matched = BankMatch {
            statement,
            statement_lines: vec![suggestion.statement_row],
            vouchers: suggestion.vouchers.clone(),
            kind: MatchKind::AiSuggested,
            bank_date: ask.question.date,
        };
        let base = draft_activity(&stored);
        self.execute_with(Command::ConfirmBankMatch(matched), |_| {
            vec![Event::AiActivityRecorded(AiActivity {
                kind: AiActivityKind::DraftAccepted {
                    draft,
                    voucher: None,
                    human_edits_json: None,
                },
                ..base
            })]
        })
    }

    /// The lines of `statement` to ask about: those the exact matcher leaves and no confirmed
    /// match covers, each with the open vouchers the matcher leaves in its direction.
    fn bank_asks(&self, statement: BankStatementId) -> Result<Outcome<Vec<Ask>>, ApiError> {
        let confirmed: Vec<u32> = self
            .bank_matches(statement)?
            .into_iter()
            .flat_map(|m| m.statement_lines)
            .collect();
        let held = self.held_statement(statement)?;
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let exact = suggest_matches(held, books, &[]);
        let claimed_lines: Vec<u32> = exact
            .iter()
            .flat_map(|c| c.statement_lines.iter().copied())
            .chain(confirmed)
            .collect();
        let claimed_vouchers: Vec<VoucherId> = exact
            .iter()
            .flat_map(|c| c.vouchers.iter().copied())
            .collect();
        let open: Vec<OpenEntry> = open_entries(books, held.ledger())
            .into_iter()
            .filter(|e| !claimed_vouchers.contains(&e.voucher))
            .collect();
        Ok(Outcome::Value(
            held.lines()
                .iter()
                .filter(|line| !claimed_lines.contains(&line.row))
                .filter_map(|line| Ask::new(statement, held.ledger(), line, &open, books))
                .collect(),
        ))
    }

    /// Stores the checked `suggestion` for `ask` as an open draft.
    fn record_bank_draft(
        &mut self,
        info: &ProviderInfo,
        ask: &Ask,
        suggestion: &MatchSuggestion,
    ) -> Result<EventSeq, ApiError> {
        let activity = bank_activity(
            info,
            ask,
            AiActivityKind::ReadFailed {
                reason: String::new(),
            },
        );
        self.record_ai_draft(NewAiDraft {
            feature: activity.feature,
            provider: activity.provider,
            model_version: activity.model_version,
            prompt_template_version: activity.prompt_template_version,
            input_label: activity.input_label,
            input_hash: activity.input_hash,
            proposal: Proposal::MatchSuggestion(suggestion.clone()),
            confidence_permille: suggestion.confidence_permille.min(1000),
        })
    }

    /// The bank lines whose read already failed with this provider and model version, by input
    /// hash, with the latest reason. Such a line is not asked again (nor logged again) until the
    /// provider or model changes; a read that was cancelled or failed on the network or the key
    /// is asked again.
    fn earlier_bank_failures(
        &self,
        info: &ProviderInfo,
    ) -> Result<Vec<(String, String)>, ApiError> {
        let mut failed: Vec<(String, String)> = Vec::new();
        for e in self.ai_activity()? {
            let a = e.activity;
            if a.feature != AiFeature::BankSuggestion
                || a.provider != info.name
                || a.model_version != info.version
            {
                continue;
            }
            match a.kind {
                AiActivityKind::ReadFailed { reason } => {
                    failed.retain(|(hash, _)| *hash != a.input_hash);
                    if !passing(&reason) {
                        failed.push((a.input_hash, reason));
                    }
                }
                AiActivityKind::DraftStored { .. }
                | AiActivityKind::DraftAccepted { .. }
                | AiActivityKind::DraftRejected { .. }
                | AiActivityKind::Answered { .. }
                | AiActivityKind::CloudConsent { .. }
                | AiActivityKind::NotRun { .. } => {}
            }
        }
        Ok(failed)
    }

    /// Every bank suggestion stored so far, by input hash: open ones with their proposal, decided
    /// ones without.
    fn earlier_bank_drafts(&self) -> Result<Vec<Earlier>, ApiError> {
        let open: Vec<(EventSeq, MatchSuggestion)> = self
            .ai_drafts()?
            .into_iter()
            .filter_map(|d| match d.proposal {
                Proposal::MatchSuggestion(s) => Some((d.id, s)),
                Proposal::VoucherDraft(_)
                | Proposal::ReportQuery(_)
                | Proposal::MappingSuggestion(_) => None,
            })
            .collect();
        Ok(self
            .ai_activity()?
            .into_iter()
            .filter(|e| e.activity.feature == AiFeature::BankSuggestion)
            .filter_map(|e| match e.activity.kind {
                AiActivityKind::DraftStored { .. } => Some(Earlier {
                    hash: e.activity.input_hash,
                    open: open.iter().find(|(id, _)| *id == e.seq).cloned(),
                }),
                AiActivityKind::DraftAccepted { .. }
                | AiActivityKind::DraftRejected { .. }
                | AiActivityKind::Answered { .. }
                | AiActivityKind::ReadFailed { .. }
                | AiActivityKind::CloudConsent { .. }
                | AiActivityKind::NotRun { .. } => None,
            })
            .collect())
    }
}

/// Whether a logged failure `reason` was a passing one, worth asking again: the read was
/// cancelled, or failed on the network or the key. The prefixes are the errors' own wording.
fn passing(reason: &str) -> bool {
    let empty = String::new;
    [
        AssistError::Cancelled.to_string(),
        AssistError::Network { reason: empty() }.to_string(),
        AssistError::Key { reason: empty() }.to_string(),
    ]
    .iter()
    .any(|prefix| reason.starts_with(prefix.as_str()))
}

/// The log entry for `ask` with `kind`.
fn bank_activity(info: &ProviderInfo, ask: &Ask, kind: AiActivityKind) -> AiActivity {
    AiActivity {
        feature: AiFeature::BankSuggestion,
        provider: info.name.clone(),
        model_version: info.version.clone(),
        prompt_template_version: BANK_TEMPLATE.to_owned(),
        input_label: ask.question.description.clone(),
        input_hash: ask.hash.clone(),
        kind,
    }
}

/// A bank suggestion stored earlier: its line's hash, and its proposal while it is open.
struct Earlier {
    hash: String,
    open: Option<(EventSeq, MatchSuggestion)>,
}

/// One line to ask about, with the vouchers offered for it.
struct Ask {
    question: BankLineQuestion,
    offered: Vec<OpenVoucher>,
    hash: String,
}

impl Ask {
    /// The question for `line`, offering the vouchers of `open` in its direction. `None` for a
    /// line that moves no money.
    fn new(
        statement: BankStatementId,
        bank: LedgerId,
        line: &RawLine,
        open: &[OpenEntry],
        books: &Books,
    ) -> Option<Self> {
        let (direction, amount) = line_flow(line)?;
        let offered = open
            .iter()
            .filter(|e| e.direction == direction)
            .map(|e| OpenVoucher {
                id: e.voucher,
                date: e.date,
                ledger_name: other_side(books, e.voucher, bank),
                amount: e.amount,
                deposit: e.direction == LineDirection::Deposit,
            })
            .collect();
        let key = format!(
            "{}:{}:{}:{}:{}",
            statement.0,
            line.row,
            line.date,
            amount.get(),
            line.description
        );
        Some(Ask {
            question: BankLineQuestion {
                row: line.row,
                date: line.date,
                description: line.description.clone(),
                amount,
                deposit: direction == LineDirection::Deposit,
            },
            offered,
            hash: hex(&Sha256::digest(key.as_bytes())),
        })
    }

    /// The engine's check of `s` for this line.
    fn check(&self, s: &MatchSuggestion) -> Result<(), String> {
        if s.statement_row != self.question.row {
            return Err(format!(
                "the suggestion is for another statement line (row {}), not row {}",
                s.statement_row, self.question.row
            ));
        }
        if s.ledger_name.is_some() {
            return Err(
                "the suggestion posts a new voucher; a voucher from a bank line is not built"
                    .to_owned(),
            );
        }
        let [voucher] = s.vouchers.as_slice() else {
            return Err(format!(
                "a bank suggestion names exactly one voucher; this one names {}",
                s.vouchers.len()
            ));
        };
        let Some(offered) = self.offered.iter().find(|o| o.id == *voucher) else {
            return Err(format!(
                "voucher {} is not one the engine offered for this line (open, on this bank, in \
                 this direction)",
                voucher.0
            ));
        };
        if offered.amount != self.question.amount {
            return Err(format!(
                "the amounts differ: the line is {} and voucher {} is {} on the bank",
                self.question.amount.to_rupees(),
                voucher.0,
                offered.amount.to_rupees()
            ));
        }
        Ok(())
    }

    /// The match `s` (already checked) shows, stored as `draft`.
    fn checked(&self, draft: EventSeq, s: &MatchSuggestion) -> Option<AiBankMatch> {
        let voucher = *s.vouchers.first()?;
        let offered = self.offered.iter().find(|o| o.id == voucher)?;
        self.check(s).ok()?;
        Some(AiBankMatch {
            draft,
            line: self.question.row,
            voucher,
            amount: self.question.amount,
            days_apart: (self.question.date - offered.date).whole_days().abs(),
            bank_date: self.question.date,
            confidence_permille: s.confidence_permille.min(1000),
        })
    }
}

/// The name of the first ledger of `voucher` that is not `bank`.
fn other_side(books: &Books, voucher: VoucherId, bank: LedgerId) -> String {
    books
        .vouchers
        .get(&voucher)
        .and_then(|v| v.ledger_lines.iter().find(|l| l.ledger != bank))
        .and_then(|l| books.masters.ledgers.get(&l.ledger))
        .map_or_else(String::new, |l| l.name.clone())
}
