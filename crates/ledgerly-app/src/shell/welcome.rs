//! The screen a tab shows until a feature card supplies a real one.

use crate::{AppCtx, Screen, ScreenAction};

/// Root screen of a new tab. It shows no figures.
#[derive(Debug, Default, Clone, Copy)]
pub struct WelcomeScreen;

impl Screen for WelcomeScreen {
    fn title(&self) -> String {
        "Home".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        ui.heading("Ledgerly");
        if ctx.books.is_some() {
            ui.label("A company is open. Its screens appear here as they are built.");
        } else {
            ui.label("No company is open.");
        }
        ScreenAction::Stay
    }
}
