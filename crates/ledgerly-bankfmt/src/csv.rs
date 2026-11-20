//! CSV statements. **Card BF2** implements this module.
//!
//! Frozen interface (plan card BF2):
//!
//! ```text
//! pub fn parse_csv(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError>;
//! ```
