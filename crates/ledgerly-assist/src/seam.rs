use ledgerly_core::{CancelToken, Date, SignedAmount, Unsupported};
use thiserror::Error;

use crate::proposal::Proposal;

/// The assistant seam (rule C2). Implementations: local model, cloud BYOK, recorded test double.
pub trait Proposer {
    /// Produces a proposal for the input. Never writes anything.
    ///
    /// # Errors
    /// [`AssistError`] when no proposal can be produced.
    fn propose(&self, input: &ProposalInput, cancel: &CancelToken)
    -> Result<Proposal, AssistError>;
}

/// What the assistant is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalInput {
    /// Text the user typed ("paid rent 25k to Sharma Properties by HDFC").
    Text(String),
    /// A bill or invoice image or PDF.
    Document {
        /// File bytes.
        bytes: Vec<u8>,
        /// MIME type.
        mime: String,
        /// SHA-256, hex.
        sha256: String,
    },
    /// A bank statement line to match.
    BankLine {
        /// Row (1-based).
        row: u32,
        /// Date.
        date: Date,
        /// Description.
        description: String,
        /// Reference.
        reference: Option<String>,
        /// Amount from the bank's view.
        amount: SignedAmount,
    },
    /// Import columns to map.
    Columns {
        /// Header row.
        headers: Vec<String>,
        /// A few sample rows.
        sample_rows: Vec<Vec<String>>,
    },
    /// A question about the books.
    Question(String),
}

/// Assistant errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssistError {
    /// Cancelled by the user.
    #[error("cancelled")]
    Cancelled,
    /// The assistant is off for this company.
    #[error("the assistant is turned off for this company")]
    Disabled,
    /// The model failed or returned output that does not fit the schema.
    #[error("model output rejected: {reason}")]
    Output {
        /// Why.
        reason: String,
    },
    /// Network problem (cloud tier only).
    #[error("network error: {reason}")]
    Network {
        /// Why.
        reason: String,
    },
    /// Key missing or rejected (cloud tier only).
    #[error("API key problem: {reason}")]
    Key {
        /// Why.
        reason: String,
    },
    /// The request needs something not supported.
    #[error("unsupported: {} ({})", .0.what, .0.reason)]
    Unsupported(Unsupported),
}
