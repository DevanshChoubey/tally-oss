//! Parsing dates typed in entry forms (`1-4`, `15/1`, `2026-05-07`). **Card C3** implements this module.
//!
//! Frozen interface (plan card C3):
//!
//! ```text
//! pub enum DateEntryError { .. }   // defined by C3 in this file
//! pub fn parse_date_entry(input: &str, fy: FinancialYear, today: Date) -> Result<Date, DateEntryError>;
//! ```
//!
//! Accepted forms (separator `-`, `/` or `.`, the same one throughout):
//!
//! - `d-m`: day and month with no year. The year is the financial year's: months April to
//!   December fall in the calendar year the financial year starts, January to March in the next
//!   one (Indian financial year, 1 April to 31 March).
//! - `d`: day only; the month is the month of `today`, the year follows the `d-m` rule.
//! - `d-m-yyyy`: a full date with a four-digit year.
//! - `yyyy-m-d`: a full date, year first (`2026-05-07`).
//!
//! Full dates are returned as typed, even when outside `fy`; the caller decides whether a date
//! outside the open year is acceptable. A two-digit year is rejected because its century is a
//! guess, and a wrong guess would be a wrong date.

use crate::dates::FinancialYear;
use thiserror::Error;
use time::{Date, Month};

/// Why a typed date was not accepted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DateEntryError {
    /// The input was empty or only spaces.
    #[error("no date was typed")]
    Empty,
    /// The text is not one of the accepted date forms.
    #[error(
        "`{input}` is not a date; type day-month (15-1), day-month-year (15-1-2027) or year-month-day (2027-01-15)"
    )]
    Unrecognised {
        /// The text as typed.
        input: String,
    },
    /// A two-digit year was typed; its century is ambiguous.
    #[error("`{input}` has a two-digit year; type the year with four digits")]
    TwoDigitYear {
        /// The text as typed.
        input: String,
    },
    /// The day, month and year do not make a real calendar date.
    #[error("{day}-{month}-{year} is not a real date")]
    InvalidDate {
        /// Day of month as typed.
        day: u32,
        /// Month number as typed.
        month: u32,
        /// The year it was resolved to.
        year: i32,
    },
    /// The date falls outside the range the calendar supports.
    #[error("the date is outside the supported calendar range")]
    OutOfRange,
}

