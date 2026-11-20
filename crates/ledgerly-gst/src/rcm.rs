//! Reverse charge (CGST Act s.9(3) and s.9(4)). **Card G5** implements this module.
//!
//! Frozen interface (plan card G5):
//!
//! ```text
//! pub fn reverse_charge(v: &Voucher, masters: &Masters, rules: &RuleSet) -> Result<Outcome<RcmLiability>, GstError>;   // RcmLiability defined by G5
//! ```
//!
//! # What is decided
//!
//! For a purchase voucher this module says whether the **recipient** (the company) owes the tax
//! instead of the supplier, and how much. The answer is an [`RcmLiability`]:
//!
//! * [`RcmLiability::Payable`]: a notified class of supply matches the taxable lines, so the tax is
//!   the recipient's liability. The tax is not added to the invoice total, because the supplier
//!   does not charge it; the figure feeds the self-invoice, the payment voucher and GSTR-3B 3.1(d).
//! * [`RcmLiability::NotApplicable`]: the supply is not notified for this kind of supplier, so the
//!   ordinary forward-charge computation of card G4 applies.
//! * `Unsupported`: anything this module cannot decide (see below). Never a guess.
//!
//! # The list of notified classes is pack data
//!
//! Which supplies are under reverse charge is not hard-coded. The rule pack carries one index entry
//! and one entry per class, all selected by the time of supply (engineering rule A8):
//!
//! ```text
//! [[entry]]
//! id = "rcm.classes"                 # the index; one entry per period in which the list differs
//! effective_from = "..."
//! legal_ref = "..."
//! [entry.params]
//! ids = "gta,legal"                  # comma separated class ids in force
//!
//! [[entry]]
//! id = "rcm.class.gta"               # "rcm.class." followed by an id listed in the index
//! effective_from = "..."
//! legal_ref = "Notification No. ..." # or pending = true
//! [entry.params]
//! hsn_prefixes = "9965"              # comma separated digits; a line matches when its HSN/SAC starts with one
//! supplier = "any"                   # "any" (registered or not) or "unregistered" (only unregistered suppliers)
//! ```
//!
//! A class that is listed but pending (or not in force on that date) makes every voucher that needs
//! the list `Unsupported`: the module cannot tell whether the line belongs to it, and answering
//! "not notified" could be a wrong number. A pack without the `rcm.classes` entry is `Unsupported`
//! for the same reason; there is no default list.
//!
//! # Time of supply
//!
//! Under reverse charge the time of supply follows s.12(3) and s.13(3) (receipt, payment or a
//! number of days after the invoice), which the voucher alone does not determine. This module
//! therefore requires `Voucher::supply_date` to be set and uses it to select the rule set; a
//! voucher without it is `Unsupported`. The rule set passed in must have been selected for that
//! date, or the call is an error.
//!
//! # Not decided (returns `Unsupported`)
//!
//! Vouchers that are not purchases (sales, credit notes, debit notes and every other kind),
//! cancelled vouchers, plain Dr/Cr vouchers, a supplier that is a composition taxpayer, an SEZ unit,
//! overseas or UIN holder (import of services is IGST Act s.5(3), not handled), a supplier of
//! unknown or ambiguous state, a missing place of supply, vouchers posting to a cess ledger, taxable
//! lines without an HSN/SAC, lines of different classes or a mix of notified and non-notified
//! lines, a taxable rate with no verified pack slab, and a voucher marked as reverse charge for
//! which no notified class matches (the user's mark and the pack disagree).
//!
//! The ITC part is limited to timing: see [`RcmItc`]. Whether the credit is eligible at all
//! (s.17(5), personal use) is card G7's decision.

use ledgerly_core::{
    Date, Decimal, DrCr, MoneyError, Outcome, Paise, RoundingRule, StateCode, round_rupees_to_paise,
};
use ledgerly_model::{
    BaseVoucherKind, Company, GstHistory, Masters, RegistrationType, StockItem, TaxLedgerKind,
    Taxability, Voucher, VoucherMode, VoucherStatus,
};

