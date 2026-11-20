//! Audit chain verification and daily checkpoints (card S3, spec 6.6).
//!
//! Frozen interface:
//!
//! ```text
//! pub struct VerifyReport { .. }   // defined by S3: pass/fail, first broken seq, gaps, checkpoints
//! pub fn verify_chain(store: &SqliteStore) -> Result<VerifyReport, StoreError>;
//! pub fn write_checkpoint(store: &mut SqliteStore, day: Date, meta: &AuditMeta) -> Result<(), StoreError>;
//! ```
//!
//! # What the verifier checks
//!
//! [`verify_chain`] reads every row of `events` in sequence order, straight from SQLite (not
//! through [`crate::Store::events`], which refuses to decode a damaged row), and reports each
//! defect as a [`ChainProblem`] naming the sequence number. It never repairs anything. For each
//! row it checks that:
//!
//! 1. the sequence number is the one expected (1-based, gapless): a jump is a [`ChainProblem::Gap`];
//! 2. `prev_hash` equals the stored `hash` of the row before (32 zero bytes for the first);
//! 3. `meta` and `event` decode, and re-encode to exactly the stored bytes (so a byte flip that
//!    still decodes to the same value is caught);
//! 4. `hash` equals [`crate::commit::record_hash`] recomputed from the decoded row.
//!
//! Then every row of `checkpoints` is compared with the event it names.
//!
//! # Limits
//!
//! A hash chain alone cannot see rows removed from its tail, nor a wholesale rewrite that
//! recomputes every hash. Checkpoints narrow both: a checkpoint written earlier names a sequence
//! number and hash, so [`ChainProblem::CheckpointEventMissing`] and
//! [`ChainProblem::CheckpointMismatch`] expose a tail that was cut or rewritten after it. Copies of
//! the checkpoints kept outside the company file (for example in a backup or an auditor's notes)
//! make that check stronger.
//!
//! # Checkpoint days
//!
//! A record belongs to the calendar day of its `meta.at` in the offset it was recorded with (the
//! user's own day, spec 6.6). The checkpoint of day `D` is the chain head at the end of `D`: the
//! last record before the first record dated after `D`. Dates in the log need not rise
//! monotonically (clocks move); the prefix rule keeps the choice deterministic.

use ledgerly_core::{Date, EventSeq};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::commit::{GENESIS_HASH, record_hash};
use crate::{AuditMeta, Event, SqliteStore, StoreError};

/// One defect found by [`verify_chain`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainProblem {
    /// Records `missing_from..=missing_to` are absent from the log.
    Gap {
        /// First missing sequence number.
        missing_from: EventSeq,
        /// Last missing sequence number.
        missing_to: EventSeq,
    },
    /// The record's `prev_hash` is not the stored hash of the record before it.
    PrevHashMismatch {
        /// The record.
        seq: EventSeq,
    },
    /// The record's stored `hash` is not the hash of its content.
    HashMismatch {
        /// The record.
        seq: EventSeq,
    },
    /// The record's `meta` or `event` bytes cannot be decoded.
    Undecodable {
        /// The record.
        seq: EventSeq,
        /// Which column and why.
        detail: String,
    },
    /// The record's `meta` or `event` bytes decode but are not the canonical encoding of what
    /// they decode to, so the stored bytes were altered.
    NonCanonical {
        /// The record.
        seq: EventSeq,
    },
    /// A hash column does not hold exactly 32 bytes.
    MalformedHash {
        /// The record.
        seq: EventSeq,
    },
    /// A checkpoint names an event that is not in the log (the tail was cut, or rows deleted).
    CheckpointEventMissing {
        /// Checkpoint day as stored (`YYYY-MM-DD`).
        day: String,
        /// Sequence number the checkpoint names.
        seq: EventSeq,
    },
    /// A checkpoint's hash differs from the hash of the event it names.
    CheckpointMismatch {
        /// Checkpoint day as stored (`YYYY-MM-DD`).
        day: String,
        /// Sequence number the checkpoint names.
        seq: EventSeq,
    },
}

impl ChainProblem {
    /// The sequence number the problem is anchored at. For a gap, the first missing number.
    #[must_use]
    pub fn seq(&self) -> EventSeq {
        match self {
            ChainProblem::Gap { missing_from, .. } => *missing_from,
            ChainProblem::PrevHashMismatch { seq }
            | ChainProblem::HashMismatch { seq }
            | ChainProblem::Undecodable { seq, .. }
            | ChainProblem::NonCanonical { seq }
            | ChainProblem::MalformedHash { seq }
            | ChainProblem::CheckpointEventMissing { seq, .. }
            | ChainProblem::CheckpointMismatch { seq, .. } => *seq,
        }
    }

