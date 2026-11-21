//! Redaction before any cloud call. **Card A4** implements this module.
//!
//! Spec 6.8 and 9 and research R10 (privacy tier 2): before text leaves the machine, PAN, GSTIN,
//! Aadhaar, bank account numbers, IFSC codes and phone numbers are replaced by stable placeholders
//! such as `[PAN_1]`. The same value gets the same placeholder inside one call, so the model can
//! still see that two lines name the same party. The caller keeps the [`Redaction`] list and uses
//! [`restore`] to put the originals back into the model's answer; the originals never leave the
//! process.
//!
//! Frozen interface (plan card A4):
//!
//! ```text
//! pub fn redact(text: &str) -> (String, Vec<Redaction>);   // PAN, GSTIN, account numbers, IFSC
//! ```
//!
//! # What is recognised
//!
//! The scan splits the text into maximal runs of ASCII letters and digits ("words") and tests
//! each word by shape only. Shape, not checksum: a GSTIN with a typing error is still private, so
//! it is redacted too (fail closed).
//!
//! | Kind | Shape | Source of the shape |
//! |---|---|---|
//! | GSTIN | 15 characters: 2 digits, 5 letters, 4 digits, 1 letter, 1 alphanumeric, `Z`, 1 alphanumeric | `ledgerly-core` `Gstin` module doc |
//! | PAN | 10 characters: 5 letters, 4 digits, 1 letter | `ledgerly-core` `Pan::parse` |
//! | IFSC | 11 characters: 4 letters, `0`, 6 alphanumerics | RBI IFSC layout (bank code, zero, branch code) |
//! | Aadhaar | 12 digits, or 4-4-4 digits joined by one space or hyphen | UIDAI 12-digit number, displayed in groups of 4 |
//! | Phone | 10 digits starting 6 to 9, or 5-5 digits joined by one space or hyphen | Indian mobile numbering |
//! | Bank account | 9 to 18 digits in one run | RBI/bank practice: 9 to 18 digits |
//!
//! # Known limits
//!
//! This is a safety net, not a guarantee. It does not see identifiers spelt in words, written in
//! non-ASCII digits, or split in groups other than the two above. A plain 9-to-18 digit number is
//! always treated as an account number, so a very large amount written with no separator is
//! redacted too (it fails closed). Letters are matched in either case. Names and addresses are
//! not redacted: the assistant needs party names to resolve ledgers, and the cloud tier is opt-in
//! per company.

use std::collections::BTreeMap;
use std::fmt;

/// What kind of private identifier was redacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RedactionKind {
    /// A GST identification number.
    Gstin,
    /// A permanent account number.
    Pan,
    /// An Aadhaar number.
    Aadhaar,
    /// A phone number.
    Phone,
    /// A bank branch code (IFSC).
    Ifsc,
    /// A bank account number.
    BankAccount,
}

impl RedactionKind {
    /// The label used inside the placeholder, such as `PAN` in `[PAN_1]`.
    pub fn label(self) -> &'static str {
        match self {
            RedactionKind::Gstin => "GSTIN",
            RedactionKind::Pan => "PAN",
            RedactionKind::Aadhaar => "AADHAAR",
            RedactionKind::Phone => "PHONE",
            RedactionKind::Ifsc => "IFSC",
            RedactionKind::BankAccount => "ACCOUNT",
        }
    }
}

/// One identifier that was replaced by a placeholder.
///
/// `Debug` deliberately omits the original value so it cannot reach a log (spec 9).
#[derive(Clone, PartialEq, Eq)]
pub struct Redaction {
    /// What was redacted.
    pub kind: RedactionKind,
    /// The placeholder sent instead, such as `[PAN_1]`.
    pub placeholder: String,
    /// Byte offset of the original in the text passed to the call.
    pub start: usize,
    /// Byte offset just past the original (exclusive).
    pub end: usize,
    /// Character offset of the original in the text passed to the call.
    pub char_start: usize,
    /// The original text. Crate-private so it stays in memory and out of every API.
    pub(crate) original: String,
}

impl fmt::Debug for Redaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redaction")
            .field("kind", &self.kind)
            .field("placeholder", &self.placeholder)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

/// A byte range of the text that matched, with its kind.
#[derive(Debug, Clone, Copy)]
struct Hit {
    start: usize,
    end: usize,
    kind: RedactionKind,
}

/// Replaces every recognised identifier in `text` with a placeholder.
///
/// Returns the redacted text and one [`Redaction`] per occurrence, in text order. Offsets refer to
/// `text`. The output contains no byte of any redacted identifier.
pub fn redact(text: &str) -> (String, Vec<Redaction>) {
    let mut redactor = Redactor::default();
    let out = redactor.apply(text);
    (out, redactor.finish())
}

