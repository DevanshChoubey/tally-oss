//! The lock date in force, read through the facade (spec section 5, invariant 8: the lock date is
//! the last `PeriodLocked` event). The app's top bar and the invoice's period-lock note show it.
//!
//! Expected values are `derived`: each is the date passed to `Command::LockPeriod` in the same
//! test, or `None` when no lock was set.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, Command};
use ledgerly_core::{Clock, CompanyId, Outcome, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Lock Traders".to_owned(),
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

fn lock(api: &mut Api, until: time::Date) {
    match api.execute(Command::LockPeriod(until)) {
        Ok(Outcome::Value(_)) => {}
        other => panic!("the lock was not set: {other:?}"),
    }
}

#[test]
fn the_lock_date_is_the_last_lock_set_and_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock.ledgerly");
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    // provenance: derived, no LockPeriod has been sent.
    assert_eq!(api.lock_date(), None);

    lock(&mut api, date!(2026 - 09 - 30));
    // provenance: derived, the date just locked.
    assert_eq!(api.lock_date(), Some(date!(2026 - 09 - 30)));

    lock(&mut api, date!(2026 - 10 - 31));
    // provenance: derived, the last LockPeriod wins (invariant 8).
    assert_eq!(api.lock_date(), Some(date!(2026 - 10 - 31)));

    drop(api);
    let reopened = Api::open(&path).unwrap();
    // provenance: derived, the lock is read back from the file's audit log.
    assert_eq!(reopened.lock_date(), Some(date!(2026 - 10 - 31)));
}
