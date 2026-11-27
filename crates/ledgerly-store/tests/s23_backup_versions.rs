//! S23 tests: backups whose names never collide, the list of backups in a folder, rotation of
//! scheduled backups, and restore with the audit-chain check.
//!
//! Provenance (`derived`):
//! - TallyPrime help, "Local Backup" (<https://help.tallysolutions.com/local-backup/>, read
//!   2026-11-27): a second backup of the same company, when the user declines to overwrite, is
//!   saved with `_1`, `_2` and so on after the name; a scheduled backup has its own name
//!   (`TSDBK...`, against `TDBK...` for a manual one); "No. of Restorable Versions" (default 7,
//!   range 1 to 366) is how many scheduled backups are kept for restoring. So: the first backup
//!   keeps its name, the second in the same second gets `_1`, the third `_2`; scheduled backups
//!   are named `<stem>-scheduled-<stamp>.ledgerly`; rotation keeps the newest `keep` scheduled
//!   backups of this company and nothing else is touched.
//! - `OffsetDateTime::UNIX_EPOCH` is 1970-01-01 00:00:00 UTC by definition, so the stamp is
//!   `19700101T000000Z`; epoch + 86 400 s is 1970-01-02.
//! - Restore: the restored file must pass the audit-chain verifier (card S3); a backup whose
//!   stored event was altered (one byte flipped, hash column left alone) fails it, and the
//!   restored file is removed.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{
    Clock, CompanyId, Date, EventSeq, GroupId, LedgerId, Month, OffsetDateTime, SignedAmount,
    StateCode, UserId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, Group, LedgerLine, MasterSnapshot, NegativeStockPolicy,
    Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::backup::{
    BackupKind, list_backups, prune_scheduled, restore_verified, write_backup,
};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};
use rusqlite::Connection;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(7),
        name: "Example Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: Date::from_calendar_date(2026, Month::April, 1).unwrap(),
        books_begin: Date::from_calendar_date(2026, Month::April, 1).unwrap(),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn epoch() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn day(n: i64) -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH + time::Duration::days(n))
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: None,
        app_version: "0.0.0-test".to_owned(),
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

fn voucher(id: u64) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(format!("V{id}")),
        series: None,
        date: Date::from_calendar_date(2026, Month::May, 10).unwrap(),
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
            line(10, SignedAmount::dr(ledgerly_core::Paise::new(1_000))),
            line(20, SignedAmount::cr(ledgerly_core::Paise::new(1_000))),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn populated(dir: &Path) -> SqliteStore {
    let mut store = SqliteStore::create(&dir.join("books.ledgerly"), &company(), epoch()).unwrap();
    store
        .commit(
            &[
                Event::MasterCreated(MasterSnapshot::Group(Group {
                    ineligible_itc: None,
                    id: GroupId(1),
                    name: "Sundry Debtors".to_owned(),
                    aliases: Vec::new(),
                    parent: None,
                    reserved: None,
                    nature: None,
                    affects_gross_profit: false,
                    tally: None,
                })),
                Event::VoucherPosted(voucher(1)),
            ],
            &meta(),
        )
        .unwrap();
    store
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_str().unwrap().to_owned()
}

#[test]
fn backups_in_the_same_second_get_their_own_names() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let first = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let second = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let third = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    assert_eq!(name(&first), "books-backup-19700101T000000Z.ledgerly");
    assert_eq!(name(&second), "books-backup-19700101T000000Z_1.ledgerly");
    assert_eq!(name(&third), "books-backup-19700101T000000Z_2.ledgerly");
    // Each is a whole, separate copy.
    for path in [&first, &second, &third] {
        assert!(path.is_file());
    }
}

#[test]
fn a_scheduled_backup_has_its_own_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let path = write_backup(&store, &to, epoch(), BackupKind::Scheduled).unwrap();
    assert_eq!(name(&path), "books-scheduled-19700101T000000Z.ledgerly");
}

#[test]
fn the_list_shows_this_companys_backups_newest_first_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let a = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let b = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let c = write_backup(&store, &to, day(1), BackupKind::Scheduled).unwrap();
    // Files that only look alike: another company's backup, a half-written one, a stray file.
    std::fs::write(to.join("books-2-backup-19700105T000000Z.ledgerly"), b"x").unwrap();
    std::fs::write(
        to.join("books-backup-19700106T000000Z.ledgerly.partial"),
        b"x",
    )
    .unwrap();
    std::fs::write(to.join("books-backup-notatime.ledgerly"), b"x").unwrap();
    let list = list_backups(&to, store.path()).unwrap();
    let paths: Vec<_> = list.iter().map(|f| f.path.clone()).collect();
    assert_eq!(paths, vec![c, b, a]);
    assert_eq!(list[0].kind, BackupKind::Scheduled);
    assert_eq!(list[1].kind, BackupKind::Manual);
    assert_eq!(
        list[0].taken_at,
        OffsetDateTime::UNIX_EPOCH + time::Duration::days(1)
    );
}

#[test]
fn a_missing_folder_lists_no_backups() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let list = list_backups(&dir.path().join("none"), store.path()).unwrap();
    assert_eq!(list, Vec::new());
}

#[test]
fn rotation_keeps_the_newest_scheduled_backups_and_never_a_manual_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let manual = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let mut scheduled = Vec::new();
    for n in 1..=4 {
        scheduled.push(write_backup(&store, &to, day(n), BackupKind::Scheduled).unwrap());
    }
    let removed = prune_scheduled(&to, store.path(), 2).unwrap();
    // Derived: four scheduled backups on days 1..4, keep 2, so days 1 and 2 go, oldest first.
    assert_eq!(removed, vec![scheduled[0].clone(), scheduled[1].clone()]);
    assert!(!scheduled[0].exists() && !scheduled[1].exists());
    assert!(scheduled[2].exists() && scheduled[3].exists());
    assert!(manual.exists(), "a manual backup is never rotated");
}

#[test]
fn rotation_refuses_to_keep_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let only = write_backup(&store, &to, epoch(), BackupKind::Scheduled).unwrap();
    let result = prune_scheduled(&to, store.path(), 0);
    assert!(result.is_err(), "{result:?}");
    assert!(only.exists());
}

#[test]
fn a_verified_restore_gives_the_chain_report() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let backup = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    let target = dir.path().join("restored.ledgerly");
    let (restored, report) = restore_verified(&backup, &target).unwrap();
    assert!(report.is_ok());
    assert_eq!(
        restored.events(EventSeq(1), 1_000).unwrap(),
        store.events(EventSeq(1), 1_000).unwrap()
    );
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
fn a_restore_whose_audit_chain_does_not_verify_is_refused_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let store = populated(dir.path());
    let to = dir.path().join("backups");
    let backup = write_backup(&store, &to, epoch(), BackupKind::Manual).unwrap();
    drop(store);
    // Hand change after the backup was written: event 2 (the voucher) has one byte flipped.
    flip_first_byte_of_event(&backup, 2);
    let target = dir.path().join("restored.ledgerly");
    let result = restore_verified(&backup, &target);
    assert!(
        matches!(&result, Err(StoreError::ChainBroken(EventSeq(2)))),
        "{:?}",
        result.as_ref().err()
    );
    assert!(!target.exists(), "a refused restore leaves no file");
}
