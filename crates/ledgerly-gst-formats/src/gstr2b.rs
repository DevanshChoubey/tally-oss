//! GSTR-2B reader. **Card F3** implements this module.
//!
//! Frozen interface (plan card F3):
//!
//! ```text
//! pub struct Gstr2b { .. }   // supplier invoices, credit notes; unknown fields in `extra`
//! pub fn parse_gstr2b(json: &str) -> Result<Gstr2b, FormatError>;
//! ```