use crate::GstError;
use crate::compute::{LineSource, TaxHeads, TaxTotals};
use crate::pack::{PackEntry, RuleSet};

/// Pack entry ids that hold a rate slab (`igst_percent`, `cgst_percent`, `sgst_percent` params).
/// Same list as voucher tax (card G4); see the G4 request in `docs/interface-requests.md` for a
/// prefix lookup that would remove it.
const SLAB_IDS: [&str; 8] = [
    "slab.zero",
    "slab.five",
    "slab.twelve",
    "slab.eighteen",
    "slab.twenty_eight",
    "slab.forty",
    "special.rough_diamonds",
    "special.gold_jewellery",
];

/// Pack entry id of the index of notified classes.
const INDEX_ID: &str = "rcm.classes";
/// Prefix of the id of one notified class.
const CLASS_PREFIX: &str = "rcm.class.";

/// Why reverse charge does not apply to a purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoRcmReason {
    /// No notified class of the pack matches the taxable lines.
    NotNotified,
    /// A class matches, but it covers only unregistered suppliers and this supplier is registered.
    SupplierRegistered,
    /// The voucher has no taxable line (everything is exempt, nil-rated or outside GST).
    NoTaxableLine,
}

/// When input tax credit on the reverse-charge tax can be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RcmItc {
    /// Only after the recipient has paid the tax in cash (it cannot be paid from the credit
    /// ledger), and then only if the credit is eligible (card G7). Source: research 04-gst 2.5
    /// (R.52, tag `[K]`); the engine only reports the timing.
    AfterTaxPaid,
}

/// Tax on one line under reverse charge. Tax amounts are exact rupees, **not rounded**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcmLine {
    /// Where the line came from.
    pub source: LineSource,
    /// Taxable value.
    pub taxable: Paise,
    /// Total rate in percent (the IGST rate).
    pub rate_percent: Decimal,
    /// Unrounded CGST in rupees (zero for inter-state supplies).
    pub cgst: Decimal,
    /// Unrounded SGST in rupees (zero for inter-state supplies and union territories).
    pub sgst: Decimal,
    /// Unrounded UTGST in rupees (non-zero only for union-territory supplies).
    pub utgst: Decimal,
    /// Unrounded IGST in rupees (zero for intra-state supplies).
    pub igst: Decimal,
}

/// The tax a recipient owes under reverse charge on one purchase voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcmPayable {
    /// Version of the rule pack the rates and the class came from.
    pub pack_version: String,
    /// Id of the notified class that matched (the part after `rcm.class.`).
    pub class_id: String,
    /// The legal reference of that class entry in the pack.
    pub legal_ref: String,
    /// Which taxes apply.
    pub heads: TaxHeads,
    /// One entry per taxable line, in voucher order.
    pub lines: Vec<RcmLine>,
    /// Sum of the taxable values of the lines under reverse charge.
    pub taxable_total: Paise,
    /// Tax per head, each rounded once over the voucher, half away from zero, to the paisa.
    pub totals: TaxTotals,
    /// When the credit on this tax can be taken.
    pub itc: RcmItc,
    /// Whether the user had marked the voucher as reverse charge. The liability exists either way
    /// (it is the law, not the mark); `false` tells the caller the voucher needs to be marked.
    pub marked_on_voucher: bool,
}

/// The reverse-charge position of a purchase voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RcmLiability {
    /// The recipient owes the tax.
    Payable(RcmPayable),
    /// Reverse charge does not apply.
    NotApplicable(NoRcmReason),
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

fn not_found(what: String) -> GstError {
    GstError::NotFound { what }
}

fn invalid(field: &str, reason: String) -> GstError {
    GstError::Invalid {
        field: field.to_owned(),
        reason,
    }
}

fn pack_err(reason: String) -> GstError {
    GstError::Pack { reason }
}

