use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An HSN code (goods) or SAC (services): 4, 6 or 8 digits. SAC codes start with `99`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hsn(String);

/// Why an HSN/SAC code is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HsnError {
    /// Not 4, 6 or 8 digits.
    #[error("an HSN/SAC code is 4, 6 or 8 digits")]
    Shape,
}

impl Hsn {
    /// Parses an HSN/SAC code.
    ///
    /// # Errors
    /// [`HsnError::Shape`] when the code is not 4, 6 or 8 digits.
    pub fn parse(input: &str) -> Result<Self, HsnError> {
        let s = input.trim();
        if matches!(s.len(), 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Hsn(s.to_owned()))
        } else {
            Err(HsnError::Shape)
        }
    }

    /// The code text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` for a services accounting code (starts with `99`).
    pub fn is_sac(&self) -> bool {
        self.0.starts_with("99")
    }
}

impl TryFrom<String> for Hsn {
    type Error = HsnError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Hsn::parse(&value)
    }
}

impl From<Hsn> for String {
    fn from(value: Hsn) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        assert!(Hsn::parse("8471").is_ok());
        assert!(Hsn::parse("998314").unwrap().is_sac());
        assert_eq!(Hsn::parse("84715"), Err(HsnError::Shape));
        assert_eq!(Hsn::parse("84a1"), Err(HsnError::Shape));
    }
}
