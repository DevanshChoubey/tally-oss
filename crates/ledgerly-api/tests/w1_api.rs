//! Card W1 tests: the facade over real company files (spec sections 4, 5 and 6.2).
//!
//! Expected values are `derived` by hand from the posting rules (a payment debits the expense and
//! credits cash) and from the amounts written in each test. None is an output of the code under
//! test. Amounts are paise: Rs 1,000 is `100_000`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ledgerly_api::{Api, ApiError, Command, CommandResult, MatchedOn, SearchHit};
use ledgerly_core::{
    Clock, CompanyId, Date, EventSeq, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use ledgerly_store::{SqliteStore, Store};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);
const SHOP: LedgerId = LedgerId(3);

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

/// The id of a reserved group, from the seed the company is created with.
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

/// A payment of `rupees`: Dr Rent, Cr Cash. The number is left for the engine to allocate.
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
        extra: BTreeMap::new(),
    }
}

/// A company file with the Cash and Rent ledgers.
fn setup() -> (tempfile::TempDir, PathBuf, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    let cash = ledger(CASH, "Cash", reserved(ReservedGroup::CashInHand));
    let rent = ledger(RENT, "Rent", reserved(ReservedGroup::IndirectExpenses));
    value(api.execute(Command::CreateLedger(cash)));
    value(api.execute(Command::CreateLedger(rent)));
    (dir, path, api)
}

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn expect_locked(out: Result<Outcome<CommandResult>, ApiError>, lock: Date) {
    match out {
        Err(ApiError::Engine(EngineError::PeriodLocked { until })) => assert_eq!(until, lock),
        other => panic!("expected PeriodLocked through the facade, got {other:?}"),
    }
}

fn params(kind: ReportKind, as_of: Date) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn run_report(api: &Api, kind: ReportKind, as_of: Date) -> Report {
    api.report(&params(kind, as_of)).unwrap().value().unwrap()
}

fn trial_balance(api: &Api) -> Report {
    run_report(api, ReportKind::TrialBalance, date!(2026 - 12 - 31))
}

fn row_cells(report: &Report, label: &str) -> Option<Vec<Cell>> {
    report
        .rows
        .iter()
        .find(|row| row.label == label)
        .map(|row| row.cells.clone())
}

/// The number of audit records in the file at `path`, read through a second connection.
fn event_count(path: &Path) -> usize {
    let store = SqliteStore::open(path).unwrap();
    store.events(EventSeq(1), usize::MAX).unwrap().len()
}

#[test]
fn posted_voucher_shows_in_trial_balance() {
    let (_dir, _path, mut api) = setup();
    let result = value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    assert_eq!(result.posted.as_ref().map(|v| v.id), Some(VoucherId(1)));
    assert!(result.receipt.is_some());

    // Derived: Rs 1,000 = 100,000 paise. Dr Rent 100,000; Cr Cash 100,000; totals equal.
    let tb = trial_balance(&api);
    assert_eq!(
        row_cells(&tb, "Rent"),
        Some(vec![
            Cell::Amount(SignedAmount::dr(Paise::new(100_000))),
            Cell::Empty
        ])
    );
    assert_eq!(
        row_cells(&tb, "Cash"),
        Some(vec![
            Cell::Empty,
            Cell::Amount(SignedAmount::cr(Paise::new(100_000)))
        ])
    );
    assert_eq!(
        tb.totals,
        vec![
            Cell::Amount(SignedAmount::dr(Paise::new(100_000))),
            Cell::Amount(SignedAmount::cr(Paise::new(100_000)))
        ]
    );
}

#[test]
fn refused_command_leaves_report_unchanged() {
    let (_dir, _path, mut api) = setup();
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    let before = trial_balance(&api);

    // A ledger id that already exists is refused by the engine.
    let duplicate = ledger(CASH, "Cash again", reserved(ReservedGroup::CashInHand));
    match api.execute(Command::CreateLedger(duplicate)) {
        Err(ApiError::Engine(EngineError::Duplicate { .. })) => {}
        other => panic!("expected Duplicate from the engine, got {other:?}"),
    }

    // A voucher whose debit and credit differ is refused by the engine.
    let mut unbalanced = payment(2, date!(2026 - 06 - 11), 1000);
    unbalanced.ledger_lines[1] = line(CASH, SignedAmount::cr(Paise::new(90_000)));
    match api.execute(Command::PostVoucher(unbalanced)) {
        Err(ApiError::Engine(_)) => {}
        other => panic!("expected an engine refusal, got {other:?}"),
    }

    assert_eq!(trial_balance(&api), before);
}

#[test]
fn unsupported_command_commits_nothing() {
    let (_dir, path, mut api) = setup();
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    let before_report = trial_balance(&api);
    let before_events = event_count(&path);

    // The engine returns Unsupported for changing a reserved group's gross-profit flag.
    let mut group = reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::IndirectExpenses))
        .unwrap();
    group.affects_gross_profit = !group.affects_gross_profit;
    match api.execute(Command::AlterGroup(group)) {
        Ok(Outcome::Unsupported(u)) => assert_ne!(u.reason, ""),
        other => panic!("expected Unsupported, got {other:?}"),
    }

    assert_eq!(event_count(&path), before_events);
    assert_eq!(trial_balance(&api), before_report);
}

