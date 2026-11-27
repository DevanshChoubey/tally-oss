//! Conditions on input tax credit that GSTR-3B carries (round s23): the time limit of CGST Act
//! s.16(4) and (5), the 180-day rule of s.16(2) with Rule 37, and the attribution of common
//! credit under Rules 42 and 43.
//!
//! # The law
//!
//! Read on CBIC's tax information portal (taxinformation.cbic.gov.in) on 2026-11-27:
//!
//! * s.16(4), as substituted w.e.f. 1.10.2022 (Notification 18/2022-CT; s.100 Finance Act 2022):
//!   no credit "after the thirtieth day of November following the end of financial year to which
//!   such invoice or debit note pertains or furnishing of the relevant annual return, whichever
//!   is earlier". The financial year of FY 2021-22 documents ended under the old text on the
//!   September 2022 return's due date, after 1.10.2022, so the substituted text governs them too.
//! * s.16(5) (inserted by s.118 of the Finance (No. 2) Act 2024): "notwithstanding" (4), for
//!   documents of 2017-18 to 2020-21, credit in any return filed up to 30 November 2021.
//! * s.16(6) (same insertion), the extended limit after a revoked cancellation of registration,
//!   is not decided here: a registration's history is not an input.
//! * s.16(2) second proviso (as substituted w.e.f. 1.10.2023, Notification 28/2023-CT) and Rule
//!   37(1) (substituted w.e.f. 01.10.2022 by Notification 19/2022-CT; "whether wholly or partly",
//!   "or reverse" and "proportionate to the amount not paid" inserted w.e.f. the same day by
//!   Notification 26/2022-CT): when the value with tax is not paid "within a period of one hundred
//!   and eighty days from the date of issue of invoice", other than under reverse charge, the
//!   credit "proportionate to the amount not paid" is reversed "in FORM GSTR-3B for the tax period
//!   immediately following the period of one hundred and eighty days". Schedule I supplies
//!   without consideration and s.15(2)(b) amounts are deemed paid (the provisos).
//! * Rule 42(1): C1 = T - (T1+T2+T3), C2 = C1 - T4, D1 = (E/F) x C2, D2 = five per cent of C2
//!   when common inputs are used partly for non-business purposes, C3 = C2 - (D1+D2); D1 and D2
//!   are reversed; every amount is worked out for each tax head (clause (l)).
//! * Rule 43(1)(e): Tm = Tc / 60. Rule 43(1)(g): Te = (E/F) x Tr, but clause (f), which defined
//!   Tr, was omitted w.e.f. 01.04.2020 (Notification 16/2020-CT).
//!
//! # What is not decided
//!
//! * **How the 180 days are counted.** "From the date of issue" reads as excluding the invoice
//!   day (the 180th day is the invoice date plus 180 days) or including it (plus 179). The
//!   General Clauses Act was not read. A payment on the one day the counts disagree on, or a
//!   period whose two possible ends fall in different tax periods, is `Unsupported`.
//! * **Rounding.** Rules 37 and 42 give proportions but no rounding; a share that is not a whole
//!   number of paise is `Unsupported` rather than rounded by a guess.
//! * **Rule 42(1)(i) proviso** (no turnover: E/F of the last period with turnover), the annual
//!   recomputation of Rule 42(2), the real-estate formulas of Rules 42(3)-(6) and 43(2)-(5), and
//!   Rule 43's Te (Tr is undefined in the text read) are `Unsupported`.
//! * **Re-availing** under Rule 37(2) after a later payment is not worked out.

use crate::GstError;
use crate::gstr3b::Gstr3bTaxAmounts;
use ledgerly_core::{Date, FinancialYear, Outcome, Paise, Period};
use time::macros::date;
use time::{Duration, Month};

/// The day the CGST Act came into force.
const GST_FROM: Date = date!(2017 - 07 - 01);

/// The last day of the financial years s.16(5) covers (2017-18 to 2020-21).
const SECTION_16_5_LAST_DOCUMENT: Date = date!(2021 - 03 - 31);

/// The s.16(5) limit.
const SECTION_16_5_LIMIT: Date = date!(2021 - 11 - 30);

/// The day the substituted Rule 37(1) comes into force.
const RULE_37_FROM: Date = date!(2022 - 10 - 01);

/// The days of the s.16(2) second proviso.
const RULE_37_DAYS: i64 = 180;

/// The months over which Rule 43(1)(e) spreads common capital credit.
const RULE_43_MONTHS: i64 = 60;

/// D2 is five per cent of C2 (Rule 42(1)(j)).
const RULE_42_D2: (i64, i64) = (5, 100);

