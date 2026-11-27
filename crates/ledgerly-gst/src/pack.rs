//! Rule pack format, loading and effective dating. **Card G1** implements this module.
//!
//! Frozen interface (plan card G1):
//!
//! ```text
//! pub struct RulePack { .. }   pub struct RuleSet { .. }   // defined by G1
//! pub fn load_pack(toml: &str) -> Result<RulePack, GstError>;
//! impl RulePack { pub fn for_time_of_supply(&self, date: Date) -> Outcome<RuleSet>; pub fn version(&self) -> &str; }
//! ```
//!
//! The file format is described in `packs/README.md`. In short, a pack is a `version` plus
//! `[[entry]]` tables. Each entry has an `id`, an inclusive `effective_from` (and optional
//! `effective_to`), string `params`, and either a `legal_ref` or `pending = true`. A pending entry
//! evaluates to `Unsupported` (engineering rule A8). Entries are selected by the time of supply,
//! never by the entry date.

use std::collections::BTreeMap;

use ledgerly_core::{Date, Outcome};
use serde::Deserialize;
use time::macros::format_description;

use crate::GstError;

/// One effective-dated rule in a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackEntry {
    id: String,
    effective_from: Date,
    effective_to: Option<Date>,
    legal_ref: Option<String>,
    pending: bool,
    params: BTreeMap<String, String>,
}

impl PackEntry {
    /// The rule identifier (for example a rate or cess key). Several entries share an id when the
    /// rule changes over time.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// First day the entry applies (inclusive).
    pub fn effective_from(&self) -> Date {
        self.effective_from
    }

    /// Last day the entry applies (inclusive); `None` means no end date.
    pub fn effective_to(&self) -> Option<Date> {
        self.effective_to
    }

    /// The Act section, rule or notification the entry rests on, when given.
    pub fn legal_ref(&self) -> Option<&str> {
        self.legal_ref.as_deref()
    }

    /// True when the entry is unverified and must evaluate to `Unsupported`.
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// The entry's parameters, as written (strings; callers parse them to `Decimal` or `Paise`).
    pub fn params(&self) -> &BTreeMap<String, String> {
        &self.params
    }

    fn covers(&self, date: Date) -> bool {
        date >= self.effective_from && self.effective_to.is_none_or(|to| date <= to)
    }
}

/// A loaded, validated rule pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePack {
    version: String,
    entries: Vec<PackEntry>,
}

/// The rules in force on one date, keyed by entry id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    version: String,
    date: Date,
    entries: BTreeMap<String, PackEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPack {
    version: String,
    #[serde(default)]
    entry: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    id: String,
    effective_from: String,
    effective_to: Option<String>,
    legal_ref: Option<String>,
    #[serde(default)]
    pending: bool,
    #[serde(default)]
    params: BTreeMap<String, String>,
}

fn pack_err(reason: String) -> GstError {
    GstError::Pack { reason }
}

fn parse_date(id: &str, field: &str, text: &str) -> Result<Date, GstError> {
    Date::parse(text, format_description!("[year]-[month]-[day]")).map_err(|e| {
        pack_err(format!(
            "entry `{id}`: {field} `{text}` is not a YYYY-MM-DD date ({e})"
        ))
    })
}

fn convert(raw: RawEntry) -> Result<PackEntry, GstError> {
    let id = raw.id.trim().to_owned();
    if id.is_empty() {
        return Err(pack_err("an entry has an empty id".to_owned()));
    }
    let effective_from = parse_date(&id, "effective_from", &raw.effective_from)?;
    let effective_to = match raw.effective_to {
        Some(t) => Some(parse_date(&id, "effective_to", &t)?),
        None => None,
    };
    if let Some(to) = effective_to
        && to < effective_from
    {
        return Err(pack_err(format!(
            "entry `{id}`: effective_to {to} is before effective_from {effective_from}"
        )));
    }
    let legal_ref = raw
        .legal_ref
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    if legal_ref.is_none() && !raw.pending {
        return Err(pack_err(format!(
            "entry `{id}` from {effective_from} has neither a legal_ref nor pending = true"
        )));
    }
    Ok(PackEntry {
        id,
        effective_from,
        effective_to,
        legal_ref,
        pending: raw.pending,
        params: raw.params,
    })
}

