//! Card S6 tests: backup, restore and the rebuild-and-compare check.
//!
//! Provenance. Every expected value is derived from the card text, spec 6.6 and the SQLite
//! documentation, never from output of the code under test:
//! - Backup file name: `<stem>-backup-<YYYYMMDDTHHMMSSZ>.ledgerly`, the time from the injected
//!   clock in UTC (module doc of `ledgerly_store::backup`). `OffsetDateTime::UNIX_EPOCH` is
//!   1970-01-01 00:00:00 UTC by definition, so the stamp is `19700101T000000Z`.
//! - "Backup opens and verifies": SQLite's `PRAGMA integrity_check` returns the single row `ok`
//!   for a healthy file (`sqlite.org/pragma.html#pragma_integrity_check`).
//! - "Restore equals the original": the restored company file must give the same `Books`, the same
//!   audit records (same hashes) and the same company as the file it was backed up from (card
//!   text, spec 6.6: restore preserves events and hashes).
//! - Snapshot isolation: a backup holds the events committed before it was taken and none that
//!   come later (a copy, not a link): 3 events before, 1 more after, so the backup has 3.
//! - Rebuild: the projection tables are filled by hand here with rows that agree with the events
//!   (same ids, dates, ledgers, paise and sides as the fixture voucher lines written below), so a
//!   match is expected; each mismatch test changes exactly one stored value by hand.
//! - Nothing maintains the projection tables in this wave, so a log with events and empty tables
//!   must report `NotMaintained`, never `Match`.

#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{
    Clock, CompanyId, Date, EventSeq, GroupId, Gstin, LedgerId, Month, OffsetDateTime, Paise, Pan,
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

/// A store with one group master and two vouchers (2 + 1 events over two commits).
fn populated(dir: &Path) -> SqliteStore {
    let mut store = new_store(dir);
    store
        .commit(
            &[
                Event::MasterCreated(MasterSnapshot::Group(group(1, "Sundry Debtors"))),
                Event::VoucherPosted(two_line(1, date(2026, Month::May, 10), 1_000)),
            ],
            &meta(),
        )
        .unwrap();
    store
        .commit(
            &[Event::VoucherPosted(two_line(
                2,
                date(2026, Month::May, 11),
                2_500,
            ))],
            &meta(),
        )
        .unwrap();
    store
}

fn cbor<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out).unwrap();
    out
}

fn insert_voucher_rows(conn: &Connection, v: &Voucher, snapshot: &Voucher, posting_paise: i64) {
    conn.execute(
        "INSERT INTO vouchers(id, date, status, snapshot) VALUES (?1, ?2, 'Regular', ?3)",
        rusqlite::params![
            i64::try_from(v.id.0).unwrap(),
            v.date.to_string(),
            cbor(snapshot)
        ],
    )
    .unwrap();
    for (line_no, ledger, side) in [(1_i64, 10_i64, "Dr"), (2, 20, "Cr")] {
        conn.execute(
            "INSERT INTO postings(voucher_id, line_no, date, ledger_id, paise, side, snapshot) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                i64::try_from(v.id.0).unwrap(),
                line_no,
                v.date.to_string(),
                ledger,
                posting_paise,
                side,
                Vec::<u8>::new()
            ],
        )
        .unwrap();
    }
}

/// Fills the projection tables with rows that agree with the events of `populated`.
fn fill_matching_projections(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "INSERT INTO masters(kind, id, snapshot) VALUES ('group', 1, ?1)",
        rusqlite::params![cbor(&MasterSnapshot::Group(group(1, "Sundry Debtors")))],
    )
    .unwrap();
    let v1 = two_line(1, date(2026, Month::May, 10), 1_000);
    let v2 = two_line(2, date(2026, Month::May, 11), 2_500);
    insert_voucher_rows(&conn, &v1, &v1, 1_000);
    insert_voucher_rows(&conn, &v2, &v2, 2_500);
}

#[test]
fn backup_file_opens_and_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let backups = dir.path().join("backups");
    let file = backup_to(&store, &backups, epoch()).unwrap();
    // Derived: stem `books`, UNIX_EPOCH is 1970-01-01T00:00:00Z.
    assert_eq!(
        file.file_name().unwrap().to_str().unwrap(),
        "books-backup-19700101T000000Z.ledgerly"
    );
    assert_eq!(file.parent().unwrap(), backups);
    // No partial file is left behind.
    let names: Vec<String> = std::fs::read_dir(&backups)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["books-backup-19700101T000000Z.ledgerly".to_owned()]
    );
    // SQLite's own check says the file is healthy.
    let conn =
        Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok");
    drop(conn);
    // It opens as a company file and holds the same audit records (2 commits, 3 events).
    let opened = SqliteStore::open(&file).unwrap();
    let events = opened.events(EventSeq(1), 100).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events, store.events(EventSeq(1), 100).unwrap());
}

