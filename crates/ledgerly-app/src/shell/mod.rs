//! Window, theme switching, screen stack, breadcrumbs and tabs. **Card U1** implements this module.
//!
//! Frozen interface (plan card U1):
//!
//! ```text
//! pub struct Shell { .. }
//! impl eframe::App for Shell { .. }
//! pub fn run() -> eframe::Result<()>;   // called from main.rs (U1 also owns main.rs)
//! ```
