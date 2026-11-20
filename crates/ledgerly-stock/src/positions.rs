//! Stock positions by item, godown and batch. **Card T2** implements this module.
//!
//! Frozen interface (plan card T2):
//!
//! ```text
//! pub struct Position { pub item: StockItemId, pub godown: GodownId, pub batch: Option<BatchId>, pub qty: Qty }
//! pub fn positions(books: &Books, as_of: Date) -> Result<Vec<Position>, StockError>;
//! ```
