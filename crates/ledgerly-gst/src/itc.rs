//! Input tax credit eligibility, including s.17(5). **Card G7** implements this module.
//!
//! Frozen interface (plan card G7):
//!
//! ```text
//! pub enum ItcEligibility { Eligible, Blocked { section: String }, Ineligible { reason: String } }
//! pub fn itc_eligibility(v: &Voucher, line: usize, masters: &Masters, rules: &RuleSet) -> Result<Outcome<ItcEligibility>, GstError>;
//! ```
//!
//! # What `line` is
//!
//! `line` is a zero-based position in [`Voucher::ledger_lines`]. It must be a purchase or expense
//! line, that is a line whose ledger carries a GST classification. The category of the line comes
//! from its HSN/SAC: the ledger's own code, or, when the ledger has none, the codes of the stock
//! items of the inventory lines posted to that ledger.
//!
//! # How blocking is decided (pack-driven)
//!
//! Which HSN/SAC codes fall in a blocked category is data, not code. The rule pack holds one entry
//! per blocked category, with the ids in [`BLOCKED_RULE_IDS`] and these params (strings):
//!
//! ```text
//! section      = "17(5)(a)"        # shown to the user in `Blocked { section }`
//! hsn_prefixes = "8703,8702"       # comma separated; a code is in the category when it starts with one
//! ```
//!
//! The entries are effective-dated like any other rule, so the rule set selected for the time of
//! supply decides. If any of these entries is missing from the rule set, or is pending, the result
//! is `Unsupported`: an unverifiable blocked category means "not blocked" cannot be claimed
//! (engineering rule A8). Until the pack has these entries, every call is therefore `Unsupported`.
//!
//! # What is decided
//!
//! * `Ineligible`: the ledger is exempt, nil-rated or non-GST (no tax was charged), or the supplier
//!   is a composition dealer, or is unregistered/a consumer and the voucher is not under reverse
//!   charge (no tax invoice with tax, CGST Act s.16(2)(a), s.10(4)).
//! * `Blocked`: the HSN/SAC is in a blocked category of the pack.
//! * `Eligible`: taxable purchase from a registered (or SEZ) supplier, or a reverse-charge
//!   purchase, whose HSN/SAC is in no blocked category of the pack.
//!
//! # Not decided (the result is not a claim about these)
//!
//! `Eligible` means "not blocked by an HSN/SAC based category and no ineligibility found". It does
//! not check the conditions of s.16(2) (2B match, receipt, supplier payment, 180 days), Rules 42
//! and 43, or the s.17(5) clauses that depend on use or events the books do not record: (c) and
//! (d) construction, (e), (f), (fa), (g) personal consumption, (h) lost or gifted goods, (i). The
//! exceptions to (a) and (b) (further supply, passenger transport, mandatory by law and so on)
//! depend on use too, so a vehicle bought by a transport business is reported `Blocked` until a
//! use flag exists (see `docs/interface-requests.md`). Callers must treat the result as a
//! first-pass classification, shown to the user for confirmation.
//!
//! `Unsupported` is returned for: a voucher that is not a regular purchase, a cancelled voucher,
//! an overseas or UIN supplier (imports are not handled), a line without a classification or
//! HSN/SAC, a ledger line mixing categories, and the missing/pending pack entries above.
//!
//! Only claimable credit may reach GSTR-3B table 4(A): use [`ItcEligibility::is_claimable`].
//!
//! [`Voucher::ledger_lines`]: ledgerly_model::Voucher

use std::collections::BTreeSet;

use ledgerly_core::{Hsn, Outcome};
use ledgerly_model::{
    BaseVoucherKind, Ledger, Masters, RegistrationType, Taxability, Voucher, VoucherStatus,
};

use crate::GstError;
use crate::pack::{PackEntry, RuleSet};

/// Rule pack entry ids that hold a blocked-credit category (see the module docs for the params).
///
/// A category added to a pack under another id is not read here; the caller sees no effect, so new
/// categories are added to this list in the same change as the pack entry.
pub const BLOCKED_RULE_IDS: [&str; 3] = [
    "itc.blocked.motor_vehicles",
    "itc.blocked.vessels_aircraft",
    "itc.blocked.food_beverages",
];

/// Whether the tax on one purchase line may be claimed as input tax credit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItcEligibility {
    /// No blocking or ineligibility found (see the module docs for what is not checked).
    Eligible,
    /// Credit blocked by CGST Act s.17(5).
    Blocked {
        /// The clause, as written in the pack, for example `17(5)(a)`.
        section: String,
    },
    /// Credit not available for a reason other than s.17(5).
    Ineligible {
        /// What the user can read to understand why.
        reason: String,
    },
}

