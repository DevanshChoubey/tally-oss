//! Parsing dates typed in entry forms (`1-4`, `15/1`, `2026-05-07`). **Card C3** implements this module.
//!
//! Frozen interface (plan card C3):
//!
//! ```text
//! pub enum DateEntryError { .. }   // defined by C3 in this file
//! pub fn parse_date_entry(input: &str, fy: FinancialYear, today: Date) -> Result<Date, DateEntryError>;
//! ```
