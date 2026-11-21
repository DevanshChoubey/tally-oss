//! GSTR-3B working (books vs portal). **Card G10** implements this module.
//!
//! Frozen interface (plan card G10):
//!
//! ```text
//! pub struct Gstr3bWorking { .. }   // defined by G10
//! pub fn build_gstr3b(books: &Books, gstin: &Gstin, period: Period, pack: &RulePack, portal: Option<&Gstr3bPortalValues>) -> Result<Outcome<Gstr3bWorking>, GstError>;
//! ```
//!
//! # What is worked out
//!
//! Tables 3.1 and 3.2 are locked on the portal (spec 6.3), so Ledgerly does not file 3B. It works
//! out two tables from the books and lists where they differ from what the user reads off the
//! portal:
//!
//! * **3.1(a)**, outward taxable supplies (other than zero rated, nil rated and exempted): the sum
//!   of the taxable value of every line that G6 classifies as `Taxable`, and the tax on it from G4.
//! * **4(A)(5)**, all other ITC: the tax on every purchase line that G7 finds `Eligible`. Tax on
//!   `Blocked` and `Ineligible` lines is never added; it is shown separately in
//!   [`Gstr3bWorking::itc_not_claimable`].
//!
//! The figures are computed from the invoice vouchers by the engine, never from ledger balances,
//! so a journal posted straight to a tax ledger is not seen.
//!
//! # Which vouchers belong to the period
//!
//! * Outward (sales) vouchers: those whose time of supply falls in the period. The time of supply
//!   is the supply date when set, otherwise the voucher date, as in G4 (the full time-of-supply
//!   rules, card G12, are not applied; a payment dated before the invoice without a supply date is
//!   `Unsupported`).
//! * Purchase vouchers: those whose voucher date falls in the period. ITC is shown in the period
//!   in which the purchase is recorded. The conditions of CGST Act s.16(2) and s.16(4) (2B match,
//!   receipt of goods, payment to the supplier, time limit) are **not** checked, so 4(A)(5) is the
//!   credit the books could claim, not what 2B allows. The rule set for a purchase is selected by
//!   its time of supply as for sales.
//! * Optional and cancelled vouchers are not in the books and are left out. Regular and post-dated
//!   vouchers dated in the period are included.
//!
//! # Rounding
//!
//! Each tax head is rounded once per voucher, half away from zero, to the paisa (G4's policy), and
//! the voucher totals are added. For a purchase, only the eligible lines are summed before the
//! rounding, so a voucher with some blocked lines can differ by a paisa from the supplier's
//! invoice total. This is a product policy, not a rule of the Act.
//!
//! # Reverse charge purchases
//!
//! Purchases under reverse charge belong to tables 3.1(d) and 4(A)(3), not to 3.1(a) or 4(A)(5).
//! They are left out of both figures and listed in [`Gstr3bWorking::reverse_charge_purchases`] so
//! that nothing is dropped silently.
//!
//! # Not decided (the result is `Unsupported`)
//!
//! * GST not enabled for the company, a company with several registrations (vouchers are not tied
//!   to one GSTIN), a composition registration (composition dealers do not file GSTR-3B), or a
//!   registration that starts after the period.
//! * Credit notes and debit notes dated in the period: how they net against 3.1(a) and 4(A)(5)
//!   (CGST Act s.34, and the s.16(2) conditions for the recipient) has not been checked against
//!   the Act text yet, so it is `pending`.
//! * Outward supplies under reverse charge, receipt vouchers that carry an advance (tax on
//!   advances needs the time-of-supply rules of card G12), and outward lines that G6 classifies as
//!   exports, SEZ supplies, deemed exports or advances (tables 3.1(b) and others are not worked
//!   out here).
//! * Anything G4, G6 or G7 returns `Unsupported` for, including a purchase whose "ineligible for
//!   input credit" settings conflict (see [`crate::itc`]). The reason names the voucher.
//!
//! # Errors
//!
//! [`GstError::NotFound`] when `gstin` is not a registration of the company or a voucher refers to
//! a missing master, [`GstError::Invalid`] when the G4/G6 results do not line up, and
//! [`GstError::Money`] on overflow.

