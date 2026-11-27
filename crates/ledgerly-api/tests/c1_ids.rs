//! Card C1 (Wave 3): `Api::books`, and ids assigned to `Create*` commands that carry id 0.
//!
//! Provenance. `derived`: the seed has 28 reserved groups with ids 1..=28
//! (`ledgerly_model::seed_groups` header, from research R1), a new company file has no ledgers,
//! and the contract in `contracts.md` says id 0 gets "the next free id of that kind" (one more than
//! the largest id in use). Tally facts, read on 2026-11-21:
//! - Duplicate master names are refused: "Duplicate names are not allowed." in
//!   <https://help.tallysolutions.com/docs/te9rel55/Creating_Masters/Accounts_Info/Creating_Ledger_in_Normal_Mode.htm>
//! - Tally's date lock and back-dating controls govern vouchers ("transactions"); access to
//!   masters is a separate permission, so a locked period does not stop a master being created:
//!   <https://help.tallysolutions.com/article/Tally.ERP9/Data_Management/security-tally.htm>
//!   (search summary only; the opening-balance-under-lock case is `pending`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, ApiError, Command, CommandResult, CreatedId};
use ledgerly_core::{Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_engine::EngineError;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Group, Ledger, Nature, NegativeStockPolicy,
};
use time::OffsetDateTime;
use time::macros::date;

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

fn api() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let api = Api::create(
        &dir.path().join("c.ledgerly"),
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    (dir, api)
}

fn group(id: u64, name: &str) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: Some(Nature::Expense),
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
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

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out.unwrap() {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unsupported: {u:?}"),
    }
}

fn books(api: &Api) -> ledgerly_api::Books {
    match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("unsupported: {u:?}"),
    }
}

#[test]
fn a_new_company_reads_back_its_28_reserved_groups() {
    let (_dir, api) = api();
    // derived: seed_groups header, 28 reserved groups.
    assert_eq!(books(&api).masters.groups.len(), 28);
    assert_eq!(books(&api).company.name, "Test Traders");
}

#[test]
fn id_zero_gets_the_next_free_group_id_and_it_is_reported() {
    let (_dir, mut api) = api();
    // derived: ids 1..=28 are taken by the seed, so the first free id is 29, then 30.
    let first = value(api.execute(Command::CreateGroup(group(0, "Marketing"))));
    assert_eq!(first.created, Some(CreatedId::Group(GroupId(29))));
    let second = value(api.execute(Command::CreateGroup(group(0, "Travel"))));
    assert_eq!(second.created, Some(CreatedId::Group(GroupId(30))));
    let b = books(&api);
    assert_eq!(b.masters.groups[&GroupId(29)].name, "Marketing");
    assert_eq!(b.masters.groups[&GroupId(30)].name, "Travel");
}

#[test]
fn id_zero_gets_ledger_id_1_in_a_company_with_no_ledgers_then_2() {
    let (_dir, mut api) = api();
    // derived: no ledgers exist, so the first id is 1 (0 is "unassigned"); then 2.
    let under = GroupId(8); // IndirectExpenses, seed position 8
    let a = value(api.execute(Command::CreateLedger(ledger(0, "Rent", under))));
    assert_eq!(a.created, Some(CreatedId::Ledger(LedgerId(1))));
    let b = value(api.execute(Command::CreateLedger(ledger(0, "Wages", under))));
    assert_eq!(b.created, Some(CreatedId::Ledger(LedgerId(2))));
    let books = books(&api);
    assert_eq!(books.masters.ledgers[&LedgerId(1)].name, "Rent");
    assert_eq!(books.masters.ledgers[&LedgerId(2)].name, "Wages");
}

#[test]
fn a_non_zero_id_is_used_as_given() {
    let (_dir, mut api) = api();
    let r = value(api.execute(Command::CreateLedger(ledger(500, "Rent", GroupId(8)))));
    // derived: contracts.md, "A non-zero id is used as given".
    assert_eq!(r.created, Some(CreatedId::Ledger(LedgerId(500))));
    // and the next unassigned one follows the largest id in use.
    let n = value(api.execute(Command::CreateLedger(ledger(0, "Wages", GroupId(8)))));
    assert_eq!(n.created, Some(CreatedId::Ledger(LedgerId(501))));
}

#[test]
fn a_duplicate_name_is_refused_and_changes_nothing() {
    let (_dir, mut api) = api();
    value(api.execute(Command::CreateLedger(ledger(0, "Rent", GroupId(8)))));
    let before = books(&api);
    // Tally: "Duplicate names are not allowed." (page cited in the header).
    let out = api.execute(Command::CreateLedger(ledger(0, "Rent", GroupId(8))));
    assert!(
        matches!(out, Err(ApiError::Engine(EngineError::Duplicate { .. }))),
        "got {out:?}"
    );
    assert_eq!(books(&api), before);
}

#[test]
fn a_master_can_be_created_inside_a_locked_period() {
    let (_dir, mut api) = api();
    value(api.execute(Command::LockPeriod(date!(2026 - 06 - 30))));
    // Tally's lock governs vouchers, not masters (page cited in the header), so a ledger with no
    // opening balance is created. A ledger WITH an opening balance dated inside the lock is
    // pending: no page read says what Tally does.
    let r = value(api.execute(Command::CreateLedger(ledger(0, "Rent", GroupId(8)))));
    assert_eq!(r.created, Some(CreatedId::Ledger(LedgerId(1))));
}
