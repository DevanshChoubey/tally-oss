//! Case format with provenance. **Card Q1** implements this module.
//!
//! Frozen interface (plan card Q1):
//!
//! ```text
//! pub enum Provenance { Observed { tool: String, version: String, date: Date }, Derived { reference: String, working: String }, Pending }
//! ```