/// Two entries of one id overlap when their inclusive ranges share a day. Entries are sorted by
/// start, so checking neighbours is enough: if an earlier entry reached a later one it would also
/// reach every entry in between.
fn check_no_overlap(entries: &[PackEntry]) -> Result<(), GstError> {
    let mut by_id: BTreeMap<&str, Vec<&PackEntry>> = BTreeMap::new();
    for e in entries {
        by_id.entry(e.id.as_str()).or_default().push(e);
    }
    for (id, mut group) in by_id {
        group.sort_by_key(|e| e.effective_from);
        for (a, b) in group.iter().zip(group.iter().skip(1)) {
            if a.effective_to.is_none_or(|to| to >= b.effective_from) {
                return Err(pack_err(format!(
                    "entry `{id}`: range starting {} overlaps range starting {}",
                    a.effective_from, b.effective_from
                )));
            }
        }
    }
    Ok(())
}

/// Parses and validates a rule pack.
///
/// # Errors
/// [`GstError::Pack`] for malformed TOML, unknown fields, a blank version or id, a bad date,
/// `effective_to` before `effective_from`, an entry with neither `legal_ref` nor `pending = true`,
/// or two entries of one id whose effective ranges overlap.
pub fn load_pack(toml: &str) -> Result<RulePack, GstError> {
    let raw: RawPack =
        ::toml::from_str(toml).map_err(|e| pack_err(format!("cannot parse pack: {e}")))?;
    let version = raw.version.trim().to_owned();
    if version.is_empty() {
        return Err(pack_err("pack version is blank".to_owned()));
    }
    let entries = raw
        .entry
        .into_iter()
        .map(convert)
        .collect::<Result<Vec<_>, _>>()?;
    check_no_overlap(&entries)?;
    Ok(RulePack { version, entries })
}

impl RulePack {
    /// Same as [`load_pack`]; the plan card names it `RulePack::load`.
    ///
    /// # Errors
    /// See [`load_pack`].
    pub fn load(toml: &str) -> Result<RulePack, GstError> {
        load_pack(toml)
    }

    /// The pack's version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// All entries, in file order.
    pub fn entries(&self) -> &[PackEntry] {
        &self.entries
    }

    /// A pack holding the entries of `self` and then those of `other`, versioned
    /// `"<self>+<other>"`. Used to read the rate slabs together with the GSTR-1 and input credit
    /// rules, which ship in separate files.
    ///
    /// # Errors
    /// [`GstError::Pack`] when two entries of one id have effective ranges that share a day.
    pub fn merge(&self, other: &RulePack) -> Result<RulePack, GstError> {
        let mut entries = self.entries.clone();
        entries.extend(other.entries.iter().cloned());
        check_no_overlap(&entries)?;
        Ok(RulePack {
            version: format!("{}+{}", self.version, other.version),
            entries,
        })
    }

    /// The rules in force on `date`, which must be the time of supply (never the entry date).
    /// `Unsupported` when no entry of the pack covers that date.
    pub fn for_time_of_supply(&self, date: Date) -> Outcome<RuleSet> {
        let entries: BTreeMap<String, PackEntry> = self
            .entries
            .iter()
            .filter(|e| e.covers(date))
            .map(|e| (e.id.clone(), e.clone()))
            .collect();
        if entries.is_empty() {
            return Outcome::unsupported(
                format!("GST rule pack {} on {date}", self.version),
                "no rule in this pack is in force on the time of supply; update the rule packs or check the date",
            );
        }
        Outcome::Value(RuleSet {
            version: self.version.clone(),
            date,
            entries,
        })
    }
}

impl RuleSet {
    /// The time of supply this set was selected for.
    pub fn date(&self) -> Date {
        self.date
    }

    /// Version of the pack the set came from.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// `true` when the pack has an entry `id` (pending or not) in force on this date.
    pub fn has(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// The rule `id` in force on this date. `Unsupported` when the pack has no such rule on that
    /// date or the entry is pending; there is no default.
    pub fn get(&self, id: &str) -> Outcome<&PackEntry> {
        match self.entries.get(id) {
            None => Outcome::unsupported(
                format!("GST rule `{id}` on {}", self.date),
                format!("pack {} has no entry for it on that date", self.version),
            ),
            Some(e) if e.pending => Outcome::unsupported(
                format!("GST rule `{id}` on {}", self.date),
                format!(
                    "pack {} marks it pending: no legal reference has been checked yet",
                    self.version
                ),
            ),
            Some(e) => Outcome::Value(e),
        }
    }
}
