//! Reverse charge. **Card G5** implements this module.
//!
//! Frozen interface (plan card G5):
//!
//! ```text
//! pub fn reverse_charge(v: &Voucher, masters: &Masters, rules: &RuleSet) -> Result<Outcome<RcmLiability>, GstError>;   // RcmLiability defined by G5
//! ```