/// Decides whether the purchase voucher `v` is under reverse charge and, if so, the tax owed.
///
/// `rules` must be the rule set selected for the voucher's time of supply (`supply_date`, which
/// must be set), see [`crate::pack::RulePack::for_time_of_supply`]. This function cannot see the
/// company, so it does not check that the recipient is a registered regular taxpayer; use
/// [`reverse_charge_for_company`] to have that checked as well.
///
/// # Errors
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger or stock item
///   that is not in `masters`.
/// * [`GstError::Invalid`] when `rules` was selected for another date than `supply_date`, or a line
///   amount is negative.
/// * [`GstError::Pack`] when a pack entry that is in force is malformed (missing or unreadable
///   parameter, a slab whose CGST and SGST do not add up to its IGST).
/// * [`GstError::Money`] on arithmetic overflow.
pub fn reverse_charge(
    v: &Voucher,
    masters: &Masters,
    rules: &RuleSet,
) -> Result<Outcome<RcmLiability>, GstError> {
    evaluate(v, masters, rules, None)
}

/// Like [`reverse_charge`], and also `Unsupported` unless `company` has GST enabled and is a
/// registered regular (not composition) taxpayer on the time of supply. A public addition to the
/// frozen interface (see `docs/interface-requests.md`, card G5).
///
/// # Errors
/// As [`reverse_charge`].
pub fn reverse_charge_for_company(
    v: &Voucher,
    masters: &Masters,
    rules: &RuleSet,
    company: &Company,
) -> Result<Outcome<RcmLiability>, GstError> {
    evaluate(v, masters, rules, Some(company))
}

fn evaluate(
    v: &Voucher,
    masters: &Masters,
    rules: &RuleSet,
    company: Option<&Company>,
) -> Result<Outcome<RcmLiability>, GstError> {
    supported!(check_scope(v, masters)?);
    let Some(tos) = v.supply_date else {
        return unsupported(
            "time of supply under reverse charge",
            "set the supply date: under reverse charge it follows s.12(3) and s.13(3) and cannot be taken from the voucher date",
        );
    };
    if rules.date() != tos {
        return Err(invalid(
            "rules",
            format!(
                "the rule set is for {} but the time of supply is {tos}",
                rules.date()
            ),
        ));
    }
    if let Some(company) = company {
        supported!(recipient_ok(company, tos));
    }
    let supplier = supported!(supplier_of(v, masters)?);
    let taxable: Vec<Base> = supported!(collect_bases(v, masters, tos)?)
        .into_iter()
        .filter(|b| is_taxable(b.taxability))
        .collect();
    if taxable.is_empty() {
        return Ok(not_applicable(v, NoRcmReason::NoTaxableLine));
    }
    let classes = supported!(load_classes(rules)?);
    let Some(class) = supported!(match_class(&taxable, &classes)) else {
        return Ok(not_applicable(v, NoRcmReason::NotNotified));
    };
    if class.unregistered_only && supplier.registered {
        return Ok(not_applicable(v, NoRcmReason::SupplierRegistered));
    }
    let heads = supported!(supply_heads(v, supplier.state));
    let mut lines = Vec::with_capacity(taxable.len());
    for base in &taxable {
        lines.push(supported!(price_line(base, heads, rules)?));
    }
    let payable = assemble(lines, heads, rules.version(), class, v.reverse_charge)?;
    Ok(Outcome::Value(RcmLiability::Payable(payable)))
}

/// `NotApplicable`, unless the user marked the voucher as reverse charge: then the mark and the
/// pack disagree and the answer is `Unsupported`.
fn not_applicable(v: &Voucher, reason: NoRcmReason) -> Outcome<RcmLiability> {
    if v.reverse_charge {
        return Outcome::unsupported(
            "a voucher marked as reverse charge",
            format!(
                "the voucher is marked reverse charge but the rule pack says it is not ({reason:?}); check the supplier, the HSN/SAC and the rule pack, or clear the mark"
            ),
        );
    }
    Outcome::Value(RcmLiability::NotApplicable(reason))
}

