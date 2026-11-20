//! Company discovery. **Card B1** implements this module.
//!
//! Frozen interface (plan card B1):
//!
//! ```text
//! pub fn list_companies(t: &dyn TallyTransport) -> Result<Vec<String>, BridgeError>;   // BridgeError defined by B1 here
//! ```
