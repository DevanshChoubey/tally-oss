//! Input tax credit eligibility, including s.17(5). **Card G7** implements this module.
//!
//! The user's setting decides first (TallyPrime model; provenance in `tests/g7_itc.rs`). The user
//! marks "ineligible for input credit" on a stock item or stock group, or on an accounting ledger or
//! group, or enables it company-wide when no purchase gets credit. The purchase invoice shows the
//! master value and the user may change it for that invoice.
//!
//! ```text
//! pub fn itc_eligibility(v: &Voucher, line: usize, company: &Company, masters: &Masters,
//!     rules: &RuleSet) -> Result<Outcome<ItcEligibility>, GstError>;
//! ```
//!
//! # What `line` is
//!
//! `line` is a zero-based position in [`Voucher::ledger_lines`]. It must be a purchase or expense
//! line, that is a line whose ledger carries a GST classification.
//!
//! # Resolution, in order
//!
//! 1. Supplier and voucher checks (below). They decide first, as no tax is claimable from them.
//! 2. An explicit setting decides. The first of these that is set wins, and `Some(false)` counts:
//!    * the invoice line's own setting ([`LedgerLine::ineligible_itc`]);
//!    * the masters. For a line with inventory posted to its ledger, the stock item, then its stock
//!      group chain upward; the purchase ledger, then its group chain upward. A set value stops the
//!      walk. A loop in a parent chain is `Unsupported`; a missing parent is [`GstError::NotFound`].
//!      Items under one line that set different values are `Unsupported` (split the line). An item
//!      setting and a ledger setting that disagree are `Unsupported` (pending: not documented);
//!    * the company default ([`Company::ineligible_itc_default`]). A master value that disagrees
//!      with it is `Unsupported` (pending: not documented).
//!
//!    Set Yes gives `Ineligible`; set No gives `Eligible`, whatever the HSN/SAC.
//! 3. Nothing set at any level: the HSN/SAC rules of the rule pack apply, as before this setting
//!    existed. A line whose HSN/SAC is in a blocked category of the pack (s.17(5)) is `Unsupported`,
//!    because its exceptions depend on use, which the books do not record. A line with no HSN/SAC,
//!    or whose items fall in different categories, is `Unsupported`. Each of these reasons tells
//!    the user to mark the stock item or the purchase ledger eligible or ineligible for input credit
//!    (or to set it on the invoice), and then the setting decides. A line in no blocked category is
//!    `Eligible`. We refuse rather than guess: an unmarked motor car is not claimed.
//!
//! # What is decided
//!
//! * `Ineligible`: the ledger is exempt, nil-rated or non-GST (no tax was charged); the supplier is
//!   a composition dealer, or is unregistered/a consumer and the voucher is not under reverse charge
//!   (no tax invoice with tax, CGST Act s.16(2)(a), s.10(4)); or an explicit setting is Yes.
//! * `Eligible`: a taxable purchase from a registered (or SEZ) supplier, or a reverse-charge
//!   purchase, with an explicit No, or with nothing set and no blocked HSN/SAC category.
//! * `Unsupported`: a voucher that is not a regular purchase, a cancelled voucher, an overseas or
//!   UIN supplier, a line without a GST classification, a conflict or a broken parent chain, and
//!   the HSN/SAC fallback cases above.
//!
//! # Not decided
//!
//! The conditions of s.16(2) (2B match, receipt, supplier payment, 180 days), Rules 42 and 43, and
//! the s.17(5) clauses that depend on use are not checked. Credit that becomes ineligible after
//! purchase is reversed with a journal in Tally; that reversal is not modelled (pending).
//!
//! [`blocked_category_for`] is a separate advisory helper: it reports whether an HSN/SAC falls in a
//! blocked category of the pack. The fallback in step 3 uses it.
//!
//! Only claimable credit may reach GSTR-3B table 4(A): use [`ItcEligibility::is_claimable`].
//!
//! [`Voucher::ledger_lines`]: ledgerly_model::Voucher

use std::collections::BTreeSet;

use ledgerly_core::{GroupId, Hsn, Outcome, StockGroupId};
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
    /// Credit blocked by CGST Act s.17(5). Not returned by [`itc_eligibility`]: a blocked category
    /// is `Unsupported` until the user marks the line (see the module docs).
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

