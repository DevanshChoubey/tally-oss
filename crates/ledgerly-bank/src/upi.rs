//! UPI payment payload for printed invoices. **Card K5** implements this module.
//!
//! Frozen interface (plan card K5):
//!
//! ```text
//! pub fn upi_payload(payee_vpa: &str, payee_name: &str, amount: Paise, note: &str) -> Result<String, BankError>;
//! ```
//!
//! The payload is a `upi://pay` deep link with the query parameters `pa` (payee address), `pn`
//! (payee name), `am` (amount in rupees with two decimals), `cu` (currency, always `INR`) and,
//! when a note is given, `tn` (transaction note). Values are percent-encoded; only the RFC 3986
//! unreserved characters (letters, digits, `-`, `.`, `_`, `~`) are left as they are.
use ledgerly_core::Paise;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use crate::BankError;

/// Everything except the RFC 3986 unreserved characters is percent-encoded.
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

fn invalid(field: &str, reason: &str) -> BankError {
    BankError::Invalid {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Checks a virtual payment address: `handle@provider`, ASCII letters, digits, `.`, `-`, `_`
/// on both sides of exactly one `@`, both sides non-empty.
fn check_vpa(vpa: &str) -> Result<(), BankError> {
    let mut parts = vpa.split('@');
    let (Some(handle), Some(provider), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid("payee_vpa", "must be of the form handle@provider"));
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if ok(handle) && ok(provider) {
        Ok(())
    } else {
        Err(invalid(
            "payee_vpa",
            "handle and provider must be non-empty and use only letters, digits, '.', '-', '_'",
        ))
    }
}

/// Formats a positive paise amount as rupees with exactly two decimals, using integer
/// arithmetic only.
fn format_amount(amount: Paise) -> Result<String, BankError> {
    if amount.is_zero() || amount.is_negative() {
        return Err(invalid("amount", "must be greater than zero"));
    }
    let paise = amount.get();
    Ok(format!("{}.{:02}", paise / 100, paise % 100))
}

/// Builds the `upi://pay` payload for a printed invoice QR code.
///
/// `amount` must be positive; `payee_vpa` must look like `name@bank`; `payee_name` must not be
/// blank. An empty `note` leaves out the `tn` parameter.
///
/// # Errors
///
/// [`BankError::Invalid`] when the address, name or amount is not acceptable.
pub fn upi_payload(
    payee_vpa: &str,
    payee_name: &str,
    amount: Paise,
    note: &str,
) -> Result<String, BankError> {
    check_vpa(payee_vpa)?;
    if payee_name.trim().is_empty() {
        return Err(invalid("payee_name", "must not be blank"));
    }
    let am = format_amount(amount)?;
    let mut out = format!(
        "upi://pay?pa={}&pn={}&am={}&cu=INR",
        payee_vpa,
        utf8_percent_encode(payee_name, ENCODE_SET),
        am
    );
    if !note.is_empty() {
        out.push_str("&tn=");
        out.push_str(&utf8_percent_encode(note, ENCODE_SET).to_string());
    }
    Ok(out)
}