use ledgerly_core::{
    Date, Decimal, Gstin, MoneyError, Outcome, Paise, Period, RoundingRule, VoucherId,
    round_rupees_to_paise,
};
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, Books, Masters, Taxability, Voucher, VoucherMode, VoucherStatus,
};

pub use ledgerly_gst_formats::gstr3b::{Gstr3bPortalValues, Gstr3bTaxAmounts};

use crate::GstError;
use crate::classify::{SupplyKind, classify_line};
use crate::compute::{GstBreakup, LineSource, LineTax, compute_voucher_tax};
use crate::itc::itc_eligibility;
use crate::pack::{RulePack, RuleSet};

/// One figure of the working that is compared with the portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gstr3bField {
    /// 3.1(a) total taxable value.
    OutwardTaxableValue,
    /// 3.1(a) integrated tax.
    OutwardIntegrated,
    /// 3.1(a) central tax.
    OutwardCentral,
    /// 3.1(a) state/UT tax.
    OutwardStateUt,
    /// 3.1(a) cess.
    OutwardCess,
    /// 4(A)(5) integrated tax.
    ItcIntegrated,
    /// 4(A)(5) central tax.
    ItcCentral,
    /// 4(A)(5) state/UT tax.
    ItcStateUt,
    /// 4(A)(5) cess.
    ItcCess,
}

/// One row of the books-vs-portal comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonRow {
    /// Which figure.
    pub field: Gstr3bField,
    /// The figure worked out from the books.
    pub books: Paise,
    /// The figure the user read off the portal.
    pub portal: Paise,
    /// `books - portal`: positive when the books show more than the portal.
    pub difference: Paise,
}

/// The GSTR-3B working for one period of one registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr3bWorking {
    /// The registration the working is for.
    pub gstin: Gstin,
    /// The return period.
    pub period: Period,
    /// Version of the rule pack the rates and blocked categories came from.
    pub pack_version: String,
    /// Table 3.1(a): total taxable value.
    pub outward_taxable_value: Paise,
    /// Table 3.1(a): tax.
    pub outward_tax: Gstr3bTaxAmounts,
    /// The vouchers that contribute to 3.1(a), in voucher id order.
    pub outward_vouchers: Vec<VoucherId>,
    /// Table 4(A)(5): all other ITC (eligible lines only).
    pub itc_all_other: Gstr3bTaxAmounts,
    /// The vouchers that contribute to 4(A)(5), in voucher id order.
    pub itc_vouchers: Vec<VoucherId>,
    /// Tax on purchase lines that is blocked (s.17(5)) or otherwise ineligible. Not in 4(A)(5).
    pub itc_not_claimable: Gstr3bTaxAmounts,
    /// Purchases under reverse charge in the period: in neither 3.1(a) nor 4(A)(5).
    pub reverse_charge_purchases: Vec<VoucherId>,
    /// Books against portal, one row per figure; `None` when no portal values were given.
    pub comparison: Option<Vec<ComparisonRow>>,
}

