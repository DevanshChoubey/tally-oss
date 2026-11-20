//! Cheque books, post-dated cheques and bounced cheques. **Card K4** implements this module.
//!
//! Frozen interface (plan card K4):
//!
//! ```text
//! pub struct ChequeBook { .. }
//! pub fn check_leaf(book: &ChequeBook, number: &str, books: &Books) -> Result<(), BankError>;
//! pub fn bounce_proposal(voucher: &Voucher, charges: Paise, charges_ledger: LedgerId, date: Date) -> Result<Voucher, BankError>;   // a reversing draft, not posted
//! ```