/// How often GSTR-3B is furnished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReturnCycle {
    /// Every month.
    Monthly,
    /// Every quarter.
    Quarterly,
}

/// One inward invoice for the 180-day rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpaidInvoice {
    /// Date of issue of the supplier's invoice.
    pub invoice_date: Date,
    /// Value of the supply with the tax payable on it.
    pub payable: Paise,
    /// Part deemed paid (Schedule I value, s.15(2)(b) amounts).
    pub deemed_paid: Paise,
    /// Payments to the supplier, with their dates.
    pub payments: Vec<(Date, Paise)>,
    /// Credit availed on the invoice.
    pub itc: Gstr3bTaxAmounts,
    /// Tax on the supply is payable on reverse charge.
    pub reverse_charge: bool,
}

/// A reversal under the 180-day rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpaidReversal {
    /// The tax period whose GSTR-3B carries the reversal.
    pub tax_period: Period,
    /// The amount not paid at the end of the 180 days.
    pub unpaid: Paise,
    /// The credit reversed.
    pub reversal: Gstr3bTaxAmounts,
}

/// Inputs to Rule 42(1) for one tax period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonCreditInputs {
    /// T.
    pub total: Gstr3bTaxAmounts,
    /// T1.
    pub non_business: Gstr3bTaxAmounts,
    /// T2.
    pub exempt: Gstr3bTaxAmounts,
    /// T3.
    pub blocked: Gstr3bTaxAmounts,
    /// T4.
    pub taxable_only: Gstr3bTaxAmounts,
    /// E.
    pub exempt_turnover: Paise,
    /// F.
    pub total_turnover: Paise,
    /// Common credit partly used for non-business purposes.
    pub common_partly_non_business: bool,
}

/// Rule 42(1) results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonCredit {
    /// C1.
    pub c1: Gstr3bTaxAmounts,
    /// C2.
    pub c2: Gstr3bTaxAmounts,
    /// D1.
    pub d1: Gstr3bTaxAmounts,
    /// D2.
    pub d2: Gstr3bTaxAmounts,
    /// C3.
    pub c3: Gstr3bTaxAmounts,
    /// D1 + D2.
    pub reversal: Gstr3bTaxAmounts,
}

/// The last day on which credit on a document dated `document_date` may be taken (s.16(4),
/// s.16(5)). `annual_return` is the day the annual return of the document's financial year was
/// furnished, if it has been.
pub fn itc_time_limit(document_date: Date, annual_return: Option<Date>) -> Outcome<Date> {
    let what = || format!("the input tax credit time limit for a document of {document_date}");
    if document_date < GST_FROM {
        return Outcome::unsupported(
            what(),
            "the document is dated before GST began (1 July 2017)",
        );
    }
    if document_date <= SECTION_16_5_LAST_DOCUMENT {
        return Outcome::Value(SECTION_16_5_LIMIT);
    }
    let thirtieth_november = FinancialYear::containing(document_date)
        .ok()
        .and_then(|fy| fy.start.year().checked_add(1))
        .and_then(|y| Date::from_calendar_date(y, Month::November, 30).ok());
    let Some(limit) = thirtieth_november else {
        return Outcome::unsupported(what(), "the date is outside the supported calendar");
    };
    Outcome::Value(match annual_return {
        Some(furnished) if furnished < limit => furnished,
        Some(_) | None => limit,
    })
}

/// Whether credit taken on `taken_on` for a document dated `document_date` is within the
/// s.16(4)/(5) time limit.
pub fn itc_in_time(
    document_date: Date,
    taken_on: Date,
    annual_return: Option<Date>,
) -> Outcome<bool> {
    match itc_time_limit(document_date, annual_return) {
        Outcome::Value(limit) => Outcome::Value(taken_on <= limit),
        Outcome::Unsupported(u) => Outcome::Unsupported(u),
    }
}

