use thiserror::Error;

/// Errors reading or writing GST formats.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FormatError {
    /// Malformed JSON, with the JSON path.
    #[error("invalid JSON at {path}: {reason}")]
    Json {
        /// JSON path, such as `$.b2b[3].inv[0].val`.
        path: String,
        /// Cause.
        reason: String,
    },
    /// A required field is missing or invalid.
    #[error("{field}: {reason}")]
    Field {
        /// Field name in the official format.
        field: String,
        /// What is wrong.
        reason: String,
    },
    /// Writing a spreadsheet failed.
    #[error("spreadsheet error: {0}")]
    Spreadsheet(String),
}
