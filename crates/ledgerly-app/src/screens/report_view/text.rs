//! Text for figures, dates and periods, and parsing of a typed period.

use ledgerly_core::{Date, DrCr, Month, Period, PeriodError, SignedAmount, format::format_inr};
use thiserror::Error;

use super::ViewCell;

/// An amount as `12,34,567.00 Dr`: Indian grouping, with the side written out.
#[must_use]
pub fn amount_text(amount: SignedAmount) -> String {
    let side = match amount.side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    };
    format!("{} {side}", format_inr(amount.amount))
}

fn month_abbrev(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

/// A date as `01-Apr-2025`.
#[must_use]
pub fn date_text(date: Date) -> String {
    format!(
        "{:02}-{}-{}",
        date.day(),
        month_abbrev(date.month()),
        date.year()
    )
}

/// A date as `01-04-2025`, the form the period editor shows and [`parse_period_date`] reads.
pub(super) fn numeric_date_text(date: Date) -> String {
    format!(
        "{:02}-{:02}-{}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// A period as `01-Apr-2025 to 31-Mar-2026`.
#[must_use]
pub fn period_text(period: Period) -> String {
    format!("{} to {}", date_text(period.from()), date_text(period.to()))
}

/// The text of `cell` where `usual` is the side every figure is expected on: an amount on that
/// side, or zero, drops its Dr or Cr; an amount on the other side keeps it, so it is never read
/// as the usual side. The figure is the engine's; only the side word is left out. Every other
/// cell reads as [`cell_text`] writes it.
#[must_use]
pub(super) fn amount_on_side(cell: &ViewCell, usual: DrCr) -> String {
    let bare = |amount: SignedAmount| {
        if amount.amount.is_zero() || amount.side == usual {
            format_inr(amount.amount)
        } else {
            amount_text(amount)
        }
    };
    match cell {
        ViewCell::Amount(a) => bare(*a),
        ViewCell::Provisional { amount, reason } => {
            format!("{} (provisional: {reason})", bare(*amount))
        }
        ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => cell_text(cell),
    }
}

/// What a cell shows. Amounts carry Dr or Cr, a provisional value says so next to the figure, and
/// an unsupported cell says what is not supported and why, instead of a number.
#[must_use]
pub fn cell_text(cell: &ViewCell) -> String {
    match cell {
        ViewCell::Amount(a) => amount_text(*a),
        ViewCell::Signed(p) => format_inr(*p),
        ViewCell::Qty(q) => q.0.to_string(),
        ViewCell::Text(s) => s.clone(),
        ViewCell::Date(d) => date_text(*d),
        ViewCell::Empty => String::new(),
        ViewCell::Provisional { amount, reason } => {
            format!("{} (provisional: {reason})", amount_text(*amount))
        }
        ViewCell::Unsupported(u) => format!("Unsupported: {} ({})", u.what, u.reason),
    }
}

// ---- period entry ----------------------------------------------------------------------------

/// Why typed period text was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PeriodTextError {
    /// The text is not a date in a supported form.
    #[error("\"{text}\" is not a date; type DD-MM-YYYY, DD/MM/YYYY or YYYY-MM-DD")]
    NotADate {
        /// What was typed.
        text: String,
    },
    /// The two dates do not make a period.
    #[error(transparent)]
    Period(#[from] PeriodError),
}

/// Parses `DD-MM-YYYY`, `DD/MM/YYYY` or `YYYY-MM-DD` (surrounding spaces ignored). The year always
/// has four digits: a two-digit year is refused, not guessed, because a wrong century would give
/// a report for the wrong period.
///
/// # Errors
/// [`PeriodTextError::NotADate`] for any other text, or a day or month that does not exist.
pub fn parse_period_date(text: &str) -> Result<Date, PeriodTextError> {
    let bad = || PeriodTextError::NotADate {
        text: text.trim().to_owned(),
    };
    let parts: Vec<&str> = text.trim().split(['-', '/']).collect();
    let [a, b, c] = parts.as_slice() else {
        return Err(bad());
    };
    if !parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(bad());
    }
    let (year, month, day) = if a.len() == 4 && b.len() <= 2 && c.len() <= 2 {
        (a, b, c)
    } else if c.len() == 4 && a.len() <= 2 && b.len() <= 2 {
        (c, b, a)
    } else {
        return Err(bad());
    };
    let year: i32 = year.parse().map_err(|_| bad())?;
    let month: u8 = month.parse().map_err(|_| bad())?;
    let day: u8 = day.parse().map_err(|_| bad())?;
    let month = Month::try_from(month).map_err(|_| bad())?;
    Date::from_calendar_date(year, month, day).map_err(|_| bad())
}

/// Parses the two typed dates into a period.
///
/// # Errors
/// [`PeriodTextError::NotADate`] for an unreadable date, [`PeriodTextError::Period`] when the
/// start is after the end.
pub fn parse_period_text(from: &str, to: &str) -> Result<Period, PeriodTextError> {
    let from = parse_period_date(from)?;
    let to = parse_period_date(to)?;
    Ok(Period::new(from, to)?)
}