impl ItcEligibility {
    /// `true` only for `Eligible`. Blocked and ineligible credit must never reach GSTR-3B 4(A).
    pub fn is_claimable(&self) -> bool {
        match self {
            ItcEligibility::Eligible => true,
            ItcEligibility::Blocked { .. } | ItcEligibility::Ineligible { .. } => false,
        }
    }
}

/// Returns the value of an `Outcome`, or leaves the function with the `Unsupported`.
macro_rules! supported {
    ($e:expr) => {
        match $e {
            Outcome::Value(v) => v,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        }
    };
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "builds the Result<Outcome> every check returns, so callers can `return unsupported(..)`"
)]
fn unsupported<T>(
    what: impl Into<String>,
    reason: impl Into<String>,
) -> Result<Outcome<T>, GstError> {
    Ok(Outcome::unsupported(what, reason))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "builds the Result<Outcome> every check returns, so callers can `return value(..)`"
)]
fn value(e: ItcEligibility) -> Result<Outcome<ItcEligibility>, GstError> {
    Ok(Outcome::Value(e))
}

fn ineligible(reason: &str) -> Result<Outcome<ItcEligibility>, GstError> {
    value(ItcEligibility::Ineligible {
        reason: reason.to_owned(),
    })
}

fn not_found(what: String) -> GstError {
    GstError::NotFound { what }
}

fn pack_err(reason: String) -> GstError {
    GstError::Pack { reason }
}

/// Decides whether the tax on `v.ledger_lines[line]` is eligible input tax credit.
///
/// `rules` must be the rule set selected for the voucher's time of supply
/// ([`crate::pack::RulePack::for_time_of_supply`]); the caller owns that choice.
///
/// # Errors
/// * [`GstError::Invalid`] when `line` is not a position in the voucher's ledger lines.
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger or stock item
///   that is not in `masters`.
/// * [`GstError::Pack`] when a blocked-category entry lacks `section` or `hsn_prefixes`, or a
///   prefix is not made of digits.
pub fn itc_eligibility(
    v: &Voucher,
    line: usize,
    masters: &Masters,
    rules: &RuleSet,
) -> Result<Outcome<ItcEligibility>, GstError> {
    supported!(check_voucher(v, masters)?);
    let ledger_line = v.ledger_lines.get(line).ok_or_else(|| GstError::Invalid {
        field: "line".to_owned(),
        reason: format!(
            "the voucher has {} ledger lines, so line {line} does not exist",
            v.ledger_lines.len()
        ),
    })?;
    let ledger = masters
        .ledgers
        .get(&ledger_line.ledger)
        .ok_or_else(|| not_found(format!("ledger {}", ledger_line.ledger.0)))?;
    let Some(class) = ledger.gst.as_ref() else {
        return unsupported(
            "ITC on a ledger line without a GST classification",
            format!(
                "ledger `{}` is not a purchase or expense ledger with a GST classification",
                ledger.name
            ),
        );
    };
    match class.taxability {
        Taxability::Taxable => {}
        Taxability::Exempt | Taxability::NilRated | Taxability::NonGst => {
            return ineligible(
                "the purchase is exempt, nil-rated or outside GST, so no tax was charged",
            );
        }
    }
    if let Some(result) = supported!(supplier_check(v, masters)?) {
        return value(result);
    }
    let codes = supported!(codes_for(v, ledger, masters)?);
    let mut result: Option<ItcEligibility> = None;
    for code in codes {
        let found = supported!(classify(code, rules)?);
        match &result {
            None => result = Some(found),
            Some(previous) if *previous == found => {}
            Some(_) => {
                return unsupported(
                    "ITC on a ledger line that mixes categories",
                    format!(
                        "the items posted to ledger `{}` fall in different ITC categories; split the line",
                        ledger.name
                    ),
                );
            }
        }
    }
    match result {
        Some(r) => value(r),
        None => unsupported(
            "ITC without an HSN/SAC",
            "no HSN/SAC found for the line; set it on the ledger or the stock item",
        ),
    }
}

