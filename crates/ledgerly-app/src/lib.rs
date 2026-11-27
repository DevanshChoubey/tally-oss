//! Ledgerly desktop app (egui). The only crate that depends on UI crates (rule C1).
//!
//! Floats are allowed here for layout only; money is always `Paise` from `ledgerly-api`.
#![allow(
    clippy::float_arithmetic,
    reason = "egui layout uses f32 points; no money is a float"
)]

mod screen;

pub mod i18n;
pub mod keymap;
pub mod ocr_setup;
pub mod palette;
pub mod recent;
pub mod release;
pub mod screens;
pub mod shell;
pub mod theme;
pub mod ui;
pub mod worker;

pub use screen::{AiTier, AppCtx, KeyHint, Screen, ScreenAction};
