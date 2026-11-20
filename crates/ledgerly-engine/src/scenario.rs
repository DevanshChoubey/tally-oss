//! Which postings a scenario includes. **Card E6** implements this module.
//!
//! Frozen interface (plan card E6):
//!
//! ```text
//! pub fn effective_postings<'a>(books: &'a Books, scenario: &Scenario, as_of: Date) -> Vec<&'a Posting>;
//! ```
