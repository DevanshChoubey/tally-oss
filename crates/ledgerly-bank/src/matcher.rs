//! Matching statement lines to vouchers. **Card K2** implements this module.
//!
//! Frozen interface (plan card K2):
//!
//! ```text
//! pub struct MatchRule { .. }   pub struct Candidate { .. }
//! pub fn suggest_matches(stmt: &BankStatement, books: &Books, rules: &[MatchRule]) -> Vec<Candidate>;   // never confirmed
//! ```
