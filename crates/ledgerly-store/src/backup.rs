//! Backup, restore and projection rebuild check (**card S6**, spec 6.6).
//!
//! Frozen interface:
//!
//! ```text
//! pub struct RebuildReport { .. }   // defined by S6
//! pub fn backup_to(store: &SqliteStore, dir: &Path, clock: Clock) -> Result<PathBuf, StoreError>;
//! pub fn restore_from(backup: &Path, target: &Path) -> Result<SqliteStore, StoreError>;
//! pub fn rebuild_projections_and_compare(store: &SqliteStore) -> Result<RebuildReport, StoreError>;
//! ```
//!
//! # Backup
//!
//! [`backup_to`] writes a consistent single-file copy with SQLite's `VACUUM INTO`, which reads one
//! snapshot of the database (including the WAL), so a backup taken while the company is open is a
//! complete, self-contained file. The copy is written under a `.partial` name, verified (SQLite's
//! `integrity_check`, same application id, same newest event sequence number and hash as the
//! source, and the audit chain of card S3 via [`crate::verify::verify_chain`]) and only then
//! renamed to its final name, so a failed backup never leaves something that looks like a good
//! one. An existing file is never overwritten. The file name is
//! `<company file stem>-backup-<UTC time as YYYYMMDDTHHMMSSZ>.ledgerly`, taken from the injected
//! [`Clock`] (rule C5).
//!
//! [`restore_from`] does not repeat the audit-chain check; it compares the newest event of the
//! backup (WAL included) with the restored file, so events left in a backup's `-wal` file are never
//! silently lost. Run `verify_chain` on a restored store to re-check its chain.
//!
//! # Rebuild and compare
//!
//! The event log is the source of truth. [`rebuild_projections_and_compare`] replays it (through
//! [`crate::load::load_books`]) and compares the result with the `masters`, `vouchers` and
//! `postings` projection tables. As of wave 1 nothing writes those tables (see the module
//! documentation of [`crate::load`]), so a store with events and empty projection tables reports
//! [`RebuildOutcome::NotMaintained`] rather than a match or a mismatch: nobody can say the tables
//! are right, and calling them wrong would blame a writer that does not exist.
//!
//! What is compared:
//! - `masters`: the decoded `snapshot` of every row against the rebuilt masters, keyed by master
//!   kind and id. The text in the `kind` column is not interpreted (its spelling is not frozen);
//!   the `id` column must equal the id inside the snapshot.
//! - `vouchers`: the decoded `snapshot` against the rebuilt voucher; the `id` and `date` columns
//!   must equal the snapshot's id and ISO 8601 date.
//! - `postings`: per voucher, the ordered list of `(date, ledger, paise, side)` rows against the
//!   rebuilt postings of that voucher in line order. `line_no` is used for ordering only.
//!
//! `stock_moves` is not compared: its rebuild belongs to card S5, so the report lists it in
//! [`RebuildReport::not_compared`] instead of claiming a result.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use ledgerly_core::{Clock, DrCr};
use ledgerly_model::{Books, MasterSnapshot, Voucher};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use time::UtcOffset;

use crate::{SqliteStore, StoreError};

/// Extension of company and backup files.
const EXTENSION: &str = "ledgerly";

/// Suffix of a backup that is still being written or verified.
const PARTIAL_SUFFIX: &str = ".partial";

/// Result of [`rebuild_projections_and_compare`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildReport {
    /// Number of events in the log that were replayed.
    pub events_replayed: u64,
    /// Row counts of the projections rebuilt from the log.
    pub rebuilt: ProjectionCounts,
    /// Row counts of the stored projection tables.
    pub stored: ProjectionCounts,
    /// What the comparison found.
    pub outcome: RebuildOutcome,
    /// Projection tables that were not compared, with the reason.
    pub not_compared: Vec<NotCompared>,
}

