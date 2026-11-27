//! S23: the stored backup folder and schedule, backing up now, the scheduled backup with
//! rotation, and restore with the audit-chain check.
//!
//! Provenance (`derived`):
//! - TallyPrime help, "Local Backup" (<https://help.tallysolutions.com/local-backup/>, read
//!   2026-11-27): a backup goes to the "Backup Data Path"; a schedule's "Backup Frequency" is
//!   Daily by default, or Weekly or Monthly; "No. of Restorable Versions" is 7 by default and
//!   from 1 to 366.
//! - Due times are counted by hand from the fixed clocks below: daily is one day after the last
//!   scheduled backup, weekly seven days, monthly the same day of the next month.
//! - TallyPrime help, "Import Data" (<https://help.tallysolutions.com/import-data-from-xml-or-json/>,
//!   read 2026-11-27): "Select Yes to take a backup of the company data before importing".
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{
    Api, ApiError, BackupFrequency, BackupKind, BackupSettings, DEFAULT_KEEP, MasterBehaviour,
    XmlImport, restore_backup,
};
use ledgerly_core::{Clock, CompanyId, EventSeq, Outcome, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_store::StoreError;
use time::OffsetDateTime;
use time::macros::{date, datetime};

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn at(when: OffsetDateTime) -> Clock {
    Clock::Fixed(when)
}

fn new_company(dir: &Path) -> Api {
    match Api::create_company(
        &dir.join("traders.ledgerly"),
        &company(),
        at(datetime!(2026-11-21 10:00 UTC)),
    )
    .unwrap()
    {
        Outcome::Value(api) => api,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_company_with_no_stored_settings_has_the_default_folder_and_no_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let api = new_company(dir.path());
    let s = api.backup_settings().unwrap();
    assert_eq!(s.folder, None);
    assert_eq!(s.frequency, BackupFrequency::Off);
    assert_eq!(s.keep, DEFAULT_KEEP);
    assert_eq!(DEFAULT_KEEP, 7);
    assert_eq!(api.backup_folder().unwrap(), dir.path().join("Backups"));
    assert!(!api.scheduled_backup_due().unwrap());
}

#[test]
fn the_settings_are_stored_in_the_company_file_and_read_back_after_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    let folder = dir.path().join("safe place");
    let settings = BackupSettings {
        folder: Some(folder.clone()),
        frequency: BackupFrequency::Weekly,
        keep: 12,
    };
    api.set_backup_settings(&settings).unwrap();
    let path = dir.path().join("traders.ledgerly");
    drop(api);
    let api = Api::open(&path).unwrap();
    assert_eq!(api.backup_settings().unwrap(), settings);
    assert_eq!(api.backup_folder().unwrap(), folder);
}

#[test]
fn a_number_kept_outside_1_to_366_is_refused_and_nothing_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    for keep in [0, 367] {
        let out = api.set_backup_settings(&BackupSettings {
            folder: None,
            frequency: BackupFrequency::Daily,
            keep,
        });
        assert!(
            matches!(out, Err(ApiError::BackupSetting { .. })),
            "{keep}: {out:?}"
        );
    }
    assert_eq!(api.backup_settings().unwrap(), BackupSettings::default());
    for keep in [1, 366] {
        api.set_backup_settings(&BackupSettings {
            folder: None,
            frequency: BackupFrequency::Daily,
            keep,
        })
        .unwrap();
        assert_eq!(api.backup_settings().unwrap().keep, keep);
    }
}

#[test]
fn back_up_now_writes_into_the_stored_folder_and_twice_in_a_second_gives_two_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    let folder = dir.path().join("elsewhere");
    api.set_backup_settings(&BackupSettings {
        folder: Some(folder.clone()),
        frequency: BackupFrequency::Off,
        keep: DEFAULT_KEEP,
    })
    .unwrap();
    api.set_clock(at(datetime!(2026-11-21 10:30 UTC)));
    let first = api.back_up_now().unwrap();
    let second = api.back_up_now().unwrap();
    assert_eq!(first.parent().unwrap(), folder);
    assert_eq!(
        names(&[first, second]),
        vec![
            "traders-backup-20261121T103000Z.ledgerly",
            "traders-backup-20261121T103000Z_1.ledgerly"
        ]
    );
    let listed = api.backups().unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|b| b.kind == BackupKind::Manual));
}

#[test]
fn the_daily_schedule_runs_once_a_day_and_keeps_the_number_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    api.set_backup_settings(&BackupSettings {
        folder: None,
        frequency: BackupFrequency::Daily,
        keep: 2,
    })
    .unwrap();
    let manual = api.back_up_now().unwrap();
    // No scheduled backup yet: due at once.
    api.set_clock(at(datetime!(2026-11-21 11:00 UTC)));
    let first = api.run_scheduled_backup().unwrap().unwrap();
    assert_eq!(
        names(std::slice::from_ref(&first.path)),
        vec!["traders-scheduled-20261121T110000Z.ledgerly"]
    );
    // Derived: 23 h 59 min later is not yet a day.
    api.set_clock(at(datetime!(2026-11-22 10:59 UTC)));
    assert!(api.run_scheduled_backup().unwrap().is_none());
    // One day later: due.
    api.set_clock(at(datetime!(2026-11-22 11:00 UTC)));
    let second = api.run_scheduled_backup().unwrap().unwrap();
    assert_eq!(second.removed, Vec::<PathBuf>::new());
    api.set_clock(at(datetime!(2026-11-23 11:00 UTC)));
    let third = api.run_scheduled_backup().unwrap().unwrap();
    // Keep 2: the first scheduled backup goes; the manual one stays.
    assert_eq!(third.removed, vec![first.path.clone()]);
    assert!(!first.path.exists());
    assert!(manual.exists());
}

