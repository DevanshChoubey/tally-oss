//! Reading bills and invoices. **Card A3** implements this module.
//!
//! Frozen interface (plan card A3):
//!
//! ```text
//! pub fn ocr_to_draft(ocr: &OcrOutput, masters: &Masters) -> Result<VoucherDraft, AssistError>;
//! ```
