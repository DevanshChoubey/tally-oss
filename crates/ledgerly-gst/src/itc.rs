//! Input tax credit eligibility from the user's "ineligible for input credit" setting. **Card G7**
//! implements this module.
//!
//! Model (TallyPrime, as documented; provenance in `tests/g7_itc.rs`). The user marks a stock item
//! or stock group, or an accounting ledger or group, as ineligible for input credit, and may enable
//! it company-wide when no purchase gets credit. The purchase invoice shows the master value and the
//! user may change it for that invoice. The HSN/SAC of a line does not decide eligibility.
//!
//! ```text
//! pub fn itc_eligibility(v: &Voucher, line: usize, company: &Company, masters: &Masters)
//!     -> Result<Outcome<ItcEligibility>, GstError>;
//! ```
//!
//! # What `line` is
//!
//! `line` is a zero-based position in [`Voucher::ledger_lines`]. It must be a purchase or expense
//! line, that is a line whose ledger carries a GST classification.
//!
//! # Resolution, in order
//!
//! 1. The invoice line's own setting ([`LedgerLine::ineligible_itc`]) when set.
//! 2. The masters. For a line with inventory posted to its ledger, the stock item's setting, then
//!    its stock group chain upward. The purchase ledger's setting, then its group chain upward, is
//!    the same kind of master. A `Some` value stops the walk, `Some(false)` included. A loop in a
//!    parent chain is `Unsupported`; a missing parent is [`GstError::NotFound`].
//!    * Items under one line that set different values: `Unsupported` (split the line).
//!    * An item setting and the ledger setting that disagree: `Unsupported`. The Tally page does not
//!      say which one wins, so this is pending.
//! 3. The company default ([`Company::ineligible_itc_default`]). A master value that disagrees with
//!    a set company default is `Unsupported` for the same reason (pending).
//! 4. Nothing set anywhere: eligible.
//!
//! The result is `Ineligible` when the effective value is Yes, and `Eligible` otherwise.
//!
//! # What is decided
//!
//! * `Ineligible`: the ledger is exempt, nil-rated or non-GST (no tax was charged); the supplier is a
//!   composition dealer, or is unregistered/a consumer and the voucher is not under reverse charge
//!   (no tax invoice with tax, CGST Act s.16(2)(a), s.10(4)); or the effective setting is Yes.
//! * `Eligible`: a taxable purchase from a registered (or SEZ) supplier, or a reverse-charge
//!   purchase, whose effective setting is not Yes.
//! * `Unsupported`: a voucher that is not a regular purchase, a cancelled voucher, an overseas or
//!   UIN supplier, a line without a GST classification, a conflict described above, and a
//!   broken parent chain.
//!
//! # Not decided
//!
//! The setting is the user's record. The function does not check the s.17(5) clauses (motor
//! vehicles, food, and so on); a flag-free purchase in a blocked category is eligible here, because
//! the books record the user's marking and nothing else. Also not checked: the conditions of
//! s.16(2) (2B match, receipt, supplier payment, 180 days), Rules 42 and 43. Credit that becomes
//! ineligible after purchase is reversed with a journal in Tally; that reversal is not modelled
//! here (pending, out of scope for this card).
//!
//! The advisory helper [`blocked_category_for`] reports whether an HSN/SAC
//! falls in a blocked category of the rule pack. Nothing on the number path calls it.
//!
//! Only claimable credit may reach GSTR-3B table 4(A): use [`ItcEligibility::is_claimable`].
//!
//! [`Voucher::ledger_lines`]: ledgerly_model::Voucher

use std::collections::BTreeSet;

use ledgerly_core::{Hsn, Outcome};
use ledgerly_model::{
    BaseVoucherKind, Company, Group, Ledger, LedgerLine, Masters, RegistrationType, StockGroup,
    StockItem, Taxability, Voucher, VoucherStatus,
};

use crate::GstError;
use crate::pack::{PackEntry, RuleSet};

