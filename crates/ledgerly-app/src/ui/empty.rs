//! Empty state: a title, one sentence on what belongs here, and the action with its key
//! (`.empty` in the mock).

use egui::{RichText, Ui};

use super::{bold_font, button::ButtonKind, button::button, para_text, tokens};
use crate::theme::{SPACE_4, SPACE_6, TEXT_BODY, TEXT_H2};

/// Draws an empty state. `action` is the label and key of the one thing to do next, such as
/// `("New sale", "F8")`. Returns `true` in the frame the action is clicked.
pub fn empty_state(ui: &mut Ui, title: &str, sentence: &str, action: Option<(&str, &str)>) -> bool {
    let t = tokens(ui);
    let h2 = bold_font(ui, TEXT_H2);
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(SPACE_6);
        ui.label(RichText::new(title).font(h2).color(t.text));
        ui.add_space(SPACE_4 / 2.0);
        ui.label(para_text(sentence, TEXT_BODY, t.text_2));
        if let Some((label, key)) = action {
            ui.add_space(SPACE_4);
            clicked = button(ui, label, Some(key), ButtonKind::Primary).clicked();
        }
        ui.add_space(SPACE_6);
    });
    clicked
}
