use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A permanent account number: 5 letters, 4 digits, 1 letter. Only the shape is checked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Pan(String);

/// Why a PAN is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PanError {
    /// Not 5 capital letters, 4 digits and 1 capital letter.
    #[error("a PAN is 5 capital letters, 4 digits and 1 capital letter")]
    Shape,
}

impl Pan {
    /// Parses a PAN.
    ///
    /// # Errors
    /// [`PanError::Shape`] when the text is not shaped like a PAN.
    pub fn parse(input: &str) -> Result<Self, PanError> {
        let s = input.trim();
        let b = s.as_bytes();
        let ok = b.len() == 10
            && b.iter().take(5).all(u8::is_ascii_uppercase)
            && b.iter().skip(5).take(4).all(u8::is_ascii_digit)
            && b.get(9).is_some_and(u8::is_ascii_uppercase);
        if ok {
            Ok(Pan(s.to_owned()))
        } else {
            Err(PanError::Shape)
        }
    }

    /// The PAN text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Pan {
    type Error = PanError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Pan::parse(&value)
    }
}

impl From<Pan> for String {
    fn from(value: Pan) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        assert!(Pan::parse("ABCDE1234F").is_ok());
        assert_eq!(Pan::parse("ABCD11234F"), Err(PanError::Shape));
        assert_eq!(Pan::parse("ABCDE1234"), Err(PanError::Shape));
    }
}
