//! The Start screen and the shell's choice between it and company setup. Every test works in
//! temporary folders; none touches the user's config folder.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::Api;
use ledgerly_app::recent::{self, RecentCompany};
use ledgerly_app::screens::start::StartScreen;
use ledgerly_app::shell::Shell;
use ledgerly_core::{Clock, CompanyId, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).expect("27 is Maharashtra"),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Partnership,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn entry(n: usize) -> RecentCompany {
    RecentCompany {
        path: format!("/books/c{n}.ledgerly").into(),
        name: format!("Company {n}"),
        detail: "Partnership firm · Maharashtra".to_owned(),
        financial_year: "2026-27".to_owned(),
        last_opened: "21 Nov 2026".to_owned(),
    }
}

#[test]
fn the_shell_starts_on_setup_without_a_list_and_on_start_with_one() {
    // provenance: derived; the spec's first launch has no company to list, a later launch has.
    let none = Shell::cold_start_with_recent(None);
    assert_eq!(none.tabs().titles(), ["Company setup"]);

    let dir = tempfile::tempdir().expect("temp folder");
    let empty = Shell::cold_start_with_recent(Some(dir.path().to_path_buf()));
    assert_eq!(empty.tabs().titles(), ["Company setup"]);

    recent::record(dir.path(), entry(1)).expect("records");
    let listed = Shell::cold_start_with_recent(Some(dir.path().to_path_buf()));
    assert_eq!(listed.tabs().titles(), ["Start"]);
}

#[test]
fn a_list_that_cannot_be_read_starts_on_setup() {
    let dir = tempfile::tempdir().expect("temp folder");
    std::fs::write(dir.path().join("recent-companies.toml"), "company = 5 [").expect("written");
    let shell = Shell::cold_start_with_recent(Some(dir.path().to_path_buf()));
    assert_eq!(shell.tabs().titles(), ["Company setup"]);
}

#[test]
fn the_selection_moves_down_and_up_and_stays_inside_the_list() {
    // provenance: derived; three rows are indices 0 to 2, and J/K never leave them.
    let dir = tempfile::tempdir().expect("temp folder");
    let mut start = StartScreen::new(dir.path().to_path_buf(), vec![entry(1), entry(2), entry(3)]);
    assert_eq!(start.selected(), 0);
    start.move_by(-1);
    assert_eq!(start.selected(), 0);
    start.move_by(1);
    start.move_by(1);
    start.move_by(1);
    assert_eq!(start.selected(), 2);
    start.move_by(-1);
    assert_eq!(start.selected(), 1);
}

#[test]
fn opening_a_company_file_describes_and_records_it() {
    // provenance: derived from the company built above: name as typed, kind Partnership firm,
    // state code 27 is Maharashtra, FY starting 1 Apr 2026 is 2026-27, opened on today's date.
    let files = tempfile::tempdir().expect("temp folder");
    let config = tempfile::tempdir().expect("temp folder");
    let path = files.path().join("books.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::new_utc(
        date!(2026 - 11 - 21),
        time::Time::MIDNIGHT,
    ));
    drop(Api::create(&path, &company(), clock).expect("creates"));
    let api = recent::open_and_record(&path, Some(config.path())).expect("opens");
    drop(api);
    let list = recent::load(config.path()).expect("loads");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Fictional Traders");
    assert_eq!(list[0].detail, "Partnership firm · Maharashtra");
    assert_eq!(list[0].financial_year, "2026-27");
    assert_eq!(list[0].path, path);
}

#[test]
fn a_file_that_does_not_open_is_not_recorded() {
    let config = tempfile::tempdir().expect("temp folder");
    let missing = config.path().join("nope.ledgerly");
    assert!(recent::open_and_record(&missing, Some(config.path())).is_err());
    assert_eq!(recent::load(config.path()).expect("loads"), Vec::new());
}
