//! Staging and mapping to events. **Card B3** implements this module.
//!
//! Frozen interface (plan card B3):
//!
//! ```text
//! pub fn map_to_events(masters: &[TallyMaster], vouchers: &[TVoucher]) -> Result<Vec<Event>, MapError>;
//! ```
