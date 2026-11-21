//! Keyboard map (mock `keys`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Keyboard map screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct KeymapScreen;

impl Screen for KeymapScreen {
    fn title(&self) -> String {
        "Keyboard map".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(ui, &self.title(), "keys.classic and keys.standard");
        ScreenAction::Stay
    }
}
