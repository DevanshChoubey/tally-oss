//! Schema creation and migrations. **Card S1** implements this module and
//! `migrations/0001_init.sql`.
//!
//! Frozen interface:
//!
//! ```text
//! impl SqliteStore {
//!     pub fn create(path: &Path, company: &Company, clock: Clock) -> Result<SqliteStore, StoreError>;
//!     pub fn open(path: &Path) -> Result<SqliteStore, StoreError>;
//! }
//! pub const SCHEMA_VERSION: u32;
//! ```
