//! Card S3 tests: chain verification and daily checkpoints (spec 6.6).
//!
//! Provenance. Every expected value is derived from the definitions in spec 6.6, the S2 chain
//! definition (`ledgerly_store::commit` module doc) and the card text, never from output of the
//! code under test:
//! - Tamper detection: the stored `hash` is SHA-256 over the canonical CBOR of
//!   `(seq, prev_hash, meta, event)` and `prev_hash` is the previous record's `hash`. Changing any
//!   stored byte of record `n` (event, meta, hash or `prev_hash` column) therefore makes record `n`
//!   inconsistent with its own hash or with its predecessor, and the verifier must name `n`. When
//!   `hash` itself is flipped, record `n + 1` also stops following it, so the lowest named
//!   sequence number is still `n`.
//! - Gaps: sequence numbers are 1-based and gapless (`ledgerly_core::EventSeq` doc), so deleting
//!   rows `a..=b` must be reported as exactly the range `a..=b`.
//! - Checkpoint of day `D`: the head at the end of `D`, i.e. the last record before the first
//!   record dated after `D`, where a record's date is the calendar date of `meta.at` in its own
//!   offset (module doc of `ledgerly_store::verify`). Offset arithmetic below is done by hand.
//! - A cut tail is invisible to the chain itself; only a checkpoint naming the removed event shows
//!   it (module doc "Limits").
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_core::{
    Clock, CompanyId, Date, EventSeq, Gstin, Month, OffsetDateTime, Pan, StateCode, UserId,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use ledgerly_store::verify::{ChainProblem, verify_chain, write_checkpoint};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};
use rusqlite::Connection;
use time::UtcOffset;

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn company() -> Company {
    // Fictional data; same fixture as the S2 tests.
    Company {
        ineligible_itc_default: None,
        id: CompanyId(7),
        name: "Example Traders Pvt Ltd".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Sample Road".to_owned()],
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
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn new_store(dir: &Path) -> (PathBuf, SqliteStore) {
    let path = dir.join("books.ledgerly");
    let store =
        SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    (path, store)
}

fn meta_at(at: OffsetDateTime) -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at,
        reason: Some("test".to_owned()),
        app_version: "0.0.0-test".to_owned(),
    }
}

fn meta_on(day: Date) -> AuditMeta {
    meta_at(day.midnight().assume_utc())
}

fn meta() -> AuditMeta {
    meta_at(OffsetDateTime::UNIX_EPOCH)
}

fn setting(n: u32) -> Event {
    Event::SettingChanged {
        key: format!("key{n}"),
        before: format!("old{n}"),
        after: format!("new{n}"),
    }
}

/// A store holding `n` single-event commits with seqs 1..=n.
fn store_with(dir: &Path, n: u32) -> (PathBuf, SqliteStore) {
    let (path, mut store) = new_store(dir);
    for i in 1..=n {
        store.commit(&[setting(i)], &meta()).unwrap();
    }
    (path, store)
}

fn read_column(conn: &Connection, column: &str, seq: u64) -> Vec<u8> {
    conn.query_row(
        &format!("SELECT {column} FROM events WHERE seq = ?1"),
        [i64::try_from(seq).unwrap()],
        |r| r.get(0),
    )
    .unwrap()
}

fn write_column(conn: &Connection, column: &str, seq: u64, value: &[u8]) {
    conn.execute(
        &format!("UPDATE events SET {column} = ?1 WHERE seq = ?2"),
        rusqlite::params![value, i64::try_from(seq).unwrap()],
    )
    .unwrap();
}

fn checkpoint_rows(path: &Path) -> Vec<(String, i64, Vec<u8>)> {
    let conn = Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT day, seq, hash FROM checkpoints ORDER BY day")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn clean_chain_passes() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    let receipt = store
        .commit(&[setting(1), setting(2), setting(3)], &meta())
        .unwrap();
    let report = verify_chain(&store).unwrap();
    assert!(report.is_ok());
    assert_eq!(report.problems, Vec::<ChainProblem>::new());
    assert_eq!(report.events_checked, 3);
    assert_eq!(report.head, Some((EventSeq(3), receipt.last_hash)));
    assert_eq!(report.first_broken_seq(), None);
    assert_eq!(report.gaps(), Vec::<(EventSeq, EventSeq)>::new());
    assert!(
        report
            .certificate()
            .starts_with("Audit chain integrity: PASS\nEvents checked: 3\n")
    );
}

#[test]
fn empty_log_passes() {
    let dir = tempfile::tempdir().unwrap();
    let (_, store) = new_store(dir.path());
    let report = verify_chain(&store).unwrap();
    assert!(report.is_ok());
    assert_eq!(report.events_checked, 0);
    assert_eq!(report.head, None);
    assert!(report.certificate().contains("Head: none"));
}

