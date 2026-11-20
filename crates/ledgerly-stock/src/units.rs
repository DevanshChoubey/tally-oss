//! Units and compound conversion. **Card T1** implements this module.
//!
//! Frozen interface (plan card T1):
//!
//! ```text
//! pub fn to_base_units(qty: Qty, unit: UnitId, item: &StockItem, masters: &Masters) -> Result<Qty, StockError>;
//! ```
