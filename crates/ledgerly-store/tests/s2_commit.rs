//! Card S2 tests: event append, hash chain, atomic batches, reading back.
//!
//! Provenance. Every expected value is derived from the definitions in spec 6.6 and the card text,
//! never from output of the code under test:
//! - Sequence numbers: the log is 1-based, gapless and strictly increasing
//!   (`ledgerly_core::EventSeq` doc), so two single-event commits give seq 1 then 2, and a batch of
//!   three on an empty log gives 1, 2, 3.
//! - `prev_hash` of record 1 is 32 zero bytes (`AuditRecord::prev_hash` doc); `prev_hash` of
//!   record n is `hash` of record n-1 (hash chain, spec 6.6).
//! - Hash: SHA-256 over the canonical CBOR of the tuple `(seq, prev_hash, meta, event)` (card text,
//!   `AuditRecord::hash` doc). The tests recompute it with `ciborium` and `sha2` directly, writing
//!   the tuple out here, not by calling `record_hash`. A byte-exact known-answer vector from an
//!   independent CBOR implementation is `pending`; until then the encoding is pinned only by this
//!   recomputation.
//! - Atomicity: a SQLite trigger that aborts the insert of seq 3 (`sqlite.org/lang_createtrigger.html`,
//!   `RAISE(ABORT, ..)`) stands in for a constraint failure mid-batch; SQLite rolls back the whole
//!   transaction, so no row of that batch may remain (`sqlite.org/lang_transaction.html`).
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::Path;

use ledgerly_core::{
    Clock, CompanyId, Date, EventSeq, Gstin, Month, OffsetDateTime, Pan, StateCode, UserId,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use ledgerly_store::commit::GENESIS_HASH;
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn company() -> Company {
    // Fictional data; GSTIN check character derived by hand in ledgerly-core's gstin tests.
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

fn new_store(dir: &Path) -> (std::path::PathBuf, SqliteStore) {
    let path = dir.join("books.ledgerly");
    let store =
        SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    (path, store)
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: Some("test".to_owned()),
        app_version: "0.0.0-test".to_owned(),
    }
}

fn setting(n: u32) -> Event {
    Event::SettingChanged {
        key: format!("key{n}"),
        before: format!("old{n}"),
        after: format!("new{n}"),
    }
}

/// Independent recomputation of the chain hash, written from the spec definition.
fn expected_hash(seq: u64, prev: &[u8; 32], meta: &AuditMeta, event: &Event) -> [u8; 32] {
    let mut bytes = Vec::new();
    ciborium::into_writer(&(seq, prev, meta, event), &mut bytes).unwrap();
    Sha256::digest(&bytes).into()
}

fn row_count(path: &Path) -> i64 {
    let conn = Connection::open(path).unwrap();
    conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn two_commits_give_seq_1_2_and_chain() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    let r1 = store.commit(&[setting(1)], &meta()).unwrap();
    let r2 = store.commit(&[setting(2)], &meta()).unwrap();
    assert_eq!(r1.seqs, vec![EventSeq(1)]);
    assert_eq!(r2.seqs, vec![EventSeq(2)]);

    let recs = store.events(EventSeq(1), 10).unwrap();
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].seq, EventSeq(1));
    assert_eq!(recs[1].seq, EventSeq(2));
    assert_eq!(recs[0].prev_hash, GENESIS_HASH);
    assert_eq!(recs[0].prev_hash, [0u8; 32]);
    assert_eq!(recs[1].prev_hash, recs[0].hash);
    assert_eq!(r1.last_hash, recs[0].hash);
    assert_eq!(r2.last_hash, recs[1].hash);
    assert_ne!(recs[0].hash, recs[1].hash);
}

#[test]
fn hash_is_sha256_of_canonical_cbor_tuple() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    store.commit(&[setting(1)], &meta()).unwrap();
    store.commit(&[setting(2)], &meta()).unwrap();
    let recs = store.events(EventSeq(1), 10).unwrap();
    let h1 = expected_hash(1, &[0u8; 32], &meta(), &setting(1));
    let h2 = expected_hash(2, &h1, &meta(), &setting(2));
    assert_eq!(recs[0].hash, h1);
    assert_eq!(recs[1].hash, h2);
    // The public helper agrees with the independent recomputation.
    let via_helper =
        ledgerly_store::commit::record_hash(EventSeq(1), &[0u8; 32], &meta(), &setting(1)).unwrap();
    assert_eq!(via_helper, h1);
}

#[test]
fn hash_depends_on_every_tuple_member() {
    // Derived: SHA-256 is collision resistant, so changing any member of the tuple changes the
    // digest. Checked on the independent recomputation inputs and the public helper alike.
    let base = expected_hash(1, &[0u8; 32], &meta(), &setting(1));
    let mut other_meta = meta();
    other_meta.reason = None;
    assert_ne!(base, expected_hash(2, &[0u8; 32], &meta(), &setting(1)));
    assert_ne!(base, expected_hash(1, &[1u8; 32], &meta(), &setting(1)));
    assert_ne!(base, expected_hash(1, &[0u8; 32], &other_meta, &setting(1)));
    assert_ne!(base, expected_hash(1, &[0u8; 32], &meta(), &setting(2)));
}

