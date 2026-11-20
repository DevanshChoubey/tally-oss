//! Api facade. **Card W1** implements this module.
//!
//! Frozen interface (plan card W1):
//!
//! ```text
//! pub struct Api { .. }
//! impl Api { pub fn execute(&mut self, cmd: Command) -> Result<Outcome<CommandResult>, ApiError>; pub fn report(&self, p: &ReportParams) -> Result<Outcome<Report>, ApiError>; pub fn search(&self, q: &str) -> Vec<SearchHit>; }
//! ```
