//! Feature dependency rules. **Card M3** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub enum FeatureError { .. }                       // defined by M3 in this file
//! impl Features {
//!     pub fn validate(&self, company: &Company) -> Result<(), FeatureError>;
//!     pub fn requires(flag: Flag) -> Vec<Flag>;
//! }
//! ```
