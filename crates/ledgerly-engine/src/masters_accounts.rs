//! Group and ledger commands. **Card E1** implements this module.
//!
//! Frozen interface (plan card E1):
//!
//! ```text
//! pub fn create_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_group(id: GroupId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn create_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_ledger(id: LedgerId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
