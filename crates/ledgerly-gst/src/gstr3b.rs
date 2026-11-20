//! GSTR-3B working (books vs portal). **Card G10** implements this module.
//!
//! Frozen interface (plan card G10):
//!
//! ```text
//! pub struct Gstr3bWorking { .. }   // defined by G10
//! pub fn build_gstr3b(books: &Books, gstin: &Gstin, period: Period, pack: &RulePack, portal: Option<&Gstr3bPortalValues>) -> Result<Outcome<Gstr3bWorking>, GstError>;
//! ```
