//! Batched pull with raw capture. **Card B2** implements this module.
//!
//! Frozen interface (plan card B2):
//!
//! ```text
//! pub struct PullPlan { .. }
//! pub fn pull(t: &dyn TallyTransport, company: &str, range: Period, capture_dir: &Path, cancel: &CancelToken) -> Result<PullResult, BridgeError>;
//! ```
