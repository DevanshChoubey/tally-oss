//! Period locks. **Card E5** implements this module.
//!
//! Frozen interface (plan card E5):
//!
//! ```text
//! pub fn lock_period(until: Date, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn lock_date(books: &Books) -> Option<Date>;   // derived from PeriodLocked events, passed in Books by W1
//! pub fn check_unlocked(date: Date, lock: Option<Date>) -> Result<(), EngineError>;
//! ```
