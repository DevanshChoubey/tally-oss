//! Card Q1 tests: case format, provenance, runner and ratchet (spec section 10).
//!
//! Provenance of every expected value is written next to it. Cases here are written to a
//! temporary directory because the card does not own a `cases/` directory.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::fs;
use std::path::Path;

use ledgerly_conformance::case::{Check, Provenance, parse_cases};
use ledgerly_conformance::runner::{
    CaseStatus, ConformanceError, check_ratchet, parse_pass_list, run_all,
};
use tempfile::TempDir;

fn dir_with(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (name, text) in files {
        fs::write(dir.path().join(name), text).unwrap();
    }
    dir
}

fn status(dir: &Path, id: &str) -> CaseStatus {
    let report = run_all(dir).unwrap();
    report.status_of(id).cloned().unwrap()
}

// provenance: derived. Luhn mod-36 check character of 27ABCDE1234F1Z, by hand from the algorithm
// in `ledgerly_core::gstin_check_character`. Digits and letters map 0-9 and A=10..Z=35; from the
// left the factors are 1,2,1,2,...; each product p adds p/36 + p%36:
//   2x1=2  7x2=14  A10x1=10  B11x2=22  C12x1=12  D13x2=26  E14x1=14  1x2=2  2x1=2  3x2=6
//   4x1=4  F15x2=30  1x1=1  Z35x2=70 -> 1+34=35
//   sum = 2+14+10+22+12+26+14+2+2+6+4+30+1+35 = 180; 180 mod 36 = 0; check = (36-0) mod 36 = 0 -> '0'.
// (The algorithm itself is recalled, not quoted; see ledgerly-core.)
const PASSING_CASE: &str = r#"
[[case]]
id = "core.gstin.check_char.mh"
description = "Maharashtra fixture GSTIN check character"
[case.provenance]
kind = "derived"
reference = "Luhn mod-36 as in ledgerly-core gstin.rs"
working = "sum 180; 180 mod 36 = 0; check = 0"
[case.check]
kind = "gstin_check_character"
first14 = "27ABCDE1234F1Z"
expected = "0"
"#;

#[test]
fn case_without_provenance_reports_pending() {
    // The check below would pass if it were run, so this also proves that a case without
    // provenance is never counted as passing.
    let dir = dir_with(&[(
        "a.toml",
        r#"
[[case]]
id = "no.provenance"
[case.check]
kind = "gstin_check_character"
first14 = "27ABCDE1234F1Z"
expected = "0"
"#,
    )]);
    assert!(matches!(
        status(dir.path(), "no.provenance"),
        CaseStatus::Pending { .. }
    ));
    let report = run_all(dir.path()).unwrap();
    assert_eq!(report.passed_count(), 0);
    assert_eq!(report.pending_count(), 1);
}

#[test]
fn explicit_pending_provenance_reports_pending() {
    let dir = dir_with(&[(
        "a.toml",
        r#"
[[case]]
id = "explicit.pending"
[case.provenance]
kind = "pending"
"#,
    )]);
    assert!(matches!(
        status(dir.path(), "explicit.pending"),
        CaseStatus::Pending { .. }
    ));
}

#[test]
fn blank_provenance_text_reports_pending() {
    let dir = dir_with(&[(
        "a.toml",
        r#"
[[case]]
id = "blank.derived"
[case.provenance]
kind = "derived"
reference = ""
working = "  "
[case.check]
kind = "gstin_check_character"
first14 = "27ABCDE1234F1Z"
expected = "0"

[[case]]
id = "blank.observed"
[case.provenance]
kind = "observed"
tool = ""
version = "7.0"
date = "2026-01-05"
[case.check]
kind = "gstin_check_character"
first14 = "27ABCDE1234F1Z"
expected = "0"
"#,
    )]);
    for id in ["blank.derived", "blank.observed"] {
        assert!(
            matches!(status(dir.path(), id), CaseStatus::Pending { .. }),
            "{id}"
        );
    }
}

#[test]
fn derived_case_with_matching_check_passes() {
    let dir = dir_with(&[("a.toml", PASSING_CASE)]);
    assert_eq!(
        status(dir.path(), "core.gstin.check_char.mh"),
        CaseStatus::Passed
    );
}

#[test]
fn observed_provenance_parses_its_date() {
    let cases = parse_cases(
        r#"
[[case]]
id = "obs"
[case.provenance]
kind = "observed"
tool = "GST offline tool"
version = "3.1"
date = "2026-02-03"
"#,
    )
    .unwrap();
    let Some(case) = cases.first() else {
        panic!("one case expected");
    };
    match &case.provenance {
        Provenance::Observed {
            tool,
            version,
            date,
        } => {
            assert_eq!(tool, "GST offline tool");
            assert_eq!(version, "3.1");
            assert_eq!(date.to_string(), "2026-02-03");
        }
        other => panic!("wrong provenance {other:?}"),
    }
    assert!(case.provenance.pending_reason().is_none());
}

