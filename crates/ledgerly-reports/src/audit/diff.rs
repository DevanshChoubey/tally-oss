//! Field diff of an alteration event (part of the auditor reports, card RP10).
//!
//! Events are compared as JSON so that one routine serves every master and voucher type. Money
//! needs care: a `Paise` serialises as a bare integer number of paise and a `SignedAmount` as
//! `{"amount": <paise>, "side": "Dr" | "Cr"}`. An auditor reading `10000` could take it for rupees,
//! so both are rendered in rupees with the prefix `Rs` (and the side for a `SignedAmount`).
//!
//! A bare integer is recognised as paise by the name of its field (`amount`, `round_off`,
//! `taxable`, `tax`: every `Paise` field that an event can carry, see the model crate). A
//! `SignedAmount` is recognised by its shape. Any other integer is shown as it is.

use std::collections::BTreeSet;

use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, EventSeq, Paise};
use ledgerly_model::MasterSnapshot;
use ledgerly_store::{AuditRecord, Event};
use serde::Serialize;
use serde_json::Value;

use crate::ReportError;

/// Names of the fields that hold a bare `Paise`.
const PAISE_FIELDS: [&str; 4] = ["amount", "round_off", "taxable", "tax"];

/// One changed field of an alteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) field: String,
    pub(super) before: String,
    pub(super) after: String,
}

fn to_json<T: Serialize>(seq: EventSeq, value: &T) -> Result<Value, ReportError> {
    serde_json::to_value(value).map_err(|e| ReportError::Invalid {
        field: "audit record".to_owned(),
        reason: format!("event {} cannot be read as JSON: {e}", seq.0),
    })
}

#[allow(
    clippy::match_same_arms,
    reason = "the arms have the same text but bind values of ten different types"
)]
fn snapshot_json(seq: EventSeq, s: &MasterSnapshot) -> Result<Value, ReportError> {
    match s {
        MasterSnapshot::Group(x) => to_json(seq, x),
        MasterSnapshot::Ledger(x) => to_json(seq, x),
        MasterSnapshot::VoucherType(x) => to_json(seq, x),
        MasterSnapshot::Unit(x) => to_json(seq, x),
        MasterSnapshot::StockGroup(x) => to_json(seq, x),
        MasterSnapshot::StockItem(x) => to_json(seq, x),
        MasterSnapshot::Godown(x) => to_json(seq, x),
        MasterSnapshot::Batch(x) => to_json(seq, x),
        MasterSnapshot::CostCategory(x) => to_json(seq, x),
        MasterSnapshot::CostCentre(x) => to_json(seq, x),
    }
}

/// The fields that changed in an alteration; empty for every other event.
pub(super) fn changes_of(rec: &AuditRecord) -> Result<Vec<Change>, ReportError> {
    let mut out = Vec::new();
    match &rec.event {
        Event::MasterAltered { before, after } => diff_values(
            "",
            &snapshot_json(rec.seq, before)?,
            &snapshot_json(rec.seq, after)?,
            &mut out,
        ),
        Event::VoucherAltered { before, after } => diff_values(
            "",
            &to_json(rec.seq, before)?,
            &to_json(rec.seq, after)?,
            &mut out,
        ),
        Event::SettingChanged { key, before, after } => {
            if before != after {
                out.push(Change {
                    field: key.clone(),
                    before: before.clone(),
                    after: after.clone(),
                });
            }
        }
        Event::MasterCreated(_)
        | Event::MasterDeleted(_)
        | Event::VoucherPosted(_)
        | Event::VoucherCancelled { .. }
        | Event::VoucherDeleted { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::MatchConfirmed(_)
        | Event::AiProposalRecorded(_) => {}
    }
    Ok(out)
}

fn is_container(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Array(_))
}

fn join_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// `Some("Rs 100.00 Dr")` when `v` has the shape of a serialised `SignedAmount`.
fn signed_amount_text(v: &Value) -> Option<String> {
    let obj = v.as_object()?;
    if obj.len() != 2 {
        return None;
    }
    let paise = obj.get("amount")?.as_i64()?;
    let side = obj.get("side")?.as_str()?;
    if side != "Dr" && side != "Cr" {
        return None;
    }
    Some(format!("Rs {} {side}", format_inr(Paise::new(paise))))
}

