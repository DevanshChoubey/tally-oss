//! Chip: a bordered control in the top bar, such as the company, the date or the AI tier
//! (`.chip` in the mock: 30 px high, 10 px side padding, 6 px gaps, meta-size text in `text-2`,
//! a bold part in `text`, an optional icon first and keycap last).

use egui::{Color32, FontId, Response, Role, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use super::a11y;
use super::icon::{self, ICON_SIZE, Icon};
use super::{bold_font, keycap, kind, tokens};
use crate::theme::{RADIUS_S, TEXT_BODY, TEXT_META, border_width};

/// Height of a chip, in points (the mock's 30 px).
pub const CHIP_HEIGHT: f32 = 30.0;
/// Height of the search chip, in points (`.search{height:32px}`).
pub const SEARCH_HEIGHT: f32 = 32.0;
/// Widest the search chip grows, in points (`.search{max-width:420px}`).
pub const SEARCH_MAX_WIDTH: f32 = 420.0;
/// Narrowest the search chip is drawn, in points: the icon and a few words. The top bar leaves
/// out chips rather than squeeze the search chip below this.
pub const SEARCH_MIN_WIDTH: f32 = 120.0;
/// Room the search words need beside the keycap, in points, before the keycap is dropped.
const SEARCH_MIN_WORDS: f32 = 60.0;
/// Horizontal padding inside a chip.
const PAD_X: f32 = 10.0;
/// Gap between a chip's parts (`gap:6px`).
const GAP: f32 = 6.0;

/// What a chip shows, left to right: an icon, a bold part, the plain words and a keycap.
#[derive(Debug, Clone, Copy, Default)]
pub struct Chip<'a> {
    /// The icon first, if any.
    pub icon: Option<Icon>,
    /// Words in the bold face and the text colour (`.chip b`).
    pub bold: Option<&'a str>,
    /// Words in the chip's ink.
    pub text: &'a str,
    /// A keycap last, if any.
    pub key: Option<&'a str>,
    /// Ink and border colour instead of `text-2` and `border` (the AI tier chip).
    pub tint: Option<Color32>,
}

/// Draws a chip reading `label` and returns its response. The hover fill appears on hover.
pub fn chip(ui: &mut Ui, label: &str) -> Response {
    chip_parts(
        ui,
        Chip {
            text: label,
            ..Chip::default()
        },
    )
}

/// Draws a chip of `parts` and returns its response.
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "laying out at most four parts"
)]
pub fn chip_parts(ui: &mut Ui, parts: Chip<'_>) -> Response {
    let width = chip_width(ui, parts);
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let ink = parts.tint.unwrap_or(t.text_2);
    let edge = parts.tint.unwrap_or(t.border);
    let bold_id = bold_font(ui, TEXT_META);
    let painter = ui.painter().clone();
    let bold_galley = parts
        .bold
        .map(|b| painter.layout_no_wrap(b.to_owned(), bold_id, t.text));
    let text_galley = (!parts.text.is_empty()).then(|| {
        painter.layout_no_wrap(parts.text.to_owned(), FontId::proportional(TEXT_META), ink)
    });
    let key_w = parts.key.map(|k| keycap::width(ui, k));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CHIP_HEIGHT), Sense::click());
    a11y::name(
        ui,
        &response,
        Role::Button,
        &a11y::join(&[parts.bold.unwrap_or(""), parts.text]),
    );
    let fill = if response.hovered() {
        t.hover
    } else {
        t.surface
    };
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(rect, RADIUS_S, Stroke::new(bw, edge), StrokeKind::Inside);
    let mid = rect.center().y;
    let mut x = rect.min.x + PAD_X;
    if let Some(i) = parts.icon {
        let r = egui::Rect::from_min_size(pos2(x, mid - ICON_SIZE / 2.0), Vec2::splat(ICON_SIZE));
        icon::paint(&painter, r, i, ink);
        x += ICON_SIZE + GAP;
    }
    for galley in [bold_galley, text_galley].into_iter().flatten() {
        let w = galley.size().x;
        let top = mid - galley.size().y / 2.0;
        painter.galley(pos2(x, top), galley, ink);
        x += w + GAP;
    }
    if let (Some(k), Some(w)) = (parts.key, key_w) {
        let r = egui::Rect::from_min_size(
            pos2(x, mid - keycap::KEYCAP_HEIGHT / 2.0),
            vec2(w, keycap::KEYCAP_HEIGHT),
        );
        keycap::paint(&painter, r, k, t.surface_2, t.border, t.text_2);
    }
    a11y::focus_ring(ui, &response, rect);
    response
}

