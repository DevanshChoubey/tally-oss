//! The assistant (spec 6.8). It proposes; the engine computes; a person confirms. It never writes
//! to the store and never supplies a derived number (rule A10).

pub mod cloud;
pub mod nl_entry;
pub mod ocr;
mod proposal;
pub mod redact;
mod seam;
pub mod validate;

pub use proposal::{
    DraftItem, DraftLine, MappingSuggestion, MatchSuggestion, Proposal, ReportQuery, SourceRef,
    Transcribed, VoucherDraft,
};
pub use seam::{AssistError, ProposalInput, Proposer};
