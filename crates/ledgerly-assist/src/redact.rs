//! Redaction before any cloud call. **Card A4** implements this module.
//!
//! Frozen interface (plan card A4):
//!
//! ```text
//! pub fn redact(text: &str) -> (String, Vec<Redaction>);   // PAN, GSTIN, account numbers, IFSC
//! ```