/// Redacts several pieces of text with one numbering, so placeholders are consistent across them.
///
/// `Debug` shows placeholders and counts only: the map of originals never reaches a log (spec 9).
#[derive(Default)]
pub struct Redactor {
    seen: BTreeMap<(RedactionKind, String), String>,
    found: Vec<Redaction>,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let placeholders: Vec<&str> = self.seen.values().map(String::as_str).collect();
        f.debug_struct("Redactor")
            .field("placeholders", &placeholders)
            .field("redactions", &self.found.len())
            .finish()
    }
}

impl Redactor {
    /// Redacts one piece of text. Offsets recorded for it are relative to this piece.
    pub fn apply(&mut self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0_usize;
        for hit in scan(text) {
            out.push_str(text.get(last..hit.start).unwrap_or_default());
            let original = text.get(hit.start..hit.end).unwrap_or_default();
            let placeholder = self.placeholder_for(hit.kind, original);
            out.push_str(&placeholder);
            let char_start = text
                .get(..hit.start)
                .map_or(0, |before| before.chars().count());
            self.found.push(Redaction {
                kind: hit.kind,
                placeholder,
                start: hit.start,
                end: hit.end,
                char_start,
                original: original.to_owned(),
            });
            last = hit.end;
        }
        out.push_str(text.get(last..).unwrap_or_default());
        out
    }

    /// Every redaction made so far, in the order the pieces were applied.
    pub fn finish(self) -> Vec<Redaction> {
        self.found
    }

    /// The redactions made so far.
    pub fn redactions(&self) -> &[Redaction] {
        &self.found
    }

    fn placeholder_for(&mut self, kind: RedactionKind, original: &str) -> String {
        let key = (kind, original.to_ascii_uppercase());
        if let Some(existing) = self.seen.get(&key) {
            return existing.clone();
        }
        let number = self.seen.keys().filter(|(k, _)| *k == kind).count() + 1;
        let placeholder = format!("[{}_{number}]", kind.label());
        self.seen.insert(key, placeholder.clone());
        placeholder
    }
}

/// Puts the original identifiers back into `text` (typically the model's answer).
///
/// Every placeholder listed in `redactions` is replaced by its original. Placeholders the model
/// invented (not in the list) are left as they are.
pub fn restore(text: &str, redactions: &[Redaction]) -> String {
    let mut out = text.to_owned();
    for r in redactions {
        out = out.replace(&r.placeholder, &r.original);
    }
    out
}

/// Maps a character range in the redacted text back to the same range in the original text.
///
/// A start or end that falls inside a placeholder snaps to the start or end of the original
/// identifier it stands for. `redactions` must come from one [`redact`] call.
pub fn to_original_range(redactions: &[Redaction], start: usize, end: usize) -> (usize, usize) {
    (
        map_position(redactions, start, false),
        map_position(redactions, end, true),
    )
}

fn map_position(redactions: &[Redaction], position: usize, is_end: bool) -> usize {
    // `added` is the number of original characters swallowed so far, `removed` the number of
    // placeholder characters that replaced them; the redacted offset of an original character is
    // original + removed - added.
    let mut added = 0_usize;
    let mut removed = 0_usize;
    for r in redactions {
        let original_len = r.original.chars().count();
        let placeholder_len = r.placeholder.chars().count();
        let red_start = r.char_start.saturating_add(removed).saturating_sub(added);
        let red_end = red_start.saturating_add(placeholder_len);
        if position >= red_end {
            added = added.saturating_add(original_len);
            removed = removed.saturating_add(placeholder_len);
        } else if position > red_start {
            return if is_end {
                r.char_start.saturating_add(original_len)
            } else {
                r.char_start
            };
        } else {
            break;
        }
    }
    position.saturating_add(added).saturating_sub(removed)
}

// ---------------------------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------------------------

