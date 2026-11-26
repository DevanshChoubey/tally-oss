//! The release switch for the AI assistant.
//!
//! Ledgerly 1.0.0 ships without the AI assistant (owner decision of 2026-11-26). The AI code stays
//! in the app, behind [`AI_IN_THIS_RELEASE`]. While it is `false`:
//!
//! * every company is shown with AI **Off**, whatever tier its file stores (a file saved by an
//!   older build with the Local tier still opens, and shows Off);
//! * the AI settings screen shows the Local and Cloud choices disabled and tagged "Not in this
//!   release";
//! * the sidebar, the command palette and the search box carry no route to the AI-only screens
//!   (Plain-words entry, Ask your books, bill photo).
//!
//! A later release turns the assistant on by changing the one constant below.

use crate::AiTier;
use std::cell::Cell;

/// Whether this release includes the AI assistant. This is the one switch: set it to `true` in the
/// release that ships the assistant.
pub const AI_IN_THIS_RELEASE: bool = false;

thread_local! {
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Whether the AI assistant is part of this build, as the UI thread sees it.
///
/// This is [`AI_IN_THIS_RELEASE`] unless the calling thread was given another answer by
/// [`set_ai_enabled`].
pub fn ai_enabled() -> bool {
    OVERRIDE.with(Cell::get).unwrap_or(AI_IN_THIS_RELEASE)
}

/// Overrides [`ai_enabled`] for the calling thread only. Tests that exercise the AI screens call
/// this with `true`; the override never reaches other threads, so tests that pin the 1.0.0 screens
/// keep running with the switch off.
pub fn set_ai_enabled(on: bool) {
    OVERRIDE.with(|cell| cell.set(Some(on)));
}

/// The tier the UI acts on: what the company file stores when the assistant is part of the release,
/// and `Off` for every company, stored tier or not, when it is not.
pub fn effective_tier(stored: Option<AiTier>) -> Option<AiTier> {
    if ai_enabled() {
        stored
    } else {
        Some(AiTier::Off)
    }
}
