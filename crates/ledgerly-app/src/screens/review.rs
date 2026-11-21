//! Review queue (mock `review`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Review queue screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct ReviewScreen;

impl Screen for ReviewScreen {
    fn title(&self) -> String {
        "Review queue".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(ui, &self.title(), "review.default and review.empty");
        ScreenAction::Stay
    }
}