#[test]
fn restore_into_new_path_equals_original_books() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let file = backup_to(&store, &dir.path().join("backups"), epoch()).unwrap();
    let target = dir.path().join("restored.ledgerly");
    let restored = restore_from(&file, &target).unwrap();
    assert_eq!(restored.path(), target);
    assert_eq!(
        restored.load_books(None).unwrap(),
        store.load_books(None).unwrap()
    );
    assert_eq!(
        restored.events(EventSeq(1), 100).unwrap(),
        store.events(EventSeq(1), 100).unwrap()
    );
    assert_eq!(restored.company().unwrap(), store.company().unwrap());
    // Derived: every opened company file is in WAL mode (spec 6.6 storage; schema card S1).
    assert_eq!(restored.journal_mode().unwrap().to_lowercase(), "wal");
    // The backup itself is untouched and can be restored again elsewhere.
    let again = restore_from(&file, &dir.path().join("again.ledgerly")).unwrap();
    assert_eq!(
        again.load_books(None).unwrap(),
        store.load_books(None).unwrap()
    );
}

#[test]
fn restored_store_accepts_new_events_on_the_same_chain() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let file = backup_to(&store, &dir.path().join("backups"), epoch()).unwrap();
    let mut restored = restore_from(&file, &dir.path().join("restored.ledgerly")).unwrap();
    let receipt = restored
        .commit(
            &[Event::VoucherPosted(two_line(
                3,
                date(2026, Month::May, 12),
                100,
            ))],
            &meta(),
        )
        .unwrap();
    // Derived: the log had 3 events, so the next sequence number is 4.
    assert_eq!(receipt.seqs, vec![EventSeq(4)]);
    let records = restored.events(EventSeq(1), 100).unwrap();
    assert_eq!(records.len(), 4);
    // Hash chain: record 4 links to the hash of record 3 (spec 6.6).
    assert_eq!(records[3].prev_hash, records[2].hash);
}

#[test]
fn backup_is_a_snapshot_not_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = populated(dir.path());
    let file = backup_to(&store, &dir.path().join("backups"), epoch()).unwrap();
    store
        .commit(
            &[Event::VoucherPosted(two_line(
                3,
                date(2026, Month::May, 12),
                100,
            ))],
            &meta(),
        )
        .unwrap();
    let opened = SqliteStore::open(&file).unwrap();
    // Derived: 3 events before the backup, 1 after; the backup has 3, the original 4.
    assert_eq!(opened.events(EventSeq(1), 100).unwrap().len(), 3);
    assert_eq!(store.events(EventSeq(1), 100).unwrap().len(), 4);
}

#[test]
fn backup_of_an_empty_company_works() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(dir.path());
    let file = backup_to(&store, dir.path(), epoch()).unwrap();
    let restored = restore_from(&file, &dir.path().join("restored.ledgerly")).unwrap();
    assert_eq!(
        restored.events(EventSeq(1), 10).unwrap(),
        [] as [ledgerly_store::AuditRecord; 0]
    );
    assert_eq!(restored.company().unwrap(), company());
}

