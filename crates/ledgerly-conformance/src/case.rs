//! Case format with provenance. **Card Q1** implements this module.
//!
//! Frozen interface (plan card Q1):
//!
//! ```text
//! pub enum Provenance { Observed { tool: String, version: String, date: Date }, Derived { reference: String, working: String }, Pending }
//! ```
//!
//! A case file is TOML with one `[[case]]` table per case:
//!
//! ```toml
//! [[case]]
//! id = "core.gstin.check_char.mh"
//! description = "Maharashtra fixture GSTIN check character"
//! [case.provenance]
//! kind = "derived"            # or "observed" (tool, version, date) or "pending"
//! reference = "..."
//! working = "..."
//! [case.check]
//! kind = "gstin_check_character"
//! first14 = "27ABCDE1234F1Z"
//! expected = "0"
//! ```
//!
//! A case with no `provenance` table is [`Provenance::Pending`] (engineering rule A5). Unknown
//! keys and unknown check kinds are parse errors, so a misspelt key can never turn into a silent
//! default. One exception: serde ignores extra keys inside `kind = "pending"` (a unit variant), so
//! such a table still parses as pending. That is safe, since pending never asserts a value.

use ledgerly_core::{Date, Decimal, Paise};
use serde::{Deserialize, Serialize};

/// Where a case's expected value comes from (engineering rule A4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provenance {
    /// Captured from the product or an official tool.
    Observed {
        /// Tool name, such as the accounting product or the GST offline tool.
        tool: String,
        /// Release or version of the tool.
        version: String,
        /// Date of the capture.
        #[serde(with = "iso_date")]
        date: Date,
    },
    /// Worked out by hand from a public rule.
    Derived {
        /// The Act, rule, notification, standard or document the working relies on.
        reference: String,
        /// The arithmetic or argument, written out.
        working: String,
    },
    /// No provenance yet. The feature stays unverified.
    #[default]
    Pending,
}

impl Provenance {
    /// Why this provenance does not establish an expected value, or `None` when it does.
    ///
    /// Blank text counts as missing: an empty `reference` or `working` is no provenance.
    pub fn pending_reason(&self) -> Option<String> {
        match self {
            Provenance::Pending => Some("no provenance recorded".to_owned()),
            Provenance::Derived { reference, working } => {
                if reference.trim().is_empty() {
                    Some("derived provenance has no reference".to_owned())
                } else if working.trim().is_empty() {
                    Some("derived provenance has no working".to_owned())
                } else {
                    None
                }
            }
            Provenance::Observed { tool, version, .. } => {
                if tool.trim().is_empty() {
                    Some("observed provenance names no tool".to_owned())
                } else if version.trim().is_empty() {
                    Some("observed provenance names no version".to_owned())
                } else {
                    None
                }
            }
        }
    }
}

/// One golden case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Unique id, dotted by feature (no whitespace). The pass-list refers to cases by id.
    pub id: String,
    /// What the case covers.
    #[serde(default)]
    pub description: String,
    /// Where the expected value comes from. Absent means [`Provenance::Pending`].
    #[serde(default)]
    pub provenance: Provenance,
    /// What to run and the expected value. A pending case may leave this out.
    #[serde(default)]
    pub check: Option<Check>,
}

/// What a case runs. Closed set: a new kind needs code in the runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    /// `ledgerly_core::gstin_check_character(first14) == expected`.
    GstinCheckCharacter {
        /// The first 14 characters of a GSTIN.
        first14: String,
        /// The expected check character.
        expected: char,
    },
    /// The sum of all postings of `books_small()`, debit positive, is zero.
    FixturePostingsBalance,
    /// Net of the regular postings on a ledger of `books_small()`, debit positive.
    FixtureLedgerNet {
        /// Ledger name.
        ledger: String,
        /// Expected net: positive is debit, negative is credit.
        net_paise: Paise,
    },
    /// Opening plus quantity in minus quantity out of an item of `books_small()`, base units.
    FixtureStockQty {
        /// Stock item name.
        item: String,
        /// Expected closing quantity (written as a string, for example `"40"`).
        closing_qty: Decimal,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
    #[serde(default)]
    case: Vec<Case>,
}

/// Parses the text of a case file.
///
/// # Errors
/// The TOML error, which names the line and the unknown key or check kind.
pub fn parse_cases(text: &str) -> Result<Vec<Case>, toml::de::Error> {
    toml::from_str::<CaseFile>(text).map(|f| f.case)
}

/// `YYYY-MM-DD` dates in case files.
mod iso_date {
    use ledgerly_core::Date;
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _, ser::Error as _};
    use time::format_description::well_known::Iso8601;

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's serialize_with calls this with &Date"
    )]
    pub(super) fn serialize<S: Serializer>(date: &Date, s: S) -> Result<S::Ok, S::Error> {
        let text = date.format(&Iso8601::DATE).map_err(S::Error::custom)?;
        s.serialize_str(&text)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        let text = String::deserialize(d)?;
        Date::parse(&text, &Iso8601::DATE)
            .map_err(|e| D::Error::custom(format!("date '{text}' must be YYYY-MM-DD: {e}")))
    }
}