impl RebuildReport {
    /// `true` only when the stored projections were compared and equal the rebuild.
    pub fn is_match(&self) -> bool {
        match &self.outcome {
            RebuildOutcome::Match => true,
            RebuildOutcome::NotMaintained | RebuildOutcome::Mismatch(_) => false,
        }
    }
}

/// Row counts of the compared projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProjectionCounts {
    /// Rows of `masters`.
    pub masters: usize,
    /// Rows of `vouchers`.
    pub vouchers: usize,
    /// Rows of `postings`.
    pub postings: usize,
}

impl ProjectionCounts {
    fn is_empty(self) -> bool {
        self.masters == 0 && self.vouchers == 0 && self.postings == 0
    }
}

/// Outcome of comparing stored projections with a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildOutcome {
    /// The stored projections equal the rebuild (including: both are empty).
    Match,
    /// The log produced rows but every stored projection table is empty: nothing maintains the
    /// tables yet, so there is nothing to compare. This is not a match.
    NotMaintained,
    /// The stored projections differ from the rebuild.
    Mismatch(Vec<Difference>),
}

/// One difference between a stored projection and the rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Projection table: `masters`, `vouchers` or `postings`.
    pub table: &'static str,
    /// Key of the row or group of rows, for example `voucher 4`.
    pub key: String,
    /// What differs.
    pub detail: String,
}

/// A projection table that the comparison did not cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCompared {
    /// Table name.
    pub table: &'static str,
    /// Why it was skipped.
    pub reason: &'static str,
}

/// Writes a verified, self-contained copy of the company file into `dir` and returns its path.
///
/// `dir` is created when missing. The file name carries the time of `clock` (see the module
/// documentation). An existing file of that name is never overwritten.
///
/// # Errors
/// [`StoreError::Io`] when `dir` or the file cannot be created, or when the target name exists
/// (kind `AlreadyExists`); [`StoreError::Sqlite`] when SQLite fails; [`StoreError::Encoding`]
/// when the copy does not verify (the message names the failed check); [`StoreError::ChainBroken`]
/// naming the first bad sequence number when the audit chain or a checkpoint of the copy does not
/// verify. Nothing but possibly a
/// `.partial` file is left behind on error, and that is removed on a best-effort basis.
pub fn backup_to(store: &SqliteStore, dir: &Path, clock: Clock) -> Result<PathBuf, StoreError> {
    std::fs::create_dir_all(dir).map_err(|source| io_error(dir, source))?;
    let final_path = dir.join(backup_file_name(store.path(), clock));
    if final_path.exists() {
        return Err(io_error(
            &final_path,
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
        ));
    }
    let partial = with_suffix(&final_path, PARTIAL_SUFFIX);
    remove_if_present(&partial)?;
    let written = write_and_verify(store, &partial);
    if let Err(e) = written {
        if let Err(cleanup) = remove_if_present(&partial) {
            tracing::warn!(error = %cleanup, "could not remove partial backup");
        }
        return Err(e);
    }
    // `rename` replaces silently on Unix, so the existence check above is repeated right before
    // it; a writer racing between the two calls is outside what a local company file allows.
    if final_path.exists() {
        remove_if_present(&partial)?;
        return Err(io_error(
            &final_path,
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
        ));
    }
    std::fs::rename(&partial, &final_path).map_err(|source| io_error(&final_path, source))?;
    tracing::info!(path = %final_path.display(), "backup written");
    Ok(final_path)
}

