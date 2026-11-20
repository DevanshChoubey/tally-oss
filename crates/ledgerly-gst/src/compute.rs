//! Voucher tax computation and rounding (spec 6.3). **Card G4** implements this module.
//!
//! Frozen interface (plan card G4):
//!
//! ```text
//! pub struct GstBreakup { .. }   // defined by G4: per-line taxable value, rate, CGST/SGST/UTGST/IGST/cess, round-off
//! pub fn compute_voucher_tax(v: &Voucher, masters: &Masters, rules: &RuleSet, company: &Company) -> Result<Outcome<GstBreakup>, GstError>;
//! ```
//!
//! # What is computed
//!
//! Tax is computed for sales, purchase, credit-note and debit-note vouchers entered as item
//! invoices (one taxable base per inventory line) or accounting invoices (one taxable base per
//! ledger line whose ledger carries a GST classification). The rate comes from the item's or the
//! ledger's effective-dated rate, selected by the time of supply, and is then confirmed against a
//! **verified** slab entry of the rule pack in force: the CGST, SGST/UTGST and IGST percentages
//! are taken from the pack, never recomputed from the rate.
//!
//! # Rounding policy (spec 6.3, CGST Act s.170)
//!
//! * Line tax is kept **unrounded** (exact decimal rupees) on every [`LineTax`].
//! * Each tax head is summed over the voucher and rounded once, half away from zero, to the paisa
//!   ([`RoundingPolicy::HeadTotalsToPaiseInvoiceToRupee`]).
//! * The invoice value (taxable value plus the rounded heads) is rounded to the whole rupee
//!   (s.170 rounds amounts payable to the nearest rupee) and the difference is stored as the
//!   explicit [`GstBreakup::round_off`] line.
//!
//! # Not computed (returns `Unsupported`)
//!
//! Reverse charge, cancelled vouchers, plain Dr/Cr vouchers, composition suppliers, registrations
//! in several states, SEZ/overseas/UIN recipients (zero-rated supplies belong to place-of-supply
//! card G3), a missing place of supply, a payment dated before the invoice (time of supply,
//! card G12), vouchers posting to a cess ledger (no verified cess rate exists yet), taxable lines
//! whose rate matches no verified pack slab, and any rate whose pack entry is `pending`.
//!
//! The place of supply is never guessed: `Voucher::place_of_supply` must be set.

use ledgerly_core::{
    Date, Decimal, LedgerId, MoneyError, Outcome, Paise, RoundingRule, StateCode,
    round_rupees_to_paise,
};
use ledgerly_model::{
    BaseVoucherKind, Company, GstClassification, GstRegistration, LedgerLine, Masters,
    RegistrationType, TaxLedgerKind, Taxability, Voucher, VoucherMode, VoucherStatus,
};

use crate::GstError;
use crate::pack::{PackEntry, RuleSet};

/// Pack entry ids that hold a rate slab (`igst_percent`, `cgst_percent`, `sgst_percent` params).
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

/// Which taxes the supply attracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaxHeads {
    /// Intra-state supply in a state: CGST and SGST.
    CgstSgst,
    /// Intra-state supply in a union territory without a legislature: CGST and UTGST.
    CgstUtgst,
    /// Inter-state supply: IGST.
    Igst,
}

/// How the amounts in a [`GstBreakup`] were rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingPolicy {
    /// Line tax stays unrounded; each head's voucher total is rounded half away from zero to the
    /// paisa; the invoice value is rounded half away from zero to the rupee (s.170) and the
    /// difference is the round-off line.
    HeadTotalsToPaiseInvoiceToRupee,
}

/// Where a taxable line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineSource {
    /// The inventory line at this position in `Voucher::inventory_lines`.
    Inventory {
        /// Zero-based position.
        index: usize,
    },
    /// A ledger line posted to this (sales, purchase, income or expense) ledger.
    Ledger {
        /// The ledger.
        ledger: LedgerId,
    },
}

