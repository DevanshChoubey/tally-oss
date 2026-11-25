//! s13: the name the signed-in user is greeted by (the mock's Home head, "Good afternoon,
//! Ananya", design/mock-v2/index.html:1132).
//!
//! Provenance: derived. The name comes from the company's user directory (`SettingChanged` under
//! `user.<id>.name`, written by `Api::set_user_name`), the same directory the day book reads for
//! "Entered by". The signed-in user is the actor of the Api (`Api::set_actor`). A user with no
//! stored name has none: the Api never makes one up ("User 1" is the day book's label for a
//! voucher, not a name to greet).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::Api;
use ledgerly_core::{Clock, CompanyId, StateCode, UserId};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_store::Actor;
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

fn open() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let api = Api::create(
        &dir.path().join("c.ledgerly"),
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    (dir, api)
}

#[test]
fn a_user_with_no_stored_name_has_none() {
    // User 1 is the default actor and nobody has named them.
    let (_dir, api) = open();
    assert_eq!(api.actor_name().unwrap(), None);
}

#[test]
fn the_signed_in_users_stored_name_is_returned_and_follows_the_actor() {
    // Users 1 and 2 are named; the name returned is the signed-in actor's, and a later
    // rename replaces it (the directory's last entry wins, as in the day book).
    let (_dir, mut api) = open();
    api.set_user_name(UserId(1), "Ananya K.").unwrap();
    api.set_user_name(UserId(2), "Rohan D.").unwrap();
    assert_eq!(api.actor_name().unwrap().as_deref(), Some("Ananya K."));
    api.set_actor(Actor::User(UserId(2)));
    assert_eq!(api.actor_name().unwrap().as_deref(), Some("Rohan D."));
    api.set_actor(Actor::User(UserId(3)));
    assert_eq!(api.actor_name().unwrap(), None);
    api.set_actor(Actor::User(UserId(1)));
    api.set_user_name(UserId(1), "Ananya Kulkarni").unwrap();
    assert_eq!(
        api.actor_name().unwrap().as_deref(),
        Some("Ananya Kulkarni")
    );
}
