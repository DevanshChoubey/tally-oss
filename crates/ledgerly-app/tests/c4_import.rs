//! Card C4: the import screen's one-call path runs `Api::import_tally_http` on the worker.
//!
//! Provenance: `derived`. An unreachable port refuses the connection, so the call fails and the
//! screen must show the error and import nothing. A company that already holds data is refused by
//! the API before Tally is contacted (module header of `ledgerly-api/src/tally_import.rs`: only a
//! company holding what `Api::create` seeds can be imported into), so the screen shows that
//! refusal through `unsupported_text`.
//! Pending: showing a successful `ImportSummary` needs a recorded Tally XML server; the fixtures
//! belong to card C7, so no expected summary text is asserted here.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::import::{DirectState, ImportScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn empty_company(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    Api::create(
        &path,
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap()
}

fn cash_group() -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::CashInHand))
        .unwrap()
        .id
}

/// A port nothing listens on: bind to port 0, note the port, close the socket.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn finished(screen: &mut ImportScreen) {
    for _ in 0..4000 {
        screen.tick();
        if *screen.direct_state() != DirectState::Running {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the import did not finish within the bound");
}

#[test]
fn an_unreachable_port_shows_the_error() {
    let worker = ApiWorker::spawn(empty_company("c4_import_closed.ledgerly")).unwrap();
    let port = closed_port();
    let mut screen = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    screen.set_direct_form("127.0.0.1", &port.to_string(), "Any Company");
    screen.start_direct_import(Some(&worker));
    assert_eq!(*screen.direct_state(), DirectState::Running);
    finished(&mut screen);

    let DirectState::Failed(message) = screen.direct_state() else {
        panic!("expected a failure, got {:?}", screen.direct_state());
    };
    // The message names where we tried and which company, so the user knows what failed.
    assert!(message.contains("127.0.0.1"), "{message}");
    assert!(message.contains(&port.to_string()), "{message}");
    assert!(message.contains("Any Company"), "{message}");
    let text = screen.direct_state().text().unwrap();
    assert!(text.starts_with("Error: "), "{text}");
}

#[test]
fn a_company_that_holds_data_is_refused_with_the_reason_shown() {
    let mut api = empty_company("c4_import_refused.ledgerly");
    let ledger = Ledger {
        ineligible_itc: None,
        id: LedgerId(1),
        name: "Cash".to_owned(),
        aliases: Vec::new(),
        group: cash_group(),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    };
    match api.execute(Command::CreateLedger(ledger)) {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    // The port is closed; the refusal must come first, before Tally is contacted.
    screen.set_direct_form("127.0.0.1", &closed_port().to_string(), "Any Company");
    screen.start_direct_import(Some(&worker));
    finished(&mut screen);

    let DirectState::Refused(u) = screen.direct_state() else {
        panic!("expected a refusal, got {:?}", screen.direct_state());
    };
    let text = screen.direct_state().text().unwrap();
    assert!(text.starts_with("Not supported yet: "), "{text}");
    assert!(text.contains(&u.reason), "{text}");
}

#[test]
fn a_bad_port_or_a_missing_worker_runs_nothing() {
    let mut screen = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    screen.set_direct_form("localhost", "not-a-port", "Any Company");
    screen.start_direct_import(None);
    let DirectState::Failed(message) = screen.direct_state() else {
        panic!("expected a failure, got {:?}", screen.direct_state());
    };
    assert!(message.contains("not-a-port"), "{message}");

    screen.set_direct_form("localhost", "9000", "Any Company");
    screen.start_direct_import(None);
    let DirectState::Failed(message) = screen.direct_state() else {
        panic!("expected a failure, got {:?}", screen.direct_state());
    };
    assert!(message.contains("no company"), "{message}");
}
