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

use std::path::{Path, PathBuf};

use ledgerly_core::Clock;
use ledgerly_model::Company;
use rusqlite::{Connection, ErrorCode, OpenFlags, Transaction};
use time::format_description::well_known::Rfc3339;

use crate::{SqliteStore, StoreError};

/// Newest schema version this build reads and writes. Stored in `PRAGMA user_version`.
pub const SCHEMA_VERSION: u32 = 1;

/// Value of `PRAGMA application_id` in every Ledgerly company file (ASCII "LDGR").
const APPLICATION_ID: i32 = 0x4C44_4752;

/// Migrations in order; entry `n` takes a file from version `n` to `n + 1`. Migrations only add
/// to the schema and never touch rows of `events` (spec 6.6).
const MIGRATIONS: [&str; 1] = [include_str!("../migrations/0001_init.sql")];

impl SqliteStore {
    /// Creates a new company file at `path` (WAL mode) holding `company`, at the newest schema
    /// version. The file must not exist; on any failure nothing is left behind.
    ///
    /// # Errors
    /// [`StoreError::Io`] (kind `AlreadyExists`) when `path` exists, or when the file cannot be
    /// created; [`StoreError::Sqlite`] or [`StoreError::Encoding`] when initialisation fails.
    pub fn create(path: &Path, company: &Company, clock: Clock) -> Result<SqliteStore, StoreError> {
        let created_at = clock
            .now()
            .format(&Rfc3339)
            .map_err(|e| StoreError::Encoding(format!("creation time: {e}")))?;
        let company_blob = encode_company(company)?;
        // `create_new` fails atomically when the path exists, so we never touch another file.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| io_error(path, source))?;
        match initialise(path, &company_blob, &created_at) {
            Ok(store) => Ok(store),
            Err(e) => {
                remove_company_files(path);
                Err(e)
            }
        }
    }

    /// Opens an existing company file, upgrading an older schema in place.
    ///
    /// # Errors
    /// [`StoreError::NotACompanyFile`] for a file that is not a Ledgerly company file,
    /// [`StoreError::SchemaTooNew`] when it was written by a newer build, and
    /// [`StoreError::Io`] or [`StoreError::Sqlite`] when it cannot be read.
    pub fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        if !path.is_file() {
            return Err(io_error(
                path,
                std::io::Error::from(std::io::ErrorKind::NotFound),
            ));
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| not_a_company_file_if_garbage(e, path))?;
        // Check identity before changing anything (a foreign database must not be put in WAL).
        let found = read_version(&conn, path)?;
        if found > SCHEMA_VERSION {
            return Err(StoreError::SchemaTooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        configure(&conn)?;
        if found < SCHEMA_VERSION {
            let tx = conn.transaction()?;
            apply_migrations(&tx, found)?;
            tx.commit()?;
        }
        let store = SqliteStore {
            conn,
            path: path.to_path_buf(),
        };
        // A company file without its company row is damaged; say so now, not at first read.
        store.company().map_err(|e| match e {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => {
                StoreError::NotACompanyFile(path.display().to_string())
            }
            StoreError::Sqlite(_)
            | StoreError::Io { .. }
            | StoreError::Encoding(_)
            | StoreError::SchemaTooNew { .. }
            | StoreError::NotACompanyFile(_)
            | StoreError::ChainBroken(_)
            | StoreError::NotBuilt(_) => e,
        })?;
        Ok(store)
    }

    /// The company stored in this file.
    ///
    /// # Errors
    /// [`StoreError::Encoding`] when the stored company cannot be decoded;
    /// [`StoreError::Sqlite`] on a read failure.
    pub fn company(&self) -> Result<Company, StoreError> {
        let blob: Vec<u8> =
            self.conn
                .query_row("SELECT value FROM meta WHERE key = 'company'", [], |r| {
                    r.get(0)
                })?;
        ciborium::from_reader(blob.as_slice())
            .map_err(|e| StoreError::Encoding(format!("company record: {e}")))
    }

    /// The SQLite journal mode of the open connection (`wal` for every company file).
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] on a read failure.
    pub fn journal_mode(&self) -> Result<String, StoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))?)
    }

    /// Whether foreign key enforcement is on for the open connection.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] on a read failure.
    pub fn foreign_keys_on(&self) -> Result<bool, StoreError> {
        let on: i64 = self
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
        Ok(on == 1)
    }
}

fn initialise(
    path: &Path,
    company_blob: &[u8],
    created_at: &str,
) -> Result<SqliteStore, StoreError> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    configure(&conn)?;
    let tx = conn.transaction()?;
    apply_migrations(&tx, 0)?;
    tx.execute(
        "INSERT INTO meta(key, value) VALUES ('company', ?1), ('created_at', ?2)",
        rusqlite::params![company_blob, created_at.as_bytes()],
    )?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.commit()?;
    Ok(SqliteStore {
        conn,
        path: path.to_path_buf(),
    })
}

/// Connection settings every open connection gets: WAL, enforced foreign keys, a busy timeout.
fn configure(conn: &Connection) -> Result<(), StoreError> {
    let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Encoding(format!(
            "SQLite did not enable WAL mode (reports {mode}); company files require it"
        )));
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Applies every migration from `from` to [`SCHEMA_VERSION`], then records the new version.
fn apply_migrations(tx: &Transaction<'_>, from: u32) -> Result<(), StoreError> {
    let skip = usize::try_from(from).map_err(|_| StoreError::NotACompanyFile(String::new()))?;
    for sql in MIGRATIONS.iter().skip(skip) {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Reads the schema version, rejecting files that are not Ledgerly company files.
fn read_version(conn: &Connection, path: &Path) -> Result<u32, StoreError> {
    let not_ours = || StoreError::NotACompanyFile(path.display().to_string());
    let app_id: i32 = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|e| not_a_company_file_if_garbage(e, path))?;
    if app_id != APPLICATION_ID {
        return Err(not_ours());
    }
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| not_a_company_file_if_garbage(e, path))?;
    match u32::try_from(version) {
        Ok(v) if v >= 1 => Ok(v),
        Ok(_) | Err(_) => Err(not_ours()),
    }
}

fn not_a_company_file_if_garbage(e: rusqlite::Error, path: &Path) -> StoreError {
    if let rusqlite::Error::SqliteFailure(f, _) = &e
        && f.code == ErrorCode::NotADatabase
    {
        return StoreError::NotACompanyFile(path.display().to_string());
    }
    StoreError::Sqlite(e)
}

fn encode_company(company: &Company) -> Result<Vec<u8>, StoreError> {
    let mut out = Vec::new();
    ciborium::into_writer(company, &mut out)
        .map_err(|e| StoreError::Encoding(format!("company record: {e}")))?;
    Ok(out)
}

fn io_error(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Best-effort removal of a half-created company file and its WAL side files.
fn remove_company_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let candidate = PathBuf::from(name);
        if let Err(e) = std::fs::remove_file(&candidate)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, "could not remove partial company file");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_list_matches_schema_version() {
        // Derived: one migration file (0001_init.sql) exists, taking version 0 to 1.
        assert_eq!(u32::try_from(MIGRATIONS.len()), Ok(SCHEMA_VERSION));
    }
}
