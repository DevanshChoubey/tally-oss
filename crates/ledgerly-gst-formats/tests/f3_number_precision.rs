//! Card F3 tests: JSON numbers with more digits than an `f64` keeps are refused (rule A3).
//!
//! Provenance. Every fixture is fictional and written in the test. Expected values are derived by
//! hand: a number's significant digits are the digits from its first non-zero digit to its last
//! non-zero digit, counting integer and fraction digits together. An `f64` keeps 15 significant
//! decimal digits, so a number with 16 or more is refused, not rounded. The rule is ours (rule A3
//! and the module doc of `json_digits`), not a portal rule.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::Paise;
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::gstr2b::parse_gstr2b;
use ledgerly_gst_formats::ims::parse_ims;

/// A GSTR-2B file with one invoice whose `val` is `val`, plus an extra field `extra` on the
/// invoice (as written, for example `"zz": 1`) and an optional `rt` on one item.
fn gstr2b_with(val: &str, extra: &str, item_rt: &str) -> String {
    format!(
        r#"{{
  "data": {{
    "gstin": "29ABCDE1234F1ZW",
    "rtnprd": "042024",
    "docdata": {{
      "b2b": [
        {{
          "ctin": "24AAACB1111C1ZT",
          "inv": [
            {{
              "inum": "INV-001", "dt": "05-04-2024", "val": {val}, {extra}
              "items": [ {{ "num": 1, "rt": {item_rt}, "txval": 0, "igst": 0, "cgst": 0, "sgst": 0, "cess": 0 }} ]
            }}
          ]
        }}
      ]
    }}
  }}
}}"#
    )
}

fn first_error_reason(err: FormatError) -> String {
    match err {
        FormatError::Json { path, reason } => {
            assert_eq!(path, "$", "a digit-count refusal is reported at the root");
            reason
        }
        other @ (FormatError::Field { .. }
        | FormatError::Spreadsheet(_)
        | FormatError::Unsupported { .. }) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
}

#[test]
fn fifteen_significant_digits_are_read_exactly() {
    // provenance: derived - 1234567890123.45 has 15 significant digits (1234567890123 then 45);
    // rupees times 100 is 123456789012345 paise.
    let parsed = parse_gstr2b(&gstr2b_with("1234567890123.45", "", "18")).unwrap();
    let doc = &parsed.b2b[0].docs[0];
    assert_eq!(doc.value, Paise::new(123_456_789_012_345));
}

#[test]
fn sixteen_significant_digits_are_refused_not_rounded() {
    // provenance: derived - 1234567890123456 has 16 significant digits, one more than an f64
    // keeps. Old behaviour: read as 123456789012345600 paise without an error.
    let err = parse_gstr2b(&gstr2b_with("1234567890123456", "", "18")).unwrap_err();
    let reason = first_error_reason(err);
    assert!(
        reason.contains("16 significant digits"),
        "reason was: {reason}"
    );
}

#[test]
fn invoice_total_from_the_finding_is_refused_not_read_as_a_different_amount() {
    // provenance: derived - 1234567890123456.78 has 18 significant digits. Old behaviour: it read
    // back as 1234567890123456.8, so the ITC total was off by 0.08 with no error.
    let err = parse_gstr2b(&gstr2b_with("1234567890123456.78", "", "18")).unwrap_err();
    assert!(first_error_reason(err).contains("18 significant digits"));
}

#[test]
fn long_fraction_on_a_rate_is_refused_even_after_trailing_zeros_are_dropped() {
    // provenance: derived - "18." then seventeen zeros then 1 has 2 + 17 + 1 = 20 significant
    // digits. Old behaviour: the rate read back as 18 with no error.
    let rate = format!("18.{}1", "0".repeat(17));
    let err = parse_gstr2b(&gstr2b_with("0", "", &rate)).unwrap_err();
    assert!(first_error_reason(err).contains("20 significant digits"));
}

#[test]
fn long_number_in_an_unknown_field_is_refused_too() {
    // provenance: derived - 12345678901234567 has 17 significant digits. The field is not read by
    // the reader, but it is kept in `extra`, so the text is refused before parsing.
    let err = parse_gstr2b(&gstr2b_with("0", r#""zz_count": 12345678901234567,"#, "18")).unwrap_err();
    assert!(first_error_reason(err).contains("17 significant digits"));
}

#[test]
fn long_digit_strings_that_are_not_numbers_are_not_refused() {
    // provenance: derived - a 64-digit invoice number and a 16-digit IRN are JSON strings, not
    // numbers, so they are read as written.
    let long = "1234567890".repeat(6) + "1234";
    assert_eq!(long.len(), 64);
    let text = gstr2b_with("0", "", "18").replace(
        r#""inum": "INV-001""#,
        &format!(r#""inum": "{long}", "irn": "1234567890123456""#),
    );
    let parsed = parse_gstr2b(&text).unwrap();
    let doc = &parsed.b2b[0].docs[0];
    assert_eq!(doc.number, long);
    assert_eq!(doc.irn.as_deref(), Some("1234567890123456"));
}

#[test]
fn exponent_form_with_few_significant_digits_is_still_read() {
    // provenance: derived - 1.23e3 has 3 significant digits and is 1230 rupees = 123000 paise.
    let parsed = parse_gstr2b(&gstr2b_with("1.23e3", "", "18")).unwrap();
    assert_eq!(parsed.b2b[0].docs[0].value, Paise::new(123_000));
}

#[test]
fn long_number_is_refused_through_the_ims_reader_too() {
    // provenance: derived - the same 20-digit rate as above, through the IMS reader that shares
    // the GSTR-2B parsing path.
    let rate = format!("18.{}1", "0".repeat(17));
    let text = gstr2b_with("0", "", &rate);
    assert!(parse_ims(&text).is_err());
}
