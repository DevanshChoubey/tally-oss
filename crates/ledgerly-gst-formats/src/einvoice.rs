//! e-Invoice INV-01 JSON and local validation. **Card F4** implements this module.
//!
//! Frozen interface (plan card F4):
//!
//! ```text
//! pub struct EInvoice { .. }
//! pub fn einvoice_to_json(e: &EInvoice) -> Result<String, FormatError>;
//! pub fn validate_einvoice(e: &EInvoice) -> Result<(), Vec<FormatError>>;
//! ```
