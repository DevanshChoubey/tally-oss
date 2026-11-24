//! Tabs with a 2 px accent underline under the selected tab (`.tabs` in the mock).

use egui::{CornerRadius, FontId, Rect, Response, Role, Sense, Stroke, Ui, Vec2, pos2};

use super::{a11y, bold_font, keycap, kind, tokens};
use crate::theme::{TEXT_BODY, Tokens, border_width};

/// Height of a tab, in points.
const TAB_HEIGHT: f32 = 34.0;
/// Horizontal padding on each side of a tab label.
const TAB_PAD: f32 = 12.0;
/// Thickness of the underline of the selected tab.
const UNDERLINE: f32 = 2.0;
/// Space between a tab's label and its key (`.tabs button{gap:8px}`).
const KEY_GAP: f32 = 8.0;
/// Space under the rule before the content (`.tabs{margin-bottom:14px}`).
const RULE_GAP: f32 = 14.0;

/// What the tab strip reports this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsResponse {
    /// Index of the tab the user clicked this frame, if any.
    pub clicked: Option<usize>,
    /// The rectangle of each tab, in the order of the labels. Tests use these to click a tab.
    pub rects: Vec<Rect>,
}

/// Draws a row of tabs labelled `labels`, with the tab at `selected` underlined. Returns the tab
/// clicked this frame, if any; the caller decides what selecting it means.
pub fn tabs(ui: &mut Ui, labels: &[&str], selected: usize) -> TabsResponse {
    let items: Vec<(&str, Option<&str>)> = labels.iter().map(|l| (*l, None)).collect();
    ruled(ui, &items, selected)
}

/// Draws tabs like [`tabs`], each label followed by its key as a keycap (the mock's
/// `GSTR-1 ${kb('1')}`, index.html:1665), then the 14 point gap the mock leaves under the rule.
/// `items` are the label and key of each tab.
pub fn tabs_with_keys(ui: &mut Ui, items: &[(&str, &str)], selected: usize) -> TabsResponse {
    let items: Vec<(&str, Option<&str>)> = items.iter().map(|(l, k)| (*l, Some(*k))).collect();
    let out = ruled(ui, &items, selected);
    ui.add_space(RULE_GAP);
    out
}

/// The tab strip with the full-width rule under it.
fn ruled(ui: &mut Ui, items: &[(&str, Option<&str>)], selected: usize) -> TabsResponse {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (out, bottom) = strip(ui, items, Some(selected), &t);
    let clip = ui.max_rect();
    ui.painter().line_segment(
        [pos2(clip.min.x, bottom), pos2(clip.max.x, bottom)],
        Stroke::new(bw, t.border),
    );
    out
}

/// Draws the tabs without the rule under them, as the mock's tabs in a page head (index.html:
/// 1645, `.tabs` with `border:0`). `None` underlines no tab, for a view that is none of them.
pub fn tab_strip(ui: &mut Ui, labels: &[&str], selected: Option<usize>) -> TabsResponse {
    let t = tokens(ui);
    let items: Vec<(&str, Option<&str>)> = labels.iter().map(|l| (*l, None)).collect();
    strip(ui, &items, selected, &t).0
}

/// The width [`tab_strip`] takes for `labels`, measured with the selected (semibold) face so the
/// strip never comes out wider than this.
#[must_use]
pub fn tab_strip_width(ui: &Ui, labels: &[&str]) -> f32 {
    let font = bold_font(ui, TEXT_BODY);
    let gap = ui.spacing().item_spacing.x;
    let text: f32 = labels
        .iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap((*label).to_owned(), font.clone(), egui::Color32::BLACK)
                .size()
                .x
                + 2.0 * TAB_PAD
        })
        .sum();
    let gaps = gap * f32::from(u16::try_from(labels.len().saturating_sub(1)).unwrap_or(u16::MAX));
    text + gaps
}

/// Draws the tabs in a row and returns what was clicked and the row's bottom edge.
fn strip(
    ui: &mut Ui,
    items: &[(&str, Option<&str>)],
    selected: Option<usize>,
    t: &Tokens,
) -> (TabsResponse, f32) {
    let mut out = TabsResponse {
        clicked: None,
        rects: Vec::with_capacity(items.len()),
    };
    let row = ui.horizontal(|ui| {
        for (i, (label, key)) in items.iter().enumerate() {
            let (rect, response) = tab(ui, label, *key, selected == Some(i), t);
            if response.clicked() {
                out.clicked = Some(i);
            }
            out.rects.push(rect);
        }
    });
    (out, row.response.rect.bottom())
}

/// Draws one tab, with its key as a keycap after the label when given, and returns its rectangle
/// and response.
#[allow(
    clippy::float_arithmetic,
    reason = "placing the keycap after the label"
)]
fn tab(
    ui: &mut Ui,
    label: &str,
    key: Option<&str>,
    selected: bool,
    t: &Tokens,
) -> (Rect, Response) {
    let ink = if selected { t.accent_ink } else { t.text_2 };
    let font: FontId = if selected {
        bold_font(ui, TEXT_BODY)
    } else {
        FontId::proportional(TEXT_BODY)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, ink);
    let text_size = galley.size();
    let key_w = key.map_or(0.0, |k| KEY_GAP + keycap::width(ui, k));
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(text_size.x + key_w + 2.0 * TAB_PAD, TAB_HEIGHT),
        Sense::click(),
    );
    a11y::name_selected(ui, &response, Role::Tab, selected, label);
    let painter = ui.painter();
    let text_pos = pos2(rect.min.x + TAB_PAD, rect.center().y - text_size.y / 2.0);
    painter.galley(text_pos, galley, ink);
    if let Some(k) = key {
        let x = text_pos.x + text_size.x + KEY_GAP;
        let cap = Rect::from_min_size(
            pos2(x, rect.center().y - keycap::KEYCAP_HEIGHT / 2.0),
            Vec2::new(keycap::width(ui, k), keycap::KEYCAP_HEIGHT),
        );
        keycap::paint(painter, cap, k, t.surface_2, t.border, t.text_2);
    }
    if selected {
        let underline = Rect::from_min_max(pos2(rect.min.x, rect.max.y - UNDERLINE), rect.max);
        painter.rect_filled(underline, CornerRadius::ZERO, t.accent);
    }
    a11y::focus_ring(ui, &response, rect);
    (rect, response)
}
