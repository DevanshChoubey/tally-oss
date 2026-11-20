//! Column expansion for report parameters (card RP1, spec 6.2).
//!
//! [`expand_columns`] turns the [`ColumnSpec`] of a [`ReportParams`] into concrete column headers
//! so every report engine lays out the same columns:
//!
//! - `Single`: one column covering `params.period`.
//! - `Comparative(periods)`: one column per listed period, in the order given. An empty list is an
//!   error, not an empty report.
//! - `Monthly`: one column per calendar month the period touches. The first and last months are
//!   clipped to the period. The period must lie inside the financial year, so a monthly report
//!   never has more than 12 columns; a period reaching outside it is an error, not a silent clip.
//!
//! Labels: period columns read `01-Apr-2025 to 31-Mar-2026`; monthly columns read `Apr 2025`.

use ledgerly_core::{FinancialYear, Period, PeriodError};
use time::{Date, Month};

use crate::{ColumnHeader, ColumnSpec, ReportError, ReportParams};

/// Expands the column layout of `params` into column headers.
///
/// # Errors
/// - [`ReportError::Invalid`] for `Comparative` with no periods, or `Monthly` over a period that
///   is not inside `fy`.
/// - [`ReportError::Period`] at the edges of the supported calendar.
pub fn expand_columns(
    params: &ReportParams,
    fy: FinancialYear,
) -> Result<Vec<ColumnHeader>, ReportError> {
    match &params.columns {
        ColumnSpec::Single => Ok(vec![period_column(params.period)]),
        ColumnSpec::Comparative(periods) => {
            if periods.is_empty() {
                return Err(ReportError::Invalid {
                    field: "columns".to_owned(),
                    reason: "a comparative report needs at least one period".to_owned(),
                });
            }
            Ok(periods.iter().copied().map(period_column).collect())
        }
        ColumnSpec::Monthly => monthly_columns(params.period, fy),
    }
}

fn period_column(period: Period) -> ColumnHeader {
    ColumnHeader {
        label: format!(
            "{} to {}",
            date_label(period.from()),
            date_label(period.to())
        ),
        period: Some(period),
    }
}

fn monthly_columns(period: Period, fy: FinancialYear) -> Result<Vec<ColumnHeader>, ReportError> {
    let year = fy.period()?;
    if period.from() < year.from() || period.to() > year.to() {
        return Err(ReportError::Invalid {
            field: "columns".to_owned(),
            reason: format!(
                "monthly columns need a period inside financial year {}, got {} to {}",
                fy.label(),
                date_label(period.from()),
                date_label(period.to())
            ),
        });
    }
    let mut columns = Vec::new();
    // First day of the month being laid out.
    let mut month_start = period
        .from()
        .replace_day(1)
        .map_err(|_| PeriodError::OutOfRange)?;
    while month_start <= period.to() {
        let next_start = next_month_start(month_start)?;
        let month_end = next_start.previous_day().ok_or(PeriodError::OutOfRange)?;
        let from = month_start.max(period.from());
        let to = month_end.min(period.to());
        columns.push(ColumnHeader {
            label: format!("{} {}", month_name(month_start.month()), month_start.year()),
            period: Some(Period::new(from, to)?),
        });
        month_start = next_start;
    }
    Ok(columns)
}

fn next_month_start(month_start: Date) -> Result<Date, PeriodError> {
    let month = month_start.month().next();
    let year = if month == Month::January {
        month_start
            .year()
            .checked_add(1)
            .ok_or(PeriodError::OutOfRange)?
    } else {
        month_start.year()
    };
    Date::from_calendar_date(year, month, 1).map_err(|_| PeriodError::OutOfRange)
}

fn date_label(date: Date) -> String {
    format!(
        "{:02}-{}-{}",
        date.day(),
        month_name(date.month()),
        date.year()
    )
}

fn month_name(month: Month) -> &'static str {
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
