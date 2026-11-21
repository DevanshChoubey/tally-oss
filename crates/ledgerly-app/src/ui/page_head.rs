//! Page head: the title, a one-line purpose under it, and the page's actions on the right
//! (`.phead` in the mock). There is no breadcrumb here; the shell draws the breadcrumbs.

use egui::{Align, Layout, RichText, Ui};

use super::{bold_font, tokens};
use crate::theme::{TEXT_BODY, TEXT_H1};

/// Draws the page head: `title` in the page-title size, `sub` below it when given, and
/// `actions` laid out from the right edge.
///
/// Right-to-left layout adds the first widget at the far right. Call `actions` with the primary
/// action first, or the buttons come out in reverse.
pub fn page_head(ui: &mut Ui, title: &str, sub: Option<&str>, actions: impl FnOnce(&mut Ui)) {
    let t = tokens(ui);
    let h1 = bold_font(ui, TEXT_H1);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(h1).color(t.text));
            if let Some(sub) = sub {
                ui.label(RichText::new(sub).size(TEXT_BODY).color(t.text_2));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), actions);
    });
}