impl Gstr3bWorking {
    /// The comparison rows whose books and portal figures differ. Empty when no portal values
    /// were given or everything agrees.
    pub fn differences(&self) -> Vec<&ComparisonRow> {
        self.comparison
            .iter()
            .flatten()
            .filter(|row| !row.difference.is_zero())
            .collect()
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

fn not_found(what: String) -> GstError {
    GstError::NotFound { what }
}

fn invalid(field: &str, reason: String) -> GstError {
    GstError::Invalid {
        field: field.to_owned(),
        reason,
    }
}

/// The voucher, as a short label for messages (number when there is one, never an amount).
fn label(v: &Voucher) -> String {
    match &v.number {
        Some(n) => format!("voucher {n} (id {}) dated {}", v.id.0, v.date),
        None => format!("voucher id {} dated {}", v.id.0, v.date),
    }
}

/// Unrounded tax per head, in exact rupees, summed over some lines of one voucher.
#[derive(Debug, Clone, Copy, Default)]
struct HeadSums {
    central: Decimal,
    state_ut: Decimal,
    integrated: Decimal,
    cess: Decimal,
}

impl HeadSums {
    fn add_line(&mut self, line: &LineTax) -> Result<(), GstError> {
        let state_ut = line
            .sgst
            .checked_add(line.utgst)
            .ok_or(MoneyError::Overflow)?;
        self.central = self
            .central
            .checked_add(line.cgst)
            .ok_or(MoneyError::Overflow)?;
        self.state_ut = self
            .state_ut
            .checked_add(state_ut)
            .ok_or(MoneyError::Overflow)?;
        self.integrated = self
            .integrated
            .checked_add(line.igst)
            .ok_or(MoneyError::Overflow)?;
        self.cess = self
            .cess
            .checked_add(line.cess)
            .ok_or(MoneyError::Overflow)?;
        Ok(())
    }

    /// Each head rounded once, half away from zero, to the paisa.
    fn rounded(&self) -> Result<Gstr3bTaxAmounts, GstError> {
        let round = |d: Decimal| round_rupees_to_paise(d, RoundingRule::HalfUpToPaise);
        Ok(Gstr3bTaxAmounts {
            integrated: round(self.integrated)?,
            central: round(self.central)?,
            state_ut: round(self.state_ut)?,
            cess: round(self.cess)?,
        })
    }
}

fn add_amounts(
    total: Gstr3bTaxAmounts,
    more: Gstr3bTaxAmounts,
) -> Result<Gstr3bTaxAmounts, GstError> {
    Ok(Gstr3bTaxAmounts {
        integrated: total.integrated.checked_add(more.integrated)?,
        central: total.central.checked_add(more.central)?,
        state_ut: total.state_ut.checked_add(more.state_ut)?,
        cess: total.cess.checked_add(more.cess)?,
    })
}

/// What the loop over the vouchers has gathered so far.
#[derive(Default)]
struct Totals {
    outward_taxable_value: Paise,
    outward_tax: Gstr3bTaxAmounts,
    outward_vouchers: Vec<VoucherId>,
    itc_all_other: Gstr3bTaxAmounts,
    itc_vouchers: Vec<VoucherId>,
    itc_not_claimable: Gstr3bTaxAmounts,
    reverse_charge_purchases: Vec<VoucherId>,
}

/// Where the working is built: the books, the period and the pack.
struct Scope<'a> {
    books: &'a Books,
    period: Period,
    pack: &'a RulePack,
}

/// Builds the GSTR-3B working of `gstin` for `period` (see the module docs for what is worked
/// out and what is `Unsupported`).
///
/// `portal` holds the user's figures from the GST portal; when given, the working carries a
/// comparison column and [`Gstr3bWorking::differences`] lists what does not agree.
///
/// # Errors
/// See the module docs: [`GstError::NotFound`], [`GstError::Invalid`], [`GstError::Pack`] (from a
/// malformed pack entry) and [`GstError::Money`].
pub fn build_gstr3b(
    books: &Books,
    gstin: &Gstin,
    period: Period,
    pack: &RulePack,
    portal: Option<&Gstr3bPortalValues>,
) -> Result<Outcome<Gstr3bWorking>, GstError> {
    supported!(check_registration(books, gstin, period)?);
    let scope = Scope {
        books,
        period,
        pack,
    };
    let mut totals = Totals::default();
    for voucher in books.vouchers.values() {
        supported!(add_voucher(&scope, voucher, &mut totals)?);
    }
    let mut working = Gstr3bWorking {
        gstin: gstin.clone(),
        period,
        pack_version: pack.version().to_owned(),
        outward_taxable_value: totals.outward_taxable_value,
        outward_tax: totals.outward_tax,
        outward_vouchers: totals.outward_vouchers,
        itc_all_other: totals.itc_all_other,
        itc_vouchers: totals.itc_vouchers,
        itc_not_claimable: totals.itc_not_claimable,
        reverse_charge_purchases: totals.reverse_charge_purchases,
        comparison: None,
    };
    if let Some(portal) = portal {
        working.comparison = Some(compare(&working, portal)?);
    }
    Ok(Outcome::Value(working))
}

fn check_registration(
    books: &Books,
    gstin: &Gstin,
    period: Period,
) -> Result<Outcome<()>, GstError> {
    let company = &books.company;
    let registration = company
        .registrations
        .iter()
        .find(|r| r.gstin == *gstin)
        .ok_or_else(|| {
            not_found("a GST registration of the company with the given GSTIN".to_owned())
        })?;
    if !company.features.gst {
        return unsupported(
            "GSTR-3B working",
            "GST is not enabled for this company; enable it in the company features",
        );
    }
    if company.registrations.len() > 1 {
        return unsupported(
            "GSTR-3B working for a company with several registrations",
            "vouchers are not tied to one GSTIN, so the figures of one registration cannot be told apart",
        );
    }
    if registration.composition {
        return unsupported(
            "GSTR-3B working for a composition registration",
            "composition taxpayers do not file GSTR-3B",
        );
    }
    if registration.effective_from > period.to() {
        return unsupported(
            "GSTR-3B working before the registration",
            format!(
                "the registration is effective from {}, after the period ends on {}",
                registration.effective_from,
                period.to()
            ),
        );
    }
    Ok(Outcome::Value(()))
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

/// `true` when any of the voucher's dates falls in the period.
fn touches_period(v: &Voucher, period: Period) -> bool {
    period.contains(v.date)
        || v.supply_date.is_some_and(|d| period.contains(d))
        || v.payment_date.is_some_and(|d| period.contains(d))
}

fn add_voucher(
    scope: &Scope<'_>,
    v: &Voucher,
    totals: &mut Totals,
) -> Result<Outcome<()>, GstError> {
    match v.status {
        VoucherStatus::Optional | VoucherStatus::Cancelled => return Ok(Outcome::Value(())),
        VoucherStatus::Regular | VoucherStatus::PostDated => {}
    }
    if !touches_period(v, scope.period) {
        return Ok(Outcome::Value(()));
    }
    let voucher_type = scope
        .books
        .masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| not_found(format!("voucher type {}", v.type_id.0)))?;
    match voucher_type.base {
        BaseVoucherKind::Sales => add_sales(scope, v, totals),
        BaseVoucherKind::Purchase => {
            if scope.period.contains(v.date) {
                add_purchase(scope, v, totals)
            } else {
                Ok(Outcome::Value(()))
            }
        }
        BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => unsupported(
            format!("{:?} in a GSTR-3B period", voucher_type.base),
            format!(
                "{} is a credit or debit note; how it nets against 3.1(a) and 4(A)(5) is pending (CGST Act s.34 not yet checked)",
                label(v)
            ),
        ),
        BaseVoucherKind::Receipt => check_receipt(scope, v),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
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
        | BaseVoucherKind::PurchaseOrder => Ok(Outcome::Value(())),
    }
}

/// A receipt that carries an advance would be tax due in 3.1(a); that is not worked out.
fn check_receipt(scope: &Scope<'_>, v: &Voucher) -> Result<Outcome<()>, GstError> {
    if !scope.period.contains(v.date) {
        return Ok(Outcome::Value(()));
    }
    let has_advance = v
        .ledger_lines
        .iter()
        .flat_map(|l| l.bills.iter())
        .any(|b| b.kind == BillRefKind::Advance);
    if has_advance {
        return unsupported(
            "tax on an advance received",
            format!(
                "{} carries an advance; tax on advances needs the time-of-supply rules (card G12)",
                label(v)
            ),
        );
    }
    Ok(Outcome::Value(()))
}

/// The positions (in `inventory_lines` for item invoices, in `ledger_lines` for accounting
/// invoices) of the lines G4 taxes, in the order of [`GstBreakup::lines`].
fn line_positions(v: &Voucher, masters: &Masters) -> Result<Vec<usize>, GstError> {
    match v.mode {
        VoucherMode::ItemInvoice => Ok((0..v.inventory_lines.len()).collect()),
        VoucherMode::AccountingInvoice => {
            let mut positions = Vec::new();
            for (position, line) in v.ledger_lines.iter().enumerate() {
                let ledger = masters
                    .ledgers
                    .get(&line.ledger)
                    .ok_or_else(|| not_found(format!("ledger {}", line.ledger.0)))?;
                if ledger.gst.is_some() {
                    positions.push(position);
                }
            }
            Ok(positions)
        }
        VoucherMode::AsVoucher => Ok(Vec::new()),
    }
}

/// Checks that G4's line `line_tax` is the line at `position` of the voucher.
fn check_source(v: &Voucher, position: usize, line_tax: &LineTax) -> Result<(), GstError> {
    let matches = match (v.mode, line_tax.source) {
        (VoucherMode::ItemInvoice, LineSource::Inventory { index }) => index == position,
        (VoucherMode::AccountingInvoice, LineSource::Ledger { ledger }) => v
            .ledger_lines
            .get(position)
            .is_some_and(|l| l.ledger == ledger),
        (VoucherMode::ItemInvoice, LineSource::Ledger { .. })
        | (VoucherMode::AccountingInvoice, LineSource::Inventory { .. })
        | (VoucherMode::AsVoucher, LineSource::Inventory { .. } | LineSource::Ledger { .. }) => {
            false
        }
    };
    if matches {
        Ok(())
    } else {
        Err(invalid(
            "voucher lines",
            format!(
                "the tax lines of {} do not line up with its lines; the working cannot attribute tax",
                label(v)
            ),
        ))
    }
}

/// The voucher's tax breakup and the rule set of its time of supply.
struct Priced {
    rules: RuleSet,
    breakup: GstBreakup,
    positions: Vec<usize>,
}

fn price(scope: &Scope<'_>, v: &Voucher) -> Result<Outcome<Priced>, GstError> {
    let tos = supported!(time_of_supply(v));
    let rules = supported!(scope.pack.for_time_of_supply(tos));
    let masters = &scope.books.masters;
    let breakup = supported!(compute_voucher_tax(
        v,
        masters,
        &rules,
        &scope.books.company
    )?);
    let positions = line_positions(v, masters)?;
    if positions.len() != breakup.lines.len() {
        return Err(invalid(
            "voucher lines",
            format!(
                "{} has {} taxed lines but {} lines were expected",
                label(v),
                breakup.lines.len(),
                positions.len()
            ),
        ));
    }
    for (position, line_tax) in positions.iter().zip(&breakup.lines) {
        check_source(v, *position, line_tax)?;
    }
    Ok(Outcome::Value(Priced {
        rules,
        breakup,
        positions,
    }))
}

fn add_sales(scope: &Scope<'_>, v: &Voucher, totals: &mut Totals) -> Result<Outcome<()>, GstError> {
    let tos = supported!(time_of_supply(v));
    if !scope.period.contains(tos) {
        return Ok(Outcome::Value(()));
    }
    if v.reverse_charge {
        return unsupported(
            "outward supply under reverse charge",
            format!("{} is marked reverse charge; not worked out", label(v)),
        );
    }
    let priced = supported!(price(scope, v)?);
    let masters = &scope.books.masters;
    let mut taxable = Paise::ZERO;
    let mut heads = HeadSums::default();
    for (position, line_tax) in priced.positions.iter().zip(&priced.breakup.lines) {
        let kind = supported!(classify_line(v, *position, masters, &priced.rules)?);
        let expected = match kind {
            SupplyKind::Taxable => Taxability::Taxable,
            SupplyKind::Exempt => Taxability::Exempt,
            SupplyKind::NilRated => Taxability::NilRated,
            SupplyKind::NonGst => Taxability::NonGst,
            SupplyKind::ExportWithPayment
            | SupplyKind::ExportUnderLut
            | SupplyKind::SezWithPayment
            | SupplyKind::SezUnderLut
            | SupplyKind::DeemedExport
            | SupplyKind::Advance => {
                return unsupported(
                    format!("a {kind:?} line in GSTR-3B"),
                    format!(
                        "{} has a line outside table 3.1(a); zero-rated and other tables are not worked out",
                        label(v)
                    ),
                );
            }
        };
        if line_tax.taxability != expected {
            return Err(invalid(
                "voucher lines",
                format!(
                    "the classification and the tax of a line of {} disagree",
                    label(v)
                ),
            ));
        }
        if kind == SupplyKind::Taxable {
            taxable = taxable.checked_add(line_tax.taxable)?;
            heads.add_line(line_tax)?;
        }
    }
    if !taxable.is_zero() {
        totals.outward_taxable_value = totals.outward_taxable_value.checked_add(taxable)?;
        totals.outward_tax = add_amounts(totals.outward_tax, heads.rounded()?)?;
        totals.outward_vouchers.push(v.id);
    }
    Ok(Outcome::Value(()))
}

/// The position in `ledger_lines` that carries the purchase line `k` for the ITC check.
fn itc_line_index(v: &Voucher, k: usize, position: usize) -> Outcome<usize> {
    match v.mode {
        VoucherMode::AccountingInvoice => Outcome::Value(position),
        VoucherMode::ItemInvoice => {
            let Some(ledger) = v.inventory_lines.get(position).and_then(|l| l.ledger) else {
                return Outcome::unsupported(
                    "ITC on an inventory line without a purchase ledger",
                    format!(
                        "line {} of {} has no ledger, so its credit cannot be decided",
                        k + 1,
                        label(v)
                    ),
                );
            };
            let mut found = v
                .ledger_lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.ledger == ledger)
                .map(|(i, _)| i);
            match (found.next(), found.next()) {
                (Some(only), None) => Outcome::Value(only),
                (None, _) | (Some(_), Some(_)) => Outcome::unsupported(
                    "ITC on an inventory line whose ledger is not on exactly one ledger line",
                    format!(
                        "line {} of {} posts to a ledger that does not appear once among its ledger lines",
                        k + 1,
                        label(v)
                    ),
                ),
            }
        }
        VoucherMode::AsVoucher => Outcome::unsupported(
            "ITC on a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that purchase lines can be identified",
        ),
    }
}

