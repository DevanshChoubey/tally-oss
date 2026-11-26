//! Option card: one choice of a set, drawn as the mock's `.opt-card` (index.html:319-323): the
//! icon, then a column with the bold title and the muted text, and any keys or note below. The
//! chosen card has the accent border and the accent-soft fill (`aria-checked="true"`).
//!
//! The cards of a row are drawn side by side, in equal columns and at one height, as the mock's
//! `.grid` stretches them ([`option_grid`]).

use egui::{Frame, Id, Margin, Rect, Response, RichText, Role, Sense, Stroke, Ui};
use std::borrow::Cow;

use super::a11y;
use super::icon::{Icon, icon};
use super::keycap::keycap;
use super::{bold_font, kind, para, tokens};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, ThemeKind, border_width};

/// Padding inside a card, left and right (`.opt-card{padding:12px 14px}`).
const PAD_X: i8 = 14;
/// Padding inside a card, top and bottom.
const PAD_Y: i8 = 12;
/// Space between the icon and the text column (`gap:12px`).
const ICON_GAP: f32 = 12.0;
/// Space between cards of a grid (`.grid{gap:12px}`).
pub const GRID_GAP: f32 = 12.0;
/// The inset accent shadow inside the chosen card's border (`box-shadow:inset 0 0 0 1px
/// var(--accent)`). The chosen card's edge is the theme's border width plus this
/// ([`selected_stroke`]).
pub const SELECTED_INSET: f32 = 1.0;

/// The chosen card's edge in `kind`: the accent border at the theme's width (`var(--bw)`) and
/// the 1 px inset accent shadow, so 2 px, or 3 px in high contrast.
#[must_use]
#[allow(clippy::float_arithmetic, reason = "a border and an inset, added")]
pub fn selected_stroke(kind: ThemeKind) -> f32 {
    border_width(kind) + SELECTED_INSET
}

/// What one card shows.
#[derive(Debug, Clone)]
pub struct OptionCard<'a> {
    /// The icon at the card's left.
    pub glyph: Icon,
    /// The bold title.
    pub title: Cow<'a, str>,
    /// The muted text under the title.
    pub body: Cow<'a, str>,
    /// Whether this is the chosen card.
    pub selected: bool,
    /// Whether a click chooses the card. A card that cannot be chosen is drawn at full strength
    /// and says why in its `note`.
    pub enabled: bool,
    /// A badge after the title, such as "Current".
    pub badge: Option<Cow<'a, str>>,
    /// Keys shown as keycaps under the text (`<p class="meta">` with `kb()` in the mock).
    pub keys: &'a [&'a str],
    /// A muted line at the bottom, such as why the card cannot be chosen.
    pub note: Option<Cow<'a, str>>,
}

impl<'a> OptionCard<'a> {
    /// An enabled, unchosen card with no badge, keys or note.
    #[must_use]
    pub fn new(glyph: Icon, title: impl Into<Cow<'a, str>>, body: impl Into<Cow<'a, str>>) -> Self {
        Self {
            glyph,
            title: title.into(),
            body: body.into(),
            selected: false,
            enabled: true,
            badge: None,
            keys: &[],
            note: None,
        }
    }

    /// The same card, chosen or not.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The same card that cannot be chosen, with the reason as its note.
    #[must_use]
    pub fn unavailable(mut self, reason: impl Into<Cow<'a, str>>) -> Self {
        self.enabled = false;
        self.note = Some(reason.into());
        self
    }

    /// The same card with a badge after its title.
    #[must_use]
    pub fn badge(mut self, badge: Option<impl Into<Cow<'a, str>>>) -> Self {
        self.badge = badge.map(Into::into);
        self
    }

    /// The same card with keys shown as keycaps under its text.
    #[must_use]
    pub fn keys(mut self, keys: &'a [&'a str]) -> Self {
        self.keys = keys;
        self
    }
}

/// Where a card was drawn, and its click.
#[derive(Debug, Clone)]
pub struct OptionCardResponse {
    /// The click on the whole card. Only hover is sensed on a card that cannot be chosen.
    pub response: Response,
    /// The card's outline.
    pub card: Rect,
    /// The icon.
    pub icon: Rect,
    /// The title.
    pub title: Rect,
}

