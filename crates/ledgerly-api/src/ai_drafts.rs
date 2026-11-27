//! The assistant's draft store and activity log (round s21, plan round A; spec 6.6 and 6.8).
//!
//! The assistant never posts. What it proposes is stored as a draft: an
//! `Event::AiActivityRecorded` whose kind is `DraftStored`, with the proposal as JSON. The draft's
//! id is that event's sequence number. A draft stays open until a person accepts it (the engine
//! posts the voucher, in the same commit as the `DraftAccepted` event that names it) or rejects
//! it (`DraftRejected`, nothing posted). Every one of these is an audited event, so the AI activity
//! log is simply the list of them ([`Api::ai_activity`]).
//!
//! Figures. A draft carries only what the assistant transcribed. Its amount in the review queue,
//! and every figure a screen shows for it, comes from [`Api::check_ai_draft`], which validates the
//! draft through `ledgerly_assist::validate` (the engine and GST crates recompute it). A draft the
//! engine blocks has no amount; the block's reason is shown instead.

use ledgerly_assist::Proposal;
use ledgerly_assist::validate::{DraftCheck, validate_draft_with_place};
use ledgerly_core::{Date, EventSeq, LedgerId, Outcome, StateCode, VoucherId};
use ledgerly_gst::pack::load_pack;
use ledgerly_model::{AiActivity, AiActivityKind, AiFeature, Voucher};
use ledgerly_store::{AuditRecord, Event};
use time::OffsetDateTime;

use crate::command::Command;
use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;
use crate::gst::BUNDLED_RATE_PACK;

/// The least confidence, in per-mille, at which a transcribed value is kept. A value read with
/// less is blank and the person types it (spec 6.8: "below threshold → blank"). This is the
/// product's own setting, not a figure from any law or help page.
pub const AI_CONFIDENCE_THRESHOLD_PERMILLE: u16 = 700;

/// A proposal to store as a draft, with what produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAiDraft {
    /// The feature that produced it.
    pub feature: AiFeature,
    /// The provider's name for itself.
    pub provider: String,
    /// The provider's version.
    pub model_version: String,
    /// The prompt template version.
    pub prompt_template_version: String,
    /// A short label of the input: a file name or the typed sentence.
    pub input_label: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// The proposal.
    pub proposal: Proposal,
    /// The provider's confidence in per-mille (0..=1000).
    pub confidence_permille: u16,
}

/// An open draft: stored, and neither accepted nor rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDraft {
    /// The draft's id: the sequence number of the event that stored it.
    pub id: EventSeq,
    /// When it was stored.
    pub at: OffsetDateTime,
    /// The feature that produced it.
    pub feature: AiFeature,
    /// The provider's name for itself.
    pub provider: String,
    /// The provider's version.
    pub model_version: String,
    /// The prompt template version.
    pub prompt_template_version: String,
    /// The input label.
    pub input_label: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// The proposal, as stored.
    pub proposal: Proposal,
    /// The provider's confidence in per-mille.
    pub confidence_permille: u16,
}

/// What the person changed or supplied before accepting. Each value is recorded with the accept
/// as the person's edit; none is a transcription.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftEdits {
    /// The voucher date, when the draft has none or the person changed it (the voucher form
    /// offers today's date, as it does for any new voucher).
    pub date: Option<Date>,
    /// The place of supply the person confirmed, for a draft that needs GST.
    pub place_of_supply: Option<StateCode>,
    /// The ledgers the person chose for draft lines, by the line's index in the draft. A bill's
    /// taxable value names no purchase ledger, so the person chooses it; a choice for a line the
    /// reader matched replaces the reader's ledger (it is a human edit and is logged as one).
    pub line_ledgers: Vec<(usize, LedgerId)>,
    /// The read fields the person confirmed against the document. A value that needs review
    /// (read by ocrs, which gives no confidence) is kept only once confirmed; while any such
    /// field is unconfirmed the draft is blocked on it. Logged as a human edit.
    pub confirmed: Vec<ledgerly_assist::review::DraftField>,
}

/// One row of the AI activity log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiActivityEntry {
    /// The sequence number of the event.
    pub seq: EventSeq,
    /// When it was recorded.
    pub at: OffsetDateTime,
    /// The event as stored.
    pub activity: AiActivity,
}

