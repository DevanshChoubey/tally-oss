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

use ledgerly_core::{EventSeq, VoucherId};
use ledgerly_model::{Posting, Voucher};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::backup::{master_key, side_text};
use crate::load::{Replay, postings_of};
use crate::{AuditMeta, AuditRecord, CommitReceipt, Event, SqliteStore, StoreError};

/// `prev_hash` of the first record: 32 zero bytes.
pub const GENESIS_HASH: [u8; 32] = [0; 32];

/// Key in `meta` that marks a file whose projection tables are maintained by `commit`.
const PROJECTION_FLAG: &str = "projections";

/// Events read per query while projections are built from the log.
const PAGE: usize = 1000;

/// Appends events in one transaction. See [`crate::Store::commit`].
///
/// When the file's projections are switched on ([`SqliteStore::enable_projections`], card C6),
/// the `masters`, `vouchers` and `postings` rows are updated in the same transaction, event by
/// event, with the replay rules of [`crate::load`]. An event that does not fit the stored state
/// (create of an existing id, alter of a missing one, ...) then fails the whole commit with
/// [`StoreError::Encoding`] naming the event, and nothing is written: such a file can never hold
/// a log that cannot be replayed. A file without the switch commits events only, as before.
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
    let project = projections_enabled(&tx)?;
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
            if project {
                project_event(&tx, seq, event)?;
            }
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

// ---------------------------------------------------------------------------------------------
// Projections (card C6)
// ---------------------------------------------------------------------------------------------

/// Whether the file's projection tables are maintained by `commit`.
pub(crate) fn projections_enabled(conn: &Connection) -> Result<bool, StoreError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM meta WHERE key = ?1",
            [PROJECTION_FLAG],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Builds the projection tables from the whole log and switches maintenance on. See
/// [`SqliteStore::enable_projections`].
pub(crate) fn enable_projections(store: &SqliteStore) -> Result<bool, StoreError> {
    let conn = &store.conn;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match backfill(store) {
        Ok(changed) => {
            conn.execute_batch("COMMIT")?;
            Ok(changed)
        }
        Err(e) => {
            if let Err(rollback) = conn.execute_batch("ROLLBACK") {
                tracing::warn!(error = %rollback, "could not roll back the projection build");
            }
            Err(e)
        }
    }
}

fn backfill(store: &SqliteStore) -> Result<bool, StoreError> {
    let conn = &store.conn;
    if projections_enabled(conn)? {
        return Ok(false);
    }
    for table in ["masters", "vouchers", "postings"] {
        // `table` is one of our own constant names, never user input.
        let rows: i64 =
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
        if rows != 0 {
            return Err(StoreError::Encoding(format!(
                "projection table {table} already holds rows that no writer of this build made; \
                 refusing to overwrite them"
            )));
        }
    }
    let mut from = EventSeq(1);
    loop {
        let page = read_events(store, from, PAGE)?;
        let Some(last) = page.last().map(|r| r.seq) else {
            break;
        };
        for record in &page {
            project_event(conn, record.seq.0, &record.event)?;
        }
        if page.len() < PAGE {
            break;
        }
        from =
            EventSeq(last.0.checked_add(1).ok_or_else(|| {
                StoreError::Encoding("event sequence number overflow".to_owned())
            })?);
    }
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)",
        rusqlite::params![PROJECTION_FLAG, b"maintained".as_slice()],
    )?;
    Ok(true)
}

fn projection_error(seq: u64, message: &str) -> StoreError {
    StoreError::Encoding(format!(
        "projection update failed at event {seq}: {message}"
    ))
}

fn id_sql(id: u64) -> Result<i64, StoreError> {
    i64::try_from(id).map_err(|_| StoreError::Encoding(format!("id {id} does not fit a row")))
}

