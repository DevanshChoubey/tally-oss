//! SQLite event store (spec 4, 6.1, 6.6).
//!
//! The append-only, hash-chained event log is the source of truth. [`Store`] is the only seam
//! between the engine and storage (rule C2); [`SqliteStore`] is its only implementation.

mod event;
mod sqlite;

pub mod backup;
pub mod commit;
pub mod load;
pub mod schema;
pub mod stock_load;
pub mod verify;

pub use event::{
    Actor, AuditMeta, AuditRecord, CommitReceipt, Event, StoredImsAction, StoredStatementLine,
};
pub use sqlite::SqliteStore;

use ledgerly_core::{EventSeq, Period, VoucherId};
use ledgerly_model::Books;
use thiserror::Error;

/// Storage for one company.
pub trait Store {
    /// Appends `events` atomically, chaining each to the previous record's hash.
    ///
    /// # Errors
    /// [`StoreError`] when nothing was written.
    fn commit(&mut self, events: &[Event], meta: &AuditMeta) -> Result<CommitReceipt, StoreError>;

    /// Loads masters, vouchers, postings and stock movements. `period` limits vouchers by date;
    /// `None` loads everything.
    ///
    /// # Errors
    /// [`StoreError`] on a read failure.
    fn load_books(&self, period: Option<Period>) -> Result<Books, StoreError>;

    /// Reads up to `limit` audit records starting at `from`.
    ///
    /// # Errors
    /// [`StoreError`] on a read failure.
    fn events(&self, from: EventSeq, limit: usize) -> Result<Vec<AuditRecord>, StoreError>;
}

/// Storage errors.
#[derive(Debug, Error)]
pub enum StoreError {
    /// SQLite failed.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// File system failed.
    #[error("file error at {path}: {source}")]
    Io {
        /// Path involved.
        path: String,
        /// Cause.
        source: std::io::Error,
    },
    /// Serialising or deserialising an event failed, or stored data does not fit the replay.
    #[error("event encoding error: {0}")]
    Encoding(String),
    /// A voucher uses a feature whose effect is not documented, so no figure is computed for it.
    /// This is valid input that Ledgerly does not handle yet, not corrupt data.
    #[error("voucher {voucher:?} is unsupported: {reason}")]
    Unsupported {
        /// The voucher that cannot be projected.
        voucher: VoucherId,
        /// What is not handled.
        reason: String,
    },
    /// The file was written by a newer version of Ledgerly.
    #[error("this company file uses schema version {found}; this build supports up to {supported}")]
    SchemaTooNew {
        /// Version in the file.
        found: u32,
        /// Newest version this build reads.
        supported: u32,
    },
    /// The file is not a Ledgerly company file.
    #[error("{0} is not a Ledgerly company file")]
    NotACompanyFile(String),
    /// The audit chain is broken at this sequence number.
    #[error("audit chain broken at event {0:?}")]
    ChainBroken(EventSeq),
    /// A part of the store is not built yet (Wave 0 placeholder; names the plan card).
    #[error("not built yet (plan card {0})")]
    NotBuilt(&'static str),
}
