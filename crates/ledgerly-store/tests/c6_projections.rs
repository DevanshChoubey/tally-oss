//! Card C6 tests: `Store::commit` keeps the `masters`, `vouchers` and `postings` projections
//! current once they are switched on (`SqliteStore::enable_projections`), and the rebuild check
//! reports a match, a mismatch, or "not maintained" correctly.
//!
//! Provenance. Every expected value is `derived`, by hand from the event list built in each test:
//! - the counts of the 50-event test are worked out in a comment next to the events;
//! - "not maintained" for a file that never had the switch follows the module documentation of
//!   `ledgerly_store::backup` (no writer, so nothing to compare) and the card text (old files are
//!   reported as not maintained, never as a mismatch with figures);
//! - tamper tests change exactly one stored value by hand, so exactly one difference is expected.
//!
//! Spec 6.6 (event log is the source of truth, projections are rebuilt from it).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, Gstin, LedgerId, Month, OffsetDateTime, Paise, Pan,
    SignedAmount, StateCode, UserId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, Group, GstRegistration, LedgerLine, MasterSnapshot,
    NegativeStockPolicy, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::backup::{
    RebuildOutcome, backup_to, rebuild_projections_and_compare, restore_from,
};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};
use rusqlite::Connection;

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

fn new_store(dir: &Path) -> SqliteStore {
    let path = dir.join("books.ledgerly");
    SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: None,
        app_version: "0.0.0-test".to_owned(),
    }
}

fn epoch() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn group(id: u64, name: &str) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        tally: None,
    }
}

fn line(ledger: u64, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: LedgerId(ledger),
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// Voucher `id` on `on` with Dr ledger 10 and Cr ledger 20 for `paise`.
fn two_line(id: u64, on: Date, paise: i64) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(format!("V{id}")),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![
            line(10, SignedAmount::dr(Paise::new(paise))),
            line(20, SignedAmount::cr(Paise::new(paise))),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn cbor<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out).unwrap();
    out
}

fn d(day: u8) -> Date {
    date(2026, Month::May, day)
}

fn row_count(path: &Path, table: &str) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// 50 events: 7 master events (5 groups created, group 1 altered, group 5 deleted) and 43 voucher
/// events (39 posted, vouchers 3 and 4 altered, voucher 5 cancelled, voucher 6 deleted).
fn fifty_events() -> Vec<Event> {
    let mut events = Vec::new();
    for id in 1..=5_u64 {
        events.push(Event::MasterCreated(MasterSnapshot::Group(group(
            id,
            &format!("Group {id}"),
        ))));
    }
    events.push(Event::MasterAltered {
        before: MasterSnapshot::Group(group(1, "Group 1")),
        after: MasterSnapshot::Group(group(1, "Group One")),
    });
    for id in 1..=39_u64 {
        let day = u8::try_from(id % 28 + 1).unwrap();
        events.push(Event::VoucherPosted(two_line(
            id,
            d(day),
            i64::try_from(id).unwrap() * 100,
        )));
    }
    for id in [3_u64, 4] {
        let day = u8::try_from(id % 28 + 1).unwrap();
        let before = two_line(id, d(day), i64::try_from(id).unwrap() * 100);
        let mut after = two_line(id, d(day), 9_000 + i64::try_from(id).unwrap());
        "corrected".clone_into(&mut after.narration);
        events.push(Event::VoucherAltered { before, after });
    }
    events.push(Event::VoucherCancelled {
        before: two_line(5, d(6), 500),
    });
    events.push(Event::VoucherDeleted {
        before: two_line(6, d(7), 600),
    });
    events.push(Event::MasterDeleted(MasterSnapshot::Group(group(
        5, "Group 5",
    ))));
    events
}

fn committed_in_batches(store: &mut SqliteStore, events: &[Event]) {
    for chunk in events.chunks(7) {
        store.commit(chunk, &meta()).unwrap();
    }
}

#[test]
fn fifty_mixed_events_rebuild_and_match() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    assert!(store.enable_projections().unwrap());
    let events = fifty_events();
    // Derived: 5 + 1 + 39 + 2 + 1 + 1 + 1 = 50 events.
    assert_eq!(events.len(), 50);
    committed_in_batches(&mut store, &events);
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.outcome, RebuildOutcome::Match, "{report:?}");
    assert!(report.is_match());
    assert_eq!(report.events_replayed, 50);
    // Derived: groups 1..4 remain (5 created, 5 deleted) = 4 masters; vouchers 39 posted minus
    // voucher 6 deleted = 38 (cancelled voucher 5 stays listed); postings: 38 vouchers minus the
    // cancelled one = 37 vouchers with 2 ledger lines each = 74.
    assert_eq!(report.rebuilt.masters, 4);
    assert_eq!(report.rebuilt.vouchers, 38);
    assert_eq!(report.rebuilt.postings, 74);
    assert_eq!(report.stored, report.rebuilt);
    // The rows are really on disk, written by commit.
    assert_eq!(row_count(store.path(), "postings"), 74);
}

#[test]
fn match_survives_reopen_backup_and_restore() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    committed_in_batches(&mut store, &fifty_events());
    let path = store.path().to_path_buf();
    drop(store);
    let reopened = SqliteStore::open(&path).unwrap();
    // The switch is stored in the file, so it is still on after reopening.
    assert!(reopened.projections_enabled().unwrap());
    let file = backup_to(&reopened, &dir.path().join("backups"), epoch()).unwrap();
    let mut restored = restore_from(&file, &dir.path().join("restored.ledgerly")).unwrap();
    assert!(restored.projections_enabled().unwrap());
    assert!(
        rebuild_projections_and_compare(&restored)
            .unwrap()
            .is_match()
    );
    restored
        .commit(
            &[Event::VoucherPosted(two_line(100, d(20), 1_234))],
            &meta(),
        )
        .unwrap();
    let report = rebuild_projections_and_compare(&restored).unwrap();
    assert!(report.is_match(), "{report:?}");
    // Derived: one more voucher with 2 postings.
    assert_eq!(report.stored.vouchers, 39);
    assert_eq!(report.stored.postings, 76);
}

