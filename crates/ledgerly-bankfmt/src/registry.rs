//! Format registry. **Card BF1** implements this module.
//!
//! Frozen interface (plan card BF1):
//!
//! ```text
//! pub struct FormatSpec { .. }   pub struct Registry { .. }
//! pub fn load_registry(dir_contents: &[(String, String)]) -> Result<Registry, BankFmtError>;   // (file name, TOML)
//! ```
