//! Rule pack format, loading and effective dating. **Card G1** implements this module.
//!
//! Frozen interface (plan card G1):
//!
//! ```text
//! pub struct RulePack { .. }   pub struct RuleSet { .. }   // defined by G1
//! pub fn load_pack(toml: &str) -> Result<RulePack, GstError>;
//! impl RulePack { pub fn for_time_of_supply(&self, date: Date) -> Outcome<RuleSet>; pub fn version(&self) -> &str; }
//! ```
