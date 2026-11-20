//! Card G2 tests: the shipped rate packs (`packs/rates-*.toml`), spec 6.3, rules A4, A5, A8.
//!
//! Provenance. The slab figures below come from `docs/research/04-gst.md` section 2.2, row "GST 2.0
//! slabs", tagged `[V]` (verified against official advisories / copies of N.9/2025-CT(R),
//! N.15/2025-CT(R), dated 17 Sep 2025, effective 22 Sep 2025). Nobody on this project has read the
//! notification text itself, so every such expectation is `derived (research [V])`, and the arithmetic
//! relating CGST, SGST and IGST is hand-derived in the test. Pre-change slabs, cess figures and
//! HSN mappings are `[K]`/`[S]` in the research, so they are `pending` and must evaluate to
//! `Unsupported`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration-test helpers outside #[test] fns may panic on a broken fixture"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::Outcome;
use ledgerly_gst::pack::{PackEntry, RulePack, RuleSet, load_pack};
use rust_decimal::Decimal;
use std::str::FromStr;
use time::macros::date;

fn pack_files() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packs");
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with("rates-")
            && path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("toml"))
        {
            out.push((name, fs::read_to_string(&path).unwrap()));
        }
    }
    out.sort();
    out
}

fn packs() -> Vec<(String, RulePack)> {
    pack_files()
        .into_iter()
        .map(|(n, t)| {
            let p = load_pack(&t).unwrap_or_else(|e| panic!("{n} does not load: {e}"));
            (n, p)
        })
        .collect()
}

fn slabs() -> RulePack {
    let (_, text) = pack_files()
        .into_iter()
        .find(|(n, _)| n == "rates-slabs.toml")
        .expect("rates-slabs.toml exists");
    load_pack(&text).unwrap()
}