#[test]
fn established_provenance_without_check_fails_never_passes() {
    let dir = dir_with(&[(
        "a.toml",
        r#"
[[case]]
id = "no.check"
[case.provenance]
kind = "derived"
reference = "r"
working = "w"
"#,
    )]);
    assert!(matches!(
        status(dir.path(), "no.check"),
        CaseStatus::Failed { .. }
    ));
}

#[test]
fn wrong_expected_value_fails_with_both_values() {
    // provenance: derived. The check character of 27ABCDE1234F1Z is '0' (hand derivation above),
    // so claiming '1' must fail.
    let dir = dir_with(&[(
        "a.toml",
        &PASSING_CASE.replace("expected = \"0\"", "expected = \"1\""),
    )]);
    match status(dir.path(), "core.gstin.check_char.mh") {
        CaseStatus::Failed { detail } => {
            assert!(detail.contains('0') && detail.contains('1'), "{detail}");
        }
        other => panic!("expected failure, got {other:?}"),
    }
}

#[test]
fn unknown_check_kind_is_a_parse_error_naming_the_file() {
    let dir = dir_with(&[(
        "bad.toml",
        r#"
[[case]]
id = "x"
[case.check]
kind = "made_up_check"
"#,
    )]);
    match run_all(dir.path()) {
        Err(ConformanceError::Parse { path, .. }) => {
            assert!(path.to_string_lossy().ends_with("bad.toml"));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn unknown_field_is_a_parse_error() {
    // A misspelt `provenence` must not silently turn a case into a pending one.
    let dir = dir_with(&[(
        "bad.toml",
        r#"
[[case]]
id = "x"
provenence = "oops"
"#,
    )]);
    assert!(matches!(
        run_all(dir.path()),
        Err(ConformanceError::Parse { .. })
    ));
}

#[test]
fn duplicate_ids_across_files_are_rejected() {
    let dir = dir_with(&[("a.toml", PASSING_CASE), ("b.toml", PASSING_CASE)]);
    assert!(matches!(
        run_all(dir.path()),
        Err(ConformanceError::DuplicateId { .. })
    ));
}

#[test]
fn ids_with_whitespace_or_empty_are_rejected() {
    for bad in ["", "has space"] {
        let dir = dir_with(&[("a.toml", &format!("[[case]]\nid = \"{bad}\"\n"))]);
        assert!(
            matches!(run_all(dir.path()), Err(ConformanceError::BadId { .. })),
            "{bad:?}"
        );
    }
}

#[test]
fn missing_directory_is_an_io_error() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope");
    assert!(matches!(
        run_all(&missing),
        Err(ConformanceError::Io { .. })
    ));
}

#[test]
fn results_are_ordered_by_id_and_non_toml_files_are_ignored() {
    let dir = dir_with(&[
        (
            "z.toml",
            "[[case]]\nid = \"b.second\"\n[[case]]\nid = \"a.first\"\n",
        ),
        ("notes.txt", "[[case]]\nid = \"ignored\"\n"),
    ]);
    let report = run_all(dir.path()).unwrap();
    let ids: Vec<&str> = report.results.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["a.first", "b.second"]);
}

#[test]
fn fixture_checks_run_against_books_small() {
    // provenance: derived. Each expected value is worked from the fixture definition.
    //  - R3 closing quantity: 100 opening + 50 - 120 + 40 - 30 = 40 (docs/research/03-inventory.md,
    //    own worked data table).
    //  - Cash: V1 opening capital Dr 1,00,000; V12 contra Cr 5,000; V13 receipt Dr 2,832
    //    => 100000 - 5000 + 2832 = 97,832.00 = 9,783,200 paise.
    //  - Double entry: every voucher has equal debits and credits, so all postings sum to 0.
    let dir = dir_with(&[(
        "fx.toml",
        r#"
[[case]]
id = "fx.r3.closing_qty"
[case.provenance]
kind = "derived"
reference = "docs/research/03-inventory.md worked data"
working = "100 + 50 - 120 + 40 - 30 = 40"
[case.check]
kind = "fixture_stock_qty"
item = "Item R3"
closing_qty = "40"

[[case]]
id = "fx.cash.net"
[case.provenance]
kind = "derived"
reference = "fixture voucher definitions V1, V12, V13"
working = "100000 - 5000 + 2832 = 97832 rupees"
[case.check]
kind = "fixture_ledger_net"
ledger = "Cash"
net_paise = 9783200

[[case]]
id = "fx.double_entry"
[case.provenance]
kind = "derived"
reference = "double-entry identity"
working = "sum of debits = sum of credits in every voucher, so the sum of all postings is 0"
[case.check]
kind = "fixture_postings_balance"
"#,
    )]);
    for id in ["fx.r3.closing_qty", "fx.cash.net", "fx.double_entry"] {
        assert_eq!(status(dir.path(), id), CaseStatus::Passed, "{id}");
    }
}

#[test]
fn fixture_check_naming_a_missing_ledger_fails() {
    let dir = dir_with(&[(
        "fx.toml",
        r#"
[[case]]
id = "fx.missing"
[case.provenance]
kind = "derived"
reference = "r"
working = "w"
[case.check]
kind = "fixture_ledger_net"
ledger = "No Such Ledger"
net_paise = 0
"#,
    )]);
    assert!(matches!(
        status(dir.path(), "fx.missing"),
        CaseStatus::Failed { .. }
    ));
}

#[test]
fn check_kinds_round_trip_through_the_parser() {
    let cases = parse_cases(PASSING_CASE).unwrap();
    assert_eq!(cases.len(), 1);
    assert!(matches!(
        cases.first().and_then(|c| c.check.as_ref()),
        Some(Check::GstinCheckCharacter { expected: '0', .. })
    ));
}

// ---- ratchet ----

#[test]
fn passing_case_removed_from_passing_fails_the_ratchet() {
    // The pass-list says the case passed before. Now its expected value is wrong, so it fails.
    let broken = PASSING_CASE.replace("expected = \"0\"", "expected = \"1\"");
    let dir = dir_with(&[("a.toml", &broken)]);
    let report = run_all(dir.path()).unwrap();
    let errors = check_ratchet(&report, "core.gstin.check_char.mh\n").unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("core.gstin.check_char.mh"));
}