    /// Like [`ChainProblem::seq`], but `None` for problems found in the checkpoints table
    /// rather than in the log itself.
    #[must_use]
    pub fn chain_seq(&self) -> Option<EventSeq> {
        match self {
            ChainProblem::Gap { missing_from, .. } => Some(*missing_from),
            ChainProblem::PrevHashMismatch { seq }
            | ChainProblem::HashMismatch { seq }
            | ChainProblem::Undecodable { seq, .. }
            | ChainProblem::NonCanonical { seq }
            | ChainProblem::MalformedHash { seq } => Some(*seq),
            ChainProblem::CheckpointEventMissing { .. }
            | ChainProblem::CheckpointMismatch { .. } => None,
        }
    }

    /// One line describing the problem, for the integrity certificate.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            ChainProblem::Gap {
                missing_from,
                missing_to,
            } => format!(
                "events {} to {} are missing from the log",
                missing_from.0, missing_to.0
            ),
            ChainProblem::PrevHashMismatch { seq } => format!(
                "event {} does not follow the event before it (prev_hash differs)",
                seq.0
            ),
            ChainProblem::HashMismatch { seq } => format!(
                "event {} does not match its stored hash (content or hash altered)",
                seq.0
            ),
            ChainProblem::Undecodable { seq, detail } => {
                format!("event {} cannot be read: {detail}", seq.0)
            }
            ChainProblem::NonCanonical { seq } => format!(
                "event {} holds bytes that are not the canonical encoding of its content",
                seq.0
            ),
            ChainProblem::MalformedHash { seq } => {
                format!("event {} has a hash column that is not 32 bytes", seq.0)
            }
            ChainProblem::CheckpointEventMissing { day, seq } => format!(
                "checkpoint {day} names event {}, which is not in the log",
                seq.0
            ),
            ChainProblem::CheckpointMismatch { day, seq } => format!(
                "checkpoint {day} does not match the hash of event {}",
                seq.0
            ),
        }
    }
}

/// A stored checkpoint: the chain head at the end of `day`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Day, `YYYY-MM-DD`, as stored.
    pub day: String,
    /// Sequence number of the head at the end of the day.
    pub seq: EventSeq,
    /// Hash of that head record.
    pub hash: [u8; 32],
}

/// Result of [`verify_chain`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Number of rows of the log that were read.
    pub events_checked: u64,
    /// Sequence number and hash of the last row read, or `None` for an empty log.
    pub head: Option<(EventSeq, [u8; 32])>,
    /// Every defect found: chain problems in sequence order, then checkpoint problems in day
    /// order. Empty when the log verifies.
    pub problems: Vec<ChainProblem>,
    /// The checkpoints found in the file, in day order.
    pub checkpoints: Vec<Checkpoint>,
}

impl VerifyReport {
    /// True when no problem was found.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }

    /// The lowest sequence number any problem is anchored at, or `None` when the log verifies.
    #[must_use]
    pub fn first_broken_seq(&self) -> Option<EventSeq> {
        self.problems.iter().map(ChainProblem::seq).min()
    }

    /// Ranges `(from, to)` (inclusive) of sequence numbers missing from the log.
    #[must_use]
    pub fn gaps(&self) -> Vec<(EventSeq, EventSeq)> {
        self.problems
            .iter()
            .filter_map(|p| match p {
                ChainProblem::Gap {
                    missing_from,
                    missing_to,
                } => Some((*missing_from, *missing_to)),
                ChainProblem::PrevHashMismatch { .. }
                | ChainProblem::HashMismatch { .. }
                | ChainProblem::Undecodable { .. }
                | ChainProblem::NonCanonical { .. }
                | ChainProblem::MalformedHash { .. }
                | ChainProblem::CheckpointEventMissing { .. }
                | ChainProblem::CheckpointMismatch { .. } => None,
            })
            .collect()
    }

