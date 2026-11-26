use ledgerly_core::{BankStatementId, Date, EventSeq, VoucherId};
use serde::{Deserialize, Serialize};

/// A user-confirmed bank match, stored as an audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankMatch {
    /// Statement.
    pub statement: BankStatementId,
    /// Statement line numbers (1-based) in the match.
    pub statement_lines: Vec<u32>,
    /// Vouchers in the match.
    pub vouchers: Vec<VoucherId>,
    /// How it was found.
    pub kind: MatchKind,
    /// Bank date set on the vouchers' bank lines.
    pub bank_date: Date,
}

/// How a bank match was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatchKind {
    /// Exact amount and reference.
    Exact,
    /// A user rule.
    Rule,
    /// Suggested by the assistant.
    AiSuggested,
    /// Chosen by hand.
    Manual,
}

/// A record of one assistant proposal and what the user did with it (spec 6.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalRecord {
    /// Model name.
    pub model: String,
    /// Model version.
    pub model_version: String,
    /// Prompt template version.
    pub prompt_template_version: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// The proposal as produced, JSON.
    pub output_json: String,
    /// The user's edits before accepting, JSON.
    pub human_edits_json: Option<String>,
    /// Whether the user accepted it.
    pub accepted: bool,
}

/// An assistant feature (spec 6.8). Each can be switched off on its own for a company.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AiFeature {
    /// A bill photo or PDF read into a draft purchase.
    BillReading,
    /// A sentence in plain words turned into a draft voucher.
    PlainWords,
    /// A bank statement line matched to a voucher or a ledger.
    BankSuggestion,
    /// A question about the books turned into a report query.
    Ask,
    /// A GSTR-2B mismatch explained for a purchase record.
    Gstr2bExplanation,
}

impl AiFeature {
    /// Every feature, in the order the AI settings list them.
    pub const ALL: [AiFeature; 5] = [
        AiFeature::BillReading,
        AiFeature::PlainWords,
        AiFeature::BankSuggestion,
        AiFeature::Ask,
        AiFeature::Gstr2bExplanation,
    ];
}

/// One assistant event, kept in the company file's audit log (spec 6.6 and 6.8). The AI activity
/// log is the list of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiActivity {
    /// The feature that produced the event.
    pub feature: AiFeature,
    /// The provider that answered, by the name it gives itself (for example the local model file
    /// or the cloud model name).
    pub provider: String,
    /// The provider's version, as it states it.
    pub model_version: String,
    /// The prompt template version used.
    pub prompt_template_version: String,
    /// A short label of the input the user gave, such as a file name or the typed sentence.
    pub input_label: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// What happened.
    pub kind: AiActivityKind,
}

/// What an [`AiActivity`] records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiActivityKind {
    /// A proposal was stored as a draft waiting for a decision. Its id is the sequence number of
    /// this event.
    DraftStored {
        /// The proposal as produced, JSON.
        proposal_json: String,
        /// The provider's confidence in per-mille (0..=1000).
        confidence_permille: u16,
    },
    /// A person accepted a draft; the engine posted it.
    DraftAccepted {
        /// The draft (sequence number of its `DraftStored` event).
        draft: EventSeq,
        /// The voucher the engine posted, when the draft was a voucher.
        voucher: Option<VoucherId>,
        /// The person's edits before accepting, JSON.
        human_edits_json: Option<String>,
    },
    /// A person rejected a draft. Nothing was posted.
    DraftRejected {
        /// The draft (sequence number of its `DraftStored` event).
        draft: EventSeq,
        /// The reason the person gave, if any.
        reason: Option<String>,
    },
    /// A question was answered with a report query the engine ran.
    Answered {
        /// The query as produced, JSON.
        output_json: String,
    },
    /// The provider could not produce a proposal. Nothing was stored but this event.
    ReadFailed {
        /// Why, in words the screen shows.
        reason: String,
    },
    /// The person was asked before a document went to their cloud provider.
    CloudConsent {
        /// Whether they agreed to send it.
        granted: bool,
    },
}