fn add_purchase(
    scope: &Scope<'_>,
    v: &Voucher,
    totals: &mut Totals,
) -> Result<Outcome<()>, GstError> {
    if v.reverse_charge {
        totals.reverse_charge_purchases.push(v.id);
        return Ok(Outcome::Value(()));
    }
    let priced = supported!(price(scope, v)?);
    let masters = &scope.books.masters;
    let mut eligible = HeadSums::default();
    let mut not_claimable = HeadSums::default();
    for (k, (position, line_tax)) in priced
        .positions
        .iter()
        .zip(&priced.breakup.lines)
        .enumerate()
    {
        match line_tax.taxability {
            Taxability::Taxable => {}
            Taxability::Exempt | Taxability::NilRated | Taxability::NonGst => continue,
        }
        let index = supported!(itc_line_index(v, k, *position));
        let eligibility = supported!(itc_eligibility(v, index, &scope.books.company, masters)?);
        if eligibility.is_claimable() {
            eligible.add_line(line_tax)?;
        } else {
            not_claimable.add_line(line_tax)?;
        }
    }
    let eligible = eligible.rounded()?;
    if eligible != Gstr3bTaxAmounts::default() {
        totals.itc_all_other = add_amounts(totals.itc_all_other, eligible)?;
        totals.itc_vouchers.push(v.id);
    }
    totals.itc_not_claimable = add_amounts(totals.itc_not_claimable, not_claimable.rounded()?)?;
    Ok(Outcome::Value(()))
}

