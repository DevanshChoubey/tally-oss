//! Validating a draft by recomputing through the engine and GST crates. **Card A1** implements this module.
//!
//! Frozen interface (plan card A1):
//!
//! ```text
//! pub enum DraftCheck { Ok(Voucher), Blocked { field: String, reason: String } }
//! pub fn validate_draft(d: &VoucherDraft, books: &Books, rules: &RuleSet, threshold_permille: u16) -> Result<DraftCheck, AssistError>;
//! ```
