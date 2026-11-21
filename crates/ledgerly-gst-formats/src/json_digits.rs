//! Refuses JSON numbers that `serde_json` would round (rule A1, rule A3).
//!
//! The readers parse a file into `serde_json::Value`. This crate does not enable the
//! `arbitrary_precision` feature of `serde_json`, so every number is held as an `f64`, and an
//! `f64` keeps at most 15 significant decimal digits reliably. A longer number would read back
//! as a different number with no error, for example `1234567890123456.78` as
//! `1234567890123456.8`.
//!
//! [`reject_long_numbers`] scans the file text, which the caller has already checked is valid
//! JSON, and refuses any number token with more than [`MAX_SIGNIFICANT_DIGITS`] significant
//! digits. Significant digits are the digits of the mantissa from the first non-zero digit to the
//! last non-zero digit, counting the integer and fraction parts together; leading zeros,
//! trailing zeros and the exponent do not count. Text inside JSON strings is skipped, so a long
//! identifier written as a string is not a number and is never refused.

use crate::FormatError;

/// Most significant digits a JSON number may carry. Any decimal of up to 15 significant digits
/// converts to an `f64` and back to the same digits, so such a number is read exactly.
pub(crate) const MAX_SIGNIFICANT_DIGITS: usize = 15;

/// Refuses every number in `json` with more than [`MAX_SIGNIFICANT_DIGITS`] significant digits.
///
/// `json` must already be valid JSON; the scan relies on that.
///
/// # Errors
/// [`FormatError::Json`] at path `$`, naming the line and column of the number and its digit
/// count.
pub(crate) fn reject_long_numbers(json: &str) -> Result<(), FormatError> {
    let bytes = json.as_bytes();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    while let Some(&b) = bytes.get(i) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i = i.saturating_add(1);
        } else if b == b'"' {
            in_string = true;
            i = i.saturating_add(1);
        } else if b == b'-' || b.is_ascii_digit() {
            let start = i;
            let end = number_end(bytes, start);
            let Some(token) = bytes.get(start..end) else {
                break;
            };
            let digits = significant_digits(token);
            if digits > MAX_SIGNIFICANT_DIGITS {
                return Err(too_long(bytes, start, digits));
            }
            i = end;
        } else {
            i = i.saturating_add(1);
        }
    }
    Ok(())
}

/// Index just past the number token that starts at `start`. In valid JSON a number is the run of
/// digits, signs, the decimal point and the exponent marker.
fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while let Some(&b) = bytes.get(end) {
        if b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E') {
            end = end.saturating_add(1);
        } else {
            break;
        }
    }
    end
}

/// Significant digits of the mantissa of a number token.
fn significant_digits(token: &[u8]) -> usize {
    let mantissa = token
        .split(|&b| b == b'e' || b == b'E')
        .next()
        .unwrap_or(&[]);
    let digits: Vec<u8> = mantissa
        .iter()
        .copied()
        .filter(u8::is_ascii_digit)
        .collect();
    match (
        digits.iter().position(|&d| d != b'0'),
        digits.iter().rposition(|&d| d != b'0'),
    ) {
        (Some(first), Some(last)) => last.saturating_sub(first).saturating_add(1),
        _ => 0,
    }
}

#[expect(
    clippy::naive_bytecount,
    reason = "error path only, run once per refused number; not worth a new dependency"
)]
fn too_long(bytes: &[u8], start: usize, digits: usize) -> FormatError {
    let before = bytes.get(..start).unwrap_or(&[]);
    let line = before
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        .saturating_add(1);
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p.saturating_add(1));
    let column = start.saturating_sub(line_start).saturating_add(1);
    FormatError::Json {
        path: "$".to_owned(),
        reason: format!(
            "number at line {line}, column {column} has {digits} significant digits; \
             at most {MAX_SIGNIFICANT_DIGITS} are kept exactly, so it is refused rather than rounded"
        ),
    }
}
