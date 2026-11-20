use serde::{Deserialize, Serialize};

/// The result of a computation that may legitimately not be supported (rule A1).
///
/// Errors are carried separately in a `Result`; `Unsupported` is a correct answer meaning
/// "Ledgerly does not compute this yet", never a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome<T> {
    /// The computed value.
    Value(T),
    /// The input needs behaviour Ledgerly does not implement.
    Unsupported(Unsupported),
}

/// Why a computation is unsupported.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unsupported {
    /// The construct, rule or case that is not handled (for example `"GST place of supply: s.12(8) courier"`).
    pub what: String,
    /// Why, and what the user can do.
    pub reason: String,
}

impl<T> Outcome<T> {
    /// Shorthand for an unsupported outcome.
    pub fn unsupported(what: impl Into<String>, reason: impl Into<String>) -> Self {
        Outcome::Unsupported(Unsupported {
            what: what.into(),
            reason: reason.into(),
        })
    }

    /// Maps the value, keeping `Unsupported` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Outcome<U> {
        match self {
            Outcome::Value(v) => Outcome::Value(f(v)),
            Outcome::Unsupported(u) => Outcome::Unsupported(u),
        }
    }

    /// Chains a computation that may itself be unsupported.
    pub fn and_then<U>(self, f: impl FnOnce(T) -> Outcome<U>) -> Outcome<U> {
        match self {
            Outcome::Value(v) => f(v),
            Outcome::Unsupported(u) => Outcome::Unsupported(u),
        }
    }

    /// The value, if any.
    pub fn value(self) -> Option<T> {
        match self {
            Outcome::Value(v) => Some(v),
            Outcome::Unsupported(_) => None,
        }
    }

    /// `true` for `Unsupported`.
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Outcome::Unsupported(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keeps_unsupported() {
        let u: Outcome<i32> = Outcome::unsupported("x", "y");
        assert!(u.map(|v| v + 1).is_unsupported());
        assert_eq!(Outcome::Value(1).map(|v| v + 1), Outcome::Value(2));
    }
}