#[test]
fn flipping_any_byte_of_a_stored_event_names_that_seq() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = store_with(dir.path(), 3);
    let side = Connection::open(&path).unwrap();
    for column in ["event", "meta", "hash", "prev_hash"] {
        for seq in 1..=3u64 {
            let original = read_column(&side, column, seq);
            assert_ne!(original, [] as [u8; 0]);
            for (i, byte) in original.iter().enumerate() {
                let mut tampered = original.clone();
                tampered[i] = byte ^ 0x01;
                write_column(&side, column, seq, &tampered);
                let report = verify_chain(&store).unwrap();
                assert!(!report.is_ok(), "{column} of {seq} byte {i} went unnoticed");
                assert_eq!(
                    report.first_broken_seq(),
                    Some(EventSeq(seq)),
                    "{column} of {seq} byte {i}: {:?}",
                    report.problems
                );
            }
            write_column(&side, column, seq, &original);
            assert!(verify_chain(&store).unwrap().is_ok());
        }
    }
}

#[test]
fn certificate_for_a_broken_chain_names_the_event() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = store_with(dir.path(), 3);
    let side = Connection::open(&path).unwrap();
    let mut hash = read_column(&side, "hash", 2);
    hash[0] ^= 0xFF;
    write_column(&side, "hash", 2, &hash);
    let report = verify_chain(&store).unwrap();
    assert!(
        report
            .problems
            .contains(&ChainProblem::HashMismatch { seq: EventSeq(2) })
    );
    // Event 3 no longer follows event 2's (altered) hash.
    assert!(
        report
            .problems
            .contains(&ChainProblem::PrevHashMismatch { seq: EventSeq(3) })
    );
    let text = report.certificate();
    assert!(text.starts_with("Audit chain integrity: FAIL\n"));
    assert!(text.contains("event 2 does not match its stored hash"));
}

#[test]
fn deleting_a_middle_run_of_rows_names_the_gap() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = store_with(dir.path(), 5);
    let side = Connection::open(&path).unwrap();
    side.execute("DELETE FROM events WHERE seq IN (2, 3)", [])
        .unwrap();
    let report = verify_chain(&store).unwrap();
    assert!(!report.is_ok());
    assert_eq!(report.gaps(), vec![(EventSeq(2), EventSeq(3))]);
    assert_eq!(report.first_broken_seq(), Some(EventSeq(2)));
    assert_eq!(report.events_checked, 3);
    // Event 4 was chained to the deleted event 3, so it no longer follows event 1.
    assert!(
        report
            .problems
            .contains(&ChainProblem::PrevHashMismatch { seq: EventSeq(4) })
    );
}

#[test]
fn deleting_the_first_row_names_the_gap() {
    let dir = tempfile::tempdir().unwrap();
    let (path, store) = store_with(dir.path(), 3);
    let side = Connection::open(&path).unwrap();
    side.execute("DELETE FROM events WHERE seq = 1", [])
        .unwrap();
    let report = verify_chain(&store).unwrap();
    assert_eq!(report.gaps(), vec![(EventSeq(1), EventSeq(1))]);
    assert_eq!(report.first_broken_seq(), Some(EventSeq(1)));
}

#[test]
fn checkpoint_names_the_head_at_the_end_of_the_day() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let d1 = date(2026, Month::April, 1);
    let d2 = date(2026, Month::April, 2);
    let r1 = store
        .commit(&[setting(1), setting(2)], &meta_on(d1))
        .unwrap();
    store.commit(&[setting(3)], &meta_on(d2)).unwrap();

    write_checkpoint(&mut store, d1, &meta_on(d1)).unwrap();
    // Two records are dated 1 April, so the head is seq 2, whose hash is the receipt's last hash.
    assert_eq!(
        checkpoint_rows(&path),
        vec![("2026-04-01".to_owned(), 2, r1.last_hash.to_vec())]
    );

    let report = verify_chain(&store).unwrap();
    assert!(report.is_ok());
    assert_eq!(report.checkpoints.len(), 1);
    assert_eq!(report.checkpoints[0].day, "2026-04-01");
    assert_eq!(report.checkpoints[0].seq, EventSeq(2));
}

#[test]
fn checkpoint_is_idempotent_and_advances_within_a_day() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let d1 = date(2026, Month::April, 1);
    store.commit(&[setting(1)], &meta_on(d1)).unwrap();
    write_checkpoint(&mut store, d1, &meta_on(d1)).unwrap();
    write_checkpoint(&mut store, d1, &meta_on(d1)).unwrap();
    assert_eq!(checkpoint_rows(&path).len(), 1);
    assert_eq!(checkpoint_rows(&path)[0].1, 1);

    // A later record dated the same day moves that day's checkpoint to seq 2, still one row.
    let r2 = store.commit(&[setting(2)], &meta_on(d1)).unwrap();
    write_checkpoint(&mut store, d1, &meta_on(d1)).unwrap();
    let rows = checkpoint_rows(&path);
    assert_eq!(
        rows,
        vec![("2026-04-01".to_owned(), 2, r2.last_hash.to_vec())]
    );
    assert!(verify_chain(&store).unwrap().is_ok());
}

