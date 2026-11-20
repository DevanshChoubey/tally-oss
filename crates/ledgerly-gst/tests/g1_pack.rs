//! Card G1 tests: rule pack loading and effective dating (spec 6.3, engineering rule A8).
//!
//! The packs here are fictional fixtures. The expected values are consequences of the dates written
//! in the fixture itself (inclusive ranges, selection by the supplied date), so each is `derived`
//! from the format definition in `packs/README.md`, not from any real notification.

use ledgerly_core::Outcome;
use ledgerly_gst::GstError;
use ledgerly_gst::pack::load_pack;
use time::macros::date;

const GOOD: &str = r#"
version = "test-1"

[[entry]]
id = "fixture.rate"
effective_from = "2025-04-01"
effective_to = "2025-09-21"
legal_ref = "fixture notification A dated 2025-03-01"
[entry.params]
percent = "12"

[[entry]]
id = "fixture.rate"
effective_from = "2025-09-22"
legal_ref = "fixture notification B dated 2025-09-03"
[entry.params]
percent = "18"

[[entry]]
id = "fixture.cess"
effective_from = "2025-04-01"
pending = true
[entry.params]
percent = "1"
"#;

fn percent(out: &Outcome<ledgerly_gst::pack::RuleSet>, id: &str) -> Option<String> {
    let Outcome::Value(set) = out else {
        return None;
    };
    match set.get(id) {
        Outcome::Value(e) => e.params().get("percent").cloned(),
        Outcome::Unsupported(_) => None,
    }
}

#[test]
fn pack_selected_by_time_of_supply_not_entry_date() {
    let pack = load_pack(GOOD).unwrap();
    assert_eq!(pack.version(), "test-1");
    // Derived: the supply happened on 2025-09-21, the last day of the 12 entry (inclusive end).
    // It is booked on 2025-09-25, after the 18 entry started; the entry date is never an input,
    // so the 12 entry applies.
    let supply = date!(2025 - 09 - 21);
    let _entry_date = date!(2025 - 09 - 25);
    assert_eq!(
        percent(&pack.for_time_of_supply(supply), "fixture.rate").as_deref(),
        Some("12")
    );
    // Derived: 2025-09-22 is the first day of the 18 entry (inclusive start).
    assert_eq!(
        percent(
            &pack.for_time_of_supply(date!(2025 - 09 - 22)),
            "fixture.rate"
        )
        .as_deref(),
        Some("18")
    );
    // Derived: the open-ended entry still applies in a far later year.
    assert_eq!(
        percent(
            &pack.for_time_of_supply(date!(2040 - 01 - 01)),
            "fixture.rate"
        )
        .as_deref(),
        Some("18")
    );
}

#[test]
fn date_before_every_entry_is_unsupported() {
    let pack = load_pack(GOOD).unwrap();
    // Derived: the earliest entry starts 2025-04-01, so 2025-03-31 has no rule in force.
    assert!(
        pack.for_time_of_supply(date!(2025 - 03 - 31))
            .is_unsupported()
    );
}

#[test]
fn pending_entry_evaluates_to_unsupported() {
    let pack = load_pack(GOOD).unwrap();
    let Outcome::Value(set) = pack.for_time_of_supply(date!(2025 - 10 - 01)) else {
        panic!("rules are in force on this date");
    };
    // Derived (rule A8): fixture.cess is pending, so it must not yield a value.
    assert!(set.get("fixture.cess").is_unsupported());
    // Derived: an id the pack never defines is also unsupported, not a default.
    assert!(set.get("fixture.nothing").is_unsupported());
}

#[test]
fn gap_between_entries_of_one_id_is_unsupported() {
    let toml = r#"
version = "gap"
[[entry]]
id = "x"
effective_from = "2025-01-01"
effective_to = "2025-01-31"
legal_ref = "ref 1"
[[entry]]
id = "x"
effective_from = "2025-03-01"
legal_ref = "ref 2"
[[entry]]
id = "other"
effective_from = "2025-01-01"
legal_ref = "ref 3"
"#;
    let pack = load_pack(toml).unwrap();
    // Derived: 2025-02-15 lies between x's two ranges, so x has no rule; "other" does.
    let Outcome::Value(set) = pack.for_time_of_supply(date!(2025 - 02 - 15)) else {
        panic!("other is in force");
    };
    assert!(set.get("x").is_unsupported());
    assert!(!set.get("other").is_unsupported());
}