/// Tax on one taxable line. Tax amounts are exact rupees, **not rounded**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineTax {
    /// Where the line came from.
    pub source: LineSource,
    /// Taxability of the line.
    pub taxability: Taxability,
    /// Taxable value.
    pub taxable: Paise,
    /// Total rate in percent (the IGST rate); zero for exempt, nil-rated and non-GST lines.
    pub rate_percent: Decimal,
    /// Unrounded CGST in rupees (zero for inter-state supplies).
    pub cgst: Decimal,
    /// Unrounded SGST in rupees (zero for inter-state supplies and union territories).
    pub sgst: Decimal,
    /// Unrounded UTGST in rupees (non-zero only for union-territory supplies).
    pub utgst: Decimal,
    /// Unrounded IGST in rupees (zero for intra-state supplies).
    pub igst: Decimal,
    /// Unrounded cess in rupees. Always zero in this version: no verified cess rate exists, and a
    /// voucher with a cess ledger line is `Unsupported`.
    pub cess: Decimal,
}

/// Voucher totals per tax head, each rounded to the paisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxTotals {
    /// CGST.
    pub cgst: Paise,
    /// SGST.
    pub sgst: Paise,
    /// UTGST.
    pub utgst: Paise,
    /// IGST.
    pub igst: Paise,
    /// Cess (always zero in this version, see [`LineTax::cess`]).
    pub cess: Paise,
}

/// The GST on one voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstBreakup {
    /// Version of the rule pack the rates came from.
    pub pack_version: String,
    /// Which taxes apply.
    pub heads: TaxHeads,
    /// One entry per taxable line, in voucher order.
    pub lines: Vec<LineTax>,
    /// Sum of the taxable values.
    pub taxable_total: Paise,
    /// Tax per head, rounded.
    pub totals: TaxTotals,
    /// Taxable value plus the rounded tax heads, before the round-off line.
    pub total_before_round_off: Paise,
    /// The explicit round-off line: rounded invoice value minus `total_before_round_off`.
    pub round_off: Paise,
    /// How the figures were rounded.
    pub policy: RoundingPolicy,
}

/// A taxable base found on the voucher, before its rate is resolved.
struct Base {
    source: LineSource,
    taxability: Taxability,
    taxable: Paise,
    rate: Option<Decimal>,
}

/// CGST, SGST and IGST percentages of one slab entry.
struct Slab {
    igst: Decimal,
    cgst: Decimal,
    sgst: Decimal,
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

/// Computes the GST on a sales, purchase, credit-note or debit-note voucher.
///
/// `rules` must be the rule set selected for this voucher's time of supply (`supply_date` when
/// set, otherwise the voucher date), see [`crate::pack::RulePack::for_time_of_supply`].
///
/// # Errors
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger or stock item
///   that is not in `masters`.
/// * [`GstError::Invalid`] when `rules` was selected for another date than the voucher's time of
///   supply, or a line amount is negative.
/// * [`GstError::Pack`] when a verified slab entry lacks a percentage or its CGST and SGST do not
///   add up to its IGST.
/// * [`GstError::Money`] on arithmetic overflow.
pub fn compute_voucher_tax(
    v: &Voucher,
    masters: &Masters,
    rules: &RuleSet,
    company: &Company,
) -> Result<Outcome<GstBreakup>, GstError> {
    supported!(check_scope(v, masters, company)?);
    let tos = supported!(time_of_supply(v));
    if rules.date() != tos {
        return Err(invalid(
            "rules",
            format!(
                "the rule set is for {} but the time of supply is {tos}",
                rules.date()
            ),
        ));
    }
    let heads = supported!(supply_heads(v, masters, company, tos)?);
    let bases = supported!(collect_bases(v, masters, tos)?);
    let mut lines = Vec::with_capacity(bases.len());
    for base in &bases {
        lines.push(supported!(price_line(base, heads, rules)?));
    }
    let breakup = assemble(lines, heads, rules.version().to_owned())?;
    Ok(Outcome::Value(breakup))
}

fn check_scope(v: &Voucher, masters: &Masters, company: &Company) -> Result<Outcome<()>, GstError> {
    if !company.features.gst {
        return unsupported(
            "GST on a voucher",
            "GST is not enabled for this company; enable it in the company features",
        );
    }
    match v.status {
        VoucherStatus::Cancelled => {
            return unsupported(
                "GST on a cancelled voucher",
                "a cancelled voucher has no amounts and no tax",
            );
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    if v.reverse_charge {
        return unsupported(
            "GST under reverse charge",
            "reverse charge liability is not computed by voucher tax",
        );
    }
    let voucher_type = masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| not_found(format!("voucher type {}", v.type_id.0)))?;
    match voucher_type.base {
        BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => {}
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
                format!("GST on a {:?} voucher", voucher_type.base),
                "only sales, purchase, credit-note and debit-note vouchers are taxed",
            );
        }
    }
    match v.mode {
        VoucherMode::AccountingInvoice | VoucherMode::ItemInvoice => Ok(Outcome::Value(())),
        VoucherMode::AsVoucher => unsupported(
            "GST on a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that taxable lines can be identified",
        ),
    }
}

