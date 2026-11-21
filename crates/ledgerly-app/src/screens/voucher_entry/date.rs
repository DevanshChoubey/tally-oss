//! Typed dates for entry forms: reading them ([`parse_entry_date`]) and writing them back
//! ([`format_entry_date`]).

use ledgerly_core::{Date, FinancialYear, Month};
use thiserror::Error;

/// Why a typed date could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EntryDateError {
    /// Nothing was typed.
    #[error("type a date such as 15-4 or 15-04-2026")]
    Empty,
    /// Not day-month, day-month-year or year-month-day.
    #[error("'{0}' is not a date; use day-month, day-month-year or year-month-day")]
    Format(String),
    /// The numbers do not make a calendar date.
    #[error("'{0}' is not a date in the calendar")]
    NotADate(String),
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Reads a date typed in an entry form: `15-4`, `15/4`, `15-04-2026`, `2026-04-15`.
///
/// With no year, the year is the one of the financial year `fy` in which that month falls (months
/// from the month `fy` starts in belong to its first calendar year, the rest to the next).
/// Two-digit years are refused as ambiguous.
///
/// # Errors
/// An [`EntryDateError`].
pub fn parse_entry_date(text: &str, fy: FinancialYear) -> Result<Date, EntryDateError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EntryDateError::Empty);
    }
    let bad = || EntryDateError::Format(text.to_owned());
    let not_a_date = || EntryDateError::NotADate(text.to_owned());
    let parts: Vec<&str> = text.split(['-', '/']).collect();
    if !parts.iter().all(|p| all_digits(p)) {
        return Err(bad());
    }
    let (day, month, year) = match parts.as_slice() {
        [d, m] => (*d, *m, None),
        [a, b, c] if a.len() == 4 => (*c, *b, Some(*a)),
        [a, b, c] if c.len() == 4 => (*a, *b, Some(*c)),
        _ => return Err(bad()),
    };
    let day: u8 = day.parse().map_err(|_| bad())?;
    let month_number: u8 = month.parse().map_err(|_| bad())?;
    let month = Month::try_from(month_number).map_err(|_| not_a_date())?;
    let year: i32 = match year {
        Some(y) => y.parse().map_err(|_| bad())?,
        None if month >= fy.start.month() => fy.start.year(),
        None => fy.start.year().checked_add(1).ok_or_else(not_a_date)?,
    };
    Date::from_calendar_date(year, month, day).map_err(|_| not_a_date())
}

/// `dd-mm-yyyy`, the form the date field shows after Enter.
#[must_use]
pub fn format_entry_date(date: Date) -> String {
    format!(
        "{:02}-{:02}-{}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}