/// Draws one card filling the width of `ui`, sensing clicks under `id`, at least `min_height` tall (the outline included).
/// Returns where it was drawn and the height its content needs, outline included.
pub fn option_card(
    ui: &mut Ui,
    id: Id,
    card: &OptionCard<'_>,
    min_height: f32,
) -> (OptionCardResponse, f32) {
    let t = tokens(ui);
    let k = kind(ui);
    let (fill, stroke) = if card.selected {
        (t.accent_soft, Stroke::new(selected_stroke(k), t.accent))
    } else {
        (t.surface, Stroke::new(border_width(k), t.border))
    };
    #[expect(
        clippy::float_arithmetic,
        reason = "the card's padding and edge, both sides"
    )]
    let frame_extra = 2.0 * (f32::from(PAD_Y) + stroke.width);
    let mut icon_rect = Rect::NOTHING;
    let mut title_rect = Rect::NOTHING;
    let mut natural = 0.0;
    let inner = Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Set before the content: egui measures a minimum height from the cursor.
            #[expect(
                clippy::float_arithmetic,
                reason = "the outline's height less the frame"
            )]
            let content_min = (min_height - frame_extra).max(0.0);
            ui.set_min_height(content_min);
            let content = ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    // `.opt-card .ico{margin-top:2px}`.
                    ui.add_space(2.0);
                    icon_rect = icon(ui, card.glyph, t.text_2).rect;
                });
                ui.add_space(ICON_GAP);
                title_rect = ui.vertical(|ui| text_column(ui, card)).inner;
            });
            natural = content.response.rect.height();
        });
    let sense = if card.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let rect = inner.response.rect;
    let response = ui.interact(rect, id, sense);
    a11y::name_selected(
        ui,
        &response,
        Role::RadioButton,
        card.selected,
        &a11y::join(&[&card.title, &card.body]),
    );
    a11y::focus_ring(ui, &response, rect);
    #[expect(
        clippy::float_arithmetic,
        reason = "the content's height plus the frame"
    )]
    let needed = natural + frame_extra;
    (
        OptionCardResponse {
            response,
            card: rect,
            icon: icon_rect,
            title: title_rect,
        },
        needed,
    )
}

/// Draws `cards`, sensing clicks under ids made from `id`, in rows of `columns` equal columns, every card of a row as tall as its tallest
/// (the mock's `.grid.g2` and `.grid.g3`). Returns each card's response, in order.
pub fn option_grid(
    ui: &mut Ui,
    id: Id,
    columns: usize,
    cards: &[OptionCard<'_>],
) -> Vec<OptionCardResponse> {
    let columns = columns.max(1);
    let base = id;
    let mut out = Vec::with_capacity(cards.len());
    for (row_n, row) in cards.chunks(columns).enumerate() {
        if row_n > 0 {
            ui.add_space(GRID_GAP);
        }
        // The row's height is the tallest content of the last frame, so the cards come out level
        // from the second frame on.
        let height_id = base.with(("row-height", row_n));
        let min_height = ui
            .ctx()
            .data(|d| d.get_temp::<f32>(height_id))
            .unwrap_or(0.0);
        let mut tallest = 0.0_f32;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = GRID_GAP;
            ui.columns(columns, |cols| {
                for (n, (card, col)) in row.iter().zip(cols.iter_mut()).enumerate() {
                    let (response, needed) =
                        option_card(col, base.with((row_n, n)), card, min_height);
                    tallest = tallest.max(needed);
                    out.push(response);
                }
            });
        });
        #[expect(clippy::float_arithmetic, reason = "how far the row's height moved")]
        let moved = (tallest - min_height).abs();
        if moved > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(height_id, tallest));
            ui.ctx().request_repaint();
        }
    }
    out
}

/// The text column of a card: the bold title (and badge), the muted text, any keys and note.
/// Returns where the title was drawn.
fn text_column(ui: &mut Ui, card: &OptionCard<'_>) -> Rect {
    let t = tokens(ui);
    ui.spacing_mut().item_spacing.y = 2.0;
    let title = ui
        .horizontal(|ui| {
            let title = ui
                .add(egui::Label::new(
                    RichText::new(&*card.title)
                        .font(bold_font(ui, TEXT_BODY))
                        .color(t.text),
                ))
                .rect;
            if let Some(text) = card.badge.as_deref() {
                super::badge::badge(ui, super::badge::BadgeKind::Accent, text);
            }
            title
        })
        .inner;
    para(ui, &card.body, TEXT_META, t.text_2);
    if !card.keys.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for key in card.keys {
                keycap(ui, key);
            }
        });
    }
    if let Some(note) = card.note.as_deref() {
        ui.add_space(4.0);
        para(ui, note, TEXT_META, t.text_3);
    }
    title
}