/// The time of supply taken from the voucher: its `supply_date`, else its date. A payment dated
/// before the invoice can move the time of supply, which this module does not decide.
fn time_of_supply(v: &Voucher) -> Outcome<Date> {
    match (v.supply_date, v.payment_date) {
        (Some(d), Some(_) | None) => Outcome::Value(d),
        (None, Some(p)) if p < v.date => Outcome::unsupported(
            "time of supply with a payment dated before the invoice",
            "set the supply date explicitly; the time-of-supply rules are not applied here",
        ),
        (None, Some(_) | None) => Outcome::Value(v.date),
    }
}

/// The state of the single registration that supplies on `tos`.
fn supplier_state(company: &Company, tos: Date) -> Outcome<StateCode> {
    let regs: Vec<&GstRegistration> = company
        .registrations
        .iter()
        .filter(|r| r.effective_from <= tos)
        .collect();
    let Some(first) = regs.first() else {
        return Outcome::unsupported(
            format!("GST on a supply dated {tos}"),
            "the company has no GST registration in force on the time of supply",
        );
    };
    if regs.iter().any(|r| r.composition) {
        return Outcome::unsupported(
            "GST by a composition taxpayer",
            "a composition supplier does not charge tax on its invoices; not computed",
        );
    }
    if regs.iter().any(|r| r.state != first.state) {
        return Outcome::unsupported(
            "GST with registrations in several states",
            "the voucher does not say which registration supplies; not computed",
        );
    }
    Outcome::Value(first.state)
}

/// `Unsupported` when the party is a kind of recipient that this module does not tax.
fn party_supported(v: &Voucher, masters: &Masters) -> Result<Outcome<()>, GstError> {
    let Some(party_id) = v.party else {
        return Ok(Outcome::Value(()));
    };
    let party = masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    let Some(details) = &party.party else {
        return Ok(Outcome::Value(()));
    };
    match details.registration {
        RegistrationType::Regular
        | RegistrationType::Composition
        | RegistrationType::Unregistered
        | RegistrationType::Consumer => Ok(Outcome::Value(())),
        RegistrationType::Sez | RegistrationType::Overseas | RegistrationType::Uin => unsupported(
            format!("GST on a supply to a {:?} party", details.registration),
            "zero-rated, SEZ and UIN supplies are not computed by voucher tax",
        ),
    }
}

fn supply_heads(
    v: &Voucher,
    masters: &Masters,
    company: &Company,
    tos: Date,
) -> Result<Outcome<TaxHeads>, GstError> {
    let supplier = supported!(supplier_state(company, tos));
    let Some(place) = v.place_of_supply else {
        return unsupported(
            "GST place of supply",
            "the voucher has no place of supply; set it before computing tax",
        );
    };
    supported!(party_supported(v, masters)?);
    if place.get() == 99 {
        return unsupported(
            "GST with the centre jurisdiction as place of supply",
            "state code 99 is not a state for charging CGST/SGST or IGST",
        );
    }
    let heads = if place != supplier {
        TaxHeads::Igst
    } else if place.is_utgst_territory() {
        TaxHeads::CgstUtgst
    } else {
        TaxHeads::CgstSgst
    };
    Ok(Outcome::Value(heads))
}

