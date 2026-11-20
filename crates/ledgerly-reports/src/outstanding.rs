//! Bills outstanding and ageing. **Card RP7** implements this module.
//!
//! Frozen interface (plan card RP7):
//!
//! ```text
//! pub fn outstanding(books: &Books, params: &ReportParams, receivable: bool) -> Result<Outcome<Report>, ReportError>;
//! ```
