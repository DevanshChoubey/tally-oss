//! Ask your books (mock `ask`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Ask your books screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct AskScreen;

impl Screen for AskScreen {
    fn title(&self) -> String {
        "Ask your books".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(
            ui,
            &self.title(),
            "ask.result, ask.loading, ask.low-confidence, ask.unsupported and ask.offline-ai",
        );
        ScreenAction::Stay
    }
}
