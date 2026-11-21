//! Plain-words entry (mock `quick-entry`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Plain-words entry screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct QuickEntryScreen;

impl Screen for QuickEntryScreen {
    fn title(&self) -> String {
        "Plain-words entry".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(ui, &self.title(), "quick-entry.proposal, quick-entry.hinglish, quick-entry.unknown-ledger, quick-entry.loading and quick-entry.rejected");
        ScreenAction::Stay
    }
}