/// Restores `backup` to a new company file at `target` and opens it.
///
/// The backup is checked first (SQLite `integrity_check` and a company-file identity check), then
/// copied to `target`, which must not exist, and opened with [`SqliteStore::open`]. On any failure
/// `target` is removed again and the backup is left untouched.
///
/// # Errors
/// [`StoreError::Io`] when `target` exists (kind `AlreadyExists`) or a file cannot be read or
/// written; [`StoreError::NotACompanyFile`] and [`StoreError::SchemaTooNew`] as for
/// [`SqliteStore::open`]; [`StoreError::Encoding`] when the backup fails `integrity_check`, or
/// when the restored file's newest event differs from the backup's (for example because events
/// were only in the backup's `-wal` file, which is not copied); [`StoreError::Sqlite`] when
/// `backup` is not a SQLite database at all.
pub fn restore_from(backup: &Path, target: &Path) -> Result<SqliteStore, StoreError> {
    if !backup.is_file() {
        return Err(io_error(
            backup,
            std::io::Error::from(std::io::ErrorKind::NotFound),
        ));
    }
    check_integrity(backup)?;
    // Reserve the target atomically; fails when it exists, so no other file is touched.
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|source| io_error(target, source))?,
    );
    let restored = copy_and_open(backup, target);
    if restored.is_err() {
        remove_company_files(target);
    }
    restored
}

/// Replays the event log and compares the result with the stored projection tables.
///
/// See the module documentation for what is compared. Only differences that are found are
/// reported; the function never repairs a table.
///
/// # Errors
/// [`StoreError`] when the log cannot be replayed (see [`crate::load::load_books`]) or a stored
/// projection row cannot be read or decoded.
pub fn rebuild_projections_and_compare(store: &SqliteStore) -> Result<RebuildReport, StoreError> {
    let books = crate::load::load_books(store, None)?;
    let events_replayed = count_rows(&store.conn, "SELECT count(*) FROM events")?;
    let events_replayed = u64::try_from(events_replayed)
        .map_err(|_| StoreError::Encoding("negative event count".to_owned()))?;

    let rebuilt_masters = rebuilt_masters(&books);
    let rebuilt_postings = rebuilt_postings(&books);
    let rebuilt = ProjectionCounts {
        masters: rebuilt_masters.len(),
        vouchers: books.vouchers.len(),
        postings: books.postings.len(),
    };

    let mut differences = Vec::new();
    let stored_masters = stored_masters(&store.conn, &mut differences)?;
    let stored_vouchers = stored_vouchers(&store.conn, &mut differences)?;
    let (stored_postings, stored_posting_rows) = stored_postings(&store.conn)?;
    let stored = ProjectionCounts {
        masters: table_rows(&store.conn, "masters")?,
        vouchers: table_rows(&store.conn, "vouchers")?,
        postings: stored_posting_rows,
    };

    let outcome = if stored.is_empty() && !rebuilt.is_empty() {
        RebuildOutcome::NotMaintained
    } else {
        diff_maps(
            "masters",
            &rebuilt_masters,
            &stored_masters,
            &mut differences,
        );
        diff_maps(
            "vouchers",
            &books.vouchers,
            &stored_vouchers,
            &mut differences,
        );
        diff_maps(
            "postings",
            &rebuilt_postings,
            &stored_postings,
            &mut differences,
        );
        if differences.is_empty() {
            RebuildOutcome::Match
        } else {
            RebuildOutcome::Mismatch(differences)
        }
    };
    Ok(RebuildReport {
        events_replayed,
        rebuilt,
        stored,
        outcome,
        not_compared: vec![NotCompared {
            table: "stock_moves",
            reason: "the stock rebuild belongs to card S5 and is not compared here",
        }],
    })
}

// ---------------------------------------------------------------------------------------------
// Backup helpers
// ---------------------------------------------------------------------------------------------

