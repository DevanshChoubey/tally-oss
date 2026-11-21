//! First-run company setup (mock `setup`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Company setup screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct SetupScreen;

impl Screen for SetupScreen {
    fn title(&self) -> String {
        "Company setup".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(
            ui,
            &self.title(),
            "setup.welcome, setup.company, setup.gstin-error, setup.features, setup.keyboard, setup.ai, setup.backup and setup.done",
        );
        ScreenAction::Stay
    }
}