/// The reversal the 180-day rule requires for one invoice, or `None` when nothing is reversed.
///
/// # Errors
/// [`GstError::Invalid`] for a negative amount, [`GstError::Money`] on overflow.
pub fn rule37_reversal(
    inv: &UnpaidInvoice,
    cycle: ReturnCycle,
) -> Result<Outcome<Option<UnpaidReversal>>, GstError> {
    for (field, amount) in [("payable", inv.payable), ("deemed paid", inv.deemed_paid)]
        .into_iter()
        .chain(inv.payments.iter().map(|(_, p)| ("payment", *p)))
    {
        if amount.is_negative() {
            return Err(invalid(
                field,
                "an amount paid or payable cannot be negative",
            ));
        }
    }
    if inv.reverse_charge {
        return Ok(Outcome::Value(None));
    }
    let what = format!(
        "the 180-day reversal of the invoice of {}",
        inv.invoice_date
    );
    if inv.invoice_date < RULE_37_FROM {
        return Ok(Outcome::unsupported(
            what,
            "Rule 37(1) was substituted from 1 October 2022; the earlier text was not read",
        ));
    }
    let (Some(late_end), Some(early_end)) = (
        inv.invoice_date.checked_add(Duration::days(RULE_37_DAYS)),
        inv.invoice_date
            .checked_add(Duration::days(RULE_37_DAYS - 1)),
    ) else {
        return Ok(Outcome::unsupported(
            what,
            "the date is outside the supported calendar",
        ));
    };
    if inv.payments.iter().any(|(d, _)| *d == late_end) {
        return Ok(Outcome::unsupported(
            what,
            format!(
                "a payment on {late_end} is on the 180th day or the 181st depending on whether \
                 the invoice day is counted, which the texts read do not settle"
            ),
        ));
    }
    let (Some(period), Some(other)) = (
        next_tax_period(late_end, cycle),
        next_tax_period(early_end, cycle),
    ) else {
        return Ok(Outcome::unsupported(
            what,
            "the date is outside the supported calendar",
        ));
    };
    if period != other {
        return Ok(Outcome::unsupported(
            what,
            format!(
                "the 180 days end on {early_end} or {late_end} depending on whether the invoice \
                 day is counted, and the tax period immediately following differs"
            ),
        ));
    }
    let mut paid = inv.deemed_paid;
    for (d, p) in &inv.payments {
        if *d <= early_end {
            paid = paid.checked_add(*p)?;
        }
    }
    let unpaid = inv.payable.checked_sub(paid)?;
    if unpaid.get() <= 0 {
        return Ok(Outcome::Value(None));
    }
    let reversal = match share(inv.itc, unpaid.get(), inv.payable.get(), &what)? {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    Ok(Outcome::Value(Some(UnpaidReversal {
        tax_period: period,
        unpaid,
        reversal,
    })))
}

/// The Rule 42(1) attribution of credit on inputs and input services for one tax period.
///
/// # Errors
/// [`GstError::Invalid`] when the parts exceed the whole or the turnover is inconsistent,
/// [`GstError::Money`] on overflow.
pub fn rule42_common_credit(i: &CommonCreditInputs) -> Result<Outcome<CommonCredit>, GstError> {
    if i.exempt_turnover.is_negative() || i.total_turnover.is_negative() {
        return Err(invalid("turnover", "turnover cannot be negative"));
    }
    if i.exempt_turnover > i.total_turnover {
        return Err(invalid(
            "exempt turnover",
            "E is part of F and cannot exceed it",
        ));
    }
    let what = "the Rule 42 attribution of common credit".to_owned();
    let excluded = add(add(i.non_business, i.exempt)?, i.blocked)?;
    let c1 = sub(
        i.total,
        excluded,
        "T1 + T2 + T3 are parts of T and cannot exceed it",
    )?;
    let c2 = sub(c1, i.taxable_only, "T4 is part of C1 and cannot exceed it")?;
    if i.total_turnover.is_zero() {
        return Ok(Outcome::unsupported(
            what,
            "there is no turnover in the period; Rule 42(1)(i) then takes E/F from the last \
             period with turnover, which is not worked out here",
        ));
    }
    let d1 = match share(c2, i.exempt_turnover.get(), i.total_turnover.get(), &what)? {
        Outcome::Value(d) => d,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let d2 = if i.common_partly_non_business {
        match share(c2, RULE_42_D2.0, RULE_42_D2.1, &what)? {
            Outcome::Value(d) => d,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        }
    } else {
        Gstr3bTaxAmounts::default()
    };
    let reversal = add(d1, d2)?;
    let c3 = sub(c2, reversal, "D1 + D2 cannot exceed C2")?;
    Ok(Outcome::Value(CommonCredit {
        c1,
        c2,
        d1,
        d2,
        c3,
        reversal,
    }))
}

/// Tm, the credit on common capital goods attributable to one tax period (Rule 43(1)(e)).
///
/// # Errors
/// [`GstError::Money`] on overflow.
pub fn rule43_monthly_common_credit(
    tc: Gstr3bTaxAmounts,
) -> Result<Outcome<Gstr3bTaxAmounts>, GstError> {
    share(tc, 1, RULE_43_MONTHS, "the Rule 43 monthly common credit")
}

/// Te, the common capital credit attributable to exempt supplies (Rule 43(1)(g)): always
/// `Unsupported`, because the text read computes it from 'Tr', whose defining clause (f) was
/// omitted w.e.f. 01.04.2020.
pub fn rule43_exempt_share(
    _tm: Gstr3bTaxAmounts,
    _exempt_turnover: Paise,
    _total_turnover: Paise,
) -> Outcome<Gstr3bTaxAmounts> {
    Outcome::unsupported(
        "the Rule 43 share of common capital credit for exempt supplies (Te)",
        "Rule 43(1)(g) computes Te from 'Tr', defined by clause (f), which was omitted from \
         1 April 2020 (Notification 16/2020-CT); the text read defines Tr nowhere else",
    )
}

/// The tax period that immediately follows the one containing `day`.
fn next_tax_period(day: Date, cycle: ReturnCycle) -> Option<Period> {
    let months: u8 = match cycle {
        ReturnCycle::Monthly => 1,
        ReturnCycle::Quarterly => 3,
    };
    let day_month_index = u8::from(day.month()).checked_sub(1)?;
    let start0 = day_month_index
        .checked_sub(day_month_index % months)?
        .checked_add(months)?;
    let (year, start0) = if start0 >= 12 {
        (day.year().checked_add(1)?, start0.checked_sub(12)?)
    } else {
        (day.year(), start0)
    };
    let from =
        Date::from_calendar_date(year, Month::try_from(start0.checked_add(1)?).ok()?, 1).ok()?;
    let end0 = start0.checked_add(months)?;
    let next_start = if end0 >= 12 {
        Date::from_calendar_date(
            year.checked_add(1)?,
            Month::try_from(end0.checked_sub(11)?).ok()?,
            1,
        )
    } else {
        Date::from_calendar_date(year, Month::try_from(end0.checked_add(1)?).ok()?, 1)
    }
    .ok()?;
    Period::new(from, next_start.previous_day()?).ok()
}

/// Every head of `amounts` times `num / den`, when each is a whole number of paise.
fn share(
    amounts: Gstr3bTaxAmounts,
    num: i64,
    den: i64,
    what: &str,
) -> Result<Outcome<Gstr3bTaxAmounts>, GstError> {
    if den <= 0 {
        return Err(invalid(
            "proportion",
            "the whole of a proportion must be above zero",
        ));
    }
    let one = |p: Paise| -> Option<Option<Paise>> {
        let product = i128::from(p.get()).checked_mul(i128::from(num))?;
        if product % i128::from(den) != 0 {
            return Some(None);
        }
        i64::try_from(product / i128::from(den))
            .ok()
            .map(|v| Some(Paise::new(v)))
    };
    let heads = [
        amounts.integrated,
        amounts.central,
        amounts.state_ut,
        amounts.cess,
    ];
    let mut out = [Paise::ZERO; 4];
    for (slot, head) in out.iter_mut().zip(heads) {
        match one(head) {
            None => return Err(GstError::Money(ledgerly_core::MoneyError::Overflow)),
            Some(None) => {
                return Ok(Outcome::unsupported(
                    what,
                    format!(
                        "{num}/{den} of {} paise is not a whole number of paise, and the rule \
                         does not say how the share is rounded",
                        head.get()
                    ),
                ));
            }
            Some(Some(v)) => *slot = v,
        }
    }
    let [integrated, central, state_ut, cess] = out;
    Ok(Outcome::Value(Gstr3bTaxAmounts {
        integrated,
        central,
        state_ut,
        cess,
    }))
}

fn add(a: Gstr3bTaxAmounts, b: Gstr3bTaxAmounts) -> Result<Gstr3bTaxAmounts, GstError> {
    Ok(Gstr3bTaxAmounts {
        integrated: a.integrated.checked_add(b.integrated)?,
        central: a.central.checked_add(b.central)?,
        state_ut: a.state_ut.checked_add(b.state_ut)?,
        cess: a.cess.checked_add(b.cess)?,
    })
}

fn sub(
    a: Gstr3bTaxAmounts,
    b: Gstr3bTaxAmounts,
    reason: &str,
) -> Result<Gstr3bTaxAmounts, GstError> {
    let out = Gstr3bTaxAmounts {
        integrated: a.integrated.checked_sub(b.integrated)?,
        central: a.central.checked_sub(b.central)?,
        state_ut: a.state_ut.checked_sub(b.state_ut)?,
        cess: a.cess.checked_sub(b.cess)?,
    };
    if [out.integrated, out.central, out.state_ut, out.cess]
        .iter()
        .any(|p| p.is_negative())
    {
        return Err(invalid("Rule 42 amounts", reason));
    }
    Ok(out)
}

fn invalid(field: &str, reason: &str) -> GstError {
    GstError::Invalid {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}