fn set_on(pack: &RulePack, d: time::Date) -> RuleSet {
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn param(set: &RuleSet, id: &str, key: &str) -> Option<String> {
    match set.get(id) {
        Outcome::Value(e) => e.params().get(key).cloned(),
        Outcome::Unsupported(_) => None,
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn the_expected_pack_files_exist() {
    let names: Vec<String> = pack_files().into_iter().map(|(n, _)| n).collect();
    for want in ["rates-cess.toml", "rates-hsn.toml", "rates-slabs.toml"] {
        assert!(names.iter().any(|n| n == want), "missing {want}");
    }
}

#[test]
fn every_entry_has_legal_ref_xor_pending() {
    for (name, pack) in packs() {
        assert!(!pack.version().trim().is_empty(), "{name}: blank version");
        assert!(!pack.entries().is_empty(), "{name}: no entries");
        for e in pack.entries() {
            // Rule A8: exactly one of legal_ref / pending. Pending entries keep their unverified
            // source in params.source_note, never in legal_ref.
            let has_ref = e.legal_ref().is_some_and(|r| !r.trim().is_empty());
            assert!(
                has_ref != e.is_pending(),
                "{name}: entry {} from {} must have legal_ref xor pending",
                e.id(),
                e.effective_from()
            );
            if e.is_pending() {
                assert!(
                    e.params().contains_key("source_note"),
                    "{name}: pending entry {} should say where the fact came from",
                    e.id()
                );
            }
        }
    }
}

#[test]
fn verified_entries_name_a_notification_with_number_and_date() {
    for (name, pack) in packs() {
        for e in pack.entries().iter().filter(|e| !e.is_pending()) {
            let r = e.legal_ref().unwrap();
            assert!(
                r.contains("Notification No.") && r.contains("2025"),
                "{name}: {} legal_ref `{r}` lacks a notification number and date",
                e.id()
            );
        }
    }
}

#[test]
fn no_hsn_appears_twice_in_one_range() {
    for (name, pack) in packs() {
        let mut by_hsn: BTreeMap<String, Vec<(time::Date, Option<time::Date>)>> = BTreeMap::new();
        for e in pack.entries() {
            if let Some(h) = e.params().get("hsn") {
                by_hsn
                    .entry(h.clone())
                    .or_default()
                    .push((e.effective_from(), e.effective_to()));
            }
        }
        for (hsn, mut ranges) in by_hsn {
            ranges.sort();
            for w in ranges.windows(2) {
                // Derived: inclusive ranges overlap when the earlier one reaches the later start.
                let overlaps = w[0].1.is_none_or(|to| to >= w[1].0);
                assert!(!overlaps, "{name}: HSN {hsn} twice in an overlapping range");
            }
        }
    }
}

#[test]
fn hsn_duplicate_check_would_catch_a_duplicate() {
    // Guards the check above against being vacuous: a fixture with two entries for one HSN whose
    // ranges share 2025-10-01 (derived from the dates written here).
    let toml = r#"
version = "dup"
[[entry]]
id = "a"
effective_from = "2025-09-22"
pending = true
[entry.params]
hsn = "9999"
[[entry]]
id = "b"
effective_from = "2025-10-01"
pending = true
[entry.params]
hsn = "9999"
"#;
    let pack = load_pack(toml).unwrap();
    let ranges: Vec<_> = pack
        .entries()
        .iter()
        .map(|e| (e.effective_from(), e.effective_to()))
        .collect();
    assert!(ranges[0].1.is_none_or(|to| to >= ranges[1].0));
}

#[test]
fn gst2_slabs_from_22_sep_2025_are_0_5_18_40() {
    let set = set_on(&slabs(), date!(2025 - 09 - 22));
    // Derived (research 04-gst.md 2.2 [V]; N.9/2025-CT(R)): slabs 0, 5, 18, 40 percent, split as
    // CGST 2.5/9/20 plus an equal SGST, or full IGST. 2.5+2.5 = 5; 9+9 = 18; 20+20 = 40.
    for (id, igst, half) in [
        ("slab.zero", "0", "0"),
        ("slab.five", "5", "2.5"),
        ("slab.eighteen", "18", "9"),
        ("slab.forty", "40", "20"),
    ] {
        assert_eq!(
            param(&set, id, "igst_percent").as_deref(),
            Some(igst),
            "{id}"
        );
        assert_eq!(
            param(&set, id, "cgst_percent").as_deref(),
            Some(half),
            "{id}"
        );
        assert_eq!(
            param(&set, id, "sgst_percent").as_deref(),
            Some(half),
            "{id}"
        );
    }
}

#[test]
fn cgst_plus_sgst_equals_igst_in_every_verified_slab() {
    for e in slabs().entries().iter().filter(|e| !e.is_pending()) {
        let p = e.params();
        let (Some(c), Some(s), Some(i)) = (
            p.get("cgst_percent"),
            p.get("sgst_percent"),
            p.get("igst_percent"),
        ) else {
            continue;
        };
        // Derived: the research states CGST and SGST are equal halves of the IGST rate.
        assert_eq!(dec(c), dec(s), "{}", e.id());
        assert_eq!(dec(c) + dec(s), dec(i), "{}", e.id());
    }
}

#[test]
fn the_28_and_12_slabs_ended_and_the_40_slab_did_not_exist_before() {
    let pack = slabs();
    // Derived: GST 2.0 has only 0/5/18/40 from 22 Sep 2025, so on 2025-09-22 there is no entry in
    // force for the 12 and 28 slabs, and none for 40 on 2025-09-21.
    let after = set_on(&pack, date!(2025 - 09 - 22));
    assert!(after.get("slab.twelve").is_unsupported());
    assert!(after.get("slab.twenty_eight").is_unsupported());
    let before = set_on(&pack, date!(2025 - 09 - 21));
    assert!(before.get("slab.forty").is_unsupported());
}

#[test]
fn pre_change_slabs_are_pending_and_unsupported() {
    let before = set_on(&slabs(), date!(2025 - 09 - 21));
    // Derived (rule A5/A8): the pre-22-Sep-2025 slab structure is `[K]` in the research, so it is
    // pending and must not yield a number, including for the slabs that kept their percent.
    for id in [
        "slab.zero",
        "slab.five",
        "slab.twelve",
        "slab.eighteen",
        "slab.twenty_eight",
    ] {
        assert!(before.get(id).is_unsupported(), "{id} must be pending");
    }
}

#[test]
fn rate_is_selected_by_time_of_supply_around_the_change() {
    let pack = slabs();
    // Derived: 2025-09-21 is the day before the effective date; 2025-09-22 the first day (inclusive).
    assert!(
        set_on(&pack, date!(2025 - 09 - 21))
            .get("slab.eighteen")
            .is_unsupported()
    );
    assert_eq!(
        param(
            &set_on(&pack, date!(2025 - 09 - 22)),
            "slab.eighteen",
            "igst_percent"
        )
        .as_deref(),
        Some("18")
    );
    // Derived: the post-change entries are open-ended.
    assert_eq!(
        param(
            &set_on(&pack, date!(2040 - 01 - 01)),
            "slab.forty",
            "igst_percent"
        )
        .as_deref(),
        Some("40")
    );
}

#[test]
fn date_before_gst_exists_is_unsupported() {
    // Derived: the Act took effect on 1 Jul 2017 (research 2.1 `[K]`, used here only as the
    // earliest start of the pending entries); nothing is in force on 2017-06-30.
    assert!(
        slabs()
            .for_time_of_supply(date!(2017 - 06 - 30))
            .is_unsupported()
    );
}

#[test]
fn special_rates_are_verified_only_where_the_research_is() {
    let set = set_on(&slabs(), date!(2025 - 09 - 22));
    // Derived (research 2.2 [V], N.9/2025-CT(R)): special rates 0.25 (rough diamonds) and 3
    // (gold and jewellery) remain. The 6 percent CGST construction rate cites N.14/2025 whose date
    // the research does not give, so it is pending.
    assert_eq!(
        param(&set, "special.rough_diamonds", "igst_percent").as_deref(),
        Some("0.25")
    );
    assert_eq!(
        param(&set, "special.gold_jewellery", "igst_percent").as_deref(),
        Some("3")
    );
    assert!(set.get("special.construction_n14_2025").is_unsupported());
}

#[test]
fn cess_entries_are_all_pending() {
    let cess = pack_files()
        .into_iter()
        .find(|(n, _)| n == "rates-cess.toml")
        .map(|(_, t)| load_pack(&t).unwrap())
        .unwrap();
    // Derived (rule A5): the cess facts are `[V]` for dates only and `[S]`/unconfirmed for rates in
    // the research, so no cess entry is verified.
    assert!(cess.entries().iter().all(PackEntry::is_pending));
    let set = set_on(&cess, date!(2026 - 02 - 01));
    assert!(set.get("cess.tobacco").is_unsupported());
}

#[test]
fn hsn_entries_are_all_pending() {
    let hsn = pack_files()
        .into_iter()
        .find(|(n, _)| n == "rates-hsn.toml")
        .map(|(_, t)| load_pack(&t).unwrap())
        .unwrap();
    // Derived (rule A5): no HSN-to-rate mapping is `[V]` in the research; the schedule was not read.
    assert!(hsn.entries().iter().all(PackEntry::is_pending));
    assert!(hsn.entries().iter().all(|e| e.params().contains_key("hsn")));
}