/// Rule pack entry ids that hold a blocked-credit category (see [`blocked_category_for`]).
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
    /// No ineligibility found (see the module docs for what is not checked).
    Eligible,
    /// Credit blocked by CGST Act s.17(5). Not returned by [`itc_eligibility`]: the blocked
    /// categories are advisory (see [`blocked_category_for`]).
    Blocked {
        /// The clause, as written in the pack, for example `17(5)(a)`.
        section: String,
    },
    /// Credit not available: the user marked it ineligible, or the supplier's registration rules
    /// it out, or no tax was charged.
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
fn value<T>(e: T) -> Result<Outcome<T>, GstError> {
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

/// Decides whether the tax on `v.ledger_lines[line]` is eligible input tax credit, from the
/// user's "ineligible for input credit" settings (module docs, resolution order).
///
/// # Errors
/// * [`GstError::Invalid`] when `line` is not a position in the voucher's ledger lines.
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger, group, stock
///   item or stock group that is not in `masters`, or a parent group is missing.
pub fn itc_eligibility(
    v: &Voucher,
    line: usize,
    company: &Company,
    masters: &Masters,
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
    let flag = supported!(effective_flag(v, ledger_line, ledger, company, masters)?);
    if flag {
        ineligible("the purchase is marked ineligible for input credit (on the invoice, a master, or the company setting)")
    } else {
        value(ItcEligibility::Eligible)
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

/// The effective "ineligible" value of one purchase line: invoice, then masters, then company.
fn effective_flag(
    v: &Voucher,
    ledger_line: &LedgerLine,
    ledger: &Ledger,
    company: &Company,
    masters: &Masters,
) -> Result<Outcome<bool>, GstError> {
    if let Some(flag) = ledger_line.ineligible_itc {
        return value(flag);
    }
    let master = supported!(master_flag(v, ledger, masters)?);
    let disagree = matches!(
        (master, company.ineligible_itc_default),
        (Some(m), Some(c)) if m != c
    );
    if disagree {
        return unsupported(
            format!("ITC on ledger `{}`", ledger.name),
            "the master's setting and the company setting disagree, and the books do not say which one applies; set the same value on both, or set it on the invoice",
        );
    }
    value(
        master
            .or(company.ineligible_itc_default)
            .unwrap_or(false),
    )
}

/// The setting from the masters for one line: the stock items posted to the ledger and their
/// groups, and the purchase ledger and its groups. `None` when no master sets it.
fn master_flag(v: &Voucher, ledger: &Ledger, masters: &Masters) -> Result<Outcome<Option<bool>>, GstError> {
    let mut item_values: BTreeSet<bool> = BTreeSet::new();
    for inv in v
        .inventory_lines
        .iter()
        .filter(|l| l.ledger == Some(ledger.id))
    {
        let stock_item = masters
            .stock_items
            .get(&inv.item)
            .ok_or_else(|| not_found(format!("stock item {}", inv.item.0)))?;
        if let Some(flag) = supported!(stock_item_flag(stock_item, masters)?) {
            item_values.insert(flag);
        }
    }
    if item_values.len() > 1 {
        return unsupported(
            format!("ITC on ledger `{}`", ledger.name),
            "the items posted to this ledger line are marked differently for input credit; split the line",
        );
    }
    let item_flag = item_values.first().copied();
    let ledger_flag = supported!(ledger_flag(ledger, masters)?);
    let disagree = matches!(
        (item_flag, ledger_flag),
        (Some(i), Some(l)) if i != l
    );
    if disagree {
        return unsupported(
            format!("ITC on ledger `{}`", ledger.name),
            "the stock item and the purchase ledger are marked differently for input credit, and the books do not say which one applies; set the same value on both, or set it on the invoice",
        );
    }
    value(item_flag.or(ledger_flag))
}

/// The stock item's setting, else the nearest stock group that sets it.
fn stock_item_flag(item: &StockItem, masters: &Masters) -> Result<Outcome<Option<bool>>, GstError> {
    if let Some(flag) = item.ineligible_itc {
        return value(Some(flag));
    }
    let mut seen: BTreeSet<ledgerly_core::StockGroupId> = BTreeSet::new();
    let mut next = item.group;
    while let Some(id) = next {
        if !seen.insert(id) {
            return unsupported(
                format!("ITC on stock item `{}`", item.name),
                "the stock groups form a loop in their parent chain; fix the parent of one group",
            );
        }
        let group: &StockGroup = masters
            .stock_groups
            .get(&id)
            .ok_or_else(|| not_found(format!("stock group {}", id.0)))?;
        if let Some(flag) = group.ineligible_itc {
            return value(Some(flag));
        }
        next = group.parent;
    }
    value(None)
}

/// The ledger's setting, else the nearest group that sets it.
fn ledger_flag(ledger: &Ledger, masters: &Masters) -> Result<Outcome<Option<bool>>, GstError> {
    if let Some(flag) = ledger.ineligible_itc {
        return value(Some(flag));
    }
    let mut seen: BTreeSet<ledgerly_core::GroupId> = BTreeSet::new();
    let mut next = Some(ledger.group);
    while let Some(id) = next {
        if !seen.insert(id) {
            return unsupported(
                format!("ITC on ledger `{}`", ledger.name),
                "the groups form a loop in their parent chain; fix the parent of one group",
            );
        }
        let group: &Group = masters
            .groups
            .get(&id)
            .ok_or_else(|| not_found(format!("group {}", id.0)))?;
        if let Some(flag) = group.ineligible_itc {
            return value(Some(flag));
        }
        next = group.parent;
    }
    value(None)
}

/// Advisory only, not on the number path: the blocked category of CGST Act s.17(5) that holds
/// `code`, read from the rule pack. `Some(section)` names the clause (for example `17(5)(a)`);
/// `None` means no blocked category of the pack holds the code.
///
/// Which HSN/SAC codes fall in a category is pack data. Each entry in [`BLOCKED_RULE_IDS`] carries
/// `section` and `hsn_prefixes` (comma separated digit strings; a code is in the category when it
/// starts with one). A missing or pending entry is `Unsupported`.
///
/// # Errors
/// [`GstError::Pack`] when an entry lacks `section` or `hsn_prefixes`, or a prefix is not digits.
pub fn blocked_category_for(code: &Hsn, rules: &RuleSet) -> Result<Outcome<Option<String>>, GstError> {
    for id in BLOCKED_RULE_IDS {
        let entry = supported!(rules.get(id));
        if category_contains(entry, code)? {
            let section = param(entry, "section")?.trim().to_owned();
            return value(Some(section));
        }
    }
    value(None)
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