/// Decides whether the tax on `v.ledger_lines[line]` is eligible input tax credit (module docs,
/// resolution order).
///
/// `rules` must be the rule set selected for the voucher's time of supply
/// ([`crate::pack::RulePack::for_time_of_supply`]); the caller owns that choice. It is used only
/// when no setting applies (the HSN/SAC fallback).
///
/// # Errors
/// * [`GstError::Invalid`] when `line` is not a position in the voucher's ledger lines.
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger, stock item,
///   stock group or group that is not in `masters`, or a parent group is missing.
/// * [`GstError::Pack`] when a blocked-category entry lacks `section` or `hsn_prefixes`, or a
///   prefix is not made of digits.
pub fn itc_eligibility(
    v: &Voucher,
    line: usize,
    company: &Company,
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
    match supported!(effective_flag(v, ledger_line, ledger, company, masters)?) {
        Some(true) => ineligible(
            "the purchase is marked ineligible for input credit (on the invoice, a master, or the company setting)",
        ),
        Some(false) => value(ItcEligibility::Eligible),
        None => hsn_fallback(v, ledger, masters, rules),
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

/// The explicit "ineligible for input credit" setting of one purchase line: the invoice, then the
/// masters, then the company default. `None` when nothing is set at any level.
fn effective_flag(
    v: &Voucher,
    ledger_line: &LedgerLine,
    ledger: &Ledger,
    company: &Company,
    masters: &Masters,
) -> Result<Outcome<Option<bool>>, GstError> {
    if let Some(flag) = ledger_line.ineligible_itc {
        return value(Some(flag));
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
    value(master.or(company.ineligible_itc_default))
}

/// The setting from the masters for one line: the stock items posted to the ledger and their
/// groups, and the purchase ledger and its groups. `None` when no master sets it.
fn master_flag(
    v: &Voucher,
    ledger: &Ledger,
    masters: &Masters,
) -> Result<Outcome<Option<bool>>, GstError> {
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
    let mut seen: BTreeSet<StockGroupId> = BTreeSet::new();
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
    let mut seen: BTreeSet<GroupId> = BTreeSet::new();
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

/// The HSN/SAC rules, used only when no setting applies (module docs, step 3).
fn hsn_fallback(
    v: &Voucher,
    ledger: &Ledger,
    masters: &Masters,
    rules: &RuleSet,
) -> Result<Outcome<ItcEligibility>, GstError> {
    let codes = supported!(codes_for(v, ledger, masters)?);
    let mut result: Option<ItcEligibility> = None;
    for code in codes {
        let found = supported!(classify(code, rules)?);
        match result.take() {
            None => result = Some(found),
            Some(previous) if previous == found => result = Some(previous),
            Some(_) => {
                return unsupported(
                    "ITC on a ledger line that mixes categories",
                    format!(
                        "the items posted to ledger `{}` fall in different ITC categories; split the line, or mark the items eligible or ineligible for input credit",
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
            "no HSN/SAC found for the line; set it on the ledger or the stock item, or mark the stock item or purchase ledger eligible or ineligible for input credit",
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
                    format!(
                        "stock item `{}` has no HSN/SAC; set it, or mark the item eligible or ineligible for input credit",
                        item.name
                    ),
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

/// One code under the fallback: `Unsupported` in a blocked category, `Eligible` otherwise.
///
/// A code in a blocked category is `Unsupported`, never `Blocked`: each category is a clause of
/// s.17(5) whose exceptions depend on how the goods or services are used, and the books record no
/// use. The reason tells the user how to settle the line.
fn classify(code: &Hsn, rules: &RuleSet) -> Result<Outcome<ItcEligibility>, GstError> {
    match supported!(blocked_category_for(code, rules)?) {
        Some(section) => unsupported(
            format!("ITC on HSN/SAC {} under s.{section}", code.as_str()),
            format!(
                "s.{section} blocks this category except where an exception applies; the exceptions depend on how the goods or services are used, which the books do not record. Mark the stock item or purchase ledger eligible or ineligible for input credit (or set it on the invoice) and the setting decides"
            ),
        ),
        None => value(ItcEligibility::Eligible),
    }
}

/// Advisory helper, also used by the fallback: the blocked category of CGST Act s.17(5) that holds
/// `code`, read from the rule pack. `Some(section)` names the clause (for example `17(5)(a)`);
/// `None` means no blocked category of the pack holds the code.
///
/// Which HSN/SAC codes fall in a category is pack data. Each entry in [`BLOCKED_RULE_IDS`] carries
/// `section` and `hsn_prefixes` (comma separated digit strings; a code is in the category when it
/// starts with one). A missing or pending entry is `Unsupported`.
///
/// # Errors
/// [`GstError::Pack`] when an entry lacks `section` or `hsn_prefixes`, or a prefix is not digits.
pub fn blocked_category_for(
    code: &Hsn,
    rules: &RuleSet,
) -> Result<Outcome<Option<String>>, GstError> {
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