#[test]
fn overlapping_effective_ranges_error() {
    // Derived: both ranges contain 2025-06-15.
    let toml = r#"
version = "overlap"
[[entry]]
id = "x"
effective_from = "2025-01-01"
effective_to = "2025-06-30"
legal_ref = "ref 1"
[[entry]]
id = "x"
effective_from = "2025-06-15"
legal_ref = "ref 2"
"#;
    assert!(matches!(load_pack(toml), Err(GstError::Pack { .. })));
}

#[test]
fn ranges_sharing_one_day_overlap() {
    // Derived: ranges are inclusive, so ending and starting on 2025-06-30 share that day.
    let toml = r#"
version = "touch"
[[entry]]
id = "x"
effective_from = "2025-01-01"
effective_to = "2025-06-30"
legal_ref = "ref 1"
[[entry]]
id = "x"
effective_from = "2025-06-30"
legal_ref = "ref 2"
"#;
    assert!(matches!(load_pack(toml), Err(GstError::Pack { .. })));
}

#[test]
fn two_open_ended_entries_of_one_id_overlap() {
    // Derived: both extend forever from 2025, so they overlap from the later start.
    let toml = r#"
version = "open"
[[entry]]
id = "x"
effective_from = "2025-01-01"
legal_ref = "ref 1"
[[entry]]
id = "x"
effective_from = "2026-01-01"
legal_ref = "ref 2"
"#;
    assert!(matches!(load_pack(toml), Err(GstError::Pack { .. })));
}

#[test]
fn entry_with_neither_legal_ref_nor_pending_errors() {
    let toml = r#"
version = "v"
[[entry]]
id = "x"
effective_from = "2025-01-01"
"#;
    assert!(matches!(load_pack(toml), Err(GstError::Pack { .. })));
    // Derived: pending = false and a blank legal_ref are the same as neither.
    let blank = r#"
version = "v"
[[entry]]
id = "x"
effective_from = "2025-01-01"
pending = false
legal_ref = "   "
"#;
    assert!(matches!(load_pack(blank), Err(GstError::Pack { .. })));
}

#[test]
fn malformed_packs_error() {
    // Each is rejected by a rule in packs/README.md: bad TOML, missing version, bad date,
    // from after to, empty id, unknown field, non-string param.
    let cases = [
        "this is = not [ toml",
        "[[entry]]\nid = \"x\"\neffective_from = \"2025-01-01\"\nlegal_ref = \"r\"\n",
        "version = \"v\"\n[[entry]]\nid = \"x\"\neffective_from = \"2025-13-01\"\nlegal_ref = \"r\"\n",
        "version = \"v\"\n[[entry]]\nid = \"x\"\neffective_from = \"2025-05-01\"\neffective_to = \"2025-04-01\"\nlegal_ref = \"r\"\n",
        "version = \"v\"\n[[entry]]\nid = \"\"\neffective_from = \"2025-01-01\"\nlegal_ref = \"r\"\n",
        "version = \"v\"\n[[entry]]\nid = \"x\"\neffective_from = \"2025-01-01\"\nlegal_ref = \"r\"\nbogus = 1\n",
        "version = \"v\"\n[[entry]]\nid = \"x\"\neffective_from = \"2025-01-01\"\nlegal_ref = \"r\"\n[entry.params]\npercent = 12.5\n",
        "version = \"  \"\n",
    ];
    for c in cases {
        assert!(
            matches!(load_pack(c), Err(GstError::Pack { .. })),
            "accepted: {c}"
        );
    }
}

#[test]
fn pending_entry_may_omit_legal_ref() {
    let toml = r#"
version = "v"
[[entry]]
id = "x"
effective_from = "2025-01-01"
pending = true
"#;
    assert!(load_pack(toml).is_ok());
}
