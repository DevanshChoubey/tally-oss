//! Reconciliation report: ours vs Tally's. **Card B4** implements this module.
//!
//! Frozen interface (plan card B4):
//!
//! ```text
//! pub struct ReconReport { .. }
//! pub fn reconcile(ours: &Books, theirs: &TallyReports, period: Period) -> Result<ReconReport, BridgeError>;
//! ```