impl Api {
    /// Every assistant event of the company file, in the order it was recorded.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn ai_activity(&self) -> Result<Vec<AiActivityEntry>, ApiError> {
        let records = read_all(self.store())?;
        Ok(records.into_iter().filter_map(activity_of).collect())
    }

    /// The open drafts, oldest first.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read.
    /// - [`ApiError::AiDraftUnreadable`] when a stored proposal is not one this build can read.
    pub fn ai_drafts(&self) -> Result<Vec<AiDraft>, ApiError> {
        let entries = self.ai_activity()?;
        let decided: Vec<EventSeq> = entries
            .iter()
            .filter_map(|e| decided_draft(&e.activity.kind))
            .collect();
        let mut out = Vec::new();
        for entry in entries {
            let AiActivityKind::DraftStored {
                proposal_json,
                confidence_permille,
            } = &entry.activity.kind
            else {
                continue;
            };
            if decided.contains(&entry.seq) {
                continue;
            }
            let proposal = parse_proposal(entry.seq, proposal_json)?;
            out.push(AiDraft {
                id: entry.seq,
                at: entry.at,
                feature: entry.activity.feature,
                provider: entry.activity.provider.clone(),
                model_version: entry.activity.model_version.clone(),
                prompt_template_version: entry.activity.prompt_template_version.clone(),
                input_label: entry.activity.input_label.clone(),
                input_hash: entry.activity.input_hash.clone(),
                proposal,
                confidence_permille: *confidence_permille,
            });
        }
        Ok(out)
    }

    /// The open draft `id`.
    ///
    /// # Errors
    /// - [`ApiError::UnknownAiDraft`] when no open draft has that id (never stored, or decided).
    /// - As [`Api::ai_drafts`].
    pub fn ai_draft(&self, id: EventSeq) -> Result<AiDraft, ApiError> {
        self.ai_drafts()?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(ApiError::UnknownAiDraft { id: id.0 })
    }

    /// Stores `draft` as an open draft, as an audited assistant event. Nothing is posted.
    ///
    /// # Errors
    /// - [`ApiError::AiDraftUnreadable`] when the proposal cannot be written as JSON.
    /// - [`ApiError::Store`] when the commit fails.
    pub fn record_ai_draft(&mut self, draft: NewAiDraft) -> Result<EventSeq, ApiError> {
        let proposal_json =
            serde_json::to_string(&draft.proposal).map_err(|e| ApiError::AiDraftUnreadable {
                id: 0,
                reason: e.to_string(),
            })?;
        let event = Event::AiActivityRecorded(AiActivity {
            feature: draft.feature,
            provider: draft.provider,
            model_version: draft.model_version,
            prompt_template_version: draft.prompt_template_version,
            input_label: draft.input_label,
            input_hash: draft.input_hash,
            kind: AiActivityKind::DraftStored {
                proposal_json,
                confidence_permille: draft.confidence_permille,
            },
        });
        let actor = self.actor().clone();
        let receipt = self.commit_as(&[event], &actor)?;
        receipt
            .and_then(|r| r.seqs.first().copied())
            .ok_or(ApiError::Stale)
    }

    /// Records an assistant event that is not a draft (a question answered, a read that failed,
    /// a cloud consent).
    ///
    /// # Errors
    /// [`ApiError::Store`] when the commit fails.
    pub fn record_ai_activity(&mut self, activity: AiActivity) -> Result<(), ApiError> {
        let actor = self.actor().clone();
        self.commit_as(&[Event::AiActivityRecorded(activity)], &actor)?;
        Ok(())
    }

    /// The engine's check of draft `id` with the person's `edits`: the voucher the draft becomes,
    /// with every figure recomputed, or why it is blocked. `Unsupported` for a draft that is not a
    /// voucher, or one the engine cannot check (item lines, GST without a place of supply).
    ///
    /// # Errors
    /// - [`ApiError::UnknownAiDraft`] when no open draft has that id.
    /// - [`ApiError::Gst`] when the bundled rate pack cannot be read.
    /// - [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn check_ai_draft(
        &self,
        id: EventSeq,
        edits: &DraftEdits,
    ) -> Result<Outcome<DraftCheck>, ApiError> {
        let draft = self.ai_draft(id)?;
        self.check_proposal(&draft.proposal, edits)
    }

    /// The engine's check of a proposal that is not stored yet (the screen shows the check
    /// before the person decides). See [`Api::check_ai_draft`].
    ///
    /// # Errors
    /// As [`Api::check_ai_draft`], without the unknown-draft case.
    pub fn check_proposal(
        &self,
        proposal: &Proposal,
        edits: &DraftEdits,
    ) -> Result<Outcome<DraftCheck>, ApiError> {
        let mut draft = match proposal {
            Proposal::VoucherDraft(d) => d.as_ref().clone(),
            Proposal::MatchSuggestion(_)
            | Proposal::ReportQuery(_)
            | Proposal::MappingSuggestion(_) => {
                return Ok(Outcome::unsupported(
                    "an engine check of this proposal",
                    "only a draft voucher becomes a voucher; bank matches are checked on the bank screen and questions are run as reports",
                ));
            }
        };
        if let Some(date) = edits.date {
            draft.date = Some(ledgerly_assist::Transcribed {
                value: date,
                source: ledgerly_assist::SourceRef::Edited,
                confidence_permille: 1000,
            });
        }
        for &field in &edits.confirmed {
            if !ledgerly_assist::review::confirm(&mut draft, field) {
                return Ok(Outcome::unsupported(
                    "a confirmed draft field",
                    format!("the draft has no {}", field.name()),
                ));
            }
        }
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        for &(index, ledger) in &edits.line_ledgers {
            let Some(line) = draft.lines.get_mut(index) else {
                return Ok(Outcome::unsupported(
                    "a ledger chosen for a draft line",
                    format!("the draft has no line {index}"),
                ));
            };
            line.ledger = Some(ledger);
            if let Some(master) = books.masters.ledgers.get(&ledger) {
                line.ledger_name = ledgerly_assist::Transcribed {
                    value: master.name.clone(),
                    source: ledgerly_assist::SourceRef::Edited,
                    confidence_permille: 1000,
                };
            }
        }
        let waiting = first_to_confirm(&draft);
        if let Some(field) = waiting {
            return Ok(Outcome::Value(DraftCheck::Blocked {
                field: field.name(),
                reason: "this value was read by the OCR reader and needs review: check it against the bill and confirm it".to_owned(),
            }));
        }
        let pack = load_pack(BUNDLED_RATE_PACK)?;
        let rules_date = draft
            .date
            .as_ref()
            .map_or_else(|| self.clock_date(), |d| d.value);
        let rules = match pack.for_time_of_supply(rules_date) {
            Outcome::Value(rules) => rules,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        match validate_draft_with_place(
            &draft,
            books,
            &rules,
            AI_CONFIDENCE_THRESHOLD_PERMILLE,
            edits.place_of_supply,
        ) {
            Ok(check) => Ok(Outcome::Value(check)),
            Err(ledgerly_assist::AssistError::Unsupported(u)) => Ok(Outcome::Unsupported(u)),
            Err(other) => Ok(Outcome::unsupported(
                "an engine check of this draft",
                other.to_string(),
            )),
        }
    }

    /// Accepts draft `id`: the engine checks it again with `edits`, posts the voucher, and the
    /// accept is recorded in the same commit. A blocked draft posts nothing and is returned as
    /// it was blocked; the draft stays open.
    ///
    /// # Errors
    /// - As [`Api::check_ai_draft`].
    /// - [`ApiError::Engine`] when the engine refuses the post (nothing is written).
    /// - [`ApiError::Store`] when the commit fails.
    pub fn accept_ai_draft(
        &mut self,
        id: EventSeq,
        edits: &DraftEdits,
    ) -> Result<Outcome<Accepted>, ApiError> {
        let draft = self.ai_draft(id)?;
        let voucher = match self.check_ai_draft(id, edits)? {
            Outcome::Value(DraftCheck::Ok(voucher)) => voucher,
            Outcome::Value(DraftCheck::Blocked { field, reason }) => {
                return Ok(Outcome::Value(Accepted::Blocked { field, reason }));
            }
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let human_edits_json = edits_json(edits);
        let base = draft_activity(&draft);
        let result = self.execute_with(Command::PostVoucher(voucher), |events| {
            let posted = events.iter().find_map(|e| match e {
                Event::VoucherPosted(v) => Some(v.id),
                Event::MasterCreated(_)
                | Event::MasterAltered { .. }
                | Event::MasterDeleted(_)
                | Event::VoucherAltered { .. }
                | Event::VoucherCancelled { .. }
                | Event::VoucherDeleted { .. }
                | Event::SettingChanged { .. }
                | Event::PeriodLocked { .. }
                | Event::StatementImported(_)
                | Event::MatchConfirmed(_)
                | Event::AiProposalRecorded(_)
                | Event::BankStatementStored { .. }
                | Event::Gstr2bImported { .. }
                | Event::ImsActionRecorded { .. }
                | Event::Gstr3bFiled { .. }
                | Event::AiActivityRecorded(_) => None,
            });
            vec![Event::AiActivityRecorded(AiActivity {
                kind: AiActivityKind::DraftAccepted {
                    draft: id,
                    voucher: posted,
                    human_edits_json,
                },
                ..base
            })]
        })?;
        match result {
            Outcome::Value(result) => match result.posted {
                Some(voucher) => Ok(Outcome::Value(Accepted::Posted(Box::new(voucher)))),
                None => Ok(Outcome::unsupported(
                    "accepting this draft",
                    "the engine posted nothing for it",
                )),
            },
            Outcome::Unsupported(u) => Ok(Outcome::Unsupported(u)),
        }
    }

    /// Rejects draft `id` with the person's optional `reason`. Nothing is posted.
    ///
    /// # Errors
    /// - [`ApiError::UnknownAiDraft`] when no open draft has that id.
    /// - [`ApiError::Store`] when the commit fails.
    pub fn reject_ai_draft(
        &mut self,
        id: EventSeq,
        reason: Option<String>,
    ) -> Result<(), ApiError> {
        let draft = self.ai_draft(id)?;
        let event = Event::AiActivityRecorded(AiActivity {
            kind: AiActivityKind::DraftRejected { draft: id, reason },
            ..draft_activity(&draft)
        });
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(())
    }

    /// The voucher an accepted draft became, by voucher id: the draft's feature and provider, for
    /// the day book's Origin cell. `None` when the voucher was not posted from a draft.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn ai_origin(&self, voucher: VoucherId) -> Result<Option<AiActivity>, ApiError> {
        Ok(self
            .ai_activity()?
            .into_iter()
            .find_map(|e| match &e.activity.kind {
                AiActivityKind::DraftAccepted {
                    voucher: Some(v), ..
                } if *v == voucher => Some(e.activity),
                AiActivityKind::DraftStored { .. }
                | AiActivityKind::DraftAccepted { .. }
                | AiActivityKind::DraftRejected { .. }
                | AiActivityKind::Answered { .. }
                | AiActivityKind::ReadFailed { .. }
                | AiActivityKind::CloudConsent { .. }
                | AiActivityKind::NotRun { .. } => None,
            }))
    }
}