fn check_scope(v: &Voucher, masters: &Masters) -> Result<Outcome<()>, GstError> {
    match v.status {
        VoucherStatus::Cancelled => {
            return unsupported(
                "reverse charge on a cancelled voucher",
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
        BaseVoucherKind::Purchase => {}
        BaseVoucherKind::Sales | BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => {
            return unsupported(
                format!("reverse charge on a {:?} voucher", voucher_type.base),
                "only purchase vouchers are checked for reverse charge; notes and sales are not",
            );
        }
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => {
            return unsupported(
                format!("reverse charge on a {:?} voucher", voucher_type.base),
                "only purchase vouchers are checked for reverse charge",
            );
        }
    }
    match v.mode {
        VoucherMode::AccountingInvoice | VoucherMode::ItemInvoice => Ok(Outcome::Value(())),
        VoucherMode::AsVoucher => unsupported(
            "reverse charge on a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that taxable lines can be identified",
        ),
    }
}

/// The recipient must be a registered regular taxpayer on the time of supply.
fn recipient_ok(company: &Company, tos: Date) -> Outcome<()> {
    if !company.features.gst {
        return Outcome::unsupported(
            "reverse charge for a company without GST",
            "GST is not enabled for this company; enable it in the company features",
        );
    }
    let regs: Vec<_> = company
        .registrations
        .iter()
        .filter(|r| r.effective_from <= tos)
        .collect();
    if regs.is_empty() {
        return Outcome::unsupported(
            format!("reverse charge on a supply dated {tos}"),
            "the company has no GST registration in force on the time of supply",
        );
    }
    if regs.iter().any(|r| r.composition) {
        return Outcome::unsupported(
            "reverse charge for a composition taxpayer",
            "a composition recipient's reverse-charge liability is not computed",
        );
    }
    Outcome::Value(())
}

/// What is known about the supplier of the purchase.
struct Supplier {
    /// A regular registered taxpayer (as against unregistered).
    registered: bool,
    /// The supplier's state, when known.
    state: Option<StateCode>,
}

fn supplier_of(v: &Voucher, masters: &Masters) -> Result<Outcome<Supplier>, GstError> {
    let Some(party_id) = v.party else {
        return unsupported(
            "reverse charge on a purchase without a party",
            "the supplier's registration decides the liability; set the supplier as the party",
        );
    };
    let party = masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    let Some(details) = &party.party else {
        return unsupported(
            "reverse charge on a purchase from a ledger without party details",
            "the supplier's registration is not on the party ledger; add it",
        );
    };
    let registered = match details.registration {
        RegistrationType::Regular => true,
        RegistrationType::Unregistered | RegistrationType::Consumer => false,
        RegistrationType::Composition => {
            return unsupported(
                "reverse charge on a purchase from a composition supplier",
                "a composition supplier's supplies under s.9(3) are not decided here",
            );
        }
        RegistrationType::Sez | RegistrationType::Overseas | RegistrationType::Uin => {
            return unsupported(
                format!(
                    "reverse charge on a purchase from a {:?} supplier",
                    details.registration
                ),
                "imports (IGST Act s.5(3)) and SEZ or UIN supplies are not decided here",
            );
        }
    };
    let from_gstin = details.gstin.as_ref().and_then(ledgerly_core::Gstin::state);
    let state = match (details.state, from_gstin) {
        (Some(a), Some(b)) if a != b => {
            return unsupported(
                "reverse charge on a purchase from a supplier whose state is ambiguous",
                "the party ledger's state and its GSTIN's state differ; correct the party ledger",
            );
        }
        (Some(a), Some(_) | None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    };
    Ok(Outcome::Value(Supplier { registered, state }))
}

/// Which taxes apply: the supplier's state against the place of supply (IGST Act s.7, s.8).
fn supply_heads(v: &Voucher, supplier_state: Option<StateCode>) -> Outcome<TaxHeads> {
    let Some(place) = v.place_of_supply else {
        return Outcome::unsupported(
            "GST place of supply",
            "the voucher has no place of supply; set it before computing tax",
        );
    };
    if place.get() == 99 {
        return Outcome::unsupported(
            "reverse charge with the centre jurisdiction as place of supply",
            "state code 99 is not a state for charging CGST/SGST or IGST",
        );
    }
    let Some(supplier) = supplier_state else {
        return Outcome::unsupported(
            "reverse charge on a purchase from a supplier of unknown state",
            "the party ledger has neither a state nor a GSTIN; set the supplier's state",
        );
    };
    let heads = if place != supplier {
        TaxHeads::Igst
    } else if place.is_utgst_territory() {
        TaxHeads::CgstUtgst
    } else {
        TaxHeads::CgstSgst
    };
    Outcome::Value(heads)
}

/// A line of the voucher with its tax facts, before the rate is resolved.
struct Base {
    source: LineSource,
    taxability: Taxability,
    taxable: Paise,
    rate: Option<Decimal>,
    hsn: Option<String>,
}

fn is_taxable(t: Taxability) -> bool {
    t == Taxability::Taxable
}

/// The classified ledger lines of the voucher, with their side. `Unsupported` for a cess ledger.
fn ledger_bases(v: &Voucher, masters: &Masters) -> Result<Outcome<Vec<(Base, DrCr)>>, GstError> {
    let mut out = Vec::new();
    for line in &v.ledger_lines {
        let ledger = masters
            .ledgers
            .get(&line.ledger)
            .ok_or_else(|| not_found(format!("ledger {}", line.ledger.0)))?;
        if let Some(tax) = &ledger.tax
            && tax.kind == TaxLedgerKind::Cess
        {
            return unsupported(
                "GST cess",
                "the voucher posts to a cess ledger; no verified cess rate is available",
            );
        }
        let Some(class) = &ledger.gst else {
            continue;
        };
        if line.amount.amount.is_negative() {
            return Err(invalid(
                "ledger line amount",
                format!("ledger {} has a negative line amount", line.ledger.0),
            ));
        }
        let base = Base {
            source: LineSource::Ledger {
                ledger: line.ledger,
            },
            taxability: class.taxability,
            taxable: line.amount.amount,
            rate: class.rate_percent.map(|r| r.0),
            hsn: class.hsn.as_ref().map(|h| h.as_str().to_owned()),
        };
        out.push((base, line.amount.side));
    }
    Ok(Outcome::Value(out))
}

fn collect_bases(
    v: &Voucher,
    masters: &Masters,
    tos: Date,
) -> Result<Outcome<Vec<Base>>, GstError> {
    let ledger_bases = supported!(ledger_bases(v, masters)?);
    match v.mode {
        VoucherMode::ItemInvoice => {
            let bases = supported!(item_bases(v, masters, tos)?);
            let inventory_sum = sum_taxable(bases.iter())?;
            let ledger_sum = sum_taxable(ledger_bases.iter().map(|(b, _)| b))?;
            if !ledger_sum.is_zero() && ledger_sum != inventory_sum {
                return unsupported(
                    "reverse charge on an item invoice with extra taxable ledger lines",
                    "the classified ledger lines do not add up to the inventory lines (for example freight); not computed",
                );
            }
            Ok(Outcome::Value(bases))
        }
        VoucherMode::AccountingInvoice => {
            let first_side = ledger_bases.first().map(|(_, s)| *s);
            if ledger_bases.iter().any(|(_, s)| Some(*s) != first_side) {
                return unsupported(
                    "reverse charge on an invoice with taxable lines on both sides",
                    "debit and credit taxable lines in one voucher are not netted automatically",
                );
            }
            if ledger_bases.is_empty() {
                return unsupported(
                    "reverse charge on an accounting invoice",
                    "no ledger line has a GST classification, so there is nothing to tax",
                );
            }
            Ok(Outcome::Value(
                ledger_bases.into_iter().map(|(b, _)| b).collect(),
            ))
        }
        VoucherMode::AsVoucher => unsupported(
            "reverse charge on a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that taxable lines can be identified",
        ),
    }
}

fn sum_taxable<'a>(bases: impl Iterator<Item = &'a Base>) -> Result<Paise, GstError> {
    let mut sum = Paise::ZERO;
    for b in bases {
        sum = sum.checked_add(b.taxable)?;
    }
    Ok(sum)
}

fn item_bases(v: &Voucher, masters: &Masters, tos: Date) -> Result<Outcome<Vec<Base>>, GstError> {
    if v.inventory_lines.is_empty() {
        return unsupported(
            "reverse charge on an item invoice",
            "the voucher has no inventory lines, so there is nothing to tax",
        );
    }
    let mut bases = Vec::with_capacity(v.inventory_lines.len());
    for (index, line) in v.inventory_lines.iter().enumerate() {
        let item = masters
            .stock_items
            .get(&line.item)
            .ok_or_else(|| not_found(format!("stock item {}", line.item.0)))?;
        let history = supported!(item_history(item, tos)?);
        if line.amount.is_negative() {
            return Err(invalid(
                "inventory line amount",
                format!("line {} of the voucher has a negative amount", index + 1),
            ));
        }
        bases.push(Base {
            source: LineSource::Inventory { index },
            taxability: history.taxability,
            taxable: line.amount,
            rate: history.rate_percent.map(|r| r.0),
            hsn: item.hsn.as_ref().map(|h| h.as_str().to_owned()),
        });
    }
    Ok(Outcome::Value(bases))
}

/// The item's GST details in force on `tos`: the entry with the latest `effective_from` not after
/// it. Two entries with that same date are ambiguous master data and an error.
fn item_history(item: &StockItem, tos: Date) -> Result<Outcome<&GstHistory>, GstError> {
    let latest = item
        .gst_history
        .iter()
        .filter(|h| h.effective_from <= tos)
        .map(|h| h.effective_from)
        .max();
    let winners: Vec<&GstHistory> = item
        .gst_history
        .iter()
        .filter(|h| Some(h.effective_from) == latest)
        .collect();
    match winners.as_slice() {
        [] => unsupported(
            format!("GST rate of item `{}` on {tos}", item.name),
            "the item has no GST details effective on the time of supply; add them to the item",
        ),
        [one] => Ok(Outcome::Value(*one)),
        [..] => Err(invalid(
            "stock item GST history",
            format!(
                "item `{}` has several GST detail entries effective from the same date; keep one",
                item.name
            ),
        )),
    }
}

/// One notified class read from the pack.
struct Class {
    id: String,
    legal_ref: String,
    prefixes: Vec<String>,
    unregistered_only: bool,
}

fn csv(text: &str) -> Vec<&str> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

fn text_param<'a>(entry: &'a PackEntry, key: &str) -> Result<&'a str, GstError> {
    entry
        .params()
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| pack_err(format!("entry `{}` has no `{key}` parameter", entry.id())))
}

