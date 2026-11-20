//! Voucher number allocation. **Card E4** implements this module.
//!
//! Frozen interface (plan card E4):
//!
//! ```text
//! pub fn next_number(vt: &VoucherType, series: Option<&str>, date: Date, books: &Books) -> Result<String, EngineError>;
//! ```
