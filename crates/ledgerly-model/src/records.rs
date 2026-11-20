use ledgerly_core::{BankStatementId, Date, VoucherId};
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
