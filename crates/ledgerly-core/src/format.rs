//! Indian number formatting and amounts in words. **Card C1** implements this module.
//!
//! Frozen interface (spec section 7; plan card C1):
//!
//! ```text
//! pub fn format_inr(amount: Paise) -> String;        // 1_234_567_00 -> "12,34,567.00"
//! pub fn amount_in_words(amount: Paise) -> String;   // "Rupees ... and Fifty Paise Only"
//! ```
