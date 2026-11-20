//! Imported statements and the balance check. **Card K1** implements this module.
//!
//! Frozen interface (plan card K1):
//!
//! ```text
//! pub struct BankStatement { .. }   pub enum StatementError { BalanceMismatch { .. }, RunningBalance { row: u32, .. }, Duplicate { file_hash: String }, .. }
//! pub fn check_statement(raw: RawStatement, file_bytes: &[u8], ledger: LedgerId, known_hashes: &[String]) -> Result<BankStatement, StatementError>;
//! ```