#[test]
fn backup_never_overwrites_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let backups = dir.path().join("backups");
    let file = backup_to(&store, &backups, epoch()).unwrap();
    let before = std::fs::read(&file).unwrap();
    let second = backup_to(&store, &backups, epoch());
    assert!(
        matches!(&second, Err(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::AlreadyExists),
        "{second:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn a_later_clock_gives_a_second_backup_with_its_own_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let backups = dir.path().join("backups");
    backup_to(&store, &backups, epoch()).unwrap();
    // Derived: epoch + 86_400 s is 1970-01-02 00:00:00 UTC.
    let next_day = OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(86_400);
    let second = backup_to(&store, &backups, Clock::Fixed(next_day)).unwrap();
    assert_eq!(
        second.file_name().unwrap().to_str().unwrap(),
        "books-backup-19700102T000000Z.ledgerly"
    );
}

#[test]
fn restore_refuses_an_existing_target_and_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let file = backup_to(&store, &dir.path().join("backups"), epoch()).unwrap();
    let target = dir.path().join("existing.ledgerly");
    std::fs::write(&target, b"keep me").unwrap();
    let result = restore_from(&file, &target);
    assert!(
        matches!(&result, Err(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::AlreadyExists),
        "{:?}",
        result.as_ref().err()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"keep me");
}

#[test]
fn restore_of_a_missing_backup_is_a_not_found_error() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("restored.ledgerly");
    let result = restore_from(&dir.path().join("nope.ledgerly"), &target);
    assert!(
        matches!(&result, Err(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound),
        "{:?}",
        result.as_ref().err()
    );
    assert!(!target.exists());
}

#[test]
fn restore_of_a_database_that_is_not_a_company_file_fails_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let foreign = dir.path().join("other.db");
    {
        let conn = Connection::open(&foreign).unwrap();
        conn.execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
    }
    let target = dir.path().join("restored.ledgerly");
    let result = restore_from(&foreign, &target);
    assert!(
        matches!(&result, Err(StoreError::NotACompanyFile(_))),
        "{:?}",
        result.as_ref().err()
    );
    assert!(!target.exists(), "failed restore must remove its target");
}

#[test]
fn restore_of_a_file_that_is_not_sqlite_fails_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let junk = dir.path().join("junk.ledgerly");
    std::fs::write(&junk, vec![b'x'; 4096]).unwrap();
    let target = dir.path().join("restored.ledgerly");
    assert!(restore_from(&junk, &target).is_err());
    assert!(!target.exists());
}

#[test]
fn rebuild_of_an_empty_company_matches_trivially() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(dir.path());
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.events_replayed, 0);
    assert_eq!(report.outcome, RebuildOutcome::Match);
    assert!(report.is_match());
    assert_eq!(
        report.rebuilt.masters + report.rebuilt.vouchers + report.rebuilt.postings,
        0
    );
}

#[test]
fn rebuild_counts_what_the_log_produces() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let report = rebuild_projections_and_compare(&store).unwrap();
    // Derived: 3 events; 1 group; 2 vouchers; 2 ledger lines each = 4 postings.
    assert_eq!(report.events_replayed, 3);
    assert_eq!(report.rebuilt.masters, 1);
    assert_eq!(report.rebuilt.vouchers, 2);
    assert_eq!(report.rebuilt.postings, 4);
}

#[test]
fn unmaintained_projection_tables_are_reported_not_matched() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let report = rebuild_projections_and_compare(&store).unwrap();
    // Derived: nothing writes the projection tables in this wave, so they hold 0 rows.
    assert_eq!(
        report.stored.masters + report.stored.vouchers + report.stored.postings,
        0
    );
    assert_eq!(report.outcome, RebuildOutcome::NotMaintained);
    assert!(!report.is_match());
}

#[test]
fn stock_moves_are_listed_as_not_compared() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.not_compared.len(), 1);
    assert_eq!(report.not_compared[0].table, "stock_moves");
}

#[test]
fn projections_that_agree_with_the_log_match() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    fill_matching_projections(store.path());
    let report = rebuild_projections_and_compare(&store).unwrap();
    assert_eq!(report.outcome, RebuildOutcome::Match, "{report:?}");
    assert_eq!(report.stored, report.rebuilt);
    assert!(report.is_match());
}

#[test]
fn a_changed_stored_voucher_is_a_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    fill_matching_projections(store.path());
    // Hand change: the stored snapshot of voucher 2 carries a narration the log does not have.
    let mut changed = two_line(2, date(2026, Month::May, 11), 2_500);
    changed.narration = "edited outside the log".to_owned();
    Connection::open(store.path())
        .unwrap()
        .execute(
            "UPDATE vouchers SET snapshot = ?1 WHERE id = 2",
            rusqlite::params![cbor(&changed)],
        )
        .unwrap();
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    // Derived: exactly one stored value differs, in table `vouchers`.
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!(diffs[0].table, "vouchers");
    assert!(!report.is_match());
}

#[test]
fn a_changed_stored_posting_amount_is_a_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    fill_matching_projections(store.path());
    // Hand change: the Dr line of voucher 1 says 1,001 paise instead of 1,000.
    Connection::open(store.path())
        .unwrap()
        .execute(
            "UPDATE postings SET paise = 1001 WHERE voucher_id = 1 AND line_no = 1",
            [],
        )
        .unwrap();
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!(diffs[0].table, "postings");
}

