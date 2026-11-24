//! Errors of loading the sample company.

use std::path::PathBuf;

use ledgerly_api::ApiError;

/// Why the sample company could not be built.
#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    /// The embedded seed text is not valid TOML for the seed's shape.
    #[error("the sample seed could not be read: {0}")]
    Seed(#[from] toml::de::Error),
    /// The seed names a version this build does not know.
    #[error("the sample seed is version {found}; this build reads version {expected}")]
    Version {
        /// The version in the seed.
        found: u32,
        /// The version this build reads.
        expected: u32,
    },
    /// A value in the seed is not valid (a date, an amount, a GSTIN, a key that names nothing).
    #[error("the sample seed has a bad value for {field}: {reason}")]
    Value {
        /// Where, for example `ledger sgt`.
        field: String,
        /// What is wrong.
        reason: String,
    },
    /// The API refused a command or a query.
    #[error("the sample company could not be built at {what}: {source}")]
    Api {
        /// The step, for example `voucher SI/0412`.
        what: String,
        /// The API's error.
        source: Box<ApiError>,
    },
    /// The engine reported a step as unsupported. The sample never posts around that.
    #[error("the engine does not support {what}: {reason}")]
    Unsupported {
        /// The construct.
        what: String,
        /// The engine's reason.
        reason: String,
    },
    /// A file or folder for the sample company could not be prepared.
    #[error("the sample company's file {path}: {source}")]
    File {
        /// The path.
        path: PathBuf,
        /// The system error.
        source: std::io::Error,
    },
}