fn collect_bases(
    v: &Voucher,
    masters: &Masters,
    tos: Date,
) -> Result<Outcome<Vec<Base>>, GstError> {
    let mut ledger_bases = Vec::new();
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
        if let Some(class) = &ledger.gst {
            ledger_bases.push((ledger_base(line, class)?, line));
        }
    }
    match v.mode {
        VoucherMode::ItemInvoice => {
            let bases = supported!(item_bases(v, masters, tos)?);
            let inventory_sum = sum_taxable(bases.iter())?;
            let ledger_sum = sum_taxable(ledger_bases.iter().map(|(b, _)| b))?;
            if !ledger_sum.is_zero() && ledger_sum != inventory_sum {
                return unsupported(
                    "GST on an item invoice with extra taxable ledger lines",
                    "the classified ledger lines do not add up to the inventory lines (for example freight); not computed",
                );
            }
            Ok(Outcome::Value(bases))
        }
        VoucherMode::AccountingInvoice => accounting_bases(ledger_bases),
        VoucherMode::AsVoucher => unsupported(
            "GST on a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that taxable lines can be identified",
        ),
    }
}

fn accounting_bases(
    ledger_bases: Vec<(Base, &LedgerLine)>,
) -> Result<Outcome<Vec<Base>>, GstError> {
    let first_side = ledger_bases.first().map(|(_, l)| l.amount.side);
    if ledger_bases
        .iter()
        .any(|(_, l)| Some(l.amount.side) != first_side)
    {
        return unsupported(
            "GST on an invoice with taxable lines on both sides",
            "debit and credit taxable lines in one voucher are not netted automatically",
        );
    }
    if ledger_bases.is_empty() {
        return unsupported(
            "GST on an accounting invoice",
            "no ledger line has a GST classification, so there is nothing to tax",
        );
    }
    Ok(Outcome::Value(
        ledger_bases.into_iter().map(|(b, _)| b).collect(),
    ))
}

fn sum_taxable<'a>(bases: impl Iterator<Item = &'a Base>) -> Result<Paise, GstError> {
    let mut sum = Paise::ZERO;
    for b in bases {
        sum = sum.checked_add(b.taxable)?;
    }
    Ok(sum)
}

fn ledger_base(line: &LedgerLine, class: &GstClassification) -> Result<Base, GstError> {
    if line.amount.amount.is_negative() {
        return Err(invalid(
            "ledger line amount",
            format!("ledger {} has a negative line amount", line.ledger.0),
        ));
    }
    Ok(Base {
        source: LineSource::Ledger {
            ledger: line.ledger,
        },
        taxability: class.taxability,
        taxable: line.amount.amount,
        rate: class.rate_percent.map(|r| r.0),
    })
}

fn item_bases(v: &Voucher, masters: &Masters, tos: Date) -> Result<Outcome<Vec<Base>>, GstError> {
    if v.inventory_lines.is_empty() {
        return unsupported(
            "GST on an item invoice",
            "the voucher has no inventory lines, so there is nothing to tax",
        );
    }
    let mut bases = Vec::with_capacity(v.inventory_lines.len());
    for (index, line) in v.inventory_lines.iter().enumerate() {
        let item = masters
            .stock_items
            .get(&line.item)
            .ok_or_else(|| not_found(format!("stock item {}", line.item.0)))?;
        let Some(history) = item
            .gst_history
            .iter()
            .filter(|h| h.effective_from <= tos)
            .max_by_key(|h| h.effective_from)
        else {
            return unsupported(
                format!("GST rate of item `{}` on {tos}", item.name),
                "the item has no GST details effective on the time of supply; add them to the item",
            );
        };
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
        });
    }
    Ok(Outcome::Value(bases))
}