/// What accepting a draft did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accepted {
    /// The engine posted this voucher.
    Posted(Box<Voucher>),
    /// The engine blocked the draft; nothing was posted and the draft stays open.
    Blocked {
        /// The draft field at fault.
        field: String,
        /// Why, in words the screen shows.
        reason: String,
    },
}

/// The assistant event of a record, if it is one.
fn activity_of(record: AuditRecord) -> Option<AiActivityEntry> {
    match record.event {
        Event::AiActivityRecorded(activity) => Some(AiActivityEntry {
            seq: record.seq,
            at: record.meta.at,
            activity,
        }),
        Event::MasterCreated(_)
        | Event::MasterAltered { .. }
        | Event::MasterDeleted(_)
        | Event::VoucherPosted(_)
        | Event::VoucherAltered { .. }
        | Event::VoucherCancelled { .. }
        | Event::VoucherDeleted { .. }
        | Event::SettingChanged { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::MatchConfirmed(_)
        | Event::AiProposalRecorded(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. } => None,
    }
}

/// The draft a decision names, if the event is a decision.
fn decided_draft(kind: &AiActivityKind) -> Option<EventSeq> {
    match kind {
        AiActivityKind::DraftAccepted { draft, .. }
        | AiActivityKind::DraftRejected { draft, .. } => Some(*draft),
        AiActivityKind::DraftStored { .. }
        | AiActivityKind::Answered { .. }
        | AiActivityKind::ReadFailed { .. }
        | AiActivityKind::CloudConsent { .. }
        | AiActivityKind::NotRun { .. } => None,
    }
}

/// The stored proposal, read back.
fn parse_proposal(id: EventSeq, json: &str) -> Result<Proposal, ApiError> {
    serde_json::from_str(json).map_err(|e| ApiError::AiDraftUnreadable {
        id: id.0,
        reason: e.to_string(),
    })
}

/// The fields a decision event repeats from the draft it decides (its kind is set by the caller).
pub(crate) fn draft_activity(draft: &AiDraft) -> AiActivity {
    AiActivity {
        feature: draft.feature,
        provider: draft.provider.clone(),
        model_version: draft.model_version.clone(),
        prompt_template_version: draft.prompt_template_version.clone(),
        input_label: draft.input_label.clone(),
        input_hash: draft.input_hash.clone(),
        kind: AiActivityKind::Answered {
            output_json: String::new(),
        },
    }
}

/// The person's edits as JSON, or `None` when they changed nothing.
fn edits_json(edits: &DraftEdits) -> Option<String> {
    if edits.date.is_none()
        && edits.place_of_supply.is_none()
        && edits.line_ledgers.is_empty()
        && edits.confirmed.is_empty()
    {
        return None;
    }
    let mut map = serde_json::Map::new();
    if let Some(date) = edits.date {
        map.insert(
            "date".to_owned(),
            serde_json::Value::String(date.to_string()),
        );
    }
    if let Some(place) = edits.place_of_supply {
        map.insert(
            "place_of_supply".to_owned(),
            serde_json::Value::from(place.get()),
        );
    }
    if !edits.line_ledgers.is_empty() {
        let chosen = edits
            .line_ledgers
            .iter()
            .map(|&(index, ledger)| serde_json::json!({ "line": index, "ledger": ledger.0 }))
            .collect();
        map.insert("line_ledgers".to_owned(), serde_json::Value::Array(chosen));
    }
    if !edits.confirmed.is_empty() {
        let names = edits
            .confirmed
            .iter()
            .map(|f| serde_json::Value::String(f.name()))
            .collect();
        map.insert("confirmed".to_owned(), serde_json::Value::Array(names));
    }
    Some(serde_json::Value::Object(map).to_string())
}

/// What an assistant event produced, as the AI activity log's Output column names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiOutput {
    /// A draft voucher of this kind.
    DraftVoucher(ledgerly_model::BaseVoucherKind),
    /// A bank-match suggestion.
    MatchSuggestion,
    /// A report query (the engine runs it).
    ReportQuery,
    /// A column-mapping suggestion.
    MappingSuggestion,
    /// An answer, as stored JSON.
    Answered,
    /// Nothing was read; the reason.
    ReadFailed(String),
    /// The provider answered but the engine could not run it; the reason.
    NotRun(String),
    /// A cloud consent, granted or not.
    CloudConsent(bool),
}