/// How wide [`chip_parts`] draws `parts`, in points, without drawing anything (the top bar uses
/// it to leave out a chip that would not fit).
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "adding at most four widths"
)]
pub fn chip_width(ui: &Ui, parts: Chip<'_>) -> f32 {
    let bold_id = bold_font(ui, TEXT_META);
    let measure = |text: &str, font: FontId| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
            .size()
            .x
    };
    let widths: Vec<f32> = [
        parts.icon.map(|_| ICON_SIZE),
        parts.bold.map(|b| measure(b, bold_id.clone())),
        (!parts.text.is_empty()).then(|| measure(parts.text, FontId::proportional(TEXT_META))),
        parts.key.map(|k| keycap::width(ui, k)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let gaps = GAP * widths.len().saturating_sub(1) as f32;
    2.0 * PAD_X + widths.iter().sum::<f32>() + gaps
}

/// The search chip (`.chip.search`): `width` points wide, the search icon and `text` at the left
/// in `text-3`, the keycap `key` at the right.
#[allow(clippy::float_arithmetic, reason = "laying out the parts")]
pub fn search_chip(ui: &mut Ui, width: f32, text: &str, key: &str) -> Response {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, SEARCH_HEIGHT), Sense::click());
    a11y::name(ui, &response, Role::Button, text);
    let fill = if response.hovered() {
        t.hover
    } else {
        t.surface
    };
    let key_w = keycap::width(ui, key);
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(
        rect,
        RADIUS_S,
        Stroke::new(bw, t.border),
        StrokeKind::Inside,
    );
    let mid = rect.center().y;
    let icon_rect = egui::Rect::from_min_size(
        pos2(rect.min.x + PAD_X, mid - ICON_SIZE / 2.0),
        Vec2::splat(ICON_SIZE),
    );
    icon::paint(painter, icon_rect, Icon::Search, t.text_3);
    let text_x = icon_rect.max.x + 8.0;
    // A narrow window: the words end in an ellipsis before the keycap, and the keycap goes
    // when even a few words would not fit beside it, so nothing runs past the chip.
    let show_key = rect.max.x - PAD_X - key_w - 8.0 - text_x >= SEARCH_MIN_WORDS;
    let words_end = if show_key {
        rect.max.x - PAD_X - key_w - 8.0
    } else {
        rect.max.x - PAD_X
    };
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::proportional(TEXT_BODY),
        t.text_3,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((words_end - text_x).max(0.0));
    let galley = painter.layout_job(job);
    painter.galley(pos2(text_x, mid - galley.size().y / 2.0), galley, t.text_3);
    if show_key {
        let key_rect = egui::Rect::from_min_size(
            pos2(
                rect.max.x - PAD_X - key_w,
                mid - keycap::KEYCAP_HEIGHT / 2.0,
            ),
            vec2(key_w, keycap::KEYCAP_HEIGHT),
        );
        keycap::paint(painter, key_rect, key, t.surface_2, t.border, t.text_2);
    }
    a11y::focus_ring(ui, &response, rect);
    response
}

/// A 32-point square icon button (`.iconbtn`): `text-2` ink, the hover fill and `text` ink on
/// hover. `name` is both the hover text and the accessible name, so the two cannot drift apart.
pub fn icon_button(ui: &mut Ui, which: Icon, name: &str) -> Response {
    let t = tokens(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SEARCH_HEIGHT), Sense::click());
    a11y::name(ui, &response, Role::Button, name);
    let response = response.on_hover_text(name);
    let ink = if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS_S, t.hover);
        t.text
    } else {
        t.text_2
    };
    let r = egui::Rect::from_center_size(rect.center(), Vec2::splat(ICON_SIZE));
    icon::paint(ui.painter(), r, which, ink);
    a11y::focus_ring(ui, &response, rect);
    response
}