/// `<stem>-backup-<YYYYMMDDTHHMMSSZ>.ledgerly`, the time in UTC.
fn backup_file_name(company_file: &Path, clock: Clock) -> String {
    let stem = company_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("company");
    let at = clock.now().to_offset(UtcOffset::UTC);
    format!(
        "{stem}-backup-{:04}{:02}{:02}T{:02}{:02}{:02}Z.{EXTENSION}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

fn write_and_verify(store: &SqliteStore, partial: &Path) -> Result<(), StoreError> {
    let partial_str = partial.to_str().ok_or_else(|| {
        StoreError::Encoding(format!(
            "backup path {} is not valid UTF-8",
            partial.display()
        ))
    })?;
    // VACUUM INTO reads one consistent snapshot, WAL included, and writes a fresh file.
    store.conn.execute("VACUUM INTO ?1", [partial_str])?;
    check_integrity(partial)?;
    let source_head = chain_head(&store.conn)?;
    let source_app_id = application_id(&store.conn)?;
    let copy = open_read_only(partial)?;
    let copy_head = chain_head(&copy)?;
    if copy_head != source_head {
        return Err(StoreError::Encoding(format!(
            "backup verification failed: newest event is {copy_head:?} in the copy but {source_head:?} in the company file"
        )));
    }
    let copy_app_id = application_id(&copy)?;
    if copy_app_id != source_app_id {
        return Err(StoreError::Encoding(
            "backup verification failed: application id differs from the company file".to_owned(),
        ));
    }
    // The head comparison above cannot see an altered stored event (its hash column is intact),
    // so the copy also goes through the audit-chain verifier of card S3.
    let verifier = SqliteStore {
        conn: copy,
        path: partial.to_path_buf(),
    };
    let report = crate::verify::verify_chain(&verifier)?;
    if let Some(seq) = report.first_broken_seq() {
        return Err(StoreError::ChainBroken(seq));
    }
    Ok(())
}

/// Runs SQLite's `integrity_check` on the file at `path` (opened read-only).
fn check_integrity(path: &Path) -> Result<(), StoreError> {
    let conn = open_read_only(path)?;
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut problems = Vec::new();
    for row in rows {
        let line = row?;
        if line != "ok" {
            problems.push(line);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(StoreError::Encoding(format!(
            "{} failed the SQLite integrity check: {}",
            path.display(),
            problems.join("; ")
        )))
    }
}

fn open_read_only(path: &Path) -> Result<Connection, StoreError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(f, _) = &e
            && f.code == rusqlite::ErrorCode::NotADatabase
        {
            return StoreError::NotACompanyFile(path.display().to_string());
        }
        StoreError::Sqlite(e)
    })
}

/// Sequence number and hash of the newest event, `None` for an empty log.
fn chain_head(conn: &Connection) -> Result<Option<(i64, [u8; 32])>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT seq, hash FROM events ORDER BY seq DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn application_id(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.pragma_query_value(None, "application_id", |r| r.get(0))?)
}

fn copy_and_open(backup: &Path, target: &Path) -> Result<SqliteStore, StoreError> {
    // Read before copying: the backup's newest event, WAL included. `fs::copy` copies only the
    // main database file, so events still in a `-wal` file next to the backup are not in `target`.
    let source = open_read_only(backup)?;
    // A file that is not a company file has no events table; say so before reading it.
    crate::schema::read_version(&source, backup)?;
    let source_head = chain_head(&source)?;
    drop(source);
    std::fs::copy(backup, target).map_err(|source| io_error(target, source))?;
    let restored = SqliteStore::open(target)?;
    let restored_head = chain_head(&restored.conn)?;
    if restored_head != source_head {
        return Err(StoreError::Encoding(format!(
            "restore verification failed: the newest event is {restored_head:?} in the restored \
             file but {source_head:?} in the backup; events only in the backup's WAL file were \
             not copied"
        )));
    }
    Ok(restored)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_if_present(path: &Path) -> Result<(), StoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path, source)),
    }
}

/// Best-effort removal of a half-restored company file and its WAL side files.
fn remove_company_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let candidate = with_suffix(path, suffix);
        if let Err(e) = remove_if_present(&candidate) {
            tracing::warn!(error = %e, "could not remove partial restore");
        }
    }
}

fn io_error(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.display().to_string(),
        source,
    }
}

// ---------------------------------------------------------------------------------------------
// Rebuild helpers
// ---------------------------------------------------------------------------------------------

