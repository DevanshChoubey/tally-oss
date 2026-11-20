//! HTTP transport. **Card B1** implements this module.
//!
//! Frozen interface (plan card B1):
//!
//! ```text
//! pub struct HttpTransport { .. }
//! impl HttpTransport { pub fn new(host: &str, port: u16) -> Self; }
//! impl TallyTransport for HttpTransport { .. }
//! ```