/// Reads the index and every class it lists. `Unsupported` when the index or any listed class is
/// missing, pending or not in force on the date (see the module docs).
fn load_classes(rules: &RuleSet) -> Result<Outcome<Vec<Class>>, GstError> {
    let index = supported!(rules.get(INDEX_ID));
    let ids = csv(text_param(index, "ids")?);
    if ids.is_empty() {
        return Err(pack_err(format!("entry `{INDEX_ID}` lists no class id")));
    }
    let mut classes: Vec<Class> = Vec::with_capacity(ids.len());
    for id in ids {
        if classes.iter().any(|c| c.id == id) {
            return Err(pack_err(format!(
                "entry `{INDEX_ID}` lists class `{id}` twice"
            )));
        }
        let entry = supported!(rules.get(&format!("{CLASS_PREFIX}{id}")));
        classes.push(read_class(id, entry)?);
    }
    Ok(Outcome::Value(classes))
}

fn read_class(id: &str, entry: &PackEntry) -> Result<Class, GstError> {
    let legal_ref = entry
        .legal_ref()
        .ok_or_else(|| pack_err(format!("entry `{}` has no legal reference", entry.id())))?
        .to_owned();
    let prefixes: Vec<String> = csv(text_param(entry, "hsn_prefixes")?)
        .into_iter()
        .map(str::to_owned)
        .collect();
    if prefixes.is_empty() {
        return Err(pack_err(format!(
            "entry `{}` has no hsn_prefixes",
            entry.id()
        )));
    }
    if prefixes
        .iter()
        .any(|p| !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(pack_err(format!(
            "entry `{}`: hsn_prefixes must be digits only",
            entry.id()
        )));
    }
    let unregistered_only = match text_param(entry, "supplier")? {
        "any" => false,
        "unregistered" => true,
        other => {
            return Err(pack_err(format!(
                "entry `{}`: supplier `{other}` is neither `any` nor `unregistered`",
                entry.id()
            )));
        }
    };
    Ok(Class {
        id: id.to_owned(),
        legal_ref,
        prefixes,
        unregistered_only,
    })
}