#[test]
fn period_lock_refuses_on_or_before_lock_and_allows_after() {
    let lock = date!(2026 - 06 - 30);
    let (_dir, _path, mut api) = setup();

    // Posted before the lock, so it is on or before the lock date when it is changed later.
    let early = value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 05 - 10),
        1000,
    ))))
    .posted
    .unwrap();
    value(api.execute(Command::LockPeriod(lock)));

    // Post on the lock date and before it: refused.
    expect_locked(
        api.execute(Command::PostVoucher(payment(2, lock, 500))),
        lock,
    );
    expect_locked(
        api.execute(Command::PostVoucher(payment(3, date!(2026 - 06 - 29), 500))),
        lock,
    );

    // Alter, cancel and delete a voucher dated before the lock: refused.
    let mut altered = early.clone();
    altered.narration = "edited".to_owned();
    expect_locked(api.execute(Command::AlterVoucher(altered)), lock);
    expect_locked(api.execute(Command::CancelVoucher(VoucherId(1))), lock);
    expect_locked(api.execute(Command::DeleteVoucher(VoucherId(1))), lock);

    // Dated after the lock: post, alter, cancel and delete all succeed.
    let after = value(api.execute(Command::PostVoucher(payment(4, date!(2026 - 07 - 01), 300))))
        .posted
        .unwrap();
    let mut after_altered = after.clone();
    after_altered.narration = "edited".to_owned();
    assert!(
        value(api.execute(Command::AlterVoucher(after_altered)))
            .receipt
            .is_some()
    );
    assert!(
        value(api.execute(Command::CancelVoucher(VoucherId(4))))
            .receipt
            .is_some()
    );
    value(api.execute(Command::PostVoucher(payment(5, date!(2026 - 07 - 02), 200))));
    assert!(
        value(api.execute(Command::DeleteVoucher(VoucherId(5))))
            .receipt
            .is_some()
    );

    // Derived: only the early Rs 1,000 payment stays live (the Rs 300 one is cancelled, the
    // Rs 200 one deleted, and the refused posts never happened). Rent Dr 100,000.
    let tb = trial_balance(&api);
    assert_eq!(
        row_cells(&tb, "Rent"),
        Some(vec![
            Cell::Amount(SignedAmount::dr(Paise::new(100_000))),
            Cell::Empty
        ])
    );
}

#[test]
fn reopening_gives_identical_report_output_and_keeps_the_lock() {
    let lock = date!(2026 - 06 - 30);
    let (_dir, path, mut api) = setup();
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    value(api.execute(Command::PostVoucher(payment(2, date!(2026 - 07 - 15), 250))));
    value(api.execute(Command::LockPeriod(lock)));
    let tb = trial_balance(&api);
    let day_book = run_report(&api, ReportKind::DayBook, date!(2026 - 12 - 31));
    drop(api);

    let mut reopened = Api::open(&path).unwrap();
    assert_eq!(trial_balance(&reopened), tb);
    assert_eq!(
        run_report(&reopened, ReportKind::DayBook, date!(2026 - 12 - 31)),
        day_book
    );

    // The lock is read back from the audit log.
    expect_locked(
        reopened.execute(Command::PostVoucher(payment(3, lock, 100))),
        lock,
    );
}

#[test]
fn search_finds_ledger_by_alias_and_voucher_by_number_and_amount() {
    let (_dir, _path, mut api) = setup();
    let mut shop = ledger(
        SHOP,
        "Shop Premises",
        reserved(ReservedGroup::IndirectExpenses),
    );
    shop.aliases = vec!["Godown Rent".to_owned()];
    value(api.execute(Command::CreateLedger(shop)));
    let posted = value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))))
    .posted
    .unwrap();

    // Case-insensitive alias match.
    let by_alias = api.search("GODOWN rent");
    assert!(by_alias.iter().any(|hit| matches!(
        hit,
        SearchHit::Ledger { id, matched, .. } if *id == SHOP && *matched == MatchedOn::Alias
    )));

    // The voucher is found by the number the engine gave it.
    let number = posted.number.clone().unwrap();
    let by_number = api.search(&number);
    assert!(by_number.iter().any(|hit| matches!(
        hit,
        SearchHit::Voucher { id, .. } if *id == VoucherId(1)
    )));

    // Derived: Rs 1,000 is 1000 rupees, which is the voucher's amount, and the query matches no
    // number or name.
    let by_amount = api.search("1000");
    assert!(by_amount.iter().any(|hit| matches!(
        hit,
        SearchHit::Voucher { id, matched, .. }
            if *id == VoucherId(1) && *matched == MatchedOn::Amount
    )));

    // Reports are searched by title, and the order is stable.
    let reports = api.search("trial");
    assert!(reports.iter().any(|hit| matches!(
        hit,
        SearchHit::Report {
            kind: ReportKind::TrialBalance,
            ..
        }
    )));
    assert_eq!(api.search("godown rent"), by_alias);
    assert_eq!(api.search("   "), Vec::<SearchHit>::new());
}