/// Parses a date typed into an entry field, relative to the open financial year `fy` and the
/// date `today` (used only for a bare day number).
///
/// # Errors
/// [`DateEntryError`] for empty input, unrecognised text, a two-digit year, or a day/month pair
/// that is not a real date in the resolved year.
pub fn parse_date_entry(
    input: &str,
    fy: FinancialYear,
    today: Date,
) -> Result<Date, DateEntryError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(DateEntryError::Empty);
    }
    let unrecognised = || DateEntryError::Unrecognised {
        input: input.to_owned(),
    };
    let separator = ['-', '/', '.']
        .into_iter()
        .find(|s| text.contains(*s))
        .unwrap_or('-');
    let parts: Vec<&str> = text.split(separator).collect();
    if parts.iter().any(|p| !is_digits(p)) {
        return Err(unrecognised());
    }
    match parts.as_slice() {
        [day] if day.len() <= 2 => {
            let day = number(day)?;
            from_parts(
                day,
                u32::from(u8::from(today.month())),
                fy_year(fy, today.month())?,
            )
        }
        [day, month] if day.len() <= 2 && month.len() <= 2 => {
            let (day, month) = (number(day)?, number(month)?);
            let year = fy_year(fy, month_of(month, day)?)?;
            from_parts(day, month, year)
        }
        [year, month, day] if year.len() == 4 && month.len() <= 2 && day.len() <= 2 => {
            from_parts(number(day)?, number(month)?, year_number(year)?)
        }
        [day, month, year] if year.len() == 4 && day.len() <= 2 && month.len() <= 2 => {
            from_parts(number(day)?, number(month)?, year_number(year)?)
        }
        [day, month, year] if year.len() <= 2 && day.len() <= 2 && month.len() <= 2 => {
            Err(DateEntryError::TwoDigitYear {
                input: input.to_owned(),
            })
        }
        _ => Err(unrecognised()),
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Parses a digit-only field of at most four characters.
fn number(s: &str) -> Result<u32, DateEntryError> {
    s.parse::<u32>().map_err(|_| DateEntryError::Unrecognised {
        input: s.to_owned(),
    })
}

fn year_number(s: &str) -> Result<i32, DateEntryError> {
    s.parse::<i32>().map_err(|_| DateEntryError::Unrecognised {
        input: s.to_owned(),
    })
}

/// The month for a typed month number, or `InvalidDate` (year 0 is a placeholder: the real year
/// is not known until the month is).
fn month_of(month: u32, day: u32) -> Result<Month, DateEntryError> {
    u8::try_from(month)
        .ok()
        .and_then(|m| Month::try_from(m).ok())
        .ok_or(DateEntryError::InvalidDate {
            day,
            month,
            year: 0,
        })
}

/// The calendar year a month falls in within `fy`: April to December in the year the financial
/// year starts, January to March in the following one.
fn fy_year(fy: FinancialYear, month: Month) -> Result<i32, DateEntryError> {
    let start = fy.start.year();
    if month >= Month::April {
        Ok(start)
    } else {
        start.checked_add(1).ok_or(DateEntryError::OutOfRange)
    }
}

fn from_parts(day: u32, month: u32, year: i32) -> Result<Date, DateEntryError> {
    let invalid = DateEntryError::InvalidDate { day, month, year };
    let month_value = u8::try_from(month)
        .ok()
        .and_then(|m| Month::try_from(m).ok())
        .ok_or_else(|| invalid.clone())?;
    let day_value = u8::try_from(day).map_err(|_| invalid.clone())?;
    Date::from_calendar_date(year, month_value, day_value).map_err(|_| invalid)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]
mod tests {
    use super::*;
    use time::macros::date;

    fn fy(year: i32) -> FinancialYear {
        FinancialYear::april(year).unwrap()
    }

    fn parse(input: &str) -> Result<Date, DateEntryError> {
        parse_date_entry(input, fy(2026), date!(2026 - 06 - 20))
    }

    #[test]
    fn april_first_is_start_of_the_year() {
        // provenance: derived — FY 2026-27 runs 1 Apr 2026 to 31 Mar 2027; April is in 2026.
        assert_eq!(parse("1-4"), Ok(date!(2026 - 04 - 01)));
    }

    #[test]
    fn months_before_april_fall_in_the_second_calendar_year() {
        // provenance: derived — January is after 31 Dec 2026 and before 31 Mar 2027, so FY
        // 2026-27 contains 15 Jan 2027 (not 2026).
        assert_eq!(parse("15/1"), Ok(date!(2027 - 01 - 15)));
        // provenance: derived — 31 March is the last day of FY 2026-27, in 2027.
        assert_eq!(parse("31-3"), Ok(date!(2027 - 03 - 31)));
        // provenance: derived — December is April or later, so 2026.
        assert_eq!(parse("31.12"), Ok(date!(2026 - 12 - 31)));
    }

    #[test]
    fn impossible_day_month_errors() {
        // provenance: derived — February has at most 29 days; 31-2 is never a date.
        assert!(matches!(
            parse("31-2"),
            Err(DateEntryError::InvalidDate {
                day: 31,
                month: 2,
                year: 2027
            })
        ));
        // provenance: derived — April has 30 days.
        assert!(matches!(
            parse("31-4"),
            Err(DateEntryError::InvalidDate { .. })
        ));
        // provenance: derived — there is no month 13 and no day 0.
        assert!(matches!(
            parse("15-13"),
            Err(DateEntryError::InvalidDate { .. })
        ));
        assert!(matches!(
            parse("0-5"),
            Err(DateEntryError::InvalidDate { .. })
        ));
    }

    #[test]
    fn leap_day_depends_on_the_resolved_year() {
        // provenance: derived — Feb in FY 2026-27 is Feb 2027 (not a leap year): 29-2 is invalid.
        assert!(matches!(
            parse("29-2"),
            Err(DateEntryError::InvalidDate { .. })
        ));
        // provenance: derived — FY 2027-28 has Feb 2028, a leap year (divisible by 4, not a
        // century), so 29-2 is 2028-02-29.
        let r = parse_date_entry("29-2", fy(2027), date!(2027 - 06 - 01));
        assert_eq!(r, Ok(date!(2028 - 02 - 29)));
    }

    #[test]
    fn iso_full_date_is_returned_as_typed() {
        // provenance: derived — 2026-05-07 is 7 May 2026, inside FY 2026-27.
        assert_eq!(parse("2026-05-07"), Ok(date!(2026 - 05 - 07)));
        // provenance: derived — a typed year is taken literally, even outside the open FY.
        assert_eq!(parse("2026-03-31"), Ok(date!(2026 - 03 - 31)));
    }

    #[test]
    fn day_month_year_full_date() {
        // provenance: derived — d-m-yyyy: 7 May 2026.
        assert_eq!(parse("7-5-2026"), Ok(date!(2026 - 05 - 07)));
        assert_eq!(parse("07/05/2026"), Ok(date!(2026 - 05 - 07)));
        // provenance: derived — 31 April does not exist.
        assert!(matches!(
            parse("31-4-2026"),
            Err(DateEntryError::InvalidDate { .. })
        ));
    }

    #[test]
    fn two_digit_year_is_rejected() {
        // provenance: derived — the century of "26" is a guess; the module rejects it by design.
        assert!(matches!(
            parse("7-5-26"),
            Err(DateEntryError::TwoDigitYear { .. })
        ));
    }

    #[test]
    fn bare_day_uses_the_month_of_today() {
        // provenance: derived — today is 20 Jun 2026 (in FY 2026-27), so "5" is 5 Jun 2026.
        assert_eq!(parse("5"), Ok(date!(2026 - 06 - 05)));
        // provenance: derived — today in Feb 2027 resolves into the FY's second year.
        let r = parse_date_entry("5", fy(2026), date!(2027 - 02 - 10));
        assert_eq!(r, Ok(date!(2027 - 02 - 05)));
        // provenance: derived — Feb 2027 has 28 days, so day 30 is invalid.
        let r = parse_date_entry("30", fy(2026), date!(2027 - 02 - 10));
        assert!(matches!(r, Err(DateEntryError::InvalidDate { .. })));
    }

    #[test]
    fn surrounding_spaces_are_ignored() {
        // provenance: derived — whitespace is not part of a date.
        assert_eq!(parse("  1-4 "), Ok(date!(2026 - 04 - 01)));
    }

    #[test]
    fn garbage_is_rejected() {
        // provenance: derived — none of these is a date form.
        assert_eq!(parse(""), Err(DateEntryError::Empty));
        assert_eq!(parse("   "), Err(DateEntryError::Empty));
        for bad in [
            "abc", "1-", "-4", "1--4", "1-4-", "1-2-3-4", "+1-4", "1-4/2026", "123-4", "1 4",
        ] {
            assert!(
                matches!(parse(bad), Err(DateEntryError::Unrecognised { .. })),
                "{bad}"
            );
        }
    }
}
