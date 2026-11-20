//! Column expansion for report parameters. **Card RP1** implements this module.
//!
//! Frozen interface (plan card RP1):
//!
//! ```text
//! pub fn expand_columns(params: &ReportParams, fy: FinancialYear) -> Result<Vec<ColumnHeader>, ReportError>;
//! ```