/// Key of a master: kind name and raw id.
type MasterKey = (&'static str, u64);

/// One posting row as compared: `(date, ledger, paise, side)`.
type PostingRow = (String, u64, i64, &'static str);

fn master_key(snapshot: &MasterSnapshot) -> MasterKey {
    match snapshot {
        MasterSnapshot::Group(v) => ("group", v.id.0),
        MasterSnapshot::Ledger(v) => ("ledger", v.id.0),
        MasterSnapshot::VoucherType(v) => ("voucher type", v.id.0),
        MasterSnapshot::Unit(v) => ("unit", v.id.0),
        MasterSnapshot::StockGroup(v) => ("stock group", v.id.0),
        MasterSnapshot::StockItem(v) => ("stock item", v.id.0),
        MasterSnapshot::Godown(v) => ("godown", v.id.0),
        MasterSnapshot::Batch(v) => ("batch", v.id.0),
        MasterSnapshot::CostCategory(v) => ("cost category", v.id.0),
        MasterSnapshot::CostCentre(v) => ("cost centre", v.id.0),
    }
}

fn rebuilt_masters(books: &Books) -> BTreeMap<MasterKey, MasterSnapshot> {
    let m = &books.masters;
    let mut out = BTreeMap::new();
    let mut put = |snapshot: MasterSnapshot| {
        out.insert(master_key(&snapshot), snapshot);
    };
    m.groups
        .values()
        .for_each(|v| put(MasterSnapshot::Group(v.clone())));
    m.ledgers
        .values()
        .for_each(|v| put(MasterSnapshot::Ledger(v.clone())));
    m.voucher_types
        .values()
        .for_each(|v| put(MasterSnapshot::VoucherType(v.clone())));
    m.units
        .values()
        .for_each(|v| put(MasterSnapshot::Unit(v.clone())));
    m.stock_groups
        .values()
        .for_each(|v| put(MasterSnapshot::StockGroup(v.clone())));
    m.stock_items
        .values()
        .for_each(|v| put(MasterSnapshot::StockItem(v.clone())));
    m.godowns
        .values()
        .for_each(|v| put(MasterSnapshot::Godown(v.clone())));
    m.batches
        .values()
        .for_each(|v| put(MasterSnapshot::Batch(v.clone())));
    m.cost_categories
        .values()
        .for_each(|v| put(MasterSnapshot::CostCategory(v.clone())));
    m.cost_centres
        .values()
        .for_each(|v| put(MasterSnapshot::CostCentre(v.clone())));
    out
}

fn side_text(side: DrCr) -> &'static str {
    match side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    }
}

/// Rebuilt postings grouped by voucher id, each group in line order (`Books::postings` keeps the
/// lines of one voucher in line order).
fn rebuilt_postings(books: &Books) -> BTreeMap<u64, Vec<PostingRow>> {
    let mut out: BTreeMap<u64, Vec<PostingRow>> = BTreeMap::new();
    for p in &books.postings {
        out.entry(p.voucher.0).or_default().push((
            p.date.to_string(),
            p.ledger.0,
            p.amount.amount.get(),
            side_text(p.amount.side),
        ));
    }
    out
}

fn count_rows(conn: &Connection, sql: &str) -> Result<i64, StoreError> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

fn table_rows(conn: &Connection, table: &'static str) -> Result<usize, StoreError> {
    // `table` is one of our own constant names, never user input.
    let n = count_rows(conn, &format!("SELECT count(*) FROM {table}"))?;
    usize::try_from(n).map_err(|_| StoreError::Encoding(format!("negative row count in {table}")))
}

fn to_u64(value: i64, what: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Encoding(format!("negative {what} {value}")))
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], what: &str) -> Result<T, StoreError> {
    ciborium::from_reader(bytes).map_err(|e| StoreError::Encoding(format!("{what}: {e}")))
}

