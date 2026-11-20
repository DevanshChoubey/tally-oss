//! Template format. **Card PR1** implements this module.
//!
//! Frozen interface (plan card PR1):
//!
//! ```text
//! pub struct Template { .. }
//! pub fn load_template(toml: &str) -> Result<Template, PrintError>;   // PrintError defined by PR1 here
//! ```