#[test]
fn case_that_lost_its_provenance_fails_the_ratchet() {
    let stripped = "[[case]]\nid = \"core.gstin.check_char.mh\"\n";
    let dir = dir_with(&[("a.toml", stripped)]);
    let report = run_all(dir.path()).unwrap();
    let errors = check_ratchet(&report, "core.gstin.check_char.mh").unwrap_err();
    assert!(errors[0].contains("pending"), "{errors:?}");
}

#[test]
fn case_deleted_from_the_suite_fails_the_ratchet() {
    let dir = dir_with(&[("a.toml", "[[case]]\nid = \"other\"\n")]);
    let report = run_all(dir.path()).unwrap();
    let errors = check_ratchet(&report, "core.gstin.check_char.mh").unwrap_err();
    assert!(errors[0].contains("not in the suite"), "{errors:?}");
}

#[test]
fn ratchet_passes_when_every_listed_case_passes() {
    let dir = dir_with(&[("a.toml", PASSING_CASE)]);
    let report = run_all(dir.path()).unwrap();
    assert_eq!(check_ratchet(&report, "core.gstin.check_char.mh\n"), Ok(()));
}

#[test]
fn newly_passing_case_not_on_the_list_is_allowed() {
    let dir = dir_with(&[("a.toml", PASSING_CASE)]);
    let report = run_all(dir.path()).unwrap();
    assert_eq!(check_ratchet(&report, "# nothing listed yet\n"), Ok(()));
    assert_eq!(report.pass_list_text(), "core.gstin.check_char.mh\n");
}

#[test]
fn pass_list_ignores_comments_and_blanks_and_rejects_duplicates() {
    let ok = parse_pass_list("# header\n\n  a.b  \nc.d\n").unwrap();
    assert_eq!(ok, ["a.b", "c.d"]);
    let errors = parse_pass_list("a.b\na.b\n").unwrap_err();
    assert!(errors[0].contains("a.b"));
    let errors = parse_pass_list("has space\n").unwrap_err();
    assert!(errors[0].contains("has space"));
}

#[test]
fn duplicate_pass_list_lines_fail_the_ratchet() {
    let dir = dir_with(&[("a.toml", PASSING_CASE)]);
    let report = run_all(dir.path()).unwrap();
    assert!(
        check_ratchet(
            &report,
            "core.gstin.check_char.mh\ncore.gstin.check_char.mh\n"
        )
        .is_err()
    );
}

#[test]
fn the_checked_in_pass_list_is_well_formed() {
    let text = include_str!("../pass-list.txt");
    assert!(parse_pass_list(text).is_ok());
}
