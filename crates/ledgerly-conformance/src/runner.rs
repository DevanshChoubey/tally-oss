//! Runner and ratchet. **Card Q1** implements this module.
//!
//! Frozen interface (plan card Q1):
//!
//! ```text
//! pub fn run_all(cases_dir: &Path) -> Result<RunReport, ConformanceError>;
//! pub fn check_ratchet(report: &RunReport, pass_list: &str) -> Result<(), Vec<String>>;
//! ```