#[test]
fn a_tampered_posting_row_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    committed_in_batches(&mut store, &fifty_events());
    // Hand change: the Dr line of voucher 1 (100 paise) says 101.
    Connection::open(store.path())
        .unwrap()
        .execute(
            "UPDATE postings SET paise = 101 WHERE voucher_id = 1 AND line_no = 1",
            [],
        )
        .unwrap();
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!(diffs[0].table, "postings");
    assert!(!report.is_match());
}

#[test]
fn a_tampered_voucher_snapshot_and_a_removed_master_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    committed_in_batches(&mut store, &fifty_events());
    let mut changed = two_line(2, d(3), 200);
    changed.narration = "edited outside the log".to_owned();
    let conn = Connection::open(store.path()).unwrap();
    conn.execute(
        "UPDATE vouchers SET snapshot = ?1 WHERE id = 2",
        rusqlite::params![cbor(&changed)],
    )
    .unwrap();
    conn.execute("DELETE FROM masters WHERE id = 2", [])
        .unwrap();
    drop(conn);
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    // Derived: one voucher differs, one master is missing.
    assert_eq!(diffs.len(), 2, "{diffs:?}");
    assert!(diffs.iter().any(|x| x.table == "vouchers"));
    assert!(diffs.iter().any(|x| x.table == "masters"));
}

#[test]
fn emptied_tables_on_a_maintained_file_are_a_mismatch_not_unmaintained() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    committed_in_batches(&mut store, &fifty_events());
    let conn = Connection::open(store.path()).unwrap();
    for table in ["postings", "vouchers", "masters"] {
        conn.execute(&format!("DELETE FROM {table}"), []).unwrap();
    }
    drop(conn);
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert!(
        matches!(report.outcome, RebuildOutcome::Mismatch(_)),
        "{report:?}"
    );
}

#[test]
fn a_file_without_the_switch_is_not_maintained_and_commit_leaves_the_tables_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    committed_in_batches(&mut store, &fifty_events());
    assert!(!store.projections_enabled().unwrap());
    for table in ["masters", "vouchers", "postings"] {
        assert_eq!(row_count(store.path(), table), 0, "{table}");
    }
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.outcome, RebuildOutcome::NotMaintained);
    assert!(!report.is_match());
}

#[test]
fn enabling_builds_the_tables_from_the_existing_log() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let events = fifty_events();
    committed_in_batches(&mut store, &events[..30]);
    assert_eq!(
        rebuild_projections_and_compare(&store).unwrap().outcome,
        RebuildOutcome::NotMaintained
    );
    assert!(store.enable_projections().unwrap());
    // Derived: a second call changes nothing.
    assert!(!store.enable_projections().unwrap());
    assert!(rebuild_projections_and_compare(&store).unwrap().is_match());
    committed_in_batches(&mut store, &events[30..]);
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.outcome, RebuildOutcome::Match, "{report:?}");
    assert_eq!(report.events_replayed, 50);
}

#[test]
fn enabling_refuses_tables_that_already_hold_rows() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(dir.path());
    Connection::open(store.path())
        .unwrap()
        .execute(
            "INSERT INTO masters(kind, id, snapshot) VALUES ('group', 1, ?1)",
            rusqlite::params![cbor(&MasterSnapshot::Group(group(1, "Hand made")))],
        )
        .unwrap();
    assert!(matches!(
        store.enable_projections(),
        Err(StoreError::Encoding(_))
    ));
    assert!(!store.projections_enabled().unwrap());
}

#[test]
fn an_event_that_does_not_fit_fails_the_commit_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    let v = two_line(1, d(5), 1_000);
    store
        .commit(&[Event::VoucherPosted(v.clone())], &meta())
        .unwrap();
    // Derived: posting id 1 a second time (event 3 of this batch is the bad one) is what a valid
    // log never contains; the whole batch is refused, so the log keeps its 1 event.
    let result = store.commit(
        &[
            Event::VoucherPosted(two_line(2, d(6), 10)),
            Event::VoucherPosted(v),
        ],
        &meta(),
    );
    assert!(
        matches!(&result, Err(StoreError::Encoding(m)) if m.contains("event 3")),
        "{result:?}"
    );
    assert_eq!(row_count(store.path(), "events"), 1);
    assert_eq!(row_count(store.path(), "vouchers"), 1);
    assert!(rebuild_projections_and_compare(&store).unwrap().is_match());
}

#[test]
fn a_cancelled_voucher_keeps_its_row_and_loses_its_postings() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store.enable_projections().unwrap();
    let v = two_line(1, d(5), 1_000);
    store
        .commit(
            &[
                Event::VoucherPosted(v.clone()),
                Event::VoucherCancelled { before: v },
            ],
            &meta(),
        )
        .unwrap();
    // Derived (`Event::VoucherCancelled`: number kept, amounts removed): 1 voucher row, 0 postings.
    assert_eq!(row_count(store.path(), "vouchers"), 1);
    assert_eq!(row_count(store.path(), "postings"), 0);
    assert!(rebuild_projections_and_compare(&store).unwrap().is_match());
}
