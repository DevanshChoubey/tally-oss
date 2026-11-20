//! FIFO (annual) valuation. **Card T4** implements this module.
//!
//! Frozen interface (plan card T4):
//!
//! ```text
//! pub fn fifo(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