/// The one class that matches every taxable line, or `None` when no line matches any class.
/// `Unsupported` when a line has no HSN/SAC, matches several classes, lines differ in class, or
/// only some lines match.
fn match_class<'a>(taxable: &[Base], classes: &'a [Class]) -> Outcome<Option<&'a Class>> {
    let mut found: Option<&Class> = None;
    let mut unmatched = false;
    for base in taxable {
        let Some(hsn) = &base.hsn else {
            return Outcome::unsupported(
                "reverse charge on a taxable line without HSN/SAC",
                "the notified classes are matched by HSN/SAC; set it on the item or ledger",
            );
        };
        let hits: Vec<&Class> = classes
            .iter()
            .filter(|c| c.prefixes.iter().any(|p| hsn.starts_with(p.as_str())))
            .collect();
        match hits.as_slice() {
            [] => unmatched = true,
            [one] => {
                if let Some(f) = found
                    && f.id != one.id
                {
                    return Outcome::unsupported(
                        "reverse charge on lines of different notified classes",
                        "split the purchase into one voucher per class",
                    );
                }
                found = Some(*one);
            }
            [..] => {
                return Outcome::unsupported(
                    format!("reverse charge on HSN/SAC {hsn}"),
                    "several notified classes of the rule pack match it; fix the pack",
                );
            }
        }
    }
    if found.is_some() && unmatched {
        return Outcome::unsupported(
            "reverse charge on a voucher with notified and other taxable lines",
            "some lines are under reverse charge and some are not; split them into two vouchers",
        );
    }
    Outcome::Value(found)
}

