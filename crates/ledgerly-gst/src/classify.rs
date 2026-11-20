//! Supply classification. **Card G6** implements this module.
//!
//! Frozen interface (plan card G6):
//!
//! ```text
//! pub enum SupplyKind { Taxable, Exempt, NilRated, NonGst, ExportWithPayment, ExportUnderLut, SezWithPayment, SezUnderLut, DeemedExport, Advance }
//! pub fn classify_line(v: &Voucher, line: usize, masters: &Masters, rules: &RuleSet) -> Result<Outcome<SupplyKind>, GstError>;
//! ```
