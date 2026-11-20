//! Balance sheet. **Card RP3** implements this module.
//!
//! Frozen interface (plan card RP3):
//!
//! ```text
//! pub fn balance_sheet(books: &Books, params: &ReportParams, closing_stock: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
