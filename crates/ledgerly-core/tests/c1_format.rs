//! Card C1 tests: Indian number grouping and amount in words.
use ledgerly_core::Paise;
use ledgerly_core::format::{amount_in_words, format_inr};

#[test]
fn format_lakh_grouping() {
    // provenance: derived — Indian numbering: first group of 3 digits, then groups of 2
    // (thousand = 10^3, lakh = 10^5, crore = 10^7). 123_456_700 paise = 1234567 rupees
    // = 12,34,567 (12 lakh... 12|34|567) and 00 paise.
    assert_eq!(format_inr(Paise::new(123_456_700)), "12,34,567.00");
}

#[test]
fn format_negative() {
    // provenance: derived — 5_000 paise = 50 rupees, no grouping below 1000; sign prefixed.
    assert_eq!(format_inr(Paise::new(-5_000)), "-50.00");
}

#[test]
fn format_small_and_zero() {
    // provenance: derived — 5 paise = 0 rupees and 05 paise; 0 = 0.00.
    assert_eq!(format_inr(Paise::new(5)), "0.05");
    assert_eq!(format_inr(Paise::ZERO), "0.00");
    // provenance: derived — 999 rupees has no separator, 1000 rupees = 1,000.
    assert_eq!(format_inr(Paise::new(99_900)), "999.00");
    assert_eq!(format_inr(Paise::new(100_000)), "1,000.00");
}

#[test]
fn format_crore_boundary() {
    // provenance: derived — 10^7 rupees = 1 crore = 1,00,00,000; 10^5 rupees = 1 lakh = 1,00,000.
    assert_eq!(format_inr(Paise::new(1_000_000_000)), "1,00,00,000.00");
    assert_eq!(format_inr(Paise::new(10_000_000)), "1,00,000.00");
}

#[test]
fn format_extremes_do_not_panic() {
    // provenance: derived — i64::MAX paise = 9223372036854775807 = 92233720368547758 rupees
    // and 07 paise; groups of 2 from the right after the last 3 digits:
    // 92,23,37,20,36,85,47,758.07 (digits 92233720368547758 -> 758 | 47 | 85 | 36 | 20 | 37 | 23 | 92).
    assert_eq!(
        format_inr(Paise::new(i64::MAX)),
        "92,23,37,20,36,85,47,758.07"
    );
    // provenance: derived — i64::MIN = -9223372036854775808 -> 92233720368547758 rupees, 08 paise.
    assert_eq!(
        format_inr(Paise::new(i64::MIN)),
        "-92,23,37,20,36,85,47,758.08"
    );
}

#[test]
fn words_crore_lakh() {
    // provenance: derived — 1_234_567_850 paise = 12345678 rupees 50 paise.
    // 12345678 = 1 crore (10^7) + 23 lakh (23*10^5) + 45 thousand + 6 hundred + 78.
    assert_eq!(
        amount_in_words(Paise::new(1_234_567_850)),
        "Rupees One Crore Twenty Three Lakh Forty Five Thousand Six Hundred Seventy Eight and Fifty Paise Only"
    );
}

// PENDING (no provenance, so not asserted): wording for whole-rupee amounts ("Rupees One Only"),
// sub-rupee amounts, zero, negatives ("Minus ...") and amounts of one lakh crore or more.
