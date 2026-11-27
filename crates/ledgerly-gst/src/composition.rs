//! The composition levy's quarterly statement (FORM GST CMP-08) and annual return (FORM GSTR-4)
//! (round s23): the central tax of Rule 7 and the due dates of Rule 62. Engine-only.
//!
//! # The law
//!
//! Read on CBIC's tax information portal (taxinformation.cbic.gov.in) on 2026-11-28:
//!
//! * Rule 7 of the CGST Rules, its Table substituted w.e.f. 01.04.2020 by Notification
//!   50/2020-CT: central tax at half per cent of the turnover in the State or Union territory for
//!   manufacturers (sl.1), two and a half per cent of it for suppliers of Schedule II para 6(b)
//!   supplies (sl.2), half per cent of the turnover of taxable supplies for any other s.10(1)/(2)
//!   supplier (sl.3), and three per cent of the turnover of supplies for a person under s.10(2A)
//!   (sl.4).
//! * Rule 62(1), substituted by Notification 20/2019-CT dated 23.04.2019: CMP-08 "till the 18th
//!   day of the month succeeding such quarter"; GSTR-4 "till the thirtieth day of April following
//!   the end of such financial year"; by the proviso inserted by Notification 12/2024-CT, for
//!   FY 2024-25 onwards "till the thirtieth day of June following the end of such financial year".
//!
//! # Not decided
//!
//! * State and union territory tax: the State GST Rules and the UTGST Rules were not read, so
//!   their half of the levy is `Unsupported`.
//! * A quarter before 1 April 2020 (the earlier Rule 7 Table was not read), and a quarter or year
//!   that ended before the 2019 substitution of Rule 62.
//! * Rounding: a tax that is not a whole number of paise is `Unsupported`; Rule 7 gives none.
//! * Due dates extended by notification: only the dates in the rule text are given.
//! * The turnover itself: which category a company is in and its turnover in the State are inputs.
//!   The books do not yet record a composition category, and voucher tax refuses composition.

use crate::GstError;
use ledgerly_core::{Date, FinancialYear, Outcome, Paise};
use time::Month;
use time::macros::date;

/// The day Rule 7's substituted Table comes into force (Notification 50/2020-CT).
const RULE_7_FROM: Date = date!(2020 - 04 - 01);

/// Notification 20/2019-CT, which substituted Rule 62(1).
const RULE_62_TEXT: Date = date!(2019 - 04 - 23);

/// The first financial year the Rule 62 proviso (30 June) covers starts on this day.
const GSTR4_JUNE_FROM: Date = date!(2024 - 04 - 01);

/// The category of a composition taxpayer, as the rows of the Rule 7 Table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositionCategory {
    /// Sl.1: a manufacturer (other than of notified goods).
    Manufacturer,
    /// Sl.2: a supplier of Schedule II para 6(b) supplies (food or drink served as a service).
    ScheduleIIRestaurant,
    /// Sl.3: any other supplier under s.10(1) and (2).
    OtherSupplier,
    /// Sl.4: a person paying tax under s.10(2A).
    Section10_2A,
}

/// A quarter's outward turnover, as Rule 7 measures it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CompositionTurnover {
    /// Turnover in the State or Union territory (taxable and exempt supplies).
    pub in_state: Paise,
    /// Of that, the turnover of taxable supplies.
    pub taxable_in_state: Paise,
}

