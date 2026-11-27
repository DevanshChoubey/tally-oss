//! S22: a ledger name in Telugu script draws with real glyphs, not boxes.
//!
//! The owner's masters file has a party named in Telugu. Provenance: derived. Every character of
//! that name is in the Unicode Telugu block (U+0C00 to U+0C7F); the UI faces (Inter, Noto Sans,
//! Noto Sans Devanagari) have none of them, so a Telugu face must be in the fallback chain, regular
//! and bold.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{FontId, RawInput};
use ledgerly_app::theme::{self, ThemeKind};

#[test]
fn telugu_names_have_glyphs_in_regular_and_bold() {
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    ctx.run_ui(RawInput::default(), |_ui| {})
        .drop_without_applying_deltas();
    let name = "శ్రీ లక్ష్మి చీరలు";
    for font in [
        FontId::proportional(13.0),
        FontId::new(13.0, theme::bold_family()),
    ] {
        let missing: Vec<char> = ctx.fonts_mut(|f| {
            name.chars()
                .filter(|c| !c.is_whitespace() && !f.has_glyph(&font, *c))
                .collect()
        });
        assert!(missing.is_empty(), "{font:?} lacks {missing:?}");
    }
}
