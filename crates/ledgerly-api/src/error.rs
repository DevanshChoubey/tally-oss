//! Errors of the facade (card W1). Each variant wraps one crate's typed error.
//!
//! A refused change is `Err(ApiError::Engine(..))`. Input this build cannot compute is not an
//! error: it is `Ok(Outcome::Unsupported(..))` (rule A1).

use ledgerly_engine::EngineError;
use ledgerly_gst::GstError;
use ledgerly_gst_formats::FormatError;
use ledgerly_reports::ReportError;
use ledgerly_store::StoreError;
use thiserror::Error;

/// Why a facade call could not complete.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The engine refused the command. Nothing was written.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// The store failed. When a commit failed, nothing was written.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A report engine rejected its parameters or its input.
    #[error(transparent)]
    Report(#[from] ReportError),
    /// A GST computation failed: a rule pack is malformed, a master is missing or arithmetic
    /// overflowed.
    #[error(transparent)]
    Gst(#[from] GstError),
    /// The GST format writer refused a value (for example a document number of the wrong length).
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A change was saved, but the books could not be reloaded afterwards, so figures are not
    /// computed until a command or [`crate::Api::open`] reloads them.
    #[error(
        "the books could not be reloaded after a saved change; run a command or reopen the company file"
    )]
    Stale,
}
