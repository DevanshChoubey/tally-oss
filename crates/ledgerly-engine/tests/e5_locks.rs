//! Card E5 tests: period locks.
//!
//! Provenance for every expected value here is `derived` from spec section 5, invariant 8
//! (docs/specs/2026-11-20-ledgerly-design.md): "no create/alter/delete dated on or before the lock
//! date". So a date is blocked exactly when `date <= lock`; the day after the lock is open; with
//! no lock nothing is blocked. The error carries the lock date itself (`PeriodLocked { until }`).
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use ledgerly_core::{CompanyId, Outcome};
use ledgerly_engine::EngineError;
use ledgerly_engine::locks::{check_unlocked, lock_period};
use ledgerly_model::{Address, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy};
use ledgerly_store::Event;
use std::collections::BTreeMap;
use time::macros::date;

fn books(fy_start: time::Date, books_begin: time::Date) -> Books {
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: ledgerly_core::StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start,
            books_begin,
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters: Masters::default(),
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

#[test]
fn post_alter_delete_on_lock_date_error() {
    // Same date check guards post, alter and delete (E3 calls it for each). On the lock date: blocked.
    let lock = date!(2025 - 06 - 30);
    assert_eq!(
        check_unlocked(lock, Some(lock)),
        Err(EngineError::PeriodLocked { until: lock })
    );
}

#[test]
fn dates_before_lock_error() {
    let lock = date!(2025 - 06 - 30);
    assert_eq!(
        check_unlocked(date!(2025 - 04 - 01), Some(lock)),
        Err(EngineError::PeriodLocked { until: lock })
    );
    assert_eq!(
        check_unlocked(date!(2025 - 06 - 29), Some(lock)),
        Err(EngineError::PeriodLocked { until: lock })
    );
}

#[test]
fn day_after_lock_is_open() {
    let lock = date!(2025 - 06 - 30);
    assert_eq!(check_unlocked(date!(2025 - 07 - 01), Some(lock)), Ok(()));
    assert_eq!(check_unlocked(date!(2026 - 03 - 31), Some(lock)), Ok(()));
}

#[test]
fn no_lock_blocks_nothing() {
    assert_eq!(check_unlocked(date!(2020 - 01 - 01), None), Ok(()));
}

#[test]
fn lock_event_is_recorded() {
    let b = books(date!(2025 - 04 - 01), date!(2025 - 04 - 01));
    let until = date!(2025 - 06 - 30);
    // Spec 6.6 / inv 8: locking is an audited event naming the lock date.
    assert_eq!(
        lock_period(until, &b),
        Ok(Outcome::Value(vec![Event::PeriodLocked { until }]))
    );
}

#[test]
fn lock_on_books_begin_is_allowed() {
    // inv 6: no voucher can predate books_begin, so locking exactly books_begin freezes that one day.
    let b = books(date!(2025 - 04 - 01), date!(2025 - 04 - 01));
    let until = date!(2025 - 04 - 01);
    assert_eq!(
        lock_period(until, &b),
        Ok(Outcome::Value(vec![Event::PeriodLocked { until }]))
    );
}

#[test]
fn lock_before_books_begin_is_rejected() {
    // inv 6: dates before books_begin hold no vouchers; a lock there is meaningless, so reject.
    let b = books(date!(2025 - 04 - 01), date!(2025 - 05 - 01));
    let r = lock_period(date!(2025 - 04 - 15), &b);
    assert!(matches!(r, Err(EngineError::Invalid { .. })), "{r:?}");
}
