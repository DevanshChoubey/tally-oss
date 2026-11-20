//! Voucher parsing. **Card X3** implements this module.
//!
//! Frozen interface (plan card X3):
//!
//! ```text
//! pub struct TVoucher { .. }   // guid, alter_id, type name, date, number, ledger entries, bills, inventory, batches, extra
//! pub fn parse_vouchers(env: &Envelope) -> Result<(Vec<TVoucher>, Vec<XmlWarning>), TallyXmlError>;
//! ```
