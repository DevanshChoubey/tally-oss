//! Money arithmetic helpers. **Card C2** implements this module.
//!
//! Frozen interface (plan card C2):
//!
//! ```text
//! pub fn sum<I: IntoIterator<Item = Paise>>(amounts: I) -> Result<Paise, MoneyError>;
//! pub fn percent_of(base: Paise, percent: Rate, rule: RoundingRule) -> Result<Paise, MoneyError>;
//! pub fn split_evenly(total: Paise, parts: u32) -> Result<Vec<Paise>, MoneyError>; // remainder to the first parts
//! ```