fn row(field: Gstr3bField, books: Paise, portal: Paise) -> Result<ComparisonRow, GstError> {
    Ok(ComparisonRow {
        field,
        books,
        portal,
        difference: books.checked_sub(portal)?,
    })
}

fn compare(
    working: &Gstr3bWorking,
    portal: &Gstr3bPortalValues,
) -> Result<Vec<ComparisonRow>, GstError> {
    let books_out = working.outward_tax;
    let portal_out = portal.outward_tax;
    let books_itc = working.itc_all_other;
    let portal_itc = portal.itc_all_other;
    Ok(vec![
        row(
            Gstr3bField::OutwardTaxableValue,
            working.outward_taxable_value,
            portal.outward_taxable_value,
        )?,
        row(
            Gstr3bField::OutwardIntegrated,
            books_out.integrated,
            portal_out.integrated,
        )?,
        row(
            Gstr3bField::OutwardCentral,
            books_out.central,
            portal_out.central,
        )?,
        row(
            Gstr3bField::OutwardStateUt,
            books_out.state_ut,
            portal_out.state_ut,
        )?,
        row(Gstr3bField::OutwardCess, books_out.cess, portal_out.cess)?,
        row(
            Gstr3bField::ItcIntegrated,
            books_itc.integrated,
            portal_itc.integrated,
        )?,
        row(
            Gstr3bField::ItcCentral,
            books_itc.central,
            portal_itc.central,
        )?,
        row(
            Gstr3bField::ItcStateUt,
            books_itc.state_ut,
            portal_itc.state_ut,
        )?,
        row(Gstr3bField::ItcCess, books_itc.cess, portal_itc.cess)?,
    ])
}