/// Appends one [`Change`] per leaf that differs between `before` and `after`.
///
/// Objects are compared key by key (keys in sorted order). Arrays of the same length that hold
/// containers are compared element by element (`path[0].field`); any other array is one leaf. A
/// `SignedAmount` is one leaf, so a change of side alone is shown as `Rs 100.00 Dr` to
/// `Rs 100.00 Cr`.
pub(super) fn diff_values(path: &str, before: &Value, after: &Value, out: &mut Vec<Change>) {
    if before == after {
        return;
    }
    if signed_amount_text(before).is_some() && signed_amount_text(after).is_some() {
        out.push(Change {
            field: path.to_owned(),
            before: render(path, before),
            after: render(path, after),
        });
        return;
    }
    match (before, after) {
        (Value::Object(old), Value::Object(new)) => {
            let keys: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
            for key in keys {
                let sub = join_path(path, key);
                match (old.get(key), new.get(key)) {
                    (Some(left), Some(right)) => diff_values(&sub, left, right, out),
                    (Some(left), None) => out.push(Change {
                        before: render(&sub, left),
                        after: "(absent)".to_owned(),
                        field: sub,
                    }),
                    (None, Some(right)) => out.push(Change {
                        before: "(absent)".to_owned(),
                        after: render(&sub, right),
                        field: sub,
                    }),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(old), Value::Array(new))
            if old.len() == new.len() && old.iter().chain(new.iter()).any(is_container) =>
        {
            for (index, (left, right)) in old.iter().zip(new.iter()).enumerate() {
                diff_values(&format!("{path}[{index}]"), left, right, out);
            }
        }
        _ => out.push(Change {
            field: path.to_owned(),
            before: render(path, before),
            after: render(path, after),
        }),
    }
}

/// Text of a leaf. A field whose name contains `date` and whose value is a `(year, ordinal)`
/// pair (how `time` serialises a `Date` without its human-readable feature) is shown as a date.
/// A `SignedAmount` and an integer in a `Paise` field are shown in rupees (see the module docs).
fn render(path: &str, v: &Value) -> String {
    let name = path.rsplit(['.', '[']).next().unwrap_or(path);
    if name.contains("date")
        && let Some(d) = date_from_pair(v)
    {
        return d.to_string();
    }
    if let Some(text) = signed_amount_text(v) {
        return text;
    }
    if PAISE_FIELDS.contains(&name)
        && let Some(paise) = v.as_i64()
    {
        return format!("Rs {}", format_inr(Paise::new(paise)));
    }
    match v {
        Value::Null => "-".to_owned(),
        Value::String(s) => s.clone(),
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => v.to_string(),
    }
}

fn date_from_pair(v: &Value) -> Option<Date> {
    let arr = v.as_array()?;
    let [year, ordinal] = arr.as_slice() else {
        return None;
    };
    let year = i32::try_from(year.as_i64()?).ok()?;
    let ordinal = u16::try_from(ordinal.as_u64()?).ok()?;
    Date::from_ordinal_date(year, ordinal).ok()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]
mod tests {
    use super::*;
    use serde_json::json;

    fn changes(before: &Value, after: &Value) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        diff_values("", before, after, &mut out);
        out.into_iter()
            .map(|c| (c.field, c.before, c.after))
            .collect()
    }

    fn triple(f: &str, b: &str, a: &str) -> (String, String, String) {
        (f.to_owned(), b.to_owned(), a.to_owned())
    }

    #[test]
    fn diff_lists_only_changed_leaves() {
        // derived: of a, b, c only b differs (2 -> 3).
        let got = changes(
            &json!({"a": 1, "b": 2, "c": "x"}),
            &json!({"a": 1, "b": 3, "c": "x"}),
        );
        assert_eq!(got, vec![triple("b", "2", "3")]);
    }

    #[test]
    fn diff_recurses_into_objects_and_equal_length_arrays() {
        // derived: only lines[1].amount changed, from 5 to 6 paise, shown in rupees as 0.05 and
        // 0.06 (100 paise to the rupee).
        let got = changes(
            &json!({"lines": [{"amount": 1}, {"amount": 5}]}),
            &json!({"lines": [{"amount": 1}, {"amount": 6}]}),
        );
        assert_eq!(got, vec![triple("lines[1].amount", "Rs 0.05", "Rs 0.06")]);
    }

    #[test]
    fn diff_treats_arrays_of_different_length_as_one_leaf() {
        // derived: [1,2] became [1,2,3]; one leaf with compact JSON on both sides.
        let got = changes(&json!({"v": [1, 2]}), &json!({"v": [1, 2, 3]}));
        assert_eq!(got, vec![triple("v", "[1,2]", "[1,2,3]")]);
    }

    #[test]
    fn date_pair_renders_as_date() {
        // derived: 2026 is not a leap year; ordinal 100 = 31 + 28 + 31 + 10 = 10 April.
        let got = changes(&json!({"date": [2026, 100]}), &json!({"date": [2026, 101]}));
        assert_eq!(got, vec![triple("date", "2026-04-10", "2026-04-11")]);
    }

    #[test]
    fn null_renders_as_dash_and_string_unquoted() {
        // derived: by the rendering rule, null is "-" and strings are shown without quotes.
        let got = changes(&json!({"n": null}), &json!({"n": "text"}));
        assert_eq!(got, vec![triple("n", "-", "text")]);
    }

    #[test]
    fn paise_field_renders_in_rupees_not_paise() {
        // derived: 1 rupee = 100 paise, so 10000 paise is 100.00 rupees and 12345 paise is
        // 123.45 rupees (format_inr documents two decimals).
        let got = changes(
            &json!({"lines": [{"amount": 10000}]}),
            &json!({"lines": [{"amount": 12345}]}),
        );
        assert_eq!(
            got,
            vec![triple("lines[0].amount", "Rs 100.00", "Rs 123.45")]
        );
    }

    #[test]
    fn signed_amount_renders_in_rupees_with_its_side() {
        // derived: a SignedAmount serialises as {amount: paise, side}; 10000 paise = Rs 100.00.
        // Only the side changed, so the leaf is the whole amount, Dr before and Cr after.
        let got = changes(
            &json!({"amount": {"amount": 10000, "side": "Dr"}}),
            &json!({"amount": {"amount": 10000, "side": "Cr"}}),
        );
        assert_eq!(got, vec![triple("amount", "Rs 100.00 Dr", "Rs 100.00 Cr")]);
    }

    #[test]
    fn signed_amount_added_or_removed_is_shown_in_rupees() {
        // derived: a key present on one side only is "(absent)" on the other; the amount side
        // is Rs 5.00 Cr (500 paise).
        let got = changes(
            &json!({"x": 1}),
            &json!({"x": 1, "opening": {"amount": 500, "side": "Cr"}}),
        );
        assert_eq!(got, vec![triple("opening", "(absent)", "Rs 5.00 Cr")]);
    }

    #[test]
    fn integer_in_an_unnamed_field_is_not_taken_for_paise() {
        // derived: `count` is not a money field, so 10000 stays 10000.
        let got = changes(&json!({"count": 10000}), &json!({"count": 20000}));
        assert_eq!(got, vec![triple("count", "10000", "20000")]);
    }
}
