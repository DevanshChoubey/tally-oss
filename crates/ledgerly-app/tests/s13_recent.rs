//! The recent companies list: a file in a config folder, newest first, capped, never shared with
//! a company file. Every test works in a temporary folder; none touches the user's config folder.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::recent::{MAX_RECENT, RecentCompany, load, record};
use std::path::PathBuf;

fn entry(n: usize) -> RecentCompany {
    RecentCompany {
        path: PathBuf::from(format!("/books/company-{n}.ledgerly")),
        name: format!("Company {n}"),
        detail: "Partnership firm · Maharashtra".to_owned(),
        financial_year: "2026-27".to_owned(),
        last_opened: "21 Nov 2026".to_owned(),
    }
}

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary folder")
}

#[test]
fn a_folder_with_no_list_is_an_empty_list() {
    // provenance: derived; nothing was recorded, so nothing is listed.
    let d = dir();
    assert_eq!(load(d.path()).expect("loads"), Vec::new());
}

#[test]
fn a_recorded_company_is_listed_with_what_was_recorded() {
    let d = dir();
    record(d.path(), entry(1)).expect("records");
    assert_eq!(load(d.path()).expect("loads"), vec![entry(1)]);
}

#[test]
fn the_newest_is_first_and_a_reopened_company_moves_to_the_top_once() {
    // provenance: derived; recording 1, 2, 3 then 1 again gives 1, 3, 2 (one line per file).
    let d = dir();
    for n in [1, 2, 3, 1] {
        record(d.path(), entry(n)).expect("records");
    }
    let names: Vec<String> = load(d.path())
        .expect("loads")
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["Company 1", "Company 3", "Company 2"]);
}

#[test]
fn the_list_keeps_only_the_newest_ten() {
    // provenance: derived; 12 recorded, the cap is MAX_RECENT = 10, so 3 to 12 stay, 12 first.
    assert_eq!(MAX_RECENT, 10);
    let d = dir();
    for n in 1..=12 {
        record(d.path(), entry(n)).expect("records");
    }
    let list = load(d.path()).expect("loads");
    assert_eq!(list.len(), 10);
    assert_eq!(list.first().map(|c| c.name.as_str()), Some("Company 12"));
    assert_eq!(list.last().map(|c| c.name.as_str()), Some("Company 3"));
}

#[test]
fn a_folder_that_does_not_exist_yet_is_made() {
    let d = dir();
    let nested = d.path().join("config").join("ledgerly");
    record(&nested, entry(1)).expect("records");
    assert_eq!(load(&nested).expect("loads").len(), 1);
}

#[test]
fn a_list_that_is_not_valid_is_an_error_not_an_empty_list() {
    let d = dir();
    std::fs::write(d.path().join("recent-companies.toml"), "company = 5 [").expect("written");
    assert!(load(d.path()).is_err());
}
