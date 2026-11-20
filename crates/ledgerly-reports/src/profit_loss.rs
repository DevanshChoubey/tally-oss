//! Profit and loss in trading form. **Card RP4** implements this module.
//!
//! Frozen interface (plan card RP4):
//!
//! ```text
//! pub fn profit_loss(books: &Books, params: &ReportParams, opening_stock: &Outcome<Vec<StockValuation>>, closing_stock: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
