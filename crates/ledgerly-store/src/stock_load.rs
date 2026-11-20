//! Stock movement projection. **Card S5** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn stock_moves(store: &SqliteStore, period: Option<Period>) -> Result<Vec<StockMove>, StoreError>;
//! ```
