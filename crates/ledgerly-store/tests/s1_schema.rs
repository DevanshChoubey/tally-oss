//! Card S1 tests: schema creation, open, WAL, schema version guard.
//!
//! Provenance. These tests state properties of the file format, not computed figures:
//! - `Company` round trip: the expected value is the input value itself (identity), derived.
//! - WAL: SQLite documents that `PRAGMA journal_mode` reports `wal` for a database in WAL mode
//!   and that the mode is persistent in the file (sqlite.org/wal.html, "Persistence of WAL mode").
//! - Schema version: the version is stored in `PRAGMA user_version` (sqlite.org/pragma.html), and
//!   opening a file whose version exceeds `SCHEMA_VERSION` must give `SchemaTooNew` carrying both
//!   numbers (spec 6.6: migrations never lose events, so an older build must refuse, not guess).
//! - `SCHEMA_VERSION == 1`: `migrations/0001_init.sql` is the only migration in this wave, derived
//!   from the card text.
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::Path;

use ledgerly_core::{Clock, CompanyId, Date, Gstin, Month, OffsetDateTime, Pan, StateCode};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use ledgerly_store::schema::SCHEMA_VERSION;
use ledgerly_store::{SqliteStore, StoreError};
use rusqlite::Connection;

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn company() -> Company {
    // Fictional data. GSTIN check character derived by hand in ledgerly-core's gstin tests.
    Company {
        id: CompanyId(7),
        name: "Example Traders Pvt Ltd".to_owned(),
        mailing_name: Some("Example Traders".to_owned()),
        address: Address {
            lines: vec!["12 Sample Road".to_owned(), "Bengaluru".to_owned()],
            pincode: Some("560001".to_owned()),
            state: Some(StateCode::new(29).unwrap()),
            country: String::new(),
        },
        state: StateCode::new(29).unwrap(),
        pan: Some(Pan::parse("ABCDE1234F").unwrap()),
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date(2017, Month::July, 1),
            composition: false,
        }],
        fy_start: date(2026, Month::April, 1),
        books_begin: date(2026, Month::April, 1),
        entity_kind: EntityKind::Company,
        features: Features {
            inventory: true,
            gst: true,
            bill_wise: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn new_store(dir: &Path) -> (std::path::PathBuf, SqliteStore) {
    let path = dir.join("books.ledgerly");
    let store = SqliteStore::create(&path, &company(), clock()).unwrap();
    (path, store)
}

#[test]
fn schema_version_is_one() {
    assert_eq!(SCHEMA_VERSION, 1);
}

#[test]
fn create_then_open_round_trips_company() {
    let dir = tempfile::tempdir().unwrap();
    let (path, created) = new_store(dir.path());
    assert_eq!(created.company().unwrap(), company());
    drop(created);
    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(reopened.company().unwrap(), company());
    assert_eq!(reopened.path(), path.as_path());
}

#[test]
fn company_without_optional_fields_round_trips() {
    let mut c = company();
    c.mailing_name = None;
    c.pan = None;
    c.registrations.clear();
    c.address = Address::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("min.ledgerly");
    drop(SqliteStore::create(&path, &c, clock()).unwrap());
    assert_eq!(SqliteStore::open(&path).unwrap().company().unwrap(), c);
}

#[test]
fn wal_mode_is_on_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = new_store(dir.path());
    drop(store);
    // A fresh connection (not ours) sees the persistent journal mode.
    let raw = Connection::open(&path).unwrap();
    let mode: String = raw
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    // And opening through the store keeps it on.
    drop(raw);
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(store.journal_mode().unwrap(), "wal");
}

#[test]
fn foreign_keys_are_enforced_on_both_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (path, created) = new_store(dir.path());
    assert!(created.foreign_keys_on().unwrap());
    drop(created);
    assert!(SqliteStore::open(&path).unwrap().foreign_keys_on().unwrap());
}

#[test]
fn newer_schema_version_errors_schema_too_new() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = new_store(dir.path());
    drop(store);
    let raw = Connection::open(&path).unwrap();
    raw.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(raw);
    match SqliteStore::open(&path) {
        Err(StoreError::SchemaTooNew { found, supported }) => {
            assert_eq!(found, SCHEMA_VERSION + 1);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
}

#[test]
fn text_file_is_not_a_company_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(
        &path,
        b"this is not a database, just some text padding it out",
    )
    .unwrap();
    assert!(matches!(
        SqliteStore::open(&path),
        Err(StoreError::NotACompanyFile(_))
    ));
}

#[test]
fn foreign_sqlite_database_is_not_a_company_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("other.db");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    drop(raw);
    assert!(matches!(
        SqliteStore::open(&path),
        Err(StoreError::NotACompanyFile(_))
    ));
}

#[test]
fn missing_file_is_an_error_and_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.ledgerly");
    assert!(SqliteStore::open(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn create_refuses_to_overwrite_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.ledgerly");
    std::fs::write(&path, b"precious").unwrap();
    match SqliteStore::create(&path, &company(), clock()) {
        Err(StoreError::Io { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists);
        }
        other => panic!("expected Io(AlreadyExists), got {other:?}"),
    }
    assert_eq!(std::fs::read(&path).unwrap(), b"precious");
}

#[test]
fn create_in_missing_directory_fails_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("no-such-dir").join("books.ledgerly");
    assert!(SqliteStore::create(&path, &company(), clock()).is_err());
    assert!(!path.exists());
}

#[test]
fn damaged_company_row_is_an_encoding_error_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = new_store(dir.path());
    drop(store);
    let raw = Connection::open(&path).unwrap();
    raw.execute(
        "UPDATE meta SET value = x'ff00ff' WHERE key = 'company'",
        [],
    )
    .unwrap();
    drop(raw);
    // `open` reads the company row, so damage is reported when the file is opened.
    assert!(matches!(
        SqliteStore::open(&path),
        Err(StoreError::Encoding(_))
    ));
}

#[test]
fn schema_has_the_tables_other_cards_rely_on() {
    // Names are the contract with cards S2-S6, written in the header of 0001_init.sql.
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = new_store(dir.path());
    drop(store);
    let raw = Connection::open(&path).unwrap();
    let mut stmt = raw
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for want in [
        "checkpoints",
        "events",
        "masters",
        "meta",
        "postings",
        "stock_moves",
        "vouchers",
    ] {
        assert!(names.iter().any(|n| n == want), "missing table {want}");
    }
}

#[test]
fn events_table_enforces_a_gapless_chain_shape() {
    // The hash columns must be exactly 32 bytes and the previous hash of seq 1 is stored, so a
    // malformed row is rejected by the schema (spec 6.6 hash chain).
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = new_store(dir.path());
    drop(store);
    let raw = Connection::open(&path).unwrap();
    let bad = raw.execute(
        "INSERT INTO events(seq, prev_hash, hash, meta, event) VALUES (1, x'00', x'00', x'00', x'00')",
        [],
    );
    assert!(bad.is_err());
    let ok = raw.execute(
        "INSERT INTO events(seq, prev_hash, hash, meta, event) VALUES (1, zeroblob(32), zeroblob(32), x'00', x'00')",
        [],
    );
    assert!(ok.is_ok());
}
