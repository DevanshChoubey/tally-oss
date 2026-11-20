//! IMS reader and action file writer. **Card F3** implements this module.
//!
//! Frozen interface (plan card F3):
//!
//! ```text
//! pub struct Ims { .. }
//! pub fn parse_ims(json: &str) -> Result<Ims, FormatError>;
//! pub fn ims_actions_to_json(actions: &[ImsActionRecord]) -> Result<String, FormatError>;
//! ```
