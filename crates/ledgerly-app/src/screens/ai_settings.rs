//! AI and privacy settings (mock `ai-settings`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The AI and privacy screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct AiSettingsScreen;

impl Screen for AiSettingsScreen {
    fn title(&self) -> String {
        "AI and privacy".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(
            ui,
            &self.title(),
            "ai-settings.local, ai-settings.cloud and ai-settings.off",
        );
        ScreenAction::Stay
    }
}