/// What the person decided about an assistant event, as the log's last column says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiDecision {
    /// A draft still open in the review queue.
    Waiting,
    /// Accepted; the engine posted this voucher (its number, or its id when it has none).
    Accepted(Option<String>),
    /// Rejected, with the reason given.
    Rejected(Option<String>),
    /// Nothing to decide (an answer, a failed read, a consent).
    NoDecision,
}

/// One row of the AI activity log: an assistant event and what became of it. Accept and reject
/// events are folded into the row of the draft they decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiLogRow {
    /// When the event was recorded.
    pub at: OffsetDateTime,
    /// The feature that produced it.
    pub feature: AiFeature,
    /// The provider's name and model version.
    pub provider: String,
    /// The input's name (a file name or the typed words).
    pub input_label: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// What it produced.
    pub output: AiOutput,
    /// What the person decided.
    pub decision: AiDecision,
}

impl Api {
    /// The AI activity log as rows, oldest first: one per draft, answer, failed read or consent,
    /// each with the decision taken on it.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read.
    /// - [`ApiError::AiDraftUnreadable`] when a stored proposal is not one this build can read.
    pub fn ai_log_rows(&self) -> Result<Vec<AiLogRow>, ApiError> {
        let entries = self.ai_activity()?;
        let books = match self.books()? {
            Outcome::Value(b) => Some(b),
            Outcome::Unsupported(_) => None,
        };
        let mut rows = Vec::new();
        for entry in &entries {
            let output = match &entry.activity.kind {
                AiActivityKind::DraftStored { proposal_json, .. } => {
                    match parse_proposal(entry.seq, proposal_json)? {
                        Proposal::VoucherDraft(d) => AiOutput::DraftVoucher(d.kind),
                        Proposal::MatchSuggestion(_) => AiOutput::MatchSuggestion,
                        Proposal::ReportQuery(_) => AiOutput::ReportQuery,
                        Proposal::MappingSuggestion(_) => AiOutput::MappingSuggestion,
                    }
                }
                AiActivityKind::Answered { .. } => AiOutput::Answered,
                AiActivityKind::ReadFailed { reason } => AiOutput::ReadFailed(reason.clone()),
                AiActivityKind::CloudConsent { granted } => AiOutput::CloudConsent(*granted),
                AiActivityKind::NotRun { reason } => AiOutput::NotRun(reason.clone()),
                AiActivityKind::DraftAccepted { .. } | AiActivityKind::DraftRejected { .. } => {
                    continue;
                }
            };
            let decision = match &output {
                AiOutput::DraftVoucher(_)
                | AiOutput::MatchSuggestion
                | AiOutput::ReportQuery
                | AiOutput::MappingSuggestion => entries
                    .iter()
                    .find_map(|e| match &e.activity.kind {
                        AiActivityKind::DraftAccepted { draft, voucher, .. }
                            if *draft == entry.seq =>
                        {
                            let number = voucher.map(|id| {
                                books
                                    .as_ref()
                                    .and_then(|b| b.vouchers.get(&id))
                                    .and_then(|v| v.number.clone())
                                    .unwrap_or_else(|| id.0.to_string())
                            });
                            Some(AiDecision::Accepted(number))
                        }
                        AiActivityKind::DraftRejected { draft, reason } if *draft == entry.seq => {
                            Some(AiDecision::Rejected(reason.clone()))
                        }
                        AiActivityKind::DraftAccepted { .. }
                        | AiActivityKind::DraftRejected { .. }
                        | AiActivityKind::DraftStored { .. }
                        | AiActivityKind::Answered { .. }
                        | AiActivityKind::ReadFailed { .. }
                        | AiActivityKind::CloudConsent { .. }
                        | AiActivityKind::NotRun { .. } => None,
                    })
                    .unwrap_or(AiDecision::Waiting),
                AiOutput::Answered
                | AiOutput::ReadFailed(_)
                | AiOutput::NotRun(_)
                | AiOutput::CloudConsent(_) => AiDecision::NoDecision,
            };
            rows.push(AiLogRow {
                at: entry.at,
                feature: entry.activity.feature,
                provider: format!(
                    "{} {}",
                    entry.activity.provider, entry.activity.model_version
                ),
                input_label: entry.activity.input_label.clone(),
                input_hash: entry.activity.input_hash.clone(),
                output,
                decision,
            });
        }
        Ok(rows)
    }
}

