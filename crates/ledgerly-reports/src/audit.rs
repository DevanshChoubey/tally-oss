//! Auditor reports (headless). **Card RP10** implements this module.
//!
//! Frozen interface (plan card RP10):
//!
//! ```text
//! pub fn audit_report(records: &[AuditRecord], books: &Books, kind: &AuditReportKind, params: &ReportParams) -> Result<Outcome<Report>, ReportError>;
//! pub fn integrity_certificate(verify: &VerifyReport) -> Report;
//! ```
