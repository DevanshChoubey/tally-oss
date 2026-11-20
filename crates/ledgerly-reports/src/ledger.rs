//! Group summaries and ledger vouchers. **Card RP5** implements this module.
//!
//! Frozen interface (plan card RP5):
//!
//! ```text
//! pub fn group_summary(books: &Books, params: &ReportParams, group: GroupId) -> Result<Outcome<Report>, ReportError>;
//! pub fn ledger_vouchers(books: &Books, params: &ReportParams, ledger: LedgerId) -> Result<Outcome<Report>, ReportError>;
//! ```
