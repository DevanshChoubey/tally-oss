//! Audit chain verification and daily checkpoints. **Card S3** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub struct VerifyReport { .. }   // defined by S3: pass/fail, first broken seq, gaps, checkpoints
//! pub fn verify_chain(store: &SqliteStore) -> Result<VerifyReport, StoreError>;
//! pub fn write_checkpoint(store: &mut SqliteStore, day: Date, meta: &AuditMeta) -> Result<(), StoreError>;
//! ```
