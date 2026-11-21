//! Purchase from a photo of a bill (mock `purchase-bill`). A stub until it is built: see [`super::draw_stub`].

use crate::{AppCtx, Screen, ScreenAction};

/// The Purchase from bill photo screen. It renders its title and the not-built line only.
#[derive(Debug, Default)]
pub struct PurchaseBillScreen;

impl Screen for PurchaseBillScreen {
    fn title(&self) -> String {
        "Purchase from bill photo".to_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        super::draw_stub(ui, &self.title(), "purchase-bill.reading, purchase-bill.proposal, purchase-bill.low-confidence, purchase-bill.mismatch, purchase-bill.duplicate, purchase-bill.offline-ai, purchase-bill.cloud-consent, purchase-bill.error and purchase-bill.accepted");
        ScreenAction::Stay
    }
}