    /// Plain-text integrity certificate: PASS or FAIL, the counts, the head hash and each
    /// problem. Deterministic (no clock), so the same file always prints the same certificate.
    #[must_use]
    pub fn certificate(&self) -> String {
        let mut lines = vec![
            if self.is_ok() {
                "Audit chain integrity: PASS".to_owned()
            } else {
                "Audit chain integrity: FAIL".to_owned()
            },
            format!("Events checked: {}", self.events_checked),
            match &self.head {
                Some((seq, hash)) => format!("Head: event {} hash {}", seq.0, hex::encode(hash)),
                None => "Head: none (the log is empty)".to_owned(),
            },
            format!("Checkpoints: {}", self.checkpoints.len()),
        ];
        lines.extend(
            self.problems
                .iter()
                .map(|p| format!("Problem: {}", p.describe())),
        );
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

/// A row of `events` before decoding.
struct RawRow {
    seq: i64,
    prev_hash: Vec<u8>,
    hash: Vec<u8>,
    meta: Vec<u8>,
    event: Vec<u8>,
}

/// What the scan carries from one row to the next.
struct ScanState {
    expected_seq: u64,
    prev_row_hash: [u8; 32],
    head: Option<(EventSeq, [u8; 32])>,
    checked: u64,
}

/// Verifies the whole audit log and the checkpoints of `store`. See the module documentation.
///
/// A damaged log is not an error: it comes back as a report with [`ChainProblem`]s.
///
/// # Errors
/// [`StoreError::Sqlite`] when the file cannot be read, [`StoreError::Encoding`] when a hash
/// cannot be recomputed or a sequence number is negative.
pub fn verify_chain(store: &SqliteStore) -> Result<VerifyReport, StoreError> {
    let mut problems = Vec::new();
    let mut state = ScanState {
        expected_seq: 1,
        prev_row_hash: GENESIS_HASH,
        head: None,
        checked: 0,
    };
    {
        let mut stmt = store
            .conn
            .prepare("SELECT seq, prev_hash, hash, meta, event FROM events ORDER BY seq ASC")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let raw = RawRow {
                seq: row.get(0)?,
                prev_hash: row.get(1)?,
                hash: row.get(2)?,
                meta: row.get(3)?,
                event: row.get(4)?,
            };
            check_row(&raw, &mut state, &mut problems)?;
        }
    }
    let checkpoints = read_checkpoints(store)?;
    check_checkpoints(store, &checkpoints, &mut problems)?;
    tracing::debug!(
        events = state.checked,
        problems = problems.len(),
        "audit chain verified"
    );
    Ok(VerifyReport {
        events_checked: state.checked,
        head: state.head,
        problems,
        checkpoints,
    })
}

/// Checks one row against the scan state and records what is wrong with it.
fn check_row(
    raw: &RawRow,
    state: &mut ScanState,
    problems: &mut Vec<ChainProblem>,
) -> Result<(), StoreError> {
    let seq_u = u64::try_from(raw.seq)
        .map_err(|_| StoreError::Encoding(format!("negative event sequence {}", raw.seq)))?;
    let seq = EventSeq(seq_u);
    state.checked = state.checked.saturating_add(1);
    if seq_u > state.expected_seq {
        problems.push(ChainProblem::Gap {
            missing_from: EventSeq(state.expected_seq),
            missing_to: EventSeq(seq_u.saturating_sub(1)),
        });
    }
    state.expected_seq = seq_u.saturating_add(1);

    let prev = to_hash(&raw.prev_hash);
    let hash = to_hash(&raw.hash);
    if prev.is_none() || hash.is_none() {
        problems.push(ChainProblem::MalformedHash { seq });
    }
    if let Some(prev) = &prev
        && *prev != state.prev_row_hash
    {
        problems.push(ChainProblem::PrevHashMismatch { seq });
    }
    if let (Some(prev), Some(hash)) = (&prev, &hash)
        && let Some(problem) = check_content(seq, prev, hash, raw)?
    {
        problems.push(problem);
    }
    // The next row is judged against what this row says its hash is.
    state.prev_row_hash = hash.unwrap_or(GENESIS_HASH);
    state.head = hash.map(|h| (seq, h));
    Ok(())
}

/// Decodes the row, checks the bytes are canonical, and recomputes the hash. Returns the first
/// problem found.
fn check_content(
    seq: EventSeq,
    prev: &[u8; 32],
    hash: &[u8; 32],
    raw: &RawRow,
) -> Result<Option<ChainProblem>, StoreError> {
    let meta: AuditMeta = match decode(&raw.meta) {
        Ok(m) => m,
        Err(detail) => {
            return Ok(Some(ChainProblem::Undecodable {
                seq,
                detail: format!("audit metadata: {detail}"),
            }));
        }
    };
    let event: Event = match decode(&raw.event) {
        Ok(e) => e,
        Err(detail) => {
            return Ok(Some(ChainProblem::Undecodable {
                seq,
                detail: format!("event: {detail}"),
            }));
        }
    };
    if !is_canonical(&meta, &raw.meta) || !is_canonical(&event, &raw.event) {
        return Ok(Some(ChainProblem::NonCanonical { seq }));
    }
    if record_hash(seq, prev, &meta, &event)? != *hash {
        return Ok(Some(ChainProblem::HashMismatch { seq }));
    }
    Ok(None)
}

/// True when `value` encodes to exactly `stored`.
fn is_canonical<T: Serialize>(value: &T, stored: &[u8]) -> bool {
    encode(value).is_ok_and(|bytes| bytes == stored)
}

fn to_hash(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    ciborium::from_reader(bytes).map_err(|e| e.to_string())
}

fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn read_checkpoints(store: &SqliteStore) -> Result<Vec<Checkpoint>, StoreError> {
    let mut stmt = store
        .conn
        .prepare("SELECT day, seq, hash FROM checkpoints ORDER BY day ASC")?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let day: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        let hash: Vec<u8> = row.get(2)?;
        let seq = u64::try_from(seq)
            .map_err(|_| StoreError::Encoding(format!("negative checkpoint sequence {seq}")))?;
        let hash = to_hash(&hash).ok_or_else(|| {
            StoreError::Encoding(format!("checkpoint {day} has a hash that is not 32 bytes"))
        })?;
        out.push(Checkpoint {
            day,
            seq: EventSeq(seq),
            hash,
        });
    }
    Ok(out)
}