#[test]
fn weekly_and_monthly_schedules_are_due_after_seven_days_and_a_calendar_month() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    api.set_backup_settings(&BackupSettings {
        folder: None,
        frequency: BackupFrequency::Weekly,
        keep: DEFAULT_KEEP,
    })
    .unwrap();
    api.set_clock(at(datetime!(2027-01-31 09:00 UTC)));
    api.run_scheduled_backup().unwrap().unwrap();
    api.set_clock(at(datetime!(2027-02-06 09:00 UTC)));
    assert!(!api.scheduled_backup_due().unwrap(), "6 days");
    api.set_clock(at(datetime!(2027-02-07 09:00 UTC)));
    assert!(api.scheduled_backup_due().unwrap(), "7 days");
    api.set_backup_settings(&BackupSettings {
        folder: None,
        frequency: BackupFrequency::Monthly,
        keep: DEFAULT_KEEP,
    })
    .unwrap();
    // Derived: one month after 31 January 2027 is 28 February 2027 (2027 is not a leap year).
    api.set_clock(at(datetime!(2027-02-28 08:59 UTC)));
    assert!(!api.scheduled_backup_due().unwrap());
    api.set_clock(at(datetime!(2027-02-28 09:00 UTC)));
    assert!(api.scheduled_backup_due().unwrap());
}

#[test]
fn restore_writes_a_new_company_file_that_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let api = new_company(dir.path());
    let backup = api.back_up_now().unwrap();
    let target = dir.path().join("restored.ledgerly");
    let restored = restore_backup(&backup, &target).unwrap();
    assert!(restored.verify.is_ok());
    let reopened = Api::open(&target).unwrap();
    assert_eq!(reopened.books().unwrap(), api.books().unwrap());
}

#[test]
fn restore_never_replaces_an_existing_company_file() {
    let dir = tempfile::tempdir().unwrap();
    let api = new_company(dir.path());
    let backup = api.back_up_now().unwrap();
    let existing = dir.path().join("traders.ledgerly");
    let before = std::fs::read(&existing).unwrap();
    let out = restore_backup(&backup, &existing);
    assert!(
        matches!(&out, Err(ApiError::Store(StoreError::Io { source, .. })) if source.kind() == std::io::ErrorKind::AlreadyExists),
        "{out:?}"
    );
    assert_eq!(std::fs::read(&existing).unwrap(), before);
}

#[test]
fn restore_of_a_backup_whose_chain_was_altered_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let api = new_company(dir.path());
    let backup = api.back_up_now().unwrap();
    // Hand change: flip one bit of stored event 1, leaving its hash column alone.
    let conn = rusqlite::Connection::open(&backup).unwrap();
    let mut event: Vec<u8> = conn
        .query_row("SELECT event FROM events WHERE seq = 1", [], |r| r.get(0))
        .unwrap();
    event[0] ^= 0x01;
    conn.execute("UPDATE events SET event = ?1 WHERE seq = 1", [event])
        .unwrap();
    drop(conn);
    let target = dir.path().join("restored.ledgerly");
    let out = restore_backup(&backup, &target);
    assert!(
        matches!(
            out,
            Err(ApiError::Store(StoreError::ChainBroken(EventSeq(1))))
        ),
        "{out:?}"
    );
    assert!(!target.exists());
}

#[test]
fn the_backup_before_an_import_goes_to_the_stored_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(dir.path());
    let folder = dir.path().join("stored");
    api.set_backup_settings(&BackupSettings {
        folder: Some(folder.clone()),
        frequency: BackupFrequency::Off,
        keep: DEFAULT_KEEP,
    })
    .unwrap();
    let xml = dir.path().join("masters.xml");
    std::fs::write(
        &xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ENVELOPE><HEADER><TALLYREQUEST>Import Data\
         </TALLYREQUEST></HEADER><BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>All Masters\
         </REPORTNAME></REQUESTDESC><REQUESTDATA><TALLYMESSAGE><LEDGER NAME=\"Rent\" \
         ACTION=\"Create\"><NAME>Rent</NAME><PARENT>Indirect Expenses</PARENT></LEDGER>\
         </TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>",
    )
    .unwrap();
    let summary = match api
        .back_up_and_import_tally_xml_file(
            XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
            &xml,
        )
        .unwrap()
    {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    let backup = summary.backup.unwrap();
    assert_eq!(backup.parent().unwrap(), folder);
    assert!(backup.is_file());
}