impl Api {
    /// Writes the assistant's activity log to `path` as CSV: a header, then one line per
    /// [`AiLogRow`] with the time, feature, provider, input, full input hash, output and decision.
    /// Nothing is written when there is no row.
    ///
    /// # Errors
    /// - As [`Api::ai_log_rows`].
    /// - [`ApiError::AiLogWrite`] when the file cannot be written.
    pub fn export_ai_activity(&self, path: &std::path::Path) -> Result<Outcome<()>, ApiError> {
        use std::io::Write as _;
        let rows = self.ai_log_rows()?;
        if rows.is_empty() {
            return Ok(Outcome::unsupported(
                "AI activity log export",
                "no assistant event has been recorded for this company yet",
            ));
        }
        let write_err = |source| ApiError::AiLogWrite {
            path: path.to_path_buf(),
            source,
        };
        let when = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]");
        let mut out = String::from("When,Feature,Provider,Input,Input hash,Output,Your decision\n");
        for row in &rows {
            let at = row
                .at
                .format(&when)
                .map_err(|e| write_err(std::io::Error::other(e)))?;
            let cells = [
                at,
                feature_words(row.feature).to_owned(),
                row.provider.clone(),
                row.input_label.clone(),
                row.input_hash.clone(),
                output_words(&row.output),
                decision_words(&row.decision),
            ];
            let line: Vec<String> = cells.iter().map(|c| crate::ai::csv_field(c)).collect();
            out.push_str(&line.join(","));
            out.push('\n');
        }
        std::fs::File::create(path)
            .and_then(|mut file| file.write_all(out.as_bytes()))
            .map_err(write_err)?;
        Ok(Outcome::Value(()))
    }
}

