use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::state::StateCode;

/// A GST identification number: 2-digit state code, 10-character PAN, entity number, a 14th
/// character, and a check character.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Gstin(String);

/// Why a GSTIN is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GstinError {
    /// Not 15 characters.
    #[error("a GSTIN has 15 characters; this one has {0}")]
    Length(usize),
    /// A character outside `0-9A-Z`.
    #[error("a GSTIN may contain only digits and capital letters")]
    Charset,
    /// The first two characters are not a known state code.
    #[error("'{0}' is not a GST state code")]
    StateCode(String),
    /// Characters 3 to 12 are not shaped like a PAN.
    #[error("characters 3 to 12 must be a PAN (5 letters, 4 digits, 1 letter)")]
    PanShape,
    /// The 15th character does not match the checksum of the first 14.
    #[error("check character is {found}, expected {expected}; the GSTIN has a typing error")]
    CheckCharacter {
        /// The check character the first 14 characters imply.
        expected: char,
        /// The character given.
        found: char,
    },
}

const CHARSET: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn char_value(c: u8) -> Option<u32> {
    CHARSET
        .iter()
        .position(|&x| x == c)
        .and_then(|p| u32::try_from(p).ok())
}

/// The check character for the first 14 characters of a GSTIN.
///
/// Algorithm (Luhn mod 36): map `0-9A-Z` to 0..35; walking the 14 characters from the right,
/// multiply alternately by 2 and 1 (starting with 2); add `product / 36 + product % 36` for each;
/// the check value is `(36 - sum % 36) % 36`.
///
/// Provenance: the algorithm is recalled, not yet quoted from a GSTN publication. It reproduces
/// a widely published sample GSTIN. Quote the source here before release (rule A4).
///
/// Returns `None` when the input is not 14 characters of `0-9A-Z`.
pub fn gstin_check_character(first14: &str) -> Option<char> {
    let bytes = first14.as_bytes();
    if bytes.len() != 14 {
        return None;
    }
    let mut factor = 2u32;
    let mut sum = 0u32;
    for &b in bytes.iter().rev() {
        let product = char_value(b)?.checked_mul(factor)?;
        sum = sum.checked_add(product / 36 + product % 36)?;
        factor = if factor == 2 { 1 } else { 2 };
    }
    let check = (36 - sum % 36) % 36;
    CHARSET
        .get(usize::try_from(check).ok()?)
        .map(|&b| char::from(b))
}

fn is_pan_shape(s: &[u8]) -> bool {
    s.len() == 10
        && s.iter().take(5).all(u8::is_ascii_uppercase)
        && s.iter().skip(5).take(4).all(u8::is_ascii_digit)
        && s.get(9).is_some_and(u8::is_ascii_uppercase)
}

impl Gstin {
    /// Parses and validates a GSTIN (length, charset, state code, PAN shape, check character).
    ///
    /// # Errors
    /// A [`GstinError`] naming the first problem found.
    pub fn parse(input: &str) -> Result<Self, GstinError> {
        let s = input.trim();
        let b = s.as_bytes();
        if b.len() != 15 {
            return Err(GstinError::Length(s.chars().count()));
        }
        if !b
            .iter()
            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
        {
            return Err(GstinError::Charset);
        }
        let state = s.get(0..2).ok_or(GstinError::Length(b.len()))?;
        state
            .parse::<u8>()
            .ok()
            .and_then(|n| StateCode::new(n).ok())
            .ok_or_else(|| GstinError::StateCode(state.to_owned()))?;
        if !is_pan_shape(b.get(2..12).unwrap_or_default()) {
            return Err(GstinError::PanShape);
        }
        let first14 = s.get(0..14).ok_or(GstinError::Length(b.len()))?;
        let expected = gstin_check_character(first14).ok_or(GstinError::Charset)?;
        let found = b.get(14).map_or(' ', |&c| char::from(c));
        if expected != found {
            return Err(GstinError::CheckCharacter { expected, found });
        }
        Ok(Gstin(s.to_owned()))
    }

    /// The GSTIN text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The registration's state code (first two characters).
    pub fn state(&self) -> Option<StateCode> {
        self.0
            .get(0..2)?
            .parse::<u8>()
            .ok()
            .and_then(|n| StateCode::new(n).ok())
    }

    /// The embedded PAN (characters 3 to 12).
    pub fn pan_str(&self) -> &str {
        self.0.get(2..12).unwrap_or_default()
    }
}

impl TryFrom<String> for Gstin {
    type Error = GstinError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Gstin::parse(&value)
    }
}

impl From<Gstin> for String {
    fn from(value: Gstin) -> Self {
        value.0
    }
}

impl std::fmt::Display for Gstin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fictional GSTINs. Provenance: derived by hand with the mod-36 algorithm described in the
    // module doc. That algorithm is recalled, not quoted from GSTN, so these vectors are only as
    // good as the recall until the source is quoted (see module doc).
    //
    // Hand derivation for 29ABCDE1234F1ZW (0-9 = 0-9, A = 10 .. Z = 35; factors 1,2,1,2,..
    // from the left; each product p contributes p/36 + p%36):
    //   2x1=2 -> 2      9x2=18 -> 18    A10x1=10 -> 10  B11x2=22 -> 22  C12x1=12 -> 12
    //   D13x2=26 -> 26  E14x1=14 -> 14  1x2=2 -> 2      2x1=2 -> 2      3x2=6 -> 6
    //   4x1=4 -> 4      F15x2=30 -> 30  1x1=1 -> 1      Z35x2=70 -> 1+34=35
    //   sum = 184; 184 mod 36 = 4; check = (36 - 4) mod 36 = 32 = 'W'.
    const VALID: [&str; 3] = ["29ABCDE1234F1ZW", "07AAACL0000A1ZP", "24AAACB1111C1ZT"];

    #[test]
    fn valid_vectors_parse() {
        for g in VALID {
            let parsed = Gstin::parse(g).unwrap();
            assert_eq!(parsed.as_str(), g);
        }
        assert_eq!(
            Gstin::parse("29ABCDE1234F1ZW").unwrap().pan_str(),
            "ABCDE1234F"
        );
        assert_eq!(
            Gstin::parse("29ABCDE1234F1ZW")
                .unwrap()
                .state()
                .map(StateCode::get),
            Some(29)
        );
    }

    #[test]
    fn wrong_check_character_errors() {
        for g in VALID {
            let mut wrong = g[..14].to_owned();
            wrong.push(if g.ends_with('A') { 'B' } else { 'A' });
            assert!(
                matches!(Gstin::parse(&wrong), Err(GstinError::CheckCharacter { .. })),
                "{wrong}"
            );
        }
    }

    #[test]
    fn shape_errors() {
        assert_eq!(Gstin::parse("29ABCDE1234F1Z"), Err(GstinError::Length(14)));
        assert_eq!(Gstin::parse("29abcde1234F1ZW"), Err(GstinError::Charset));
        assert_eq!(
            Gstin::parse("00ABCDE1234F1ZW"),
            Err(GstinError::StateCode("00".into()))
        );
        assert_eq!(Gstin::parse("29ABCD11234F1ZW"), Err(GstinError::PanShape));
    }
}
