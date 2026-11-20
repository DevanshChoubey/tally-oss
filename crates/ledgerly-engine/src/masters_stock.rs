//! Inventory and cost master commands. **Card E2** implements this module.
//!
//! Frozen interface (plan card E2):
//!
//! ```text
//! Same create/alter/delete verbs as E1 for Unit, StockGroup, StockItem, Godown, Batch,
//! CostCategory and CostCentre, for example:
//! pub fn create_unit(u: Unit, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_godown(id: GodownId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