/// CGST, SGST and IGST percentages of one slab entry.
struct Slab {
    igst: Decimal,
    cgst: Decimal,
    sgst: Decimal,
}

fn param(entry: &PackEntry, key: &str) -> Result<Decimal, GstError> {
    let text = text_param(entry, key)?;
    text.trim().parse::<Decimal>().map_err(|e| {
        pack_err(format!(
            "entry `{}`: {key} `{text}` is not a decimal number ({e})",
            entry.id()
        ))
    })
}

/// Finds the verified slab whose IGST percentage equals `rate`. Pending and absent entries are
/// skipped; if nothing verified matches the answer is `Unsupported`.
fn find_slab(rules: &RuleSet, rate: Decimal) -> Result<Outcome<Slab>, GstError> {
    for id in SLAB_IDS {
        let Outcome::Value(entry) = rules.get(id) else {
            continue;
        };
        let total_pct = param(entry, "igst_percent")?;
        if total_pct != rate {
            continue;
        }
        let central_pct = param(entry, "cgst_percent")?;
        let state_pct = param(entry, "sgst_percent")?;
        let split = central_pct
            .checked_add(state_pct)
            .ok_or(MoneyError::Overflow)?;
        if split != total_pct {
            return Err(pack_err(format!(
                "entry `{id}`: cgst_percent {central_pct} + sgst_percent {state_pct} is not igst_percent {total_pct}"
            )));
        }
        return Ok(Outcome::Value(Slab {
            igst: total_pct,
            cgst: central_pct,
            sgst: state_pct,
        }));
    }
    unsupported(
        format!("GST rate {rate}% on {}", rules.date()),
        format!(
            "rule pack {} has no verified rate slab of {rate}% on that date (entries may be pending); update the rule packs or correct the rate",
            rules.version()
        ),
    )
}

