//! Average cost (periodic) valuation. **Card T3** implements this module.
//!
//! Frozen interface (plan card T3):
//!
//! ```text
//! pub fn average_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
