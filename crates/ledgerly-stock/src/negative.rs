//! Negative stock detection and policy. **Card T5** implements this module.
//!
//! Frozen interface (plan card T5):
//!
//! ```text
//! pub fn first_negative(item: StockItemId, moves: &[StockMove]) -> Option<Date>;
//! pub fn check_policy(v: &Voucher, books: &Books, policy: NegativeStockPolicy) -> Result<Vec<StockWarning>, StockError>;
//! ```
