use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, Month};

/// A financial year. Indian financial years run 1 April to 31 March.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FinancialYear {
    /// The first day of the year (normally 1 April).
    pub start: Date,
}

/// Errors building periods and years.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PeriodError {
    /// `from` is after `to`.
    #[error("period starts on {from} which is after its end {to}")]
    Reversed {
        /// Start date.
        from: Date,
        /// End date.
        to: Date,
    },
    /// The date is outside the supported calendar range.
    #[error("date out of range")]
    OutOfRange,
}

impl FinancialYear {
    /// The April-start financial year beginning 1 April `year`.
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] for years the `time` crate cannot represent.
    pub fn april(year: i32) -> Result<Self, PeriodError> {
        Date::from_calendar_date(year, Month::April, 1)
            .map(|start| FinancialYear { start })
            .map_err(|_| PeriodError::OutOfRange)
    }

    /// The April-start financial year containing `date`.
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] at the edges of the calendar.
    pub fn containing(date: Date) -> Result<Self, PeriodError> {
        let year = if date.month() >= Month::April {
            date.year()
        } else {
            date.year().checked_sub(1).ok_or(PeriodError::OutOfRange)?
        };
        FinancialYear::april(year)
    }

    /// The last day of the year (the day before the next year's start).
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] at the edges of the calendar.
    pub fn end(self) -> Result<Date, PeriodError> {
        let next_year = self
            .start
            .year()
            .checked_add(1)
            .ok_or(PeriodError::OutOfRange)?;
        let next = self
            .start
            .replace_year(next_year)
            .map_err(|_| PeriodError::OutOfRange)?;
        next.previous_day().ok_or(PeriodError::OutOfRange)
    }

    /// The whole year as a period.
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] at the edges of the calendar.
    pub fn period(self) -> Result<Period, PeriodError> {
        Period::new(self.start, self.end()?)
    }

    /// Label such as `2025-26`.
    pub fn label(self) -> String {
        let y = self.start.year();
        let next = y.rem_euclid(100).saturating_add(1).rem_euclid(100);
        format!("{y}-{next:02}")
    }
}

/// An inclusive date range with `from <= to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Period {
    from: Date,
    to: Date,
}

impl Period {
    /// Builds a period.
    ///
    /// # Errors
    /// [`PeriodError::Reversed`] when `from > to`.
    pub fn new(from: Date, to: Date) -> Result<Self, PeriodError> {
        if from > to {
            return Err(PeriodError::Reversed { from, to });
        }
        Ok(Period { from, to })
    }

    /// First day.
    pub const fn from(self) -> Date {
        self.from
    }

    /// Last day (inclusive).
    pub const fn to(self) -> Date {
        self.to
    }

    /// `true` when `date` is inside the period.
    pub fn contains(self, date: Date) -> bool {
        self.from <= date && date <= self.to
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn fy_containing_march_is_previous_april() {
        // provenance: derived — Indian financial year runs 1 April to 31 March.
        let fy = FinancialYear::containing(date!(2026 - 03 - 31)).unwrap();
        assert_eq!(fy.start, date!(2025 - 04 - 01));
        assert_eq!(fy.end().unwrap(), date!(2026 - 03 - 31));
        assert_eq!(fy.label(), "2025-26");
        assert_eq!(
            FinancialYear::containing(date!(2026 - 04 - 01))
                .unwrap()
                .label(),
            "2026-27"
        );
        assert_eq!(FinancialYear::april(1999).unwrap().label(), "1999-00");
    }

    #[test]
    fn reversed_period_errors() {
        assert!(matches!(
            Period::new(date!(2026 - 05 - 02), date!(2026 - 05 - 01)),
            Err(PeriodError::Reversed { .. })
        ));
        let p = Period::new(date!(2026 - 05 - 01), date!(2026 - 05 - 01)).unwrap();
        assert!(p.contains(date!(2026 - 05 - 01)));
    }
}
