//! e-Way bill JSON. **Card F5** implements this module.
//!
//! Frozen interface (plan card F5):
//!
//! ```text
//! pub struct EwayBill { .. }
//! pub fn ewb_to_json(e: &EwayBill) -> Result<String, FormatError>;
//! pub fn validate_ewb(e: &EwayBill) -> Result<(), Vec<FormatError>>;
//! ```
