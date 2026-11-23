//! Page head: the title, a one-line purpose under it, and the page's actions on the right
//! (`.phead` in the mock). There is no breadcrumb here; the shell draws the breadcrumbs.

use egui::{Align, Layout, Rect, RichText, Ui};

use super::{bold_font, para_text, tokens};
use crate::theme::{TEXT_BODY, TEXT_H1};

/// Draws the page head, then [`HEAD_GAP`] of space: `title` in the page-title size, `sub` below
/// it when given, and `actions` laid out from the right edge. Returns where the head was drawn
/// (without the gap).
///
/// Right-to-left layout adds the first widget at the far right. Call `actions` with the primary
/// action first, or the buttons come out in reverse.
pub fn page_head(
    ui: &mut Ui,
    title: &str,
    sub: Option<&str>,
    actions: impl FnOnce(&mut Ui),
) -> Rect {
    let t = tokens(ui);
    let sub = sub.map(|sub| {
        move |ui: &mut Ui| {
            ui.label(para_text(sub, TEXT_BODY, t.text_2));
        }
    });
    head(ui, title, sub, actions)
}

/// [`page_head`] with a subtitle the caller draws, for a subtitle with keycaps in it (the day
/// book's "F2 another date", index.html:1644). `sub` draws below the title, after the same gap.
pub fn page_head_with(
    ui: &mut Ui,
    title: &str,
    sub: impl FnOnce(&mut Ui),
    actions: impl FnOnce(&mut Ui),
) -> Rect {
    head(ui, title, Some(sub), actions)
}

/// The space between the page head and what follows it (index.html:121, `.phead`).
pub const HEAD_GAP: f32 = 16.0;

/// The page head with an optional subtitle drawer; returns where the head was drawn.
fn head(
    ui: &mut Ui,
    title: &str,
    sub: Option<impl FnOnce(&mut Ui)>,
    actions: impl FnOnce(&mut Ui),
) -> Rect {
    let t = tokens(ui);
    let h1 = bold_font(ui, TEXT_H1);
    let drawn = ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(h1).color(t.text));
            if let Some(sub) = sub {
                ui.add_space(3.0);
                sub(ui);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), actions);
    });
    // index.html:121: `.phead` has a 16 px bottom margin, so what follows starts 16 points below.
    let gap = (HEAD_GAP - ui.spacing().item_spacing.y).max(0.0);
    ui.add_space(gap);
    drawn.response.rect
}
