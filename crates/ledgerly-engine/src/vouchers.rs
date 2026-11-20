//! Voucher post, alter, cancel and delete. **Card E3** implements this module.
//!
//! Frozen interface (plan card E3):
//!
//! ```text
//! pub fn post_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn cancel_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
