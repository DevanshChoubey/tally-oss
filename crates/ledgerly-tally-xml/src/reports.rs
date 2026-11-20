//! Tally report exports. **Card X5** implements this module.
//!
//! Frozen interface (plan card X5):
//!
//! ```text
//! pub struct TallyReports { pub tb: Vec<TReportRow>, pub bs: Vec<TReportRow>, pub pl: Vec<TReportRow>, pub stock: Vec<TReportRow>, pub outstanding: Vec<TReportRow> }
//! pub fn report_request(company: &str, report: TallyReportKind, from: Date, to: Date) -> String;
//! pub fn parse_report(env: &Envelope, kind: TallyReportKind) -> Result<Vec<TReportRow>, TallyXmlError>;
//! ```
