//! Bill-wise allocation state and validation. **Card E7** implements this module.
//!
//! Frozen interface (plan card E7):
//!
//! ```text
//! pub struct OpenBill { pub ledger: LedgerId, pub name: String, pub date: Date, pub due: Option<Date>, pub open: SignedAmount }
//! pub fn open_bills(books: &Books, ledger: LedgerId, as_of: Date) -> Result<Vec<OpenBill>, EngineError>;
//! pub fn validate_bill_refs(v: &Voucher, books: &Books) -> Result<(), EngineError>;
//! ```
