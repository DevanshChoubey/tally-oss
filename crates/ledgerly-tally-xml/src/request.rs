//! Export request builders. **Card X1** implements this module.
//!
//! Frozen interface (plan card X1):
//!
//! ```text
//! pub fn list_companies_request() -> String;
//! pub fn masters_request(company: &str, collection: MasterCollection) -> String;
//! pub fn vouchers_request(company: &str, from: Date, to: Date) -> String;
//! ```