/// Compares each checkpoint with the event row it names.
fn check_checkpoints(
    store: &SqliteStore,
    checkpoints: &[Checkpoint],
    problems: &mut Vec<ChainProblem>,
) -> Result<(), StoreError> {
    let mut stmt = store
        .conn
        .prepare("SELECT hash FROM events WHERE seq = ?1")?;
    for cp in checkpoints {
        let seq_sql = i64::try_from(cp.seq.0)
            .map_err(|_| StoreError::Encoding("checkpoint sequence overflow".to_owned()))?;
        let mut rows = stmt.query([seq_sql])?;
        match rows.next()? {
            None => problems.push(ChainProblem::CheckpointEventMissing {
                day: cp.day.clone(),
                seq: cp.seq,
            }),
            Some(row) => {
                let stored: Vec<u8> = row.get(0)?;
                if to_hash(&stored) != Some(cp.hash) {
                    problems.push(ChainProblem::CheckpointMismatch {
                        day: cp.day.clone(),
                        seq: cp.seq,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Writes (or advances) the checkpoint for `day`: the chain head at the end of that day, as
/// defined in the module documentation.
///
/// The chain is verified first; a log with a broken chain gets no new checkpoint. When no record
/// is dated on or before `day`, there is nothing to attest and no row is written. Calling again
/// for the same day after more records dated that day were appended moves the day's checkpoint
/// forward; it never moves it back. `meta` is accepted for interface stability (checkpoints are
/// not audit events and carry no actor) and is not stored.
///
/// # Errors
/// [`StoreError::ChainBroken`] naming the first bad sequence number when the chain does not
/// verify; [`StoreError::Sqlite`] or [`StoreError::Encoding`] on a read or write failure.
pub fn write_checkpoint(
    store: &mut SqliteStore,
    day: Date,
    _meta: &AuditMeta,
) -> Result<(), StoreError> {
    let report = verify_chain(store)?;
    let first_chain_problem = report
        .problems
        .iter()
        .filter_map(ChainProblem::chain_seq)
        .min();
    if let Some(seq) = first_chain_problem {
        return Err(StoreError::ChainBroken(seq));
    }
    let Some((seq, hash)) = head_at_end_of(store, day)? else {
        return Ok(());
    };
    let seq_sql = i64::try_from(seq.0)
        .map_err(|_| StoreError::Encoding("event sequence number overflow".to_owned()))?;
    store.conn.execute(
        "INSERT INTO checkpoints(day, seq, hash) VALUES (?1, ?2, ?3) \
         ON CONFLICT(day) DO UPDATE SET seq = excluded.seq, hash = excluded.hash \
         WHERE excluded.seq >= checkpoints.seq",
        rusqlite::params![day_text(day), seq_sql, hash],
    )?;
    tracing::debug!(seq = seq.0, "checkpoint written");
    Ok(())
}

/// The last record before the first record dated after `day` (in the record's own offset).
fn head_at_end_of(
    store: &SqliteStore,
    day: Date,
) -> Result<Option<(EventSeq, [u8; 32])>, StoreError> {
    let mut stmt = store
        .conn
        .prepare("SELECT seq, hash, meta FROM events ORDER BY seq ASC")?;
    let mut rows = stmt.query([])?;
    let mut head = None;
    while let Some(row) = rows.next()? {
        let seq: i64 = row.get(0)?;
        let hash: Vec<u8> = row.get(1)?;
        let meta_bytes: Vec<u8> = row.get(2)?;
        let meta: AuditMeta = decode(&meta_bytes)
            .map_err(|e| StoreError::Encoding(format!("audit metadata of event {seq}: {e}")))?;
        if meta.at.date() > day {
            break;
        }
        let seq = u64::try_from(seq)
            .map_err(|_| StoreError::Encoding(format!("negative event sequence {seq}")))?;
        let hash = to_hash(&hash)
            .ok_or_else(|| StoreError::Encoding(format!("event {seq} hash is not 32 bytes")))?;
        head = Some((EventSeq(seq), hash));
    }
    Ok(head)
}

/// `YYYY-MM-DD`, the day format of the `checkpoints` table.
fn day_text(day: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}
