//! Card C2 tests: the GSTR-1 and input-credit pack file `packs/gstr-rules.toml`.
//!
//! Provenance. Every expectation is `derived` from the sources read on 2026-11-21 and cited next to
//! the test:
//! * Rule 59(4) of the CGST Rules as amended by Notification No. 12/2024-Central Tax dated
//!   10 July 2024 (<https://www.gstcouncil.gov.in/sites/default/files/2024-09/central-tax-12-2024-11072024.pdf>,
//!   text: "two and a half lakh rupees" becomes "one lakh rupees" with effect from 1 August 2024,
//!   "invoice value more than one lakh rupees") and the GSTR-1 user guide
//!   <https://tutorial.gst.gov.in/userguide/returns/Creation_of_Outward_Supplies_Return_in_GSTR-1.htm>
//!   (Table 5A, 5B: "more than Rs. 1 lakh").
//! * CGST Act s.17(5), <https://cbic-gst.gov.in/pdf/CGST-Act-Updated-31082021.pdf>: it names no HSN
//!   or SAC code and its exceptions depend on use, so no category list can be read from it. Those
//!   three entries are `pending`, and ITC under them is `Unsupported`.
//! * `gstr1.hsn_credit_notes`: neither the GSTR-1 user guide nor
//!   <https://help.tallysolutions.com/tally-prime/gstr-1/hsn-sac-summary/> says whether notes reduce
//!   Table 12, so the entry is `pending`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::Outcome;
use ledgerly_gst::gstr1::{B2CL_THRESHOLD_ID, HSN_CREDIT_NOTES_ID};
use ledgerly_gst::itc::BLOCKED_RULE_IDS;
use ledgerly_gst::pack::{RulePack, RuleSet, load_pack};
use time::macros::date;

const RULES: &str = include_str!("../packs/gstr-rules.toml");
const SLABS: &str = include_str!("../packs/rates-slabs.toml");

fn rules() -> RulePack {
    load_pack(RULES).unwrap()
}

fn on(pack: &RulePack, d: time::Date) -> RuleSet {
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

/// Derived: the notification brings the Rs 1 lakh threshold in on 1 August 2024, "more than one
/// lakh rupees" is read by the builder as strictly above, so the parameter is 100000 rupees.
#[test]
fn b2cl_threshold_is_one_lakh_from_1_august_2024() {
    let set = on(&rules(), date!(2024 - 08 - 01));
    let Outcome::Value(entry) = set.get(B2CL_THRESHOLD_ID) else {
        panic!("the b2cl threshold must be verified");
    };
    assert_eq!(
        entry.params().get("invoice_value_rupees").map(String::as_str),
        Some("100000")
    );
    let legal = entry.legal_ref().unwrap();
    assert!(legal.contains("12/2024"), "legal_ref was: {legal}");
    assert!(legal.contains("10 July 2024"), "legal_ref was: {legal}");
}

/// Derived: the first day of the rule is 1 August 2024. The day before has no entry, so the
/// threshold is `Unsupported` there (the earlier Rs 2.5 lakh rule is not in the pack, since its
/// start date was not read).
#[test]
fn b2cl_threshold_is_not_given_before_1_august_2024() {
    let pack = merged();
    let set = on(&pack, date!(2024 - 07 - 31));
    assert!(matches!(
        set.get(B2CL_THRESHOLD_ID),
        Outcome::Unsupported(_)
    ));
}

/// Derived: no source states how Table 12 treats credit notes, so the entry is pending and
/// evaluates to `Unsupported`.
#[test]
fn hsn_credit_note_treatment_is_pending() {
    let pack = rules();
    let set = on(&pack, date!(2026 - 10 - 05));
    assert!(matches!(
        set.get(HSN_CREDIT_NOTES_ID),
        Outcome::Unsupported(_)
    ));
    let entry = pack
        .entries()
        .iter()
        .find(|e| e.id() == HSN_CREDIT_NOTES_ID)
        .unwrap();
    assert!(entry.is_pending());
    assert!(entry.params().contains_key("source_note"));
}

/// Derived: s.17(5) names no HSN code, so every `itc.blocked.*` entry stays pending and names the
/// section it was read from; none may evaluate to a value.
#[test]
fn blocked_credit_entries_are_pending_with_a_source_note() {
    let pack = rules();
    let set = on(&pack, date!(2026 - 10 - 05));
    for id in BLOCKED_RULE_IDS {
        assert!(
            matches!(set.get(id), Outcome::Unsupported(_)),
            "{id} must be Unsupported"
        );
        let entry = pack.entries().iter().find(|e| e.id() == id).unwrap();
        assert!(entry.is_pending(), "{id}");
        assert!(
            entry.params()["source_note"].contains("17(5)"),
            "{id}: source_note must cite s.17(5)"
        );
    }
}

fn merged() -> RulePack {
    load_pack(SLABS).unwrap().merge(&rules()).unwrap()
}

/// Derived from the merge definition: both packs' entries are present, the version joins the two
/// versions, and the slab pack's own entries still evaluate as before.
#[test]
fn the_slab_pack_and_the_rules_pack_merge() {
    let slabs = load_pack(SLABS).unwrap();
    let both = merged();
    assert_eq!(
        both.version(),
        format!("{}+{}", slabs.version(), rules().version())
    );
    assert_eq!(
        both.entries().len(),
        slabs.entries().len() + rules().entries().len()
    );
    let set = on(&both, date!(2026 - 10 - 05));
    assert!(matches!(set.get("slab.eighteen"), Outcome::Value(_)));
    assert!(matches!(set.get(B2CL_THRESHOLD_ID), Outcome::Value(_)));
}

/// Derived: two entries of one id whose ranges share a day are refused by a merge, as by a load.
#[test]
fn merging_a_pack_with_itself_is_refused() {
    let r = rules();
    assert!(r.merge(&r).is_err());
}
