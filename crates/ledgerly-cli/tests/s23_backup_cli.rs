//! S23: `ledgerly backup` and `ledgerly restore`. Each test runs the built binary.
//!
//! Provenance (`derived`): the exit codes are the ones `main.rs` documents (0 passed, 1 a check
//! failed, 2 the command could not run); a broken audit chain is a failed check (as for
//! `ledgerly verify`). Backups go to the stored backup folder, else `Backups` beside the company
//! file (`ledgerly_api::Api::backup_folder`); a restore never replaces an existing file.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ledgerly_api::{
    Address, Api, BackupFrequency, BackupSettings, Company, EntityKind, Features,
    NegativeStockPolicy,
};
use ledgerly_core::{Clock, CompanyId, Date, Month, OffsetDateTime, StateCode};

fn company() -> Company {
    let april_first = Date::from_calendar_date(2026, Month::April, 1).unwrap();
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: april_first,
        books_begin: april_first,
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn fixture(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("s23-backup-cli-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("company.ledgerly");
    Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    path
}

fn ledgerly(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ledgerly"))
        .args(args)
        .output()
        .unwrap()
}

fn os(s: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(s)
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn backups_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    v
}

#[test]
fn backup_writes_into_the_backups_folder_beside_the_file_and_prints_the_path() {
    let file = fixture("default");
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let folder = file.parent().unwrap().join("Backups");
    let written = backups_in(&folder);
    assert_eq!(written.len(), 1);
    assert!(
        stdout(&out).contains(&written[0].display().to_string()),
        "{out:?}"
    );
}

#[test]
fn backup_twice_in_a_row_never_overwrites() {
    let file = fixture("twice");
    for _ in 0..2 {
        let out = ledgerly(&[os("backup"), file.as_os_str()]);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
    }
    assert_eq!(backups_in(&file.parent().unwrap().join("Backups")).len(), 2);
}

#[test]
fn backup_uses_the_stored_folder_and_to_overrides_it() {
    let file = fixture("stored");
    let stored = file.parent().unwrap().join("stored");
    let mut api = Api::open(&file).unwrap();
    api.set_backup_settings(&BackupSettings {
        folder: Some(stored.clone()),
        frequency: BackupFrequency::Off,
        keep: 7,
    })
    .unwrap();
    drop(api);
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(backups_in(&stored).len(), 1);
    let other = file.parent().unwrap().join("other");
    let out = ledgerly(&[
        os("backup"),
        file.as_os_str(),
        os("--to"),
        other.as_os_str(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(backups_in(&other).len(), 1);
}

#[test]
fn backup_if_due_takes_a_scheduled_backup_only_when_a_schedule_is_due() {
    let file = fixture("due");
    let out = ledgerly(&[os("backup"), file.as_os_str(), os("--if-due")]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        stdout(&out).contains("No scheduled backup is due"),
        "{out:?}"
    );
    assert!(!file.parent().unwrap().join("Backups").exists());
    let mut api = Api::open(&file).unwrap();
    api.set_backup_settings(&BackupSettings {
        folder: None,
        frequency: BackupFrequency::Daily,
        keep: 7,
    })
    .unwrap();
    drop(api);
    let out = ledgerly(&[os("backup"), file.as_os_str(), os("--if-due")]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let written = backups_in(&file.parent().unwrap().join("Backups"));
    assert_eq!(written.len(), 1);
    let name = written[0].file_name().unwrap().to_str().unwrap().to_owned();
    assert!(name.starts_with("company-scheduled-"), "{name}");
}

#[test]
fn backup_of_a_missing_file_could_not_run() {
    let file = fixture("missing").with_file_name("nothing-here.ledgerly");
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
}

#[test]
fn restore_writes_a_new_file_and_prints_the_chain_check() {
    let file = fixture("restore");
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(0));
    let backup = backups_in(&file.parent().unwrap().join("Backups")).remove(0);
    let target = file.with_file_name("restored.ledgerly");
    let out = ledgerly(&[os("restore"), backup.as_os_str(), target.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(target.is_file());
    assert!(stdout(&out).contains("Restored to"), "{out:?}");
    assert!(Api::open(&target).unwrap().verify().unwrap().is_ok());
}

#[test]
fn restore_onto_an_existing_file_could_not_run_and_leaves_it() {
    let file = fixture("existing");
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(0));
    let backup = backups_in(&file.parent().unwrap().join("Backups")).remove(0);
    let before = std::fs::read(&file).unwrap();
    let out = ledgerly(&[os("restore"), backup.as_os_str(), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn restore_of_a_backup_with_a_broken_chain_is_a_failed_check() {
    let file = fixture("broken");
    let out = ledgerly(&[os("backup"), file.as_os_str()]);
    assert_eq!(out.status.code(), Some(0));
    let backup = backups_in(&file.parent().unwrap().join("Backups")).remove(0);
    let conn = rusqlite::Connection::open(&backup).unwrap();
    let mut event: Vec<u8> = conn
        .query_row("SELECT event FROM events WHERE seq = 1", [], |r| r.get(0))
        .unwrap();
    event[0] ^= 0x01;
    conn.execute("UPDATE events SET event = ?1 WHERE seq = 1", [event])
        .unwrap();
    drop(conn);
    let target = file.with_file_name("restored.ledgerly");
    let out = ledgerly(&[os("restore"), backup.as_os_str(), target.as_os_str()]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(!target.exists());
}