/// Applies one event to the projection rows, with the same rules as the replay in [`crate::load`].
fn project_event(conn: &Connection, seq: u64, event: &Event) -> Result<(), StoreError> {
    match event {
        Event::MasterCreated(snapshot) => {
            let (kind, id) = master_key(snapshot);
            conn.execute(
                "INSERT INTO masters(kind, id, snapshot) VALUES (?1, ?2, ?3)",
                rusqlite::params![kind, id_sql(id)?, encode(snapshot, "master")?],
            )
            .map_err(|e| projection_error(seq, &format!("{kind} {id} cannot be created: {e}")))?;
            Ok(())
        }
        Event::MasterAltered { before, after } => {
            if master_key(before) != master_key(after) {
                return Err(projection_error(
                    seq,
                    &format!(
                        "master alter changes identity from {:?} to {:?}",
                        master_key(before),
                        master_key(after)
                    ),
                ));
            }
            let (kind, id) = master_key(after);
            let changed = conn.execute(
                "UPDATE masters SET snapshot = ?3 WHERE kind = ?1 AND id = ?2",
                rusqlite::params![kind, id_sql(id)?, encode(after, "master")?],
            )?;
            require_one(seq, changed, "alter", kind, id)
        }
        Event::MasterDeleted(snapshot) => {
            let (kind, id) = master_key(snapshot);
            let changed = conn.execute(
                "DELETE FROM masters WHERE kind = ?1 AND id = ?2",
                rusqlite::params![kind, id_sql(id)?],
            )?;
            require_one(seq, changed, "delete", kind, id)
        }
        Event::VoucherPosted(voucher) => project_vouchers(conn, seq, event, &[voucher.id]),
        Event::VoucherAltered { after, .. } => project_vouchers(conn, seq, event, &[after.id]),
        Event::VoucherCancelled { before } | Event::VoucherDeleted { before } => {
            project_vouchers(conn, seq, event, &[before.id])
        }
        Event::MatchConfirmed(matched) => project_vouchers(conn, seq, event, &matched.vouchers),
        Event::SettingChanged { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. }
        | Event::AiProposalRecorded(_) => Ok(()),
    }
}

fn require_one(
    seq: u64,
    changed: usize,
    what: &str,
    kind: &str,
    id: u64,
) -> Result<(), StoreError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(projection_error(
            seq,
            &format!("{what} of {kind} {id} which does not exist"),
        ))
    }
}

/// Applies a voucher event to the vouchers it names: load their rows, run the replay rule, write
/// back (or remove) each row and its postings.
fn project_vouchers(
    conn: &Connection,
    seq: u64,
    event: &Event,
    ids: &[VoucherId],
) -> Result<(), StoreError> {
    let mut replay = Replay::default();
    for id in ids {
        if let Some(voucher) = stored_voucher(conn, *id)? {
            replay.vouchers.insert(*id, voucher);
        }
    }
    replay
        .apply(event.clone())
        .map_err(|message| projection_error(seq, &message))?;
    for id in ids {
        match replay.vouchers.get(id) {
            Some(voucher) => write_voucher(conn, seq, voucher)?,
            None => {
                // The postings rows go with it (ON DELETE CASCADE).
                conn.execute("DELETE FROM vouchers WHERE id = ?1", [id_sql(id.0)?])?;
            }
        }
    }
    Ok(())
}

fn stored_voucher(conn: &Connection, id: VoucherId) -> Result<Option<Voucher>, StoreError> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT snapshot FROM vouchers WHERE id = ?1",
            [id_sql(id.0)?],
            |r| r.get(0),
        )
        .optional()?;
    blob.map(|b| decode(&b, "stored voucher snapshot"))
        .transpose()
}

fn write_voucher(conn: &Connection, seq: u64, voucher: &Voucher) -> Result<(), StoreError> {
    let id = id_sql(voucher.id.0)?;
    conn.execute(
        "INSERT INTO vouchers(id, date, status, snapshot) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(id) DO UPDATE SET date = excluded.date, status = excluded.status, \
         snapshot = excluded.snapshot",
        rusqlite::params![
            id,
            voucher.date.to_string(),
            format!("{:?}", voucher.status),
            encode(voucher, "voucher")?
        ],
    )?;
    conn.execute("DELETE FROM postings WHERE voucher_id = ?1", [id])?;
    let mut postings: Vec<Posting> = Vec::new();
    postings_of(voucher, &mut postings).map_err(|m| projection_error(seq, &m))?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO postings(voucher_id, line_no, date, ledger_id, paise, side, snapshot) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (index, posting) in postings.iter().enumerate() {
        let line_no = i64::try_from(index)
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| projection_error(seq, "too many ledger lines"))?;
        insert.execute(rusqlite::params![
            id,
            line_no,
            posting.date.to_string(),
            id_sql(posting.ledger.0)?,
            posting.amount.amount.get(),
            side_text(posting.amount.side),
            encode(posting, "posting")?
        ])?;
    }
    Ok(())
}
