//! XLSX statements. **Card BF3** implements this module.
//!
//! Frozen interface (plan card BF3):
//!
//! ```text
//! pub fn parse_xlsx(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError>;
//! ```