/// Byte ranges of maximal runs of ASCII letters and digits.
fn words(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c.is_ascii_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push((s, i));
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

fn scan(text: &str) -> Vec<Hit> {
    let spans = words(text);
    let mut hits = Vec::new();
    let mut i = 0_usize;
    while i < spans.len() {
        if let Some((hit, used)) = grouped(text, &spans, i) {
            hits.push(hit);
            i += used;
            continue;
        }
        if let Some(&(start, end)) = spans.get(i)
            && let Some(kind) = text.get(start..end).and_then(classify)
        {
            hits.push(Hit { start, end, kind });
        }
        i += 1;
    }
    hits
}

/// Identifiers written in groups: Aadhaar as 4-4-4 and a phone number as 5-5.
fn grouped(text: &str, spans: &[(usize, usize)], i: usize) -> Option<(Hit, usize)> {
    let a = *spans.get(i)?;
    let a_txt = text.get(a.0..a.1)?;
    if !all_digits(a_txt) {
        return None;
    }
    let b = *spans.get(i + 1)?;
    let b_txt = text.get(b.0..b.1)?;
    if !all_digits(b_txt) || !joined(text, a, b) {
        return None;
    }
    if a_txt.len() == 4 && b_txt.len() == 4 {
        let c = *spans.get(i + 2)?;
        let c_txt = text.get(c.0..c.1)?;
        if all_digits(c_txt) && c_txt.len() == 4 && joined(text, b, c) {
            let hit = Hit {
                start: a.0,
                end: c.1,
                kind: RedactionKind::Aadhaar,
            };
            return Some((hit, 3));
        }
    }
    if a_txt.len() == 5 && b_txt.len() == 5 && starts_mobile(a_txt) {
        let hit = Hit {
            start: a.0,
            end: b.1,
            kind: RedactionKind::Phone,
        };
        return Some((hit, 2));
    }
    None
}

/// `true` when exactly one space or hyphen separates the two words.
fn joined(text: &str, a: (usize, usize), b: (usize, usize)) -> bool {
    a.1 + 1 == b.0 && matches!(text.get(a.1..b.0), Some(" " | "-"))
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn starts_mobile(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| (b'6'..=b'9').contains(&b))
}

fn classify(word: &str) -> Option<RedactionKind> {
    let b = word.as_bytes();
    if b.len() == 15 && is_gstin_shape(b) {
        return Some(RedactionKind::Gstin);
    }
    if b.len() == 11 && is_ifsc_shape(b) {
        return Some(RedactionKind::Ifsc);
    }
    if b.len() == 10 && is_pan_shape(b) {
        return Some(RedactionKind::Pan);
    }
    if all_digits(word) {
        if b.len() == 12 {
            return Some(RedactionKind::Aadhaar);
        }
        if b.len() == 10 && starts_mobile(word) {
            return Some(RedactionKind::Phone);
        }
        if (9..=18).contains(&b.len()) {
            return Some(RedactionKind::BankAccount);
        }
    }
    None
}

fn letters(b: &[u8]) -> bool {
    b.iter().all(u8::is_ascii_alphabetic)
}

fn digits(b: &[u8]) -> bool {
    b.iter().all(u8::is_ascii_digit)
}

fn is_pan_shape(b: &[u8]) -> bool {
    b.get(0..5).is_some_and(letters)
        && b.get(5..9).is_some_and(digits)
        && b.get(9).is_some_and(u8::is_ascii_alphabetic)
}

fn is_gstin_shape(b: &[u8]) -> bool {
    b.get(0..2).is_some_and(digits)
        && b.get(2..12).is_some_and(is_pan_shape)
        && b.get(12).is_some_and(u8::is_ascii_alphanumeric)
        && b.get(13).is_some_and(|c| c.eq_ignore_ascii_case(&b'Z'))
        && b.get(14).is_some_and(u8::is_ascii_alphanumeric)
}

fn is_ifsc_shape(b: &[u8]) -> bool {
    b.get(0..4).is_some_and(letters)
        && b.get(4) == Some(&b'0')
        && b.get(5..11)
            .is_some_and(|rest| rest.iter().all(u8::is_ascii_alphanumeric))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Provenance for every case below: derived. The identifiers are fictional and are written
    // out by hand from the shape rules in the module doc; the GSTIN 29ABCDE1234F1ZW is the
    // hand-derived vector in `ledgerly-core` (its check character W is derived there). The
    // expected redacted strings are produced by hand-substituting the placeholder for the
    // identifier; none is copied from this module's output.

    #[test]
    fn pan_is_replaced() {
        let (out, found) = redact("PAN of Sharma is ABCDE1234F, ok");
        assert_eq!(out, "PAN of Sharma is [PAN_1], ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, RedactionKind::Pan);
        // Hand count: "PAN of Sharma is " is 3 + 1 + 2 + 1 + 6 + 1 + 2 + 1 = 17 bytes, so the PAN
        // starts at byte 17 and, being 10 characters long, ends (exclusive) at byte 27.
        assert_eq!(found[0].start, 17);
        assert_eq!(found[0].end, 27);
    }

    #[test]
    fn gstin_is_one_token_not_a_pan_inside_it() {
        // The GSTIN embeds the PAN ABCDE1234F at characters 3 to 12, but the whole 15 characters
        // are one word, so only one redaction is made.
        let (out, found) = redact("GSTIN 29ABCDE1234F1ZW.");
        assert_eq!(out, "GSTIN [GSTIN_1].");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn gstin_with_wrong_check_character_is_still_redacted() {
        // Fail closed: shape only. 29ABCDE1234F1ZA has a wrong check character (derived: W is
        // right, see ledgerly-core), and it is still a private identifier.
        let (out, _) = redact("29ABCDE1234F1ZA");
        assert_eq!(out, "[GSTIN_1]");
    }

    #[test]
    fn ifsc_and_account_number() {
        // Fictional bank details: bank code HDFC, branch code 000123, account 123456789012345
        // (15 digits, inside the 9 to 18 range).
        let (out, found) = redact("A/c 123456789012345 IFSC HDFC0000123");
        assert_eq!(out, "A/c [ACCOUNT_1] IFSC [IFSC_1]");
        assert_eq!(found[0].kind, RedactionKind::BankAccount);
        assert_eq!(found[1].kind, RedactionKind::Ifsc);
    }

    #[test]
    fn aadhaar_plain_and_grouped() {
        let (out, found) = redact("id 234567890123 or 2345 6789 0123 or 2345-6789-0123");
        assert_eq!(out, "id [AADHAAR_1] or [AADHAAR_2] or [AADHAAR_3]");
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn mobile_number_plain_and_grouped() {
        let (out, _) = redact("call 9876543210 or 98765 43210");
        assert_eq!(out, "call [PHONE_1] or [PHONE_2]");
    }

    #[test]
    fn same_value_gets_the_same_placeholder() {
        let (out, found) = redact("ABCDE1234F paid ABCDE1234F and FGHIJ5678K");
        assert_eq!(out, "[PAN_1] paid [PAN_1] and [PAN_2]");
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn ordinary_accounting_text_is_untouched() {
        // Amounts, dates and invoice numbers are not identifiers: a comma-grouped amount, an
        // 8-digit run (below the 9-digit account floor), a date and a short invoice number.
        let text = "Paid rent 25,000.00 on 2026-03-31 vide INV-2026-0042 ref 12345678";
        let (out, found) = redact(text);
        assert_eq!(out, text);
        assert!(found.is_empty());
    }

    #[test]
    fn redaction_is_idempotent() {
        let (once, _) = redact("ABCDE1234F 29ABCDE1234F1ZW 123456789012345 HDFC0000123 9876543210");
        let (twice, found) = redact(&once);
        assert_eq!(once, twice);
        assert!(found.is_empty());
    }

    #[test]
    fn restore_puts_the_originals_back() {
        let original = "pay ABCDE1234F 500 via 123456789012345";
        let (red, found) = redact(original);
        assert_eq!(red, "pay [PAN_1] 500 via [ACCOUNT_1]");
        assert_eq!(restore(&red, &found), original);
        // A placeholder the model invented is left alone.
        assert_eq!(restore("[PAN_9]", &found), "[PAN_9]");
    }

    #[test]
    fn offsets_map_back_across_a_multibyte_prefix() {
        // "₹" is one character but three bytes. Original: "₹ ABCDE1234F 500" has "500" at
        // characters 13..16 (2 for "₹ ", 10 for the PAN, 1 space). Redacted: "₹ [PAN_1] 500" has "500"
        // at characters 10..13 (2 for "₹ ", 7 for "[PAN_1]", 1 space).
        let (red, found) = redact("₹ ABCDE1234F 500");
        assert_eq!(red, "₹ [PAN_1] 500");
        assert_eq!(to_original_range(&found, 10, 13), (13, 16));
        // A range before the placeholder is unchanged.
        assert_eq!(to_original_range(&found, 0, 1), (0, 1));
        // A range covering the placeholder maps to the whole identifier.
        assert_eq!(to_original_range(&found, 2, 9), (2, 12));
    }

    #[test]
    fn debug_output_does_not_contain_the_original() {
        let (_, found) = redact("ABCDE1234F");
        assert!(!format!("{found:?}").contains("ABCDE1234F"));
    }

    #[test]
    fn redactor_debug_output_does_not_contain_the_original() {
        let mut redactor = Redactor::default();
        let _ = redactor.apply("ABCDE1234F 29ABCDE1234F1ZW 123456789012345");
        let shown = format!("{redactor:?}");
        assert!(!shown.contains("ABCDE1234F"));
        assert!(!shown.contains("29ABCDE1234F1ZW"));
        assert!(!shown.contains("123456789012345"));
        // It still says what it holds: the placeholders (hand-written from the numbering rule).
        assert!(shown.contains("[PAN_1]"));
    }
}
