//! Empty state: a title, one sentence on what belongs here, and the action with its key
//! (`.empty` in the mock).

use egui::{Label, RichText, Ui};

use super::icon::{Icon, icon_sized};
use super::{
    bold_font, button::ButtonKind, button::button, button::button_icon, para_text, tokens,
};
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

/// Draws an empty state like [`empty_state`], as the mock's `.empty` is built (index.html:168-171):
/// an optional 36 point icon in the tertiary colour over the title, the sentence in a column 420
/// wide, and a primary action that can carry its own icon. Returns `true` in the frame the
/// action is clicked.
pub fn empty_state_with_icon(
    ui: &mut Ui,
    glyph: Option<Icon>,
    title: &str,
    sentence: &str,
    action: Option<(Option<Icon>, &str, &str)>,
) -> bool {
    let t = tokens(ui);
    let h2 = bold_font(ui, TEXT_H2);
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(SPACE_6);
        if let Some(glyph) = glyph {
            icon_sized(ui, glyph, t.text_3, EMPTY_ICON);
            ui.add_space(10.0);
        }
        ui.label(RichText::new(title).font(h2).color(t.text));
        ui.add_space(SPACE_4 / 2.0);
        ui.scope(|ui| {
            ui.set_max_width(EMPTY_TEXT_WIDTH);
            ui.add(Label::new(para_text(sentence, TEXT_BODY, t.text_2)).wrap());
        });
        if let Some((which, label, key)) = action {
            ui.add_space(14.0);
            clicked = button_icon(ui, which, label, Some(key), ButtonKind::Primary).clicked();
        }
        ui.add_space(SPACE_6);
    });
    clicked
}

/// The icon's side in an empty state (`.empty .ico{width:36px}`).
const EMPTY_ICON: f32 = 36.0;
/// The widest the sentence runs (`.empty p{max-width:420px}`).
const EMPTY_TEXT_WIDTH: f32 = 420.0;
