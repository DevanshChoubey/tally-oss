//! Period locks. **Card E5** implements this module.
//!
//! Frozen interface (plan card E5):
//!
//! ```text
//! pub fn lock_period(until: Date, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn lock_date(books: &Books) -> Option<Date>;   // derived from PeriodLocked events, passed in Books by W1
//! pub fn check_unlocked(date: Date, lock: Option<Date>) -> Result<(), EngineError>;
//! ```
//!
//! `lock_date` is deliberately not provided: [`Books`] carries no lock date, so any value computed
//! here would be a guess (an unlocked answer for a locked book is a wrong result). See the
//! interface request for E5. Callers pass the lock date they hold to [`check_unlocked`].

use ledgerly_core::{Date, Outcome};
use ledgerly_model::Books;
use ledgerly_store::Event;

use crate::EngineError;

/// Lock the books up to and including `until` (spec section 5, invariant 8).
///
/// Returns one [`Event::PeriodLocked`]. The audit reason and actor travel in the store's audit
/// metadata, which the commit path adds. Moving an existing lock earlier or later is allowed here;
/// it is the same audited event.
///
/// # Errors
/// [`EngineError::Invalid`] when `until` is before the company's `books_begin`, because no voucher
/// can exist there (invariant 6).
pub fn lock_period(until: Date, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if until < books.company.books_begin {
        return Err(EngineError::Invalid {
            field: "lock date".to_owned(),
            reason: format!(
                "{until} is before the books begin on {}; choose a date on or after it",
                books.company.books_begin
            ),
        });
    }
    Ok(Outcome::Value(vec![Event::PeriodLocked { until }]))
}

/// Check that a create, alter or delete dated `date` is allowed under `lock`.
///
/// A date on or before the lock date is blocked; with no lock nothing is.
///
/// # Errors
/// [`EngineError::PeriodLocked`] carrying the lock date.
pub fn check_unlocked(date: Date, lock: Option<Date>) -> Result<(), EngineError> {
    match lock {
        Some(until) if date <= until => Err(EngineError::PeriodLocked { until }),
        Some(_) | None => Ok(()),
    }
}
