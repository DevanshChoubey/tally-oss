//! MT940 statements. **Card BF4** implements this module.
//!
//! Frozen interface (plan card BF4):
//!
//! ```text
//! pub fn parse_mt940(text: &str) -> Result<RawStatement, BankFmtError>;
//! ```
