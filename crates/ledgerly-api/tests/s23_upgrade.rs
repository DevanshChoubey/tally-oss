//! S23: a company file made before round s22 gets its predefined ledgers when it is opened, after
//! a backup, as one audited change.
//!
//! The fixture `tests/fixtures/pre-s22.ledgerly` is `observed`: it was written by the build at
//! commit 0e1a5605 (the tree before round s22 made Cash and Profit & Loss A/c predefined), on
//! 2026-11-27, with `Api::create` at 2026-11-01 00:00 UTC, then a ledger "Cash" under
//! Cash-in-Hand (id 1) and "Rent" under Indirect Expenses (id 2) made by hand, then one payment
//! of Rs 1,000 dated 2026-10-05 (Dr Rent, Cr Cash), copied with `backup_to`. It is not written by
//! the code under test.
//!
//! Expected values (`derived`): TallyPrime provides Cash and Profit & Loss A/c as predefined
//! ledgers (<https://help.tallysolutions.com/ledgers-in-tallyprime/>, read 2026-11-27). The
//! hand-made Cash sits where the predefined one belongs, so it takes the role and keeps id 1 and
//! its voucher; Profit & Loss A/c is created with the next id, 3 (the highest ledger id is 2),
//! under Primary.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{Api, Books, PredefinedStep};
use ledgerly_core::{EventSeq, LedgerId, Outcome};
use ledgerly_model::ReservedLedger;
use ledgerly_store::{Event, SqliteStore, Store};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/pre-s22.ledgerly"
);

fn copy_fixture(dir: &Path) -> PathBuf {
    let path = dir.join("old-traders.ledgerly");
    std::fs::copy(FIXTURE, &path).unwrap();
    path
}

fn books(api: &Api) -> Books {
    match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

fn event_count(path: &Path) -> usize {
    SqliteStore::open(path)
        .unwrap()
        .events(EventSeq(1), 10_000)
        .unwrap()
        .len()
}

#[test]
fn the_fixture_is_an_old_company_without_predefined_ledgers() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture(dir.path());
    let api = Api::open(&path).unwrap();
    let b = books(&api);
    assert_eq!(b.masters.ledgers.len(), 2);
    assert!(b.masters.ledgers.values().all(|l| l.reserved.is_none()));
    assert_eq!(b.vouchers.len(), 1);
}

#[test]
fn plain_open_never_upgrades() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture(dir.path());
    let before = event_count(&path);
    drop(Api::open(&path).unwrap());
    assert_eq!(event_count(&path), before);
    assert!(!dir.path().join("Backups").exists());
}

#[test]
fn opening_an_old_company_backs_it_up_then_adds_the_predefined_ledgers() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture(dir.path());
    let old_books = {
        let api = Api::open(&path).unwrap();
        books(&api)
    };
    let before = event_count(&path);
    let (api, report) = Api::open_and_upgrade(&path).unwrap();
    let report = report.unwrap();

    // The backup holds the company as it was.
    assert_eq!(report.backup.parent().unwrap(), dir.path().join("Backups"));
    assert_eq!(event_count(&report.backup), before);
    let from_backup = Api::open(&report.backup).unwrap();
    assert_eq!(books(&from_backup), old_books);

    assert_eq!(
        report.ledgers,
        vec![
            PredefinedStep::Adopted {
                role: ReservedLedger::Cash,
                id: LedgerId(1),
                name: "Cash".to_owned()
            },
            PredefinedStep::Created {
                role: ReservedLedger::ProfitAndLoss,
                id: LedgerId(3),
                name: "Profit & Loss A/c".to_owned()
            },
        ]
    );
    let b = books(&api);
    let cash = &b.masters.ledgers[&LedgerId(1)];
    assert_eq!(cash.reserved, Some(ReservedLedger::Cash));
    assert_eq!(
        cash.opening,
        old_books.masters.ledgers[&LedgerId(1)].opening
    );
    let pl = &b.masters.ledgers[&LedgerId(3)];
    assert_eq!(pl.reserved, Some(ReservedLedger::ProfitAndLoss));
    assert_eq!(pl.group, None);
    // The voucher and its postings are untouched.
    assert_eq!(b.vouchers, old_books.vouchers);
    assert_eq!(b.postings, old_books.postings);
    // The audit chain still verifies, and the change says why it was made.
    assert!(api.verify().unwrap().is_ok());
    let records = SqliteStore::open(&path)
        .unwrap()
        .events(EventSeq(1), 10_000)
        .unwrap();
    let upgrade: Vec<_> = records.iter().skip(before).collect();
    assert_eq!(upgrade.len(), 2, "{upgrade:?}");
    assert!(matches!(upgrade[0].event, Event::MasterAltered { .. }));
    assert!(matches!(upgrade[1].event, Event::MasterCreated(_)));
    for r in &upgrade {
        let reason = r.meta.reason.as_deref().unwrap();
        assert!(
            reason.starts_with("upgrade on open by Ledgerly"),
            "{reason}"
        );
        assert!(reason.contains("backup taken first"), "{reason}");
    }
}

#[test]
fn a_second_open_does_not_upgrade_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture(dir.path());
    let (api, first) = Api::open_and_upgrade(&path).unwrap();
    assert!(first.is_some());
    drop(api);
    let after = event_count(&path);
    let (_api, second) = Api::open_and_upgrade(&path).unwrap();
    assert_eq!(second, None);
    assert_eq!(event_count(&path), after);
}

#[test]
fn a_new_company_needs_no_upgrade() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.ledgerly");
    let api = match Api::create_company(
        &path,
        &books(&Api::open(&copy_fixture(dir.path())).unwrap()).company,
        ledgerly_core::Clock::System,
    )
    .unwrap()
    {
        Outcome::Value(api) => api,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    drop(api);
    let (_api, report) = Api::open_and_upgrade(&path).unwrap();
    assert_eq!(report, None);
    assert!(!dir.path().join("Backups").exists());
}
