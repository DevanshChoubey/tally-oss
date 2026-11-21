use std::path::PathBuf;

use ledgerly_core::{EventSeq, Period};
use ledgerly_model::Books;

use crate::{AuditMeta, AuditRecord, CommitReceipt, Event, Store, StoreError};

/// A company stored in one SQLite file (WAL mode). Created and opened by
/// `SqliteStore::create`/`SqliteStore::open` in [`crate::schema`] (card S1).
#[derive(Debug)]
pub struct SqliteStore {
    /// The open connection.
    #[allow(dead_code, reason = "read by cards S1-S6; Wave 0 only declares it")]
    pub(crate) conn: rusqlite::Connection,
    /// Path of the company file.
    pub(crate) path: PathBuf,
}

impl SqliteStore {
    /// Path of the company file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Switches on maintenance of the `masters`, `vouchers` and `postings` projection tables
    /// (card C6). The tables are first built from the whole event log, then every later
    /// [`Store::commit`] updates them in the same transaction as its events, so
    /// [`crate::backup::rebuild_projections_and_compare`] reports a match.
    ///
    /// Returns `true` when this call switched maintenance on and `false` when it already was.
    /// Files are never switched on by opening them: a file written before this existed keeps
    /// empty tables and the rebuild check reports them as not maintained. The call refuses
    /// (without changing anything) a file whose projection tables already hold rows, and a file
    /// whose log cannot be replayed.
    ///
    /// # Errors
    /// [`StoreError::Encoding`] as above, [`StoreError::Sqlite`] on a write failure.
    pub fn enable_projections(&self) -> Result<bool, StoreError> {
        crate::commit::enable_projections(self)
    }

    /// Whether `commit` maintains the projection tables of this file.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] on a read failure.
    pub fn projections_enabled(&self) -> Result<bool, StoreError> {
        crate::commit::projections_enabled(&self.conn)
    }
}

impl Store for SqliteStore {
    fn commit(&mut self, events: &[Event], meta: &AuditMeta) -> Result<CommitReceipt, StoreError> {
        crate::commit::commit_events(self, events, meta)
    }

    fn load_books(&self, period: Option<Period>) -> Result<Books, StoreError> {
        crate::load::load_books(self, period)
    }

    fn events(&self, from: EventSeq, limit: usize) -> Result<Vec<AuditRecord>, StoreError> {
        crate::commit::read_events(self, from, limit)
    }
}
