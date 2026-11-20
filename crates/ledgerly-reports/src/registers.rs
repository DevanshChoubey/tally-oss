//! Day book and registers. **Card RP6** implements this module.
//!
//! Frozen interface (plan card RP6):
//!
//! ```text
//! pub fn day_book(books: &Books, params: &ReportParams) -> Result<Outcome<Report>, ReportError>;
//! pub fn register(books: &Books, params: &ReportParams, kind: BaseVoucherKind) -> Result<Outcome<Report>, ReportError>;
//! ```
