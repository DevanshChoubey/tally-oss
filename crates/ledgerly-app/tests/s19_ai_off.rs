//! Release 1.0.0 ships without the AI assistant. A company made through the setup wizard with
//! no changes must start with AI off, and the wizard must not store a tier that does nothing.
//!
//! Provenance: derived from the owner's release decision (1.0.0 ships without the AI assistant;
//! the AI screens show their AI-off state) and the mock's `setup.ai` and `ai-settings.off` states.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap"
)]

use ledgerly_api::AiTier;
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::screens::setup::form::{ai_notice, stored_ai_tier};
use ledgerly_core::{Clock, OffsetDateTime};

#[test]
fn a_new_wizard_has_ai_off_selected() {
    let screen = SetupScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH));
    assert_eq!(screen.ai_choice(), AiTier::Off);
}

#[test]
fn the_wizard_stores_ai_off_whatever_card_was_clicked() {
    for choice in [AiTier::Off, AiTier::Local, AiTier::Cloud] {
        assert_eq!(stored_ai_tier(choice), AiTier::Off, "{choice:?}");
    }
}

#[test]
fn local_and_cloud_say_on_the_step_that_they_are_not_in_this_release() {
    assert_eq!(ai_notice(AiTier::Off), None);
    for choice in [AiTier::Local, AiTier::Cloud] {
        let note = ai_notice(choice).unwrap_or_default();
        assert!(note.contains("not available in this release"), "{note}");
        assert!(note.contains("AI off"), "{note}");
    }
}
