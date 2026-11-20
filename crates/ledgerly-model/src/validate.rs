//! Store-free voucher invariants (spec 5, invariants 1, 2, 6). **Card M4** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn validate_voucher(v: &Voucher, masters: &Masters, company: &Company) -> Result<(), VoucherError>;
//! ```
