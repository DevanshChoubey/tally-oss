//! GSTR-2B / IMS reconciliation. **Card G11** implements this module.
//!
//! Frozen interface (plan card G11):
//!
//! ```text
//! pub enum MatchLevel { Exact, Normalised, Tolerance, Unmatched }
//! pub enum ImsAction { Accept, Reject, Pending }
//! pub struct ReconLine { pub portal_index: usize, pub voucher: Option<VoucherId>, pub level: MatchLevel, pub suggested: ImsAction, pub confirmed: bool }
//! pub fn match_2b(books: &Books, gstin: &Gstin, data: &Gstr2b, tolerance: Paise) -> Result<Vec<ReconLine>, GstError>;
//! ```
