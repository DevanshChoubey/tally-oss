//! Input tax credit eligibility, including s.17(5). **Card G7** implements this module.
//!
//! Frozen interface (plan card G7):
//!
//! ```text
//! pub enum ItcEligibility { Eligible, Blocked { section: String }, Ineligible { reason: String } }
//! pub fn itc_eligibility(v: &Voucher, line: usize, masters: &Masters, rules: &RuleSet) -> Result<Outcome<ItcEligibility>, GstError>;
//! ```
