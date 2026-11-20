//! Last purchase cost and standard cost valuation; dispatch by method. **Card T5** implements this module.
//!
//! Frozen interface (plan card T5):
//!
//! ```text
//! pub fn last_purchase_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! pub fn standard_cost(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! pub fn value_item(item: &StockItem, moves: &[StockMove], period: Period) -> Result<StockValuation, StockError>;
//! ```
