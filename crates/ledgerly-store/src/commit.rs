//! Event append with the hash chain, and reading events back (card S2, spec 6.6).
//!
//! Chain definition. Record `n` (1-based, gapless) stores `prev_hash` (the hash of record `n - 1`,
//! 32 zero bytes for record 1) and
//!
//! ```text
//! hash = SHA-256( canonical CBOR of the 4-tuple (seq: u64, prev_hash: [u8; 32], meta, event) )
//! ```
//!
//! The CBOR comes from `ciborium` over the serde derives of [`AuditMeta`] and [`Event`]: struct
//! fields in declaration order, enums externally tagged, no maps with unordered keys, so the same
//! values always give the same bytes (rule A7). `meta` and `event` are stored in their own columns
//! with the same encoding, so a verifier can decode them, re-encode the tuple with
//! [`record_hash`] and compare.

use ledgerly_core::EventSeq;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::{AuditMeta, AuditRecord, CommitReceipt, Event, SqliteStore, StoreError};

/// `prev_hash` of the first record: 32 zero bytes.
pub const GENESIS_HASH: [u8; 32] = [0; 32];

/// Appends events in one transaction. See [`crate::Store::commit`].
///
/// Every event of the batch shares `meta`. The write lock is taken before the chain head is read,
/// so two writers can never assign the same sequence number. An empty batch writes nothing and
/// returns a receipt with no sequence numbers and the current head hash ([`GENESIS_HASH`] for an
/// empty log).
///
/// # Errors
/// [`StoreError`]; nothing is written on error.
pub fn commit_events(
    store: &mut SqliteStore,
    events: &[Event],
    meta: &AuditMeta,
) -> Result<CommitReceipt, StoreError> {
    let meta_blob = encode(meta, "audit metadata")?;
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (mut last_seq, mut last_hash) = read_head(&tx)?;
    let mut seqs = Vec::with_capacity(events.len());
    {
        let mut insert = tx.prepare_cached(
            "INSERT INTO events(seq, prev_hash, hash, meta, event) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for event in events {
            let seq = last_seq
                .checked_add(1)
                .ok_or_else(|| StoreError::Encoding("event sequence number overflow".to_owned()))?;
            let seq_id = EventSeq(seq);
            let hash = record_hash(seq_id, &last_hash, meta, event)?;
            insert.execute(rusqlite::params![
                seq_to_sql(seq)?,
                last_hash,
                hash,
                meta_blob,
                encode(event, "event")?
            ])?;
            seqs.push(seq_id);
            last_seq = seq;
            last_hash = hash;
        }
    }
    tx.commit()?;
    tracing::debug!(count = seqs.len(), head = last_seq, "events committed");
    Ok(CommitReceipt { seqs, last_hash })
}

/// Reads audit records. See [`crate::Store::events`].
///
/// Returns up to `limit` records with `seq >= from`, in ascending sequence order.
///
/// # Errors
/// [`StoreError`] on a read failure ([`StoreError::Encoding`] when a stored record cannot be
/// decoded).
pub fn read_events(
    store: &SqliteStore,
    from: EventSeq,
    limit: usize,
) -> Result<Vec<AuditRecord>, StoreError> {
    // No stored sequence number exceeds i64::MAX, so a larger `from` matches nothing.
    let Ok(from_sql) = i64::try_from(from.0) else {
        return Ok(Vec::new());
    };
    let limit_sql = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = store.conn.prepare_cached(
        "SELECT seq, prev_hash, hash, meta, event FROM events \
         WHERE seq >= ?1 ORDER BY seq ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![from_sql, limit_sql], |r| {
        Ok(RawRecord {
            seq: r.get(0)?,
            prev_hash: r.get(1)?,
            hash: r.get(2)?,
            meta: r.get(3)?,
            event: r.get(4)?,
        })
    })?;
    let mut records = Vec::new();
    for row in rows {
        records.push(decode_record(&row?)?);
    }
    Ok(records)
}

/// The hash of record `seq`: SHA-256 over the canonical CBOR of `(seq, prev_hash, meta, event)`.
///
/// Public so the chain verifier (card S3) and external auditors recompute with the same bytes.
///
/// # Errors
/// [`StoreError::Encoding`] when the values cannot be encoded.
pub fn record_hash(
    seq: EventSeq,
    prev_hash: &[u8; 32],
    meta: &AuditMeta,
    event: &Event,
) -> Result<[u8; 32], StoreError> {
    let bytes = encode(&(seq.0, prev_hash, meta, event), "audit record")?;
    Ok(Sha256::digest(&bytes).into())
}

/// A row of `events` before decoding.
struct RawRecord {
    seq: i64,
    prev_hash: [u8; 32],
    hash: [u8; 32],
    meta: Vec<u8>,
    event: Vec<u8>,
}

fn decode_record(raw: &RawRecord) -> Result<AuditRecord, StoreError> {
    let seq = u64::try_from(raw.seq)
        .map_err(|_| StoreError::Encoding(format!("negative event sequence {}", raw.seq)))?;
    Ok(AuditRecord {
        seq: EventSeq(seq),
        prev_hash: raw.prev_hash,
        hash: raw.hash,
        meta: decode(&raw.meta, "audit metadata")?,
        event: decode(&raw.event, "event")?,
    })
}

/// Sequence number and hash of the newest record, or `(0, GENESIS_HASH)` for an empty log.
fn read_head(conn: &Connection) -> Result<(u64, [u8; 32]), StoreError> {
    let row: Option<(i64, [u8; 32])> = conn
        .query_row(
            "SELECT seq, hash FROM events ORDER BY seq DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        None => Ok((0, GENESIS_HASH)),
        Some((seq, hash)) => {
            let seq = u64::try_from(seq)
                .map_err(|_| StoreError::Encoding(format!("negative event sequence {seq}")))?;
            Ok((seq, hash))
        }
    }
}

fn seq_to_sql(seq: u64) -> Result<i64, StoreError> {
    i64::try_from(seq)
        .map_err(|_| StoreError::Encoding("event sequence number overflow".to_owned()))
}

fn encode<T: Serialize + ?Sized>(value: &T, what: &str) -> Result<Vec<u8>, StoreError> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out)
        .map_err(|e| StoreError::Encoding(format!("{what}: {e}")))?;
    Ok(out)
}

fn decode<T: DeserializeOwned>(bytes: &[u8], what: &str) -> Result<T, StoreError> {
    ciborium::from_reader(bytes).map_err(|e| StoreError::Encoding(format!("{what}: {e}")))
}
