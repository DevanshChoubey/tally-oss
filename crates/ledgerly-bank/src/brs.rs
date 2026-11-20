//! Bank reconciliation statement. **Card K3** implements this module.
//!
//! Frozen interface (plan card K3):
//!
//! ```text
//! pub fn brs(books: &Books, ledger: LedgerId, as_of: Date, statements: &[BankStatement]) -> Result<Outcome<Brs>, BankError>;
//! ```
