//! Card W2 tests: the `ledgerly` command line. They run the built binary and check its exit codes
//! and its output.
//!
//! Provenance. Expected values are `derived` from the documented exit codes in `main.rs` (0 passed,
//! 1 a check failed, 2 could not run) and from the spec: `ledgerly verify` prints a pass/fail
//! certificate (spec 6.6), and a tampered chain must fail (card W2). The tamper fixture is the
//! reserved group name `Capital Account` written by the seed (`ledgerly_model::seed_groups`).
//! The GSTIN `29ABCDE1234F1ZW` is the fictional fixture with a valid check character used by the
//! store tests. Tally import and the GSTR-1 content are not tested here: they belong to W3 and W4.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ledgerly_api::{Address, Api, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_core::{Clock, CompanyId, Date, Month, OffsetDateTime, StateCode};

fn ledgerly(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ledgerly"))
        .args(args)
        .output()
        .unwrap()
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn company() -> Company {
    let april_first = Date::from_calendar_date(2026, Month::April, 1).unwrap();
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: april_first,
        books_begin: april_first,
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A company file with the seed only. Returns the directory guard and the file path.
fn company_file() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    drop(api);
    (dir, path)
}

/// Rewrites the first occurrence of `from` in the file to `to` (same length), changing one stored
/// event without touching the hash columns.
fn rewrite_bytes_in_file(path: &Path, from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let mut bytes = std::fs::read(path).unwrap();
    let position = bytes
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .expect("the fixture text is stored in the company file");
    bytes
        .get_mut(position..position + to.len())
        .unwrap()
        .copy_from_slice(to.as_bytes());
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn help_lists_the_subcommands() {
    let out = ledgerly(&["--help"]);
    assert_eq!(exit_code(&out), 0);
    let text = stdout(&out);
    for name in [
        "import-tally",
        "verify",
        "export",
        "rebuild-check",
        "conformance",
    ] {
        assert!(text.contains(name), "help lacks {name}: {text}");
    }
}

#[test]
fn unknown_subcommand_is_a_usage_error() {
    // Derived: clap reports a usage error with exit code 2 (the "could not run" code).
    let out = ledgerly(&["frobnicate"]);
    assert_eq!(exit_code(&out), 2);
}

#[test]
fn verify_of_a_missing_file_exits_2_and_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.ledgerly");
    let out = ledgerly(&["verify", path.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 2);
    assert!(!path.exists(), "verify must not create a company file");
}

#[test]
fn verify_passes_on_a_fresh_company() {
    let (_dir, path) = company_file();
    let out = ledgerly(&["verify", path.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).starts_with("Audit chain integrity: PASS\n"));
}

#[test]
fn verify_exits_1_on_a_tampered_chain() {
    let (_dir, path) = company_file();
    rewrite_bytes_in_file(&path, "Capital Account", "Capital Accounx");
    let out = ledgerly(&["verify", path.to_str().unwrap()]);
    // Derived: spec 6.6 and card W2, a tampered chain exits 1.
    assert_eq!(exit_code(&out), 1);
    assert!(stdout(&out).starts_with("Audit chain integrity: FAIL\n"));
}

#[test]
fn rebuild_check_does_not_report_a_match_while_projections_are_not_maintained() {
    let (_dir, path) = company_file();
    let out = ledgerly(&["rebuild-check", path.to_str().unwrap()]);
    // Derived from the store rebuild contract: an event log with empty projection tables is
    // NotMaintained, which the command reports as "could not run" (exit 2), never as a pass.
    assert_eq!(exit_code(&out), 2);
    assert!(stdout(&out).contains("This is not a match."));
}

#[test]
fn export_checks_the_gstin_before_it_opens_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.ledgerly");
    let out = ledgerly(&[
        "export",
        "gstr1",
        path.to_str().unwrap(),
        "--gstin",
        "NOTAGSTIN",
        "--from",
        "2026-04-01",
        "--to",
        "2026-06-30",
    ]);
    assert_eq!(exit_code(&out), 2);
    assert!(stderr(&out).contains("GSTIN"), "{}", stderr(&out));
    assert!(!path.exists());
}

#[test]
fn export_rejects_a_reversed_period() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.ledgerly");
    let out = ledgerly(&[
        "export",
        "gstr1",
        path.to_str().unwrap(),
        "--gstin",
        "29ABCDE1234F1ZW",
        "--from",
        "2026-06-30",
        "--to",
        "2026-04-01",
    ]);
    assert_eq!(exit_code(&out), 2);
    assert!(stderr(&out).contains("--from"), "{}", stderr(&out));
}

#[test]
fn export_rejects_a_date_that_is_not_a_calendar_date() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.ledgerly");
    let out = ledgerly(&[
        "export",
        "gstr1",
        path.to_str().unwrap(),
        "--gstin",
        "29ABCDE1234F1ZW",
        "--from",
        "2026-13-01",
        "--to",
        "2026-06-30",
    ]);
    assert_eq!(exit_code(&out), 2);
    assert!(stderr(&out).contains("--from must be a date"));
}

#[test]
fn import_tally_on_a_missing_file_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.ledgerly");
    let out = ledgerly(&[
        "import-tally",
        path.to_str().unwrap(),
        "--company",
        "Example Traders",
    ]);
    assert_eq!(exit_code(&out), 2);
    assert!(!path.exists());
}

#[test]
fn conformance_with_no_cases_and_an_empty_pass_list_passes() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    let pass_list = dir.path().join("pass-list.txt");
    std::fs::write(&pass_list, "# nothing listed yet\n").unwrap();
    let out = ledgerly(&[
        "conformance",
        "--cases",
        cases.to_str().unwrap(),
        "--pass-list",
        pass_list.to_str().unwrap(),
    ]);
    // Derived from rule A6: a ratchet with nothing listed has no violation.
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
}

#[test]
fn conformance_exits_1_when_a_listed_case_is_not_in_the_suite() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    let pass_list = dir.path().join("pass-list.txt");
    std::fs::write(&pass_list, "ghost.case\n").unwrap();
    let out = ledgerly(&[
        "conformance",
        "--cases",
        cases.to_str().unwrap(),
        "--pass-list",
        pass_list.to_str().unwrap(),
    ]);
    // Derived from rule A6: a listed id that has left the suite is a violation.
    assert_eq!(exit_code(&out), 1);
    assert!(stderr(&out).contains("ghost.case"), "{}", stderr(&out));
}

#[test]
fn conformance_with_a_missing_cases_directory_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("no-such-cases");
    let pass_list = dir.path().join("pass-list.txt");
    std::fs::write(&pass_list, "").unwrap();
    let out = ledgerly(&[
        "conformance",
        "--cases",
        cases.to_str().unwrap(),
        "--pass-list",
        pass_list.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 2);
}
