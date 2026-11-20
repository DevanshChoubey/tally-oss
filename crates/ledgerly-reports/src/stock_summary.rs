//! Stock summary. **Card RP9** implements this module.
//!
//! Frozen interface (plan card RP9):
//!
//! ```text
//! pub fn stock_summary(books: &Books, params: &ReportParams, valuations: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
