//! UPI payment payload for printed invoices. **Card K5** implements this module.
//!
//! Frozen interface (plan card K5):
//!
//! ```text
//! pub fn upi_payload(payee_vpa: &str, payee_name: &str, amount: Paise, note: &str) -> Result<String, BankError>;
//! ```
