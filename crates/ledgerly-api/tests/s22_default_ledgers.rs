//! S22: a new company starts with the predefined ledgers.
//!
//! Provenance: `derived`. TallyPrime help, "Ledgers in TallyPrime"
//! (<https://help.tallysolutions.com/ledgers-in-tallyprime/>, read 2026-11-27): "TallyPrime
//! provides predefined ledgers such as Cash and Profit & Loss A/c", and a predefined master such
//! as Cash or Profit & Loss A/c cannot be deleted. Cash sits under Cash-in-Hand (research R1,
//! "Reserved ledgers"). Profit & Loss A/c sits under Primary there, which a ledger here cannot
//! (every ledger has a group); the books compute its row at query time instead (see
//! `ledgerly-reports` `balance_sheet.rs`), so only Cash is created.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_core::{Clock, CompanyId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_engine::EngineError;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, NegativeStockPolicy, ReservedGroup,
    ReservedLedger,
};
use time::OffsetDateTime;
use time::macros::date;

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::new_utc(
        date!(2026 - 11 - 21),
        time::Time::MIDNIGHT,
    ))
}

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

fn books(api: &Api) -> Books {
    match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("books unsupported: {u:?}"),
    }
}

#[test]
fn a_new_company_starts_with_the_predefined_cash_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let api = new_company(&dir.path().join("c.ledgerly"), &company(), clock());
    let b = books(&api);
    let cash_in_hand = reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::CashInHand))
        .unwrap()
        .id;
    let ledgers: Vec<_> = b.masters.ledgers.values().collect();
    assert_eq!(ledgers.len(), 1, "{ledgers:?}");
    let cash = ledgers[0];
    assert_eq!(cash.name, "Cash");
    assert_eq!(cash.group, Some(cash_in_hand));
    assert_eq!(cash.reserved, Some(ReservedLedger::Cash));
    assert_eq!(cash.opening, SignedAmount::dr(Paise::ZERO));
    assert_eq!(cash.bill_wise, None);
    assert!(b.vouchers.is_empty());
}

#[test]
fn the_predefined_cash_ledger_cannot_be_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_company(&dir.path().join("c.ledgerly"), &company(), clock());
    let id = *books(&api).masters.ledgers.keys().next().unwrap();
    let out = api.execute(Command::DeleteLedger(id));
    assert!(
        matches!(
            out,
            Err(ApiError::Engine(EngineError::ReservedLedger(
                ReservedLedger::Cash
            )))
        ),
        "{out:?}"
    );
    assert_eq!(books(&api).masters.ledgers.len(), 1);
}

#[test]
fn a_bare_store_seed_still_has_no_ledger() {
    // `Api::create` stays the bare seed whole-company importers start from.
    let dir = tempfile::tempdir().unwrap();
    let api = Api::create(&dir.path().join("c.ledgerly"), &company(), clock()).unwrap();
    assert!(books(&api).masters.ledgers.is_empty());
}

/// A company made as the setup screen makes it.
fn new_company(path: &std::path::Path, company: &Company, clock: Clock) -> Api {
    match Api::create_company(path, company, clock).unwrap() {
        Outcome::Value(api) => api,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}