#[test]
fn a_missing_and_an_extra_stored_row_are_both_reported() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    fill_matching_projections(store.path());
    let conn = Connection::open(store.path()).unwrap();
    // Hand change: the group row is removed; a master the log never created is added.
    conn.execute("DELETE FROM masters WHERE id = 1", [])
        .unwrap();
    conn.execute(
        "INSERT INTO masters(kind, id, snapshot) VALUES ('group', 9, ?1)",
        rusqlite::params![cbor(&MasterSnapshot::Group(group(9, "Ghost")))],
    )
    .unwrap();
    drop(conn);
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    // Derived: one missing (group 1) and one extra (group 9), both in `masters`.
    assert_eq!(diffs.len(), 2, "{diffs:?}");
    assert!(diffs.iter().all(|d| d.table == "masters"));
}

#[test]
fn a_voucher_date_column_that_disagrees_with_its_snapshot_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    fill_matching_projections(store.path());
    // Hand change: the date column of voucher 1 is a day off; the snapshot is still right.
    Connection::open(store.path())
        .unwrap()
        .execute("UPDATE vouchers SET date = '2026-05-09' WHERE id = 1", [])
        .unwrap();
    let report = rebuild_projections_and_compare(&store).unwrap();
    let RebuildOutcome::Mismatch(diffs) = &report.outcome else {
        panic!("expected a mismatch, got {report:?}");
    };
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!(diffs[0].table, "vouchers");
}

#[test]
fn rebuild_after_restore_gives_the_same_report() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let file = backup_to(&store, &dir.path().join("backups"), epoch()).unwrap();
    let restored = restore_from(&file, &dir.path().join("restored.ledgerly")).unwrap();
    assert_eq!(
        rebuild_projections_and_compare(&restored).unwrap(),
        rebuild_projections_and_compare(&store).unwrap()
    );
}

/// The `-wal` side file of a database path.
fn with_wal(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push("-wal");
    std::path::PathBuf::from(name)
}

/// Flips the lowest bit of the first byte of stored event `seq`, leaving its hash untouched.
fn flip_first_byte_of_event(path: &Path, seq: i64) {
    let conn = Connection::open(path).unwrap();
    let mut event: Vec<u8> = conn
        .query_row("SELECT event FROM events WHERE seq = ?1", [seq], |r| {
            r.get(0)
        })
        .unwrap();
    if let Some(byte) = event.first_mut() {
        *byte ^= 0x01;
    }
    conn.execute(
        "UPDATE events SET event = ?1 WHERE seq = ?2",
        rusqlite::params![event, seq],
    )
    .unwrap();
}

#[test]
fn backup_refuses_a_company_file_whose_audit_chain_is_altered() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    // Hand change: event 2 (the first voucher) has one stored byte flipped; its hash column still
    // matches the old head, so the head comparison alone would call the copy good.
    flip_first_byte_of_event(store.path(), 2);
    let backups = dir.path().join("backups");
    let result = backup_to(&store, &backups, epoch());
    // Derived: populated() commits event 1 (group) and events 2 and 3 (vouchers 1 and 2), so the
    // first broken record is event 2.
    assert!(
        matches!(&result, Err(StoreError::ChainBroken(EventSeq(2)))),
        "{result:?}"
    );
    assert!(
        !backups
            .join("books-backup-19700101T000000Z.ledgerly")
            .exists()
    );
}

#[test]
fn restore_of_a_backup_with_events_in_its_wal_file_fails_instead_of_losing_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    // A company file copied while it is open: the main file and its -wal side file are copied,
    // and the newest events live only in the -wal file.
    let live = dir.path().join("live");
    std::fs::create_dir_all(&live).unwrap();
    let input = live.join("books-live.ledgerly");
    std::fs::copy(store.path(), &input).unwrap();
    std::fs::copy(with_wal(store.path()), with_wal(&input)).unwrap();
    // Derived: populated() has 3 events, all visible through the -wal file.
    let conn =
        Connection::open_with_flags(&input, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let visible: i64 = conn
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(visible, 3);
    drop(conn);
    let target = dir.path().join("restored.ledgerly");
    let result = restore_from(&input, &target);
    assert!(result.is_err(), "{:?}", result.as_ref().err());
    assert!(!target.exists(), "failed restore must remove its target");
}