fn price_line(base: &Base, heads: TaxHeads, rules: &RuleSet) -> Result<Outcome<LineTax>, GstError> {
    let mut line = LineTax {
        source: base.source,
        taxability: base.taxability,
        taxable: base.taxable,
        rate_percent: Decimal::ZERO,
        cgst: Decimal::ZERO,
        sgst: Decimal::ZERO,
        utgst: Decimal::ZERO,
        igst: Decimal::ZERO,
        cess: Decimal::ZERO,
    };
    match base.taxability {
        Taxability::Taxable => {}
        Taxability::Exempt | Taxability::NilRated | Taxability::NonGst => {
            return Ok(Outcome::Value(line));
        }
    }
    let Some(rate) = base.rate else {
        return unsupported(
            "GST rate of a taxable line",
            "the item or ledger is taxable but has no rate; set the rate on the master",
        );
    };
    let slab = supported!(find_slab(rules, rate)?);
    line.rate_percent = slab.igst;
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

fn param(entry: &PackEntry, key: &str) -> Result<Decimal, GstError> {
    let text = entry.params().get(key).ok_or_else(|| GstError::Pack {
        reason: format!("entry `{}` has no `{key}` parameter", entry.id()),
    })?;
    text.trim().parse::<Decimal>().map_err(|e| GstError::Pack {
        reason: format!(
            "entry `{}`: {key} `{text}` is not a decimal number ({e})",
            entry.id()
        ),
    })
}

/// Finds the verified slab whose IGST percentage equals `rate`. Pending and absent entries are
/// skipped; if nothing verified matches the answer is `Unsupported`.
fn find_slab(rules: &RuleSet, rate: Decimal) -> Result<Outcome<Slab>, GstError> {
    for id in SLAB_IDS {
        let Outcome::Value(entry) = rules.get(id) else {
            continue;
        };
        let igst = param(entry, "igst_percent")?;
        if igst != rate {
            continue;
        }
        let cgst = param(entry, "cgst_percent")?;
        let sgst = param(entry, "sgst_percent")?;
        let split = cgst.checked_add(sgst).ok_or(MoneyError::Overflow)?;
        if split != igst {
            return Err(GstError::Pack {
                reason: format!(
                    "entry `{id}`: cgst_percent {cgst} + sgst_percent {sgst} is not igst_percent {igst}"
                ),
            });
        }
        return Ok(Outcome::Value(Slab { igst, cgst, sgst }));
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

fn head_total(lines: &[LineTax], head: impl Fn(&LineTax) -> Decimal) -> Result<Paise, GstError> {
    let mut sum = Decimal::ZERO;
    for l in lines {
        sum = sum.checked_add(head(l)).ok_or(MoneyError::Overflow)?;
    }
    round_rupees_to_paise(sum, RoundingRule::HalfUpToPaise).map_err(GstError::from)
}

fn assemble(
    lines: Vec<LineTax>,
    heads: TaxHeads,
    pack_version: String,
) -> Result<GstBreakup, GstError> {
    let mut taxable_total = Paise::ZERO;
    for l in &lines {
        taxable_total = taxable_total.checked_add(l.taxable)?;
    }
    let totals = TaxTotals {
        cgst: head_total(&lines, |l| l.cgst)?,
        sgst: head_total(&lines, |l| l.sgst)?,
        utgst: head_total(&lines, |l| l.utgst)?,
        igst: head_total(&lines, |l| l.igst)?,
        cess: head_total(&lines, |l| l.cess)?,
    };
    let mut total = taxable_total;
    for tax in [
        totals.cgst,
        totals.sgst,
        totals.utgst,
        totals.igst,
        totals.cess,
    ] {
        total = total.checked_add(tax)?;
    }
    let rounded = round_rupees_to_paise(total.to_rupees(), RoundingRule::HalfUpToRupee)?;
    let round_off = rounded.checked_sub(total)?;
    Ok(GstBreakup {
        pack_version,
        heads,
        lines,
        taxable_total,
        totals,
        total_before_round_off: total,
        round_off,
        policy: RoundingPolicy::HeadTotalsToPaiseInvoiceToRupee,
    })
}
