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
