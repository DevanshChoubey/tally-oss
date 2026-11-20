//! Voucher tax computation and rounding. **Card G4** implements this module.
//!
//! Frozen interface (plan card G4):
//!
//! ```text
//! pub struct GstBreakup { .. }   // defined by G4: per-line taxable value, rate, CGST/SGST/UTGST/IGST/cess, round-off
//! pub fn compute_voucher_tax(v: &Voucher, masters: &Masters, rules: &RuleSet, company: &Company) -> Result<Outcome<GstBreakup>, GstError>;
//! ```
