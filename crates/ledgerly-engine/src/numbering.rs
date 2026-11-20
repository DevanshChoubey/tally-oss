//! Voucher number allocation. **Card E4** implements this module.
//!
//! Frozen interface (plan card E4):
//!
//! ```text
//! pub fn next_number(vt: &VoucherType, series: Option<&str>, date: Date, books: &Books) -> Result<String, EngineError>;
//! ```
//!
//! A number is `prefix` + counter zero-padded to at least `width` digits + `suffix`. The counter
//! is one more than the highest counter already used by any voucher (cancelled ones included,
//! because a cancelled voucher keeps its number) of the same voucher type, series and numbering
//! period (spec section 5, invariant 7). A voucher number that does not fit the type's current
//! `prefix`/`suffix` format (typed by hand, or from before the format changed) is not counted,
//! and the allocator refuses to return a number already present in the same scope.
//!
//! Sales invoices of a GST company are checked against CGST Rules 2017, rule 46(b): at most 16
//! characters, made of letters, digits, hyphen and slash.

use ledgerly_core::{Date, FinancialYear, Month};
use ledgerly_model::{
    BaseVoucherKind, Books, NumberingMethod, RestartPeriod, Voucher, VoucherType,
};

use crate::EngineError;

/// Maximum length of a GST invoice serial number (CGST Rules 2017, rule 46(b)).
const GST_NUMBER_MAX: usize = 16;

/// The numbering period a date falls in, as a comparable key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeriodKey {
    /// One counter for all dates.
    Always,
    /// The financial year starting 1 April of this year.
    Year(i32),
    /// A calendar month.
    Month(i32, Month),
}

fn invalid(field: &str, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.to_string(),
        reason: reason.into(),
    }
}

fn period_key(restart: RestartPeriod, date: Date, books: &Books) -> Result<PeriodKey, EngineError> {
    match restart {
        RestartPeriod::Never => Ok(PeriodKey::Always),
        RestartPeriod::Monthly => Ok(PeriodKey::Month(date.year(), date.month())),
        RestartPeriod::Yearly => {
            let fy_start = books.company.fy_start;
            if fy_start.month() != Month::April || fy_start.day() != 1 {
                return Err(invalid(
                    "numbering",
                    "yearly restart is only supported for financial years starting on 1 April",
                ));
            }
            let fy = FinancialYear::containing(date)
                .map_err(|_| invalid("date", "date is outside the supported calendar range"))?;
            Ok(PeriodKey::Year(fy.start.year()))
        }
    }
}

/// The counter inside `number` when it has the shape `prefix` digits `suffix`.
fn counter_of(number: &str, prefix: &str, suffix: &str) -> Result<Option<u64>, EngineError> {
    let Some(rest) = number.strip_prefix(prefix) else {
        return Ok(None);
    };
    let Some(digits) = rest.strip_suffix(suffix) else {
        return Ok(None);
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    digits.parse::<u64>().map(Some).map_err(|_| {
        invalid(
            "number",
            format!("existing number '{number}' has too many digits"),
        )
    })
}

/// Applies the GST serial number rule (rule 46(b)) to a finished number.
fn check_gst_number(number: &str) -> Result<(), EngineError> {
    if !number
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/')
    {
        return Err(invalid(
            "number",
            format!(
                "'{number}' has characters a GST invoice number cannot contain; \
                 use only letters, digits, hyphen and slash (CGST Rules, rule 46(b))"
            ),
        ));
    }
    if number.chars().count() > GST_NUMBER_MAX {
        return Err(EngineError::NumberTooLong {
            number: number.to_string(),
            max: GST_NUMBER_MAX,
        });
    }
    Ok(())
}

/// Whether `v` is in the (type, series, period) scope being allocated.
fn in_scope(
    v: &Voucher,
    vt: &VoucherType,
    series: Option<&str>,
    key: PeriodKey,
    books: &Books,
) -> Result<bool, EngineError> {
    if v.type_id != vt.id || v.series.as_deref() != series {
        return Ok(false);
    }
    Ok(period_key(vt.numbering.restart, v.date, books)? == key)
}

/// Allocates the next number for a voucher of type `vt` dated `date`.
///
/// `series` must be `Some` of one of the type's configured series when the type has series, and
/// `None` when it has none.
///
/// # Errors
/// - [`EngineError::Invalid`] when the type's numbering is not automatic, the series is missing,
///   the financial year is not April-based and yearly restart is requested, an existing number
///   has an unrepresentable counter, the counter overflows, or a GST number contains a character rule 46(b) forbids.
/// - [`EngineError::NotFound`] when `series` is not configured on the type.
/// - [`EngineError::NumberTooLong`] when a GST sales invoice number exceeds 16 characters.
/// - [`EngineError::Duplicate`] when the allocated number is already in use in the same scope.
pub fn next_number(
    vt: &VoucherType,
    series: Option<&str>,
    date: Date,
    books: &Books,
) -> Result<String, EngineError> {
    let cfg = &vt.numbering;
    match cfg.method {
        NumberingMethod::Automatic => {}
        NumberingMethod::Manual => {
            return Err(invalid(
                "numbering",
                format!("'{}' uses manual numbering; type the number in", vt.name),
            ));
        }
        NumberingMethod::None => {
            return Err(invalid(
                "numbering",
                format!("'{}' does not use voucher numbers", vt.name),
            ));
        }
    }

    match series {
        Some(s) if !cfg.series.iter().any(|c| c == s) => {
            return Err(EngineError::NotFound {
                what: format!("series '{s}' of voucher type '{}'", vt.name),
            });
        }
        None if !cfg.series.is_empty() => {
            return Err(invalid(
                "series",
                format!("'{}' has several series; choose one", vt.name),
            ));
        }
        Some(_) | None => {}
    }

    let key = period_key(cfg.restart, date, books)?;
    let mut highest: u64 = 0;
    for v in books.vouchers.values() {
        if !in_scope(v, vt, series, key, books)? {
            continue;
        }
        let counter = match &v.number {
            Some(n) => counter_of(n, &cfg.prefix, &cfg.suffix)?,
            None => None,
        };
        if let Some(c) = counter {
            highest = highest.max(c);
        }
    }
    let counter = highest
        .checked_add(1)
        .ok_or_else(|| invalid("number", "the counter cannot go higher"))?;

    let number = format!(
        "{}{:0>width$}{}",
        cfg.prefix,
        counter,
        cfg.suffix,
        width = usize::from(cfg.width)
    );

    let is_gst_invoice = books.company.features.gst && vt.base == BaseVoucherKind::Sales;
    if is_gst_invoice {
        check_gst_number(&number)?;
    }

    for v in books.vouchers.values() {
        if in_scope(v, vt, series, key, books)? && v.number.as_deref() == Some(number.as_str()) {
            return Err(EngineError::Duplicate {
                what: format!("voucher number '{number}'"),
            });
        }
    }
    Ok(number)
}
