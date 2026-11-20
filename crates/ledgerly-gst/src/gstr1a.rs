//! GSTR-1A amendments. **Card G9** implements this module.
//!
//! Frozen interface (plan card G9):
//!
//! ```text
//! pub fn build_gstr1a(books: &Books, gstin: &Gstin, period: Period, filed: &Gstr1, pack: &RulePack) -> Result<Outcome<Gstr1a>, GstError>;
//! ```
