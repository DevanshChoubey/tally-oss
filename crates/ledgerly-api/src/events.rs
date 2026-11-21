//! Reads the audit log for the facade (card W1): the period lock and the audit reports.

use ledgerly_core::{Date, EventSeq};
use ledgerly_store::{AuditRecord, Event, SqliteStore, Store, StoreError};

/// Records requested per page, so the read stays bounded by one page at a time.
const PAGE: usize = 1000;

/// Every audit record of the company, in sequence order.
///
/// # Errors
/// [`StoreError`] on a read failure, or when a stored sequence number cannot be followed.
pub(crate) fn read_all(store: &SqliteStore) -> Result<Vec<AuditRecord>, StoreError> {
    let mut out = Vec::new();
    let mut from = EventSeq(1);
    loop {
        let page = store.events(from, PAGE)?;
        let count = page.len();
        let Some(last) = page.last() else {
            break;
        };
        let next = last
            .seq
            .0
            .checked_add(1)
            .ok_or_else(|| StoreError::Encoding("event sequence number overflow".to_owned()))?;
        out.extend(page);
        if count < PAGE {
            break;
        }
        from = EventSeq(next);
    }
    Ok(out)
}

/// The lock date in force: the last `PeriodLocked` event in sequence order (spec section 5,
/// invariant 8). `None` when the books were never locked.
pub(crate) fn lock_date(records: &[AuditRecord]) -> Option<Date> {
    records.iter().rev().find_map(|record| {
        if let Event::PeriodLocked { until } = &record.event {
            Some(*until)
        } else {
            None
        }
    })
}