/// The export's word for a feature (the AI settings log's words).
fn feature_words(feature: AiFeature) -> &'static str {
    match feature {
        AiFeature::BillReading => "Bill reader",
        AiFeature::PlainWords => "Plain words",
        AiFeature::BankSuggestion => "Bank matching",
        AiFeature::Ask => "Ask your books",
        AiFeature::Gstr2bExplanation => "2B assistant",
    }
}

/// The export's words for what an event produced.
fn output_words(output: &AiOutput) -> String {
    match output {
        AiOutput::DraftVoucher(kind) => format!("Draft {}", kind_word(*kind)),
        AiOutput::MatchSuggestion => "Match suggestion".to_owned(),
        AiOutput::ReportQuery => "Report query, run by the engine".to_owned(),
        AiOutput::MappingSuggestion => "Column mapping".to_owned(),
        AiOutput::Answered => "Answer".to_owned(),
        AiOutput::ReadFailed(reason) => format!("Not read: {reason}"),
        AiOutput::NotRun(reason) => format!("Not run: {reason}"),
        AiOutput::CloudConsent(true) => "Sending allowed".to_owned(),
        AiOutput::CloudConsent(false) => "Sending refused".to_owned(),
    }
}

/// The voucher kind in lower-case words, for the export.
fn kind_word(kind: ledgerly_model::BaseVoucherKind) -> &'static str {
    use ledgerly_model::BaseVoucherKind as K;
    match kind {
        K::Purchase => "purchase",
        K::Payment => "payment",
        K::Receipt => "receipt",
        K::Sales => "sales",
        K::Contra => "contra",
        K::Journal => "journal",
        K::CreditNote => "credit note",
        K::DebitNote => "debit note",
        K::Memorandum => "memorandum",
        K::ReversingJournal => "reversing journal",
        K::ReceiptNote => "receipt note",
        K::DeliveryNote => "delivery note",
        K::RejectionIn => "rejection in",
        K::RejectionOut => "rejection out",
        K::StockJournal => "stock journal",
        K::PhysicalStock => "physical stock",
        K::SalesOrder => "sales order",
        K::PurchaseOrder => "purchase order",
    }
}

