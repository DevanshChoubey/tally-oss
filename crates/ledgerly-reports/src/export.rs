//! Report exports. **Card RP8** implements this module.
//!
//! Frozen interface (plan card RP8):
//!
//! ```text
//! pub fn to_csv(report: &Report) -> Result<String, ReportError>;
//! pub fn to_xlsx(report: &Report, path: &Path) -> Result<(), ReportError>;   // atomic write (rule D3)
//! pub fn to_json(report: &Report) -> Result<String, ReportError>;
//! ```
