//! The assistant (spec 6.8). It proposes; the engine computes; a person confirms. It never writes
//! to the store and never supplies a derived number (rule A10).

pub mod cloud;
pub mod nl_entry;
pub mod ocr;
pub mod ocr_models;
mod proposal;
pub mod provider;
pub mod redact;
pub mod review;
mod seam;
pub mod validate;

pub use proposal::{
    BankLineQuestion, DraftItem, DraftLine, MappingSuggestion, MatchSuggestion, OpenVoucher,
    Proposal, REPORT_NAMES, ReportQuery, SourceRef, Transcribed, UNSUPPORTED_REPORT, VoucherDraft,
};
pub use seam::{AssistError, ProposalInput, Proposer};
