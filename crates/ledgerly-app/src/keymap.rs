//! Keymap with the classic preset; TOML round trip. **Card U2** implements this module.
//!
//! Frozen interface (plan card U2):
//!
//! ```text
//! pub enum Action { .. }   pub struct Keymap { .. }
//! pub fn classic() -> Keymap;   // loaded from assets/keymap-classic.toml
//! pub fn from_toml(s: &str) -> Result<Keymap, KeymapError>;  pub fn to_toml(k: &Keymap) -> String;
//! ```
