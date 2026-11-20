//! JSON-RPC over stdio. **Card P1** implements this module.
//!
//! Frozen interface (plan card P1):
//!
//! ```text
//! pub fn serve(api: &Api, input: impl BufRead, output: impl Write) -> Result<(), McpError>;
//! ```