/// `percent`% of `taxable`, in exact rupees (no rounding).
fn percent_of(taxable: Paise, percent: Decimal) -> Result<Decimal, GstError> {
    taxable
        .to_rupees()
        .checked_mul(percent)
        .and_then(|x| x.checked_div(Decimal::ONE_HUNDRED))
        .ok_or_else(|| GstError::from(MoneyError::Overflow))
}

fn price_line(base: &Base, heads: TaxHeads, rules: &RuleSet) -> Result<Outcome<RcmLine>, GstError> {
    let Some(rate) = base.rate else {
        return unsupported(
            "GST rate of a taxable line",
            "the item or ledger is taxable but has no rate; set the rate on the master",
        );
    };
    let slab = supported!(find_slab(rules, rate)?);
    let mut line = RcmLine {
        source: base.source,
        taxable: base.taxable,
        rate_percent: slab.igst,
        cgst: Decimal::ZERO,
        sgst: Decimal::ZERO,
        utgst: Decimal::ZERO,
        igst: Decimal::ZERO,
    };
    match heads {
        TaxHeads::CgstSgst => {
            line.cgst = percent_of(base.taxable, slab.cgst)?;
            line.sgst = percent_of(base.taxable, slab.sgst)?;
        }
        TaxHeads::CgstUtgst => {
            line.cgst = percent_of(base.taxable, slab.cgst)?;
            line.utgst = percent_of(base.taxable, slab.sgst)?;
        }
        TaxHeads::Igst => {
            line.igst = percent_of(base.taxable, slab.igst)?;
        }
    }
    Ok(Outcome::Value(line))
}

fn head_total(lines: &[RcmLine], head: impl Fn(&RcmLine) -> Decimal) -> Result<Paise, GstError> {
    let mut sum = Decimal::ZERO;
    for l in lines {
        sum = sum.checked_add(head(l)).ok_or(MoneyError::Overflow)?;
    }
    round_rupees_to_paise(sum, RoundingRule::HalfUpToPaise).map_err(GstError::from)
}

fn assemble(
    lines: Vec<RcmLine>,
    heads: TaxHeads,
    pack_version: &str,
    class: &Class,
    marked: bool,
) -> Result<RcmPayable, GstError> {
    let mut taxable_total = Paise::ZERO;
    for l in &lines {
        taxable_total = taxable_total.checked_add(l.taxable)?;
    }
    let totals = TaxTotals {
        cgst: head_total(&lines, |l| l.cgst)?,
        sgst: head_total(&lines, |l| l.sgst)?,
        utgst: head_total(&lines, |l| l.utgst)?,
        igst: head_total(&lines, |l| l.igst)?,
        cess: Paise::ZERO,
    };
    Ok(RcmPayable {
        pack_version: pack_version.to_owned(),
        class_id: class.id.clone(),
        legal_ref: class.legal_ref.clone(),
        heads,
        lines,
        taxable_total,
        totals,
        itc: RcmItc::AfterTaxPaid,
        marked_on_voucher: marked,
    })
}