fn stored_masters(
    conn: &Connection,
    differences: &mut Vec<Difference>,
) -> Result<BTreeMap<MasterKey, MasterSnapshot>, StoreError> {
    let mut stmt = conn.prepare("SELECT kind, id, snapshot FROM masters ORDER BY kind, id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (kind, id, blob) = row?;
        let id = to_u64(id, "master id")?;
        let snapshot: MasterSnapshot = decode(&blob, "stored master snapshot")?;
        let key = master_key(&snapshot);
        if key.1 != id {
            differences.push(Difference {
                table: "masters",
                key: format!("{kind} {id}"),
                detail: format!("id column is {id} but the snapshot holds {}", key.1),
            });
        }
        if out.insert(key, snapshot).is_some() {
            differences.push(Difference {
                table: "masters",
                key: format!("{} {}", key.0, key.1),
                detail: "more than one stored row holds this master".to_owned(),
            });
        }
    }
    Ok(out)
}

fn stored_vouchers(
    conn: &Connection,
    differences: &mut Vec<Difference>,
) -> Result<BTreeMap<ledgerly_core::VoucherId, Voucher>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, date, snapshot FROM vouchers ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, date, blob) = row?;
        let id = to_u64(id, "voucher id")?;
        let voucher: Voucher = decode(&blob, "stored voucher snapshot")?;
        if voucher.id.0 != id {
            differences.push(Difference {
                table: "vouchers",
                key: format!("voucher {id}"),
                detail: format!("id column is {id} but the snapshot holds {}", voucher.id.0),
            });
        }
        if voucher.date.to_string() != date {
            differences.push(Difference {
                table: "vouchers",
                key: format!("voucher {id}"),
                detail: format!(
                    "date column is {date} but the snapshot holds {}",
                    voucher.date
                ),
            });
        }
        out.insert(voucher.id, voucher);
    }
    Ok(out)
}

/// Stored postings grouped like [`rebuilt_postings`], and the total number of rows.
fn stored_postings(
    conn: &Connection,
) -> Result<(BTreeMap<u64, Vec<PostingRow>>, usize), StoreError> {
    let mut stmt = conn.prepare(
        "SELECT voucher_id, date, ledger_id, paise, side FROM postings \
         ORDER BY voucher_id, line_no",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut out: BTreeMap<u64, Vec<PostingRow>> = BTreeMap::new();
    let mut total = 0_usize;
    for row in rows {
        let (voucher, date, ledger, paise, side) = row?;
        let side = match side.as_str() {
            "Dr" => "Dr",
            "Cr" => "Cr",
            other => {
                return Err(StoreError::Encoding(format!(
                    "stored posting of voucher {voucher} has side {other:?}"
                )));
            }
        };
        out.entry(to_u64(voucher, "posting voucher id")?)
            .or_default()
            .push((date, to_u64(ledger, "posting ledger id")?, paise, side));
        total = total.saturating_add(1);
    }
    Ok((out, total))
}

/// Appends one [`Difference`] per key that is missing, extra or different.
fn diff_maps<K: Ord + Debug, V: PartialEq + Debug>(
    table: &'static str,
    rebuilt: &BTreeMap<K, V>,
    stored: &BTreeMap<K, V>,
    out: &mut Vec<Difference>,
) {
    for (key, want) in rebuilt {
        match stored.get(key) {
            None => out.push(Difference {
                table,
                key: format!("{key:?}"),
                detail: "present in the rebuild, missing from the stored projection".to_owned(),
            }),
            Some(have) if have != want => out.push(Difference {
                table,
                key: format!("{key:?}"),
                detail: "stored projection differs from the rebuild".to_owned(),
            }),
            Some(_) => {}
        }
    }
    for key in stored.keys() {
        if !rebuilt.contains_key(key) {
            out.push(Difference {
                table,
                key: format!("{key:?}"),
                detail: "present in the stored projection, absent from the rebuild".to_owned(),
            });
        }
    }
}
