//! Card K5: UPI payment payload (spec 6.5).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_bank::BankError;
use ledgerly_bank::upi::upi_payload;
use ledgerly_core::Paise;

// provenance: derived - the plan card K5 gives the shape
// upi://pay?pa=..&pn=..&am=1180.00&cu=INR&tn=INV%2F26-27%2F0001; '/' is reserved in RFC 3986 so
// it becomes %2F, '-' is unreserved and stays. 118000 paise = Rs 1180.00 by hand.
#[test]
fn invoice_payload_matches_card_shape() {
    let s = upi_payload("acme@okbank", "Acme", Paise::new(118_000), "INV/26-27/0001").unwrap();
    assert_eq!(
        s,
        "upi://pay?pa=acme@okbank&pn=Acme&am=1180.00&cu=INR&tn=INV%2F26-27%2F0001"
    );
}

// provenance: derived - 5 paise = Rs 0.05; 100 paise = Rs 1.00; 123456789 paise = Rs 1234567.89.
#[test]
fn amount_comes_from_paise_with_two_decimals() {
    let am = |p: i64| {
        let s = upi_payload("a@b", "N", Paise::new(p), "").unwrap();
        s.split("&am=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .to_owned()
    };
    assert_eq!(am(5), "0.05");
    assert_eq!(am(100), "1.00");
    assert_eq!(am(123_456_789), "1234567.89");
}

// provenance: derived - RFC 3986: space is not unreserved so it is %20, '&' is %26, '+' is %2B,
// 'e-acute' (U+00E9) is UTF-8 bytes C3 A9 so %C3%A9; '~' '.' '_' '-' stay as they are.
#[test]
fn percent_encoding_of_name_and_note() {
    let s = upi_payload("a@b", "Sharma & Sons+Co", Paise::new(100), "caf\u{e9} ~._-").unwrap();
    assert_eq!(
        s,
        "upi://pay?pa=a@b&pn=Sharma%20%26%20Sons%2BCo&am=1.00&cu=INR&tn=caf%C3%A9%20~._-"
    );
}

// provenance: derived - an empty note has nothing to encode, so the tn parameter is left out.
#[test]
fn empty_note_omits_tn() {
    let s = upi_payload("a@b", "N", Paise::new(100), "").unwrap();
    assert_eq!(s, "upi://pay?pa=a@b&pn=N&am=1.00&cu=INR");
}

// provenance: derived - a payment request for zero or a negative sum is meaningless; the module
// doc says the amount must be positive.
#[test]
fn non_positive_amount_rejected() {
    for p in [0, -1, -118_000] {
        let e = upi_payload("a@b", "N", Paise::new(p), "").unwrap_err();
        assert!(matches!(e, BankError::Invalid { ref field, .. } if field == "amount"));
    }
}

// provenance: derived - a VPA is handle@provider; anything else cannot be paid.
#[test]
fn bad_vpa_and_blank_name_rejected() {
    for v in ["", "nohandle", "@bank", "a@", "a@b@c", "a b@c", "a&pn=x@b"] {
        let e = upi_payload(v, "N", Paise::new(100), "").unwrap_err();
        assert!(
            matches!(e, BankError::Invalid { ref field, .. } if field == "payee_vpa"),
            "{v}"
        );
    }
    let e = upi_payload("a@b", "  ", Paise::new(100), "").unwrap_err();
    assert!(matches!(e, BankError::Invalid { ref field, .. } if field == "payee_name"));
}

// provenance: derived - i64::MAX paise is 92233720368547758.07 rupees by hand (9223372036854775807
// split at the last two digits); no overflow and no float rounding.
#[test]
fn max_amount_is_exact() {
    let s = upi_payload("a@b", "N", Paise::new(i64::MAX), "").unwrap();
    assert!(s.contains("&am=92233720368547758.07&"));
}
