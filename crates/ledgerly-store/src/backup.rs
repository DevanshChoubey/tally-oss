//! Backup, restore and projection rebuild check. **Card S6** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub struct RebuildReport { .. }   // defined by S6
//! pub fn backup_to(store: &SqliteStore, dir: &Path, clock: Clock) -> Result<PathBuf, StoreError>;
//! pub fn restore_from(backup: &Path, target: &Path) -> Result<SqliteStore, StoreError>;
//! pub fn rebuild_projections_and_compare(store: &SqliteStore) -> Result<RebuildReport, StoreError>;
//! ```
