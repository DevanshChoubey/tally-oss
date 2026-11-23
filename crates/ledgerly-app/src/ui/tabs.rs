//! Tabs with a 2 px accent underline under the selected tab (`.tabs` in the mock).

use egui::{CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui, Vec2, pos2};

use super::{bold_font, kind, tokens};
use crate::theme::{TEXT_BODY, Tokens, border_width};

/// Height of a tab, in points.
const TAB_HEIGHT: f32 = 34.0;
/// Horizontal padding on each side of a tab label.
const TAB_PAD: f32 = 12.0;
/// Thickness of the underline of the selected tab.
const UNDERLINE: f32 = 2.0;

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
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    let (out, bottom) = strip(ui, labels, Some(selected), &t);
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
    strip(ui, labels, selected, &t).0
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
fn strip(ui: &mut Ui, labels: &[&str], selected: Option<usize>, t: &Tokens) -> (TabsResponse, f32) {
    let mut out = TabsResponse {
        clicked: None,
        rects: Vec::with_capacity(labels.len()),
    };
    let row = ui.horizontal(|ui| {
        for (i, label) in labels.iter().enumerate() {
            let (rect, response) = tab(ui, label, selected == Some(i), t);
            if response.clicked() {
                out.clicked = Some(i);
            }
            out.rects.push(rect);
        }
    });
    (out, row.response.rect.bottom())
}

/// Draws one tab and returns its rectangle and response.
fn tab(ui: &mut Ui, label: &str, selected: bool, t: &Tokens) -> (Rect, Response) {
    let ink = if selected { t.accent_ink } else { t.text_2 };
    let font: FontId = if selected {
        bold_font(ui, TEXT_BODY)
    } else {
        FontId::proportional(TEXT_BODY)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, ink);
    let text_size = galley.size();
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(text_size.x + 2.0 * TAB_PAD, TAB_HEIGHT),
        Sense::click(),
    );
    let painter = ui.painter();
    let text_pos = pos2(rect.min.x + TAB_PAD, rect.center().y - text_size.y / 2.0);
    painter.galley(text_pos, galley, ink);
    if selected {
        let underline = Rect::from_min_max(pos2(rect.min.x, rect.max.y - UNDERLINE), rect.max);
        painter.rect_filled(underline, CornerRadius::ZERO, t.accent);
    }
    (rect, response)
}
