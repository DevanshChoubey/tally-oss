//! Event append with the hash chain, and reading events back. **Card S2** implements this module
//! (replacing both bodies below; they return [`StoreError::NotBuilt`] until then).

use ledgerly_core::EventSeq;

use crate::{AuditMeta, AuditRecord, CommitReceipt, Event, SqliteStore, StoreError};

/// Appends events in one transaction. See [`crate::Store::commit`].
///
/// # Errors
/// [`StoreError`]; nothing is written on error.
pub fn commit_events(
    store: &mut SqliteStore,
    events: &[Event],
    meta: &AuditMeta,
) -> Result<CommitReceipt, StoreError> {
    let _ = (store, events, meta);
    Err(StoreError::NotBuilt("S2"))
}

/// Reads audit records. See [`crate::Store::events`].
///
/// # Errors
/// [`StoreError`] on a read failure.
pub fn read_events(
    store: &SqliteStore,
    from: EventSeq,
    limit: usize,
) -> Result<Vec<AuditRecord>, StoreError> {
    let _ = (store, from, limit);
    Err(StoreError::NotBuilt("S2"))
}
