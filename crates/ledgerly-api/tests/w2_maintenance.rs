//! Card W2 tests: maintenance over real company files (spec 6.6): verify, tamper detection, backup
//! and restore, rebuild check, and the engine-only voucher check.
//!
//! Provenance. Expected values are `derived`:
//! - Verify: spec 6.6 ("`ledgerly verify` prints a pass/fail certificate") and the certificate
//!   wording of `ledgerly_store::verify` (`Audit chain integrity: PASS` / `FAIL`).
//! - Tamper: the stored `event` column of record `n` is covered by the hash of record `n`
//!   (store commit definition, card S2/S3). Rewriting one name inside it breaks that hash, so the
//!   verifier must report a problem. The name `Capital Account` is the reserved group name written
//!   by the seed in `ledgerly_model::seed_groups` (a fixed fixture string, not an engine output).
//! - Rebuild: the projection tables are never written by any writer (store `backup` module doc),
//!   so the comparison reports `NotMaintained`, which is not a match.
//! - Trial balance: Rs 1,000 = 100,000 paise; Dr Rent, Cr Cash (derived by hand, as in W1).
//! - Backup and restore: a restored copy must give the same reports as the original; the check is
//!   a round-trip property, and the expected trial balance is the derived one above.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{
    Api, ApiError, Command, CommandResult, RebuildOutcome, Report, ReportParams, Voucher,
    verify_file,
};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{Cell, ColumnSpec, Detail, ReportFilter, ReportKind};
use ledgerly_store::backup::restore_from;
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);

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

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn payment_type() -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group,
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// A payment of `rupees`: Dr Rent, Cr Cash.
fn payment(id: u64, on: Date, rupees: i64) -> Voucher {
    let paise = Paise::new(rupees * 100);
    Voucher {
        id: VoucherId(id),
        type_id: payment_type(),
        number: None,
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
            line(RENT, SignedAmount::dr(paise)),
            line(CASH, SignedAmount::cr(paise)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: std::collections::BTreeMap::new(),
    }
}

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with the Cash and Rent ledgers, and no vouchers.
fn setup(dir: &Path) -> (PathBuf, Api) {
    let path = dir.join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    value(api.execute(Command::CreateLedger(ledger(
        CASH,
        "Cash",
        reserved(ReservedGroup::CashInHand),
    ))));
    value(api.execute(Command::CreateLedger(ledger(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
    ))));
    (path, api)
}

fn trial_balance_params() -> ReportParams {
    ReportParams {
        kind: ReportKind::TrialBalance,
        period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
        as_of: date!(2026 - 12 - 31),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn trial_balance(api: &Api) -> Report {
    api.report(&trial_balance_params())
        .unwrap()
        .value()
        .unwrap()
}

/// Rewrites the first occurrence of `from` in the file to `to` (same length). The text lives in
/// the stored event bytes, so this changes one event without touching the chain columns.
fn rewrite_bytes_in_file(path: &Path, from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let mut bytes = std::fs::read(path).unwrap();
    let position = bytes
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .expect("the fixture text is stored in the company file");
    bytes
        .get_mut(position..position + to.len())
        .unwrap()
        .copy_from_slice(to.as_bytes());
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn fresh_company_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, api) = setup(dir.path());
    let report = api.verify().unwrap();
    // Derived: the certificate wording of ledgerly_store::verify.
    assert!(report.is_ok(), "{:?}", report.problems);
    assert!(report.events_checked > 0);
    assert!(report.head.is_some());
    assert!(
        report
            .certificate()
            .starts_with("Audit chain integrity: PASS\n")
    );
}

#[test]
fn tampered_file_fails_verify() {
    let dir = tempfile::tempdir().unwrap();
    let (path, api) = setup(dir.path());
    drop(api);
    rewrite_bytes_in_file(&path, "Capital Account", "Capital Accounx");

    // The file-level check does not need the books, so it still answers.
    let report = verify_file(&path).unwrap();
    assert!(!report.is_ok());
    assert!(report.first_broken_seq().is_some());
    assert!(
        report
            .certificate()
            .starts_with("Audit chain integrity: FAIL\n")
    );
}

#[test]
fn backup_then_restore_gives_identical_reports() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, mut api) = setup(dir.path());
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    let before = trial_balance(&api);
    // Derived: Rs 1,000 = 100,000 paise. Dr Rent 100,000; Cr Cash 100,000.
    assert_eq!(
        before.totals,
        vec![
            Cell::Amount(SignedAmount::dr(Paise::new(100_000))),
            Cell::Amount(SignedAmount::cr(Paise::new(100_000)))
        ]
    );

    let backup_dir = dir.path().join("backups");
    let backup = api.backup_to(&backup_dir).unwrap();
    assert_eq!(backup.parent(), Some(backup_dir.as_path()));
    assert!(backup.is_file());

    let target = dir.path().join("restored.ledgerly");
    let restored = restore_from(&backup, &target).unwrap();
    drop(restored);
    let reopened = Api::open(&target).unwrap();
    assert_eq!(trial_balance(&reopened), before);
    assert!(reopened.verify().unwrap().is_ok());
}

#[test]
fn backup_refuses_to_overwrite_and_leaves_source_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, api) = setup(dir.path());
    let before = api.verify().unwrap().events_checked;
    let backup_dir = dir.path().join("backups");
    let first = api.backup_to(&backup_dir).unwrap();
    assert!(first.is_file());
    // A second backup in the same directory gets a new name, or fails; it never overwrites.
    match api.backup_to(&backup_dir) {
        Ok(second_path) => assert_ne!(second_path, first),
        Err(ApiError::Store(_)) => {}
        Err(other) => panic!("unexpected error {other:?}"),
    }
    assert!(first.is_file());
    assert_eq!(api.verify().unwrap().events_checked, before);
}

#[test]
fn rebuild_check_is_not_a_match_when_projections_are_not_maintained() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, mut api) = setup(dir.path());
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    let report = api.rebuild_check().unwrap();
    // Derived from the store's rebuild contract: the projection tables are never written, so a
    // log with events and empty tables is NotMaintained, never Match and never Mismatch.
    assert_eq!(report.outcome, RebuildOutcome::NotMaintained);
    assert!(!report.is_match());
    assert!(report.events_replayed > 0);
}

#[test]
fn check_voucher_reports_engine_events_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, api) = setup(dir.path());
    let before = api.verify().unwrap().events_checked;
    let tb_before = trial_balance(&api);
    let check = api
        .check_voucher(payment(1, date!(2026 - 06 - 10), 1000))
        .unwrap();
    match check {
        Outcome::Value(check) => assert!(check.events_that_would_be_written > 0),
        Outcome::Unsupported(u) => panic!("expected a check, got unsupported: {u:?}"),
    }
    assert_eq!(api.verify().unwrap().events_checked, before);
    assert_eq!(trial_balance(&api), tb_before);
}

#[test]
fn check_voucher_refuses_an_unknown_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, api) = setup(dir.path());
    let mut draft = payment(1, date!(2026 - 06 - 10), 1000);
    draft.ledger_lines = vec![
        line(LedgerId(999), SignedAmount::dr(Paise::new(100_000))),
        line(CASH, SignedAmount::cr(Paise::new(100_000))),
    ];
    // Derived: a line that names a ledger which does not exist is refused by the engine.
    assert!(matches!(api.check_voucher(draft), Err(ApiError::Engine(_))));
}