/// The export's words for a decision.
fn decision_words(decision: &AiDecision) -> String {
    match decision {
        AiDecision::Waiting => "Waiting in review".to_owned(),
        AiDecision::Accepted(Some(number)) => format!("Accepted as {number}"),
        AiDecision::Accepted(None) => "Accepted".to_owned(),
        AiDecision::Rejected(Some(reason)) => format!("Rejected: {reason}"),
        AiDecision::Rejected(None) => "Rejected".to_owned(),
        AiDecision::NoDecision => "Nothing to decide".to_owned(),
    }
}

/// The first read value of `draft` still waiting for the person's confirmation. A line that names
/// no ledger has nothing to confirm, only a ledger to choose: the validator's missing-ledger block
/// asks for that choice.
fn first_to_confirm(
    draft: &ledgerly_assist::VoucherDraft,
) -> Option<ledgerly_assist::review::DraftField> {
    use ledgerly_assist::review::DraftField;
    ledgerly_assist::review::needs_review(draft)
        .into_iter()
        .find(|f| match *f {
            DraftField::LineLedger(n) => draft.lines.get(n).is_some_and(|l| l.ledger.is_some()),
            DraftField::Date
            | DraftField::PartyName
            | DraftField::InvoiceNumber
            | DraftField::SupplierGstin
            | DraftField::LineAmount(_)
            | DraftField::DocumentTotal
            | DraftField::DocumentTax => true,
        })
}
