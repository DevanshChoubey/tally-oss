//! Drill-down from a report row. **Card RP1** implements this module.
//!
//! Frozen interface (plan card RP1):
//!
//! ```text
//! pub fn drill_down(report: &Report, row: usize) -> Option<ReportParams>;
//! ```
