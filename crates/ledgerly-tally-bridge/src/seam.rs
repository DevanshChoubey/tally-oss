use thiserror::Error;

/// How the bridge talks to Tally (rule C2: the only seam in this crate). Tests use recorded
/// fixtures behind this trait; production uses `transport::HttpTransport` (card B1).
pub trait TallyTransport {
    /// Sends one XML request and returns the response body.
    ///
    /// # Errors
    /// [`TransportError`] when Tally cannot be reached or answers with an HTTP error.
    fn post(&self, xml_request: &str) -> Result<String, TransportError>;
}

/// Transport errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    /// Nothing is listening.
    #[error("Tally is not running or not accepting connections on port {port}")]
    TallyNotRunning {
        /// Port tried.
        port: u16,
    },
    /// HTTP error status.
    #[error("Tally answered with HTTP {status}")]
    Http {
        /// Status code.
        status: u16,
    },
    /// The request timed out.
    #[error("request to Tally timed out after {seconds} s")]
    Timeout {
        /// Seconds waited.
        seconds: u64,
    },
    /// Any other I/O problem.
    #[error("connection error: {0}")]
    Io(String),
}
