//! Inline note for information, success, warning, error or an AI remark (`.note` in the mock).
//! Used for lock bars, offline cues and errors. The words in the text say what happened; the
//! colour only repeats it.

use egui::{Color32, Frame, Margin, Response, Stroke, Ui};

use super::icon::{Icon, icon_sized};
use super::{kind, para, tokens};
use crate::theme::{RADIUS_S, TEXT_META, ThemeKind, Tokens, border_width};

/// The kind of a note, which picks its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Plain information on a surface.
    Info,
    /// Something went right.
    Ok,
    /// Something needs attention.
    Warn,
    /// Something is wrong.
    Bad,
    /// A remark from the AI, which is a suggestion and never a figure.
    Ai,
}

/// Background, text and outline colours of a note of `kind`. The outline is transparent except
/// for information notes, and in high contrast, where every note is outlined.
#[must_use]
pub fn colors(kind_of: NoteKind, t: &Tokens) -> (Color32, Color32, Color32) {
    match kind_of {
        NoteKind::Info => (t.surface_2, t.text_2, t.border),
        NoteKind::Ok => (t.ok_soft, t.ok, Color32::TRANSPARENT),
        NoteKind::Warn => (t.warn_soft, t.warn, Color32::TRANSPARENT),
        NoteKind::Bad => (t.bad_soft, t.bad, Color32::TRANSPARENT),
        NoteKind::Ai => (t.ai_soft, t.ai_ink, Color32::TRANSPARENT),
    }
}

/// Horizontal padding inside a note.
const PAD_X: i8 = 12;
/// Vertical padding inside a note.
const PAD_Y: i8 = 10;

/// Draws a note of `kind_of` saying `text`.
pub fn note(ui: &mut Ui, kind_of: NoteKind, text: &str) -> Response {
    let t = tokens(ui);
    let k = kind(ui);
    let (bg, fg, edge) = colors(kind_of, &t);
    let outline = if k == ThemeKind::HighContrast {
        Stroke::new(border_width(k), fg)
    } else {
        Stroke::new(border_width(k), edge)
    };
    Frame::new()
        .fill(bg)
        .stroke(outline)
        .corner_radius(RADIUS_S)
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            para(ui, text, TEXT_META, fg);
        })
        .response
}

/// Size of the icon at the left of an alert note, in points.
const ALERT_ICON_SIZE: f32 = 16.0;

/// Gap between the icon and the text of an alert note (`.note{gap:10px}`).
const ALERT_GAP: f32 = 10.0;

/// A note of `kind_of` with `icon` at its left and `add` drawing the content beside it, over the
/// whole width (the mock's `.note` holding an icon and a `div`). `add` receives the note's text
/// colour. The content is the caller's: this draws the frame, the icon and the layout only.
pub fn note_with_icon(
    ui: &mut Ui,
    kind_of: NoteKind,
    icon: Icon,
    add: impl FnOnce(&mut Ui, Color32),
) -> Response {
    let t = tokens(ui);
    let k = kind(ui);
    let (bg, fg, edge) = colors(kind_of, &t);
    let outline = if k == ThemeKind::HighContrast {
        Stroke::new(border_width(k), fg)
    } else {
        Stroke::new(border_width(k), edge)
    };
    Frame::new()
        .fill(bg)
        .stroke(outline)
        .corner_radius(RADIUS_S)
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = ALERT_GAP;
                icon_sized(ui, icon, fg, ALERT_ICON_SIZE);
                ui.vertical(|ui| add(ui, fg));
            });
        })
        .response
}
