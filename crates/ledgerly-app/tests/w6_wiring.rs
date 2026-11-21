//! Card W6: the entry screens' path to the engine, and the welcome screen's company form.
//!
//! Expected values are derived by hand and the derivation is written next to each one. Nothing is
//! copied from this crate's output. The company file is a fictional one in
//! `CARGO_TARGET_TMPDIR`.
//!
//! Not covered here, and pending: driving the entry screens themselves. They draw from a `Books`
//! value, and `ledgerly-api` does not yet expose the books to the app (see
//! `docs/interface-requests.md`, W6).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command};
use ledgerly_app::shell::submit::{self, CommandReply};
use ledgerly_app::shell::company_from_form;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, CompanyId, GroupId, OffsetDateTime, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::macros::date;

/// A fresh company file for `name`, created through the facade.
fn company_file(name: &str) -> Api {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("w6_wiring");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Asha Traders".to_owned(),
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
    };
    Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

/// Waits (bounded) for the answer to `ticket`, the way a screen polls it frame by frame.
fn answer(mut ticket: ledgerly_app::worker::Ticket<CommandReply>) -> CommandReply {
    let start = Instant::now();
    loop {
        if let Some(reply) = submit::settle(&mut ticket) {
            return reply;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the worker did not answer in 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_company_form_refuses_a_blank_name_a_bad_state_and_a_bad_year() {
    // derived: a company needs a name (the form's own rule).
    assert!(company_from_form("   ", "27", "2026").is_err());
    // derived: GST state codes run 01 to 38 (CBIC state code list); 99 is not one.
    assert!(company_from_form("Asha Traders", "99", "2026").is_err());
    // derived: a financial year is a year number; "twenty" is not one.
    assert!(company_from_form("Asha Traders", "27", "twenty").is_err());
}

#[test]
fn the_company_form_starts_the_books_on_1_april_of_the_year_given() {
    let company = company_from_form("  Asha Traders ", "27", "2026").unwrap();
    // derived: the Indian financial year starts on 1 April (parallel-build plan, global constraints),
    // so 2026 is 1 April 2026 to 31 March 2027, and books begin on the first day.
    assert_eq!(company.name, "Asha Traders");
    assert_eq!(company.fy_start, date!(2026 - 04 - 01));
    assert_eq!(company.books_begin, date!(2026 - 04 - 01));
    // derived: state code 27 is Maharashtra in the GST state code list.
    assert_eq!(company.state, StateCode::new(27).unwrap());
}

#[test]
fn a_refused_command_reaches_the_screen_as_failed_not_as_booked() {
    let worker = ApiWorker::spawn(company_file("refused")).unwrap();
    // derived: group id 9999 is not in a new company's books (the company has only the reserved
    // groups), and the engine refuses to delete a group it cannot find (NotFound). A refusal is
    // never a booked change, so the screen must show it as a failure.
    let reply = answer(submit::send(&worker, Command::DeleteGroup(GroupId(9999))));
    assert!(
        matches!(reply, CommandReply::Failed(_)),
        "expected a refusal, got {reply:?}"
    );
}

#[test]
fn a_refusal_does_not_stop_the_worker_for_the_next_command() {
    let worker = ApiWorker::spawn(company_file("after_refusal")).unwrap();
    let first = answer(submit::send(&worker, Command::DeleteGroup(GroupId(9999))));
    assert!(matches!(first, CommandReply::Failed(_)), "got {first:?}");
    // derived: the same refusal again, so the worker answered the second job too.
    let second = answer(submit::send(&worker, Command::DeleteGroup(GroupId(9998))));
    assert!(matches!(second, CommandReply::Failed(_)), "got {second:?}");
}
