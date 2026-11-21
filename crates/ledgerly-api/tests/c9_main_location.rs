//! Coordinator tests (Wave 3): every company file holds Tally's default godown, Main Location.
//!
//! Provenance `derived`, from the TallyPrime FAQ on godowns
//! <https://help.tallysolutions.com/tally-prime/godowns-locations/godowns-locations-faq-tally/>
//! (accessed 2026-11-21). When "Maintain Multiple Godowns" is off, TallyPrime keeps all stock
//! in an internal godown called Main Location, under the root Primary, and voucher entry offers
//! no godown list. Once the feature is on, the same Main Location is listed for stock
//! allocation. So it exists in every company from the start. Ledgerly seeds it as godown 1 with
//! no parent (Primary is the root, not a godown). The Tally.ERP 9 FAQ says the same:
//! <https://help.tallysolutions.com/docs/te9rel65/Creating_Masters/Inventory_Info/faq-godowns.htm>
//! (accessed 2026-11-21).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_core::{Clock, CompanyId, GodownId, Outcome, StateCode};
use ledgerly_model::seed_godowns::{MAIN_LOCATION, default_godowns};
use ledgerly_model::{Address, Company, EntityKind, Features, Godown, NegativeStockPolicy};
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

fn create(dir: &std::path::Path) -> Api {
    Api::create(
        &dir.join("company.ledgerly"),
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap()
}

#[test]
fn a_new_company_holds_main_location_as_its_only_godown() {
    let dir = tempfile::tempdir().unwrap();
    let api = create(dir.path());
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("a new company file has readable books");
    };
    let expected = Godown {
        id: GodownId(1),
        name: "Main Location".to_owned(),
        parent: None,
        tally: None,
    };
    assert_eq!(MAIN_LOCATION, GodownId(1));
    assert_eq!(default_godowns(), vec![expected.clone()]);
    let godowns: Vec<&Godown> = books.masters.godowns.values().collect();
    assert_eq!(godowns, vec![&expected]);
}

#[test]
fn main_location_survives_reopening_the_file() {
    let dir = tempfile::tempdir().unwrap();
    drop(create(dir.path()));
    let api = Api::open(&dir.path().join("company.ledgerly")).unwrap();
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("a reopened company file has readable books");
    };
    assert_eq!(
        books
            .masters
            .godowns
            .get(&MAIN_LOCATION)
            .map(|g| g.name.as_str()),
        Some("Main Location")
    );
}

#[test]
fn a_second_godown_named_main_location_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = create(dir.path());
    // derived: godown names are unique (engine `check_godown`), and Main Location already exists.
    let out = api.execute(Command::CreateGodown(Godown {
        id: GodownId(0),
        name: "Main Location".to_owned(),
        parent: None,
        tally: None,
    }));
    assert!(matches!(out, Err(ApiError::Engine(_))), "{out:?}");
}
