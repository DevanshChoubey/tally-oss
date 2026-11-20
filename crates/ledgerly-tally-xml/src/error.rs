use thiserror::Error;

/// Errors reading Tally XML. Every error carries the byte offset in the decoded document.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TallyXmlError {
    /// The bytes could not be decoded as text.
    #[error("cannot decode text at byte {offset}: {reason}")]
    Decode {
        /// Byte offset.
        offset: usize,
        /// Cause.
        reason: String,
    },
    /// Malformed XML.
    #[error("malformed XML at byte {offset}: {reason}")]
    Xml {
        /// Byte offset.
        offset: usize,
        /// Cause.
        reason: String,
    },
    /// The document ended early.
    #[error("document ends early at byte {offset}")]
    Truncated {
        /// Byte offset.
        offset: usize,
    },
    /// A required element is missing.
    #[error("missing <{tag}> in {path}")]
    Missing {
        /// Tag name.
        tag: String,
        /// Path to the parent element.
        path: String,
    },
    /// An element's value is invalid.
    #[error("<{tag}> value '{value}': {reason}")]
    Value {
        /// Tag name.
        tag: String,
        /// The text.
        value: String,
        /// What is wrong.
        reason: String,
    },
}

/// A non-fatal problem found while reading (for example an invalid character that was dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlWarning {
    /// Byte offset.
    pub offset: usize,
    /// What happened.
    pub message: String,
}