#[test]
fn checkpoint_uses_the_records_own_offset_for_its_day() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let apr1 = date(2026, Month::April, 1);
    let apr2 = date(2026, Month::April, 2);
    // Record 1: 31 March, UTC.
    store
        .commit(&[setting(1)], &meta_on(date(2026, Month::March, 31)))
        .unwrap();
    // Record 2: 2026-04-01 23:00 UTC seen from UTC+05:30. 23:00 + 5:30 = 28:30 = 04:30 next day,
    // so the record's own date is 2 April.
    let ist = UtcOffset::from_hms(5, 30, 0).unwrap();
    let at = apr1.with_hms(23, 0, 0).unwrap().assume_utc().to_offset(ist);
    store.commit(&[setting(2)], &meta_at(at)).unwrap();

    write_checkpoint(&mut store, apr1, &meta()).unwrap();
    write_checkpoint(&mut store, apr2, &meta()).unwrap();
    let rows = checkpoint_rows(&path);
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].0.as_str(), rows[0].1), ("2026-04-01", 1));
    assert_eq!((rows[1].0.as_str(), rows[1].1), ("2026-04-02", 2));
}

#[test]
fn checkpoint_stops_at_the_first_record_dated_after_the_day() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    // Dates 1 Apr, 3 Apr, 1 Apr (clock moved back). The checkpoint of 2 Apr is the prefix before
    // the first record dated after it: seq 1.
    store
        .commit(&[setting(1)], &meta_on(date(2026, Month::April, 1)))
        .unwrap();
    store
        .commit(&[setting(2)], &meta_on(date(2026, Month::April, 3)))
        .unwrap();
    store
        .commit(&[setting(3)], &meta_on(date(2026, Month::April, 1)))
        .unwrap();
    write_checkpoint(&mut store, date(2026, Month::April, 2), &meta()).unwrap();
    let rows = checkpoint_rows(&path);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].0.as_str(), rows[0].1), ("2026-04-02", 1));
}

#[test]
fn no_checkpoint_when_nothing_is_dated_on_or_before_the_day() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    write_checkpoint(&mut store, date(2026, Month::April, 1), &meta()).unwrap();
    store
        .commit(&[setting(1)], &meta_on(date(2026, Month::April, 5)))
        .unwrap();
    write_checkpoint(&mut store, date(2026, Month::April, 1), &meta()).unwrap();
    assert_eq!(checkpoint_rows(&path), Vec::new());
}

#[test]
fn write_checkpoint_refuses_a_broken_chain() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = store_with(dir.path(), 3);
    let side = Connection::open(&path).unwrap();
    let mut event = read_column(&side, "event", 2);
    event[0] ^= 0x01;
    write_column(&side, "event", 2, &event);
    let err = write_checkpoint(&mut store, date(2026, Month::April, 1), &meta()).unwrap_err();
    assert!(
        matches!(err, StoreError::ChainBroken(EventSeq(2))),
        "got {err:?}"
    );
    assert_eq!(checkpoint_rows(&path), Vec::new());
}

#[test]
fn altered_checkpoint_hash_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let d1 = date(2026, Month::April, 1);
    store
        .commit(&[setting(1), setting(2)], &meta_on(d1))
        .unwrap();
    write_checkpoint(&mut store, d1, &meta()).unwrap();
    let side = Connection::open(&path).unwrap();
    side.execute("UPDATE checkpoints SET hash = zeroblob(32)", [])
        .unwrap();
    let report = verify_chain(&store).unwrap();
    assert_eq!(
        report.problems,
        vec![ChainProblem::CheckpointMismatch {
            day: "2026-04-01".to_owned(),
            seq: EventSeq(2)
        }]
    );
    assert_eq!(report.first_broken_seq(), Some(EventSeq(2)));
}

#[test]
fn cutting_the_tail_is_caught_by_a_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let d1 = date(2026, Month::April, 1);
    store
        .commit(&[setting(1), setting(2), setting(3)], &meta_on(d1))
        .unwrap();
    write_checkpoint(&mut store, d1, &meta()).unwrap();
    // The side connection has foreign keys off, as an attacker's tool would. Bundled
    // libsqlite3-sys builds with SQLITE_DEFAULT_FOREIGN_KEYS=1, so turn them off explicitly.
    let side = Connection::open(&path).unwrap();
    side.pragma_update(None, "foreign_keys", false).unwrap();
    side.execute("DELETE FROM events WHERE seq = 3", [])
        .unwrap();
    let report = verify_chain(&store).unwrap();
    assert_eq!(
        report.problems,
        vec![ChainProblem::CheckpointEventMissing {
            day: "2026-04-01".to_owned(),
            seq: EventSeq(3)
        }]
    );
    assert_eq!(report.first_broken_seq(), Some(EventSeq(3)));
}
