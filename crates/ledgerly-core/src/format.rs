//! Indian number formatting and amounts in words. **Card C1** implements this module.
//!
//! Frozen interface (spec section 7; plan card C1):
//!
//! ```text
//! pub fn format_inr(amount: Paise) -> String;        // 1_234_567_00 -> "12,34,567.00"
//! pub fn amount_in_words(amount: Paise) -> String;   // "Rupees ... and Fifty Paise Only"
//! ```
//!
//! Indian grouping: the last three digits of the rupee part form one group, every group to the
//! left has two digits (thousand = 10^3, lakh = 10^5, crore = 10^7). No floats, no panics: the
//! magnitude is taken with `unsigned_abs`, so `i64::MIN` paise is handled.

use crate::money::Paise;

const ONES: [&str; 20] = [
    "Zero",
    "One",
    "Two",
    "Three",
    "Four",
    "Five",
    "Six",
    "Seven",
    "Eight",
    "Nine",
    "Ten",
    "Eleven",
    "Twelve",
    "Thirteen",
    "Fourteen",
    "Fifteen",
    "Sixteen",
    "Seventeen",
    "Eighteen",
    "Nineteen",
];

const TENS: [&str; 10] = [
    "", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety",
];

/// Formats an amount with Indian digit grouping and two decimals, e.g. `12,34,567.00`.
///
/// Negative amounts get a leading `-`. Zero is `0.00`.
pub fn format_inr(amount: Paise) -> String {
    let magnitude = amount.get().unsigned_abs();
    let rupees = magnitude / 100;
    let paise = magnitude % 100;
    let sign = if amount.is_negative() { "-" } else { "" };
    format!("{sign}{}.{paise:02}", group_indian(rupees))
}

/// Groups a rupee count: last three digits, then pairs.
fn group_indian(rupees: u64) -> String {
    let digits = rupees.to_string();
    let chars: Vec<char> = digits.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(len + len / 2);
    for (i, ch) in chars.iter().enumerate() {
        let remaining = len - i;
        // A separator goes before this digit when the digits to its right (including it)
        // number 3 + 2k for k >= 0 and it is not the first digit.
        if i > 0 && remaining >= 3 && (remaining - 3).is_multiple_of(2) {
            out.push(',');
        }
        out.push(*ch);
    }
    out
}

/// Words for `n` in `1..=99`.
fn below_hundred(n: u64, out: &mut Vec<String>) {
    if n < 20 {
        if let Some(w) = usize::try_from(n).ok().and_then(|i| ONES.get(i)) {
            out.push((*w).to_owned());
        }
        return;
    }
    let tens = usize::try_from(n / 10).ok().and_then(|i| TENS.get(i));
    if let Some(t) = tens {
        out.push((*t).to_owned());
    }
    let unit = n % 10;
    if unit != 0
        && let Some(w) = usize::try_from(unit).ok().and_then(|i| ONES.get(i))
    {
        out.push((*w).to_owned());
    }
}

/// Words for `n` in `1..=99_999` (thousands, hundreds, tens, units). Appends nothing for 0.
fn below_lakh(n: u64, out: &mut Vec<String>) {
    let thousands = n / 1000;
    let rest = n % 1000;
    if thousands > 0 {
        below_hundred(thousands, out);
        out.push("Thousand".to_owned());
    }
    let hundreds = rest / 100;
    if hundreds > 0 {
        below_hundred(hundreds, out);
        out.push("Hundred".to_owned());
    }
    let tail = rest % 100;
    if tail > 0 {
        below_hundred(tail, out);
    }
}

/// Words for a rupee count (`>= 1`): Lakh Crore, Crore, Lakh, Thousand, Hundred.
fn rupee_words(rupees: u64, out: &mut Vec<String>) {
    let crores = rupees / 10_000_000;
    let rest = rupees % 10_000_000;
    let lakh_crores = crores / 100_000;
    let crore_part = crores % 100_000;
    if lakh_crores > 0 {
        below_lakh(lakh_crores, out);
        out.push("Lakh".to_owned());
        out.push("Crore".to_owned());
    }
    if crore_part > 0 {
        below_lakh(crore_part, out);
        out.push("Crore".to_owned());
    }
    let lakhs = rest / 100_000;
    if lakhs > 0 {
        below_hundred(lakhs, out);
        out.push("Lakh".to_owned());
    }
    below_lakh(rest % 100_000, out);
}

/// Writes an amount in words, e.g. `Rupees One Crore ... and Fifty Paise Only`.
///
/// A whole-rupee amount drops the paise clause (`Rupees One Only`); a sub-rupee amount drops the
/// rupee clause (`Fifty Paise Only`); zero is `Rupees Zero Only`. A negative amount is prefixed
/// `Minus `. Those three conventions are not covered by a sourced example (see card C1 report).
pub fn amount_in_words(amount: Paise) -> String {
    let magnitude = amount.get().unsigned_abs();
    let rupees = magnitude / 100;
    let paise = magnitude % 100;
    let mut words: Vec<String> = Vec::new();
    if amount.is_negative() {
        words.push("Minus".to_owned());
    }
    if rupees > 0 || paise == 0 {
        words.push("Rupees".to_owned());
        if rupees == 0 {
            words.push("Zero".to_owned());
        } else {
            rupee_words(rupees, &mut words);
        }
    }
    if paise > 0 {
        if rupees > 0 {
            words.push("and".to_owned());
        }
        below_hundred(paise, &mut words);
        words.push("Paise".to_owned());
    }
    words.push("Only".to_owned());
    words.join(" ")
}
