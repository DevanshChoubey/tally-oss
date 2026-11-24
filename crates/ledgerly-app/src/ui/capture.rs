//! Helpers for the capture harness (`examples/ui_snapshots.rs`).

/// Parks the pointer off the window for this frame: nothing under it is hovered, so a hover
/// state left by the real pointer (or by an earlier shot's click) never reaches a capture.
pub fn park_pointer(raw: &mut egui::RawInput) {
    raw.events.insert(0, egui::Event::PointerGone);
}