/// The central tax for a quarter starting on `quarter_start` (Rule 7).
///
/// # Errors
/// [`GstError::Invalid`] when an amount is negative or the taxable part exceeds the whole.
pub fn cmp08_central_tax(
    category: CompositionCategory,
    turnover: CompositionTurnover,
    quarter_start: Date,
) -> Result<Outcome<Paise>, GstError> {
    if turnover.in_state.is_negative() || turnover.taxable_in_state.is_negative() {
        return Err(invalid("turnover cannot be negative"));
    }
    if turnover.taxable_in_state > turnover.in_state {
        return Err(invalid(
            "the turnover of taxable supplies is part of the turnover in the State",
        ));
    }
    let what = format!("the composition levy for the quarter from {quarter_start}");
    if quarter_start < RULE_7_FROM {
        return Ok(Outcome::unsupported(
            what,
            "the Rule 7 Table was substituted from 1 April 2020; the earlier one was not read",
        ));
    }
    // Rate as (numerator, denominator) of a fraction, and the turnover it applies to.
    let ((num, den), base) = match category {
        CompositionCategory::Manufacturer => ((5, 1000), turnover.in_state),
        CompositionCategory::ScheduleIIRestaurant => ((25, 1000), turnover.in_state),
        CompositionCategory::OtherSupplier => ((5, 1000), turnover.taxable_in_state),
        CompositionCategory::Section10_2A => ((3, 100), turnover.in_state),
    };
    let product = i128::from(base.get())
        .checked_mul(i128::from(num))
        .ok_or(GstError::Money(ledgerly_core::MoneyError::Overflow))?;
    if product % i128::from(den) != 0 {
        return Ok(Outcome::unsupported(
            what,
            format!(
                "{num}/{den} of {} paise is not a whole number of paise, and Rule 7 gives no \
                 rounding",
                base.get()
            ),
        ));
    }
    let tax = i64::try_from(product / i128::from(den))
        .map_err(|_| GstError::Money(ledgerly_core::MoneyError::Overflow))?;
    Ok(Outcome::Value(Paise::new(tax)))
}

/// State or union territory tax under the composition levy: always `Unsupported`, because the
/// State GST Rules and the UTGST Rules were not read.
pub fn cmp08_state_tax(category: CompositionCategory) -> Outcome<Paise> {
    Outcome::unsupported(
        format!("state or union territory tax under the composition levy ({category:?})"),
        "Rule 7 of the CGST Rules sets the central tax only; the State GST and UTGST rules were \
         not read",
    )
}

/// The last day to furnish CMP-08 for the quarter ending on `quarter_end` (Rule 62(1)(i)).
pub fn cmp08_due(quarter_end: Date) -> Outcome<Date> {
    let what = format!("the CMP-08 due date for the quarter ending {quarter_end}");
    let ends_quarter = matches!(
        quarter_end.month(),
        Month::March | Month::June | Month::September | Month::December
    ) && quarter_end.next_day().is_some_and(|d| d.day() == 1);
    if !ends_quarter {
        return Outcome::unsupported(what, "the date is not the last day of a quarter");
    }
    if quarter_end < RULE_62_TEXT {
        return Outcome::unsupported(
            what,
            "Rule 62(1) was substituted on 23 April 2019; the earlier text was not read",
        );
    }
    match quarter_end
        .next_day()
        .and_then(|d| Date::from_calendar_date(d.year(), d.month(), 18).ok())
    {
        Some(due) => Outcome::Value(due),
        None => Outcome::unsupported(what, "the date is outside the supported calendar"),
    }
}

/// The last day to furnish GSTR-4 for the financial year ending on `year_end` (Rule 62(1)(ii)
/// and its proviso).
pub fn gstr4_due(year_end: Date) -> Outcome<Date> {
    let what = format!("the GSTR-4 due date for the financial year ending {year_end}");
    let fy = match FinancialYear::containing(year_end) {
        Ok(fy) if fy.end().is_ok_and(|end| end == year_end) => fy,
        Ok(_) | Err(_) => {
            return Outcome::unsupported(what, "the date is not the last day of a financial year");
        }
    };
    if year_end < RULE_62_TEXT {
        return Outcome::unsupported(
            what,
            "Rule 62(1) was substituted on 23 April 2019; the earlier text was not read",
        );
    }
    let (month, day) = if fy.start >= GSTR4_JUNE_FROM {
        (Month::June, 30)
    } else {
        (Month::April, 30)
    };
    match Date::from_calendar_date(year_end.year(), month, day) {
        Ok(due) => Outcome::Value(due),
        Err(_) => Outcome::unsupported(what, "the date is outside the supported calendar"),
    }
}

fn invalid(reason: &str) -> GstError {
    GstError::Invalid {
        field: "composition turnover".to_owned(),
        reason: reason.to_owned(),
    }
}