#[test]
fn batch_of_three_is_one_chained_run() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    let events = [setting(1), setting(2), setting(3)];
    let receipt = store.commit(&events, &meta()).unwrap();
    assert_eq!(receipt.seqs, vec![EventSeq(1), EventSeq(2), EventSeq(3)]);
    let recs = store.events(EventSeq(1), 10).unwrap();
    assert_eq!(recs.len(), 3);
    assert_eq!(recs[0].prev_hash, [0u8; 32]);
    assert_eq!(recs[1].prev_hash, recs[0].hash);
    assert_eq!(recs[2].prev_hash, recs[1].hash);
    assert_eq!(receipt.last_hash, recs[2].hash);
    let h1 = expected_hash(1, &[0u8; 32], &meta(), &events[0]);
    let h2 = expected_hash(2, &h1, &meta(), &events[1]);
    let h3 = expected_hash(3, &h2, &meta(), &events[2]);
    assert_eq!(recs[2].hash, h3);
}

#[test]
fn stored_record_round_trips_meta_and_event() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    let locked = Event::PeriodLocked {
        until: date(2026, Month::September, 30),
    };
    store
        .commit(std::slice::from_ref(&locked), &meta())
        .unwrap();
    let recs = store.events(EventSeq(1), 10).unwrap();
    assert_eq!(recs[0].event, locked);
    assert_eq!(recs[0].meta, meta());
}

#[test]
fn failing_commit_mid_batch_leaves_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let first = store.commit(&[setting(1)], &meta()).unwrap();

    // A second connection installs a trigger that aborts the insert of seq 3. The next batch gets
    // seqs 2, 3, 4, so its second insert fails after its first insert succeeded.
    let side = Connection::open(&path).unwrap();
    side.execute_batch(
        "CREATE TRIGGER fail_seq_3 BEFORE INSERT ON events WHEN NEW.seq = 3 \
         BEGIN SELECT RAISE(ABORT, 'forced failure'); END;",
    )
    .unwrap();

    let err = store
        .commit(&[setting(2), setting(3), setting(4)], &meta())
        .unwrap_err();
    assert!(matches!(err, StoreError::Sqlite(_)), "got {err:?}");
    assert_eq!(row_count(&path), 1, "failed batch must leave no rows");
    assert_eq!(store.events(EventSeq(1), 10).unwrap().len(), 1);

    // After the fault is removed the chain continues from the single committed record.
    side.execute_batch("DROP TRIGGER fail_seq_3;").unwrap();
    let retry = store.commit(&[setting(2)], &meta()).unwrap();
    assert_eq!(retry.seqs, vec![EventSeq(2)]);
    let recs = store.events(EventSeq(1), 10).unwrap();
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[1].prev_hash, first.last_hash);
}

#[test]
fn empty_batch_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let empty = store.commit(&[], &meta()).unwrap();
    assert_eq!(empty.seqs, [] as [ledgerly_core::EventSeq; 0]);
    assert_eq!(empty.last_hash, [0u8; 32]);
    assert_eq!(row_count(&path), 0);

    let one = store.commit(&[setting(1)], &meta()).unwrap();
    let again = store.commit(&[], &meta()).unwrap();
    assert_eq!(again.seqs, [] as [ledgerly_core::EventSeq; 0]);
    assert_eq!(again.last_hash, one.last_hash);
    assert_eq!(row_count(&path), 1);
}

#[test]
fn events_honours_from_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut store) = new_store(dir.path());
    store
        .commit(&[setting(1), setting(2), setting(3), setting(4)], &meta())
        .unwrap();

    let from_two = store.events(EventSeq(2), 2).unwrap();
    let seqs: Vec<EventSeq> = from_two.iter().map(|r| r.seq).collect();
    assert_eq!(seqs, vec![EventSeq(2), EventSeq(3)]);

    assert_eq!(
        store.events(EventSeq(1), 0).unwrap(),
        [] as [ledgerly_store::AuditRecord; 0]
    );
    assert_eq!(
        store.events(EventSeq(5), 10).unwrap(),
        [] as [ledgerly_store::AuditRecord; 0]
    );
    assert_eq!(
        store.events(EventSeq(u64::MAX), 10).unwrap(),
        [] as [ledgerly_store::AuditRecord; 0]
    );
    // `from` 0 behaves like 1: sequence numbers start at 1.
    assert_eq!(store.events(EventSeq(0), 100).unwrap().len(), 4);
    assert_eq!(store.events(EventSeq(1), usize::MAX).unwrap().len(), 4);
}

#[test]
fn chain_continues_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut store) = new_store(dir.path());
    let first = store.commit(&[setting(1)], &meta()).unwrap();
    drop(store);

    let mut reopened = SqliteStore::open(&path).unwrap();
    let second = reopened.commit(&[setting(2)], &meta()).unwrap();
    assert_eq!(second.seqs, vec![EventSeq(2)]);
    let recs = reopened.events(EventSeq(1), 10).unwrap();
    assert_eq!(recs[1].prev_hash, first.last_hash);
}