/// Only a regular purchase voucher carries ITC in this version.
fn check_voucher(v: &Voucher, masters: &Masters) -> Result<Outcome<()>, GstError> {
    match v.status {
        VoucherStatus::Cancelled => {
            return unsupported(
                "ITC on a cancelled voucher",
                "a cancelled voucher has no amounts and no tax",
            );
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    let voucher_type = masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| not_found(format!("voucher type {}", v.type_id.0)))?;
    match voucher_type.base {
        BaseVoucherKind::Purchase => Ok(Outcome::Value(())),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => unsupported(
            format!("ITC on a {:?} voucher", voucher_type.base),
            "only purchase vouchers are classified; credit for debit notes and imports is not handled",
        ),
    }
}

/// `Some(Ineligible)` when the supplier's registration rules credit out; `None` to go on.
fn supplier_check(
    v: &Voucher,
    masters: &Masters,
) -> Result<Outcome<Option<ItcEligibility>>, GstError> {
    let Some(party_id) = v.party else {
        return unsupported(
            "ITC on a voucher without a party",
            "the supplier's registration decides eligibility; set the party",
        );
    };
    let party_ledger = masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    let Some(party) = party_ledger.party.as_ref() else {
        return unsupported(
            "ITC from a supplier without party details",
            format!(
                "ledger `{}` has no registration type; set it on the party",
                party_ledger.name
            ),
        );
    };
    let none = Ok(Outcome::Value(None));
    match party.registration {
        RegistrationType::Regular | RegistrationType::Sez => none,
        RegistrationType::Composition => Ok(Outcome::Value(Some(ItcEligibility::Ineligible {
            reason: "a composition dealer cannot charge tax on its invoice (CGST Act s.10(4))"
                .to_owned(),
        }))),
        RegistrationType::Unregistered | RegistrationType::Consumer => {
            if v.reverse_charge {
                none
            } else {
                Ok(Outcome::Value(Some(ItcEligibility::Ineligible {
                    reason: "the supplier is not registered and the purchase is not under reverse charge, so there is no tax invoice with tax (CGST Act s.16(2)(a))"
                        .to_owned(),
                })))
            }
        }
        RegistrationType::Overseas | RegistrationType::Uin => unsupported(
            "ITC from an overseas or UIN supplier",
            "imports and UIN supplies are not handled",
        ),
    }
}

/// The HSN/SAC codes that classify the line: the ledger's own, else its stock items'.
fn codes_for<'a>(
    v: &Voucher,
    ledger: &'a Ledger,
    masters: &'a Masters,
) -> Result<Outcome<BTreeSet<&'a Hsn>>, GstError> {
    let mut codes: BTreeSet<&Hsn> = BTreeSet::new();
    if let Some(hsn) = ledger.gst.as_ref().and_then(|c| c.hsn.as_ref()) {
        codes.insert(hsn);
        return Ok(Outcome::Value(codes));
    }
    for inv in v
        .inventory_lines
        .iter()
        .filter(|l| l.ledger == Some(ledger.id))
    {
        let item = masters
            .stock_items
            .get(&inv.item)
            .ok_or_else(|| not_found(format!("stock item {}", inv.item.0)))?;
        match item.hsn.as_ref() {
            Some(hsn) => {
                codes.insert(hsn);
            }
            None => {
                return unsupported(
                    "ITC without an HSN/SAC",
                    format!("stock item `{}` has no HSN/SAC", item.name),
                );
            }
        }
    }
    if codes.is_empty() {
        return unsupported(
            "ITC without an HSN/SAC",
            format!(
                "neither ledger `{}` nor an inventory line posted to it has an HSN/SAC",
                ledger.name
            ),
        );
    }
    Ok(Outcome::Value(codes))
}

/// Looks `code` up in every blocked category of the rule set.
fn classify(code: &Hsn, rules: &RuleSet) -> Result<Outcome<ItcEligibility>, GstError> {
    for id in BLOCKED_RULE_IDS {
        let entry = supported!(rules.get(id));
        if category_contains(entry, code)? {
            let section = param(entry, "section")?.trim().to_owned();
            return value(ItcEligibility::Blocked { section });
        }
    }
    value(ItcEligibility::Eligible)
}

fn param<'e>(entry: &'e PackEntry, name: &str) -> Result<&'e str, GstError> {
    entry
        .params()
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| pack_err(format!("entry `{}` has no `{name}` param", entry.id())))
}

fn category_contains(entry: &PackEntry, code: &Hsn) -> Result<bool, GstError> {
    let prefixes = param(entry, "hsn_prefixes")?;
    let mut hit = false;
    for prefix in prefixes.split(',').map(str::trim) {
        if prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_digit()) {
            return Err(pack_err(format!(
                "entry `{}`: hsn_prefixes `{prefixes}` must be comma separated digit strings",
                entry.id()
            )));
        }
        if code.as_str().starts_with(prefix) {
            hit = true;
        }
    }
    Ok(hit)
}
