//! GSTR-1 offline tool Excel writer. **Card F2** implements this module.
//!
//! Frozen interface (plan card F2):
//!
//! ```text
//! pub fn gstr1_to_xlsx(r: &Gstr1, path: &Path) -> Result<(), FormatError>;
//! ```
