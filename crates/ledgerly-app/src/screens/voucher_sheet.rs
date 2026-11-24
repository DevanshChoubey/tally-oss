//! The voucher sheet of the mock (`design/mock-v2/index.html`, `.vsheet`, `.vhead`, `.fld`, `.ro`,
//! `table.lines`, `.vfoot`, `.taxbox`, `.lockbar`, `.ta`; lines 233-271): one card with no padding
//! of its own, a head of four equal columns, a line table whose cells hold inputs, a foot with a
//! stack on the left and a 340 point box on the right, and the type-ahead list that floats under
//! the field it belongs to.
//!
//! The sales, purchase, payment and receipt screens draw to the same sheet, so the pieces live
//! here once. Drawing only: nothing here reads the books or works out a figure (rule C4); the
//! caller hands in text that is already formatted.

#![allow(
    clippy::float_arithmetic,
    reason = "layout arithmetic in points; no figure of the books is computed here"
)]

use egui::{
    Align, Align2, Area, Color32, CornerRadius, FontId, Frame, Id, InnerResponse, Layout, Margin,
    Order, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};

use crate::theme::{
    RADIUS_M, RADIUS_S, RADIUS_XS, TEXT_BODY, TEXT_CAP, TEXT_H2, TEXT_INPUT, TEXT_META,
    border_width, shadow_1, shadow_2,
};
use crate::ui::icon::{self, Icon};
use crate::ui::keycap::keycap;
use crate::ui::prov::{ProvKind, prov};
use crate::ui::{bold_font, kind, para_text, tokens};

/// Height of a boxed input and of a read-only value (`input{height:32px}`, `.ro{height:32px}`).
pub const FIELD_HEIGHT: f32 = 32.0;
/// Height of an input inside a line table cell (`table.lines td input{height:30px}`).
pub const CELL_INPUT_HEIGHT: f32 = 30.0;
/// Padding around the head and the foot (`.vhead{padding:16px}`).
pub const PAD: f32 = 16.0;
/// [`PAD`] as a margin.
const PAD_I: i8 = 16;
/// Gap between the head's columns (`gap:12px 16px`).
pub const COLUMN_GAP: f32 = 16.0;
/// Gap between the head's rows.
pub const ROW_GAP: f32 = 12.0;
/// Width of the foot's right box (`.vfoot{grid-template-columns:1fr 340px}`).
pub const FOOT_BOX_WIDTH: f32 = 340.0;
/// Height of a line table's heading line: 8 + 17 + 8 and its rule (`table.lines th`).
pub const HEAD_LINE_HEIGHT: f32 = 34.0;
/// Height of a line table's body line: 4 + 30 + 4 and its rule (`table.lines td`).
pub const BODY_LINE_HEIGHT: f32 = 39.0;
/// Horizontal padding of a line table cell (`padding:4px 8px`).
const CELL_PAD_X: f32 = 8.0;
/// Vertical padding of a line table cell.
const CELL_PAD_Y: f32 = 4.0;
/// Inner padding of an input (`input{padding:0 10px}`).
const INPUT_PAD_X: i8 = 10;
/// Vertical padding of the foot (`.vfoot{padding:14px 16px}`).
const FOOT_PAD_Y: f32 = 14.0;

/// The width of a column that spans `span` of the head's four columns.
#[must_use]
pub fn span_width(total: f32, span: u8) -> f32 {
    let unit = (total - 3.0 * COLUMN_GAP) / 4.0;
    unit * f32::from(span) + COLUMN_GAP * f32::from(span.saturating_sub(1))
}

/// The sheet: a card with no padding (`.card.vsheet`).
pub fn sheet<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let t = tokens(ui);
    let k = kind(ui);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
}

/// A rule across `rect`'s full width at its bottom, in the border colour.
pub fn rule_under(ui: &Ui, rect: Rect) {
    let t = tokens(ui);
    let bw = border_width(kind(ui));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - bw / 2.0,
        Stroke::new(bw, t.border),
    );
}

/// The lock bar at the top of the sheet (`.lockbar`): the lock icon, `bold` in bold, then `rest`.
pub fn lock_bar(ui: &mut Ui, bold: &str, rest: &str) {
    let t = tokens(ui);
    let width = ui.available_width();
    let top = CornerRadius {
        nw: RADIUS_M - 1,
        ne: RADIUS_M - 1,
        sw: 0,
        se: 0,
    };
    let shown = Frame::new()
        .fill(t.bad_soft)
        .corner_radius(top)
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(width - 32.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                icon::icon(ui, Icon::Lock, t.bad);
                let mut job = egui::text::LayoutJob::default();
                let line = crate::ui::line_height(TEXT_META);
                let bold_font = bold_font(ui, TEXT_META);
                job.append(
                    bold,
                    0.0,
                    egui::TextFormat {
                        font_id: bold_font,
                        color: t.bad,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                job.append(
                    &format!(" {rest}"),
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(TEXT_META),
                        color: t.bad,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                ui.add(egui::Label::new(job).wrap());
            });
        });
    rule_under(ui, shown.response.rect);
}

/// The head: padding all round and a rule under it (`.vhead`). `add` lays out the rows.
pub fn head<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui, f32) -> R) -> R {
    let width = ui.available_width();
    let inner = width - 2.0 * PAD;
    let shown = Frame::new()
        .inner_margin(Margin::same(PAD_I))
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing = vec2(COLUMN_GAP, ROW_GAP);
            add(ui, inner)
        });
    rule_under(ui, shown.response.rect);
    shown.inner
}

/// One row of the head: its cells sit side by side, tops aligned.
pub fn head_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = COLUMN_GAP;
        add(ui)
    })
    .inner
}

/// One cell of the head, `span` columns wide out of `total`: a caption, then the value. `caption`
/// draws the caption line; `body` draws the value. The response's rect is the whole cell.
pub fn fld<R>(
    ui: &mut Ui,
    total: f32,
    span: u8,
    caption: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.vertical(|ui| {
        ui.set_width(span_width(total, span));
        ui.spacing_mut().item_spacing.y = 4.0;
        caption(ui);
        body(ui)
    })
}

/// A caption in the secondary ink (`.fld label`), with an optional keycap after it.
pub fn caption(ui: &mut Ui, text: &str, key: Option<&str>) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(para_text(text, TEXT_META, t.text_2));
        if let Some(key) = key {
            keycap(ui, key);
        }
    });
}

/// A read-only value as high as an input (`.ro`).
pub fn read_only<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, FIELD_HEIGHT),
        Layout::left_to_right(Align::Center),
        add,
    )
    .inner
}

/// A hint line under a field (`.fld .hint`).
pub fn hint(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.label(para_text(text, TEXT_META, t.text_3));
}

/// How an input looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A box with a border (`input`).
    Boxed,
    /// Bare until the pointer or the keyboard is on it (`table.lines td input`).
    Cell,
}

/// What an [`input`] needs besides its text.
#[derive(Debug, Clone, Copy)]
pub struct InputSpec<'a> {
    /// The egui id, so the keyboard can be put on the field.
    pub id: Id,
    /// What an empty field says.
    pub hint: &'a str,
    /// How it looks.
    pub look: Look,
    /// Right-aligned text, for a number (`input.num`).
    pub numeric: bool,
    /// Outlined in the error colour (`input.invalid`).
    pub invalid: bool,
    /// Width in points; `None` fills what is left of the line.
    pub width: Option<f32>,
}

/// A one-line input drawn to the mock: 32 points tall (30 in a table cell), a border in the strong
/// border colour, 14 point text, the accent border and ring while it has the keyboard.
pub fn input(ui: &mut Ui, text: &mut String, spec: &InputSpec<'_>) -> Response {
    let t = tokens(ui);
    let k = kind(ui);
    let bw = border_width(k);
    let width = spec.width.unwrap_or_else(|| ui.available_width());
    let height = match spec.look {
        Look::Boxed => FIELD_HEIGHT,
        Look::Cell => CELL_INPUT_HEIGHT,
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let focused = ui.memory(|m| m.has_focus(spec.id));
    let hovered = ui.rect_contains_pointer(rect);
    let (fill, edge) = match (spec.look, focused, hovered) {
        (_, _, _) if spec.invalid => (t.surface, t.bad),
        (_, true, _) => (t.surface, t.accent),
        (Look::Boxed, false, _) => (t.surface, t.border_strong),
        (Look::Cell, false, true) => (Color32::TRANSPARENT, t.border),
        (Look::Cell, false, false) => (Color32::TRANSPARENT, Color32::TRANSPARENT),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    painter.rect_stroke(rect, RADIUS_S, Stroke::new(bw, edge), StrokeKind::Inside);
    if focused || spec.invalid {
        let ring = if spec.invalid { t.bad } else { t.accent };
        painter.rect_stroke(rect, RADIUS_S, Stroke::new(2.0, ring), StrokeKind::Outside);
    }
    let align = if spec.numeric {
        Align2::RIGHT_CENTER
    } else {
        Align2::LEFT_CENTER
    };
    let inner = rect.shrink2(vec2(f32::from(INPUT_PAD_X), 0.0));
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    cell.add(
        egui::TextEdit::singleline(text)
            .id(spec.id)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .font(FontId::proportional(TEXT_INPUT))
            .text_color(t.text)
            .hint_text(RichText::new(spec.hint).size(TEXT_INPUT).color(t.text_3))
            .align(align)
            .min_size(inner.size())
            .desired_width(inner.width()),
    )
}

/// One column of a line table.
#[derive(Debug, Clone, Copy)]
pub struct Col {
    /// The heading.
    pub label: &'static str,
    /// Width in points, or `None` for a share of what the fixed columns leave.
    pub width: Option<f32>,
    /// Figures: the heading and the cells are right-aligned (`th.num`).
    pub numeric: bool,
}

impl Col {
    /// A text column of `width` points.
    #[must_use]
    pub const fn text(label: &'static str, width: f32) -> Self {
        Self {
            label,
            width: Some(width),
            numeric: false,
        }
    }

    /// A column of figures, `width` points wide.
    #[must_use]
    pub const fn num(label: &'static str, width: f32) -> Self {
        Self {
            label,
            width: Some(width),
            numeric: true,
        }
    }

    /// A text column that takes what the others leave.
    #[must_use]
    pub const fn rest(label: &'static str) -> Self {
        Self {
            label,
            width: None,
            numeric: false,
        }
    }
}

/// The widths of `cols` across `total` points: the fixed ones as given, the rest shared out, none
/// narrower than `min_rest`.
#[must_use]
pub fn column_widths(cols: &[Col], total: f32, min_rest: f32) -> Vec<f32> {
    let fixed: f32 = cols.iter().filter_map(|c| c.width).sum();
    let flexible = cols.iter().filter(|c| c.width.is_none()).count();
    let share = if flexible == 0 {
        0.0
    } else {
        let left = (total - fixed).max(0.0);
        #[expect(clippy::cast_precision_loss, reason = "a handful of columns")]
        let n = flexible as f32;
        (left / n).max(min_rest)
    };
    cols.iter().map(|c| c.width.unwrap_or(share)).collect()
}

/// The line table (`table.lines`): a heading line on `surface-2`, then `rows` lines, each with a
/// rule under it and the accent tint when `active(row)`. `cell` draws one cell into a child
/// `Ui` that is already laid out left to right (right to left for a numeric column).
///
/// Returns the rect of each column's heading, so a test can see the order.
pub fn lines(
    ui: &mut Ui,
    cols: &[Col],
    rows: usize,
    active: impl Fn(usize) -> bool,
    mut cell: impl FnMut(&mut Ui, usize, usize),
) -> Vec<Rect> {
    let t = tokens(ui);
    let total = ui.available_width();
    let widths = column_widths(cols, total, 120.0);
    let mut heads = Vec::with_capacity(cols.len());
    let (head_rect, _) = ui.allocate_exact_size(vec2(total, HEAD_LINE_HEIGHT), Sense::hover());
    ui.painter().rect_filled(head_rect, 0.0, t.surface_2);
    rule_under(ui, head_rect);
    let mut x = head_rect.left();
    for (col, w) in cols.iter().zip(&widths) {
        let rect = Rect::from_min_size(pos2(x, head_rect.top()), vec2(*w, HEAD_LINE_HEIGHT));
        let (pos, anchor) = if col.numeric {
            (
                pos2(rect.right() - CELL_PAD_X, rect.center().y),
                Align2::RIGHT_CENTER,
            )
        } else {
            (
                pos2(rect.left() + CELL_PAD_X, rect.center().y),
                Align2::LEFT_CENTER,
            )
        };
        ui.painter()
            .text(pos, anchor, col.label, bold_font(ui, TEXT_META), t.text_3);
        heads.push(rect);
        x += *w;
    }
    for row in 0..rows {
        let (line, _) = ui.allocate_exact_size(vec2(total, BODY_LINE_HEIGHT), Sense::hover());
        if active(row) {
            ui.painter().rect_filled(line, 0.0, t.accent_soft);
        }
        rule_under(ui, line);
        let mut x = line.left();
        for (c, w) in widths.iter().enumerate() {
            let at = Rect::from_min_size(pos2(x, line.top()), vec2(*w, BODY_LINE_HEIGHT - 1.0));
            let inner = at.shrink2(vec2(CELL_PAD_X, CELL_PAD_Y));
            let layout = if cols.get(c).is_some_and(|col| col.numeric) {
                Layout::right_to_left(Align::Center)
            } else {
                Layout::left_to_right(Align::Center)
            };
            let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(layout));
            cell(&mut child, row, c);
            x += *w;
        }
    }
    heads
}

/// The foot: padding, a stack on the left and a box of [`FOOT_BOX_WIDTH`] on the right (`.vfoot`).
/// `add` draws into the left column and the right one.
pub fn foot(ui: &mut Ui, add: impl FnOnce(&mut Ui, &mut Ui)) {
    let width = ui.available_width();
    let inner = width - 2.0 * PAD;
    let origin = ui.cursor().min + vec2(PAD, FOOT_PAD_Y);
    let left_width = inner - FOOT_BOX_WIDTH - PAD;
    let tall = 10_000.0;
    let left_rect = Rect::from_min_size(origin, vec2(left_width, tall));
    let right_rect = Rect::from_min_size(
        pos2(origin.x + left_width + PAD, origin.y),
        vec2(FOOT_BOX_WIDTH, tall),
    );
    let mut left = ui.new_child(
        UiBuilder::new()
            .max_rect(left_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    left.spacing_mut().item_spacing.y = ROW_GAP;
    let mut right = ui.new_child(
        UiBuilder::new()
            .max_rect(right_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    add(&mut left, &mut right);
    let bottom = left.min_rect().bottom().max(right.min_rect().bottom()) + FOOT_PAD_Y;
    let top = ui.cursor().min;
    ui.allocate_rect(
        Rect::from_min_max(top, pos2(top.x + width, bottom)),
        Sense::hover(),
    );
}

/// One row of the tax box: the label on the left, the amount on the right (`.taxrow`).
fn tax_row(ui: &mut Ui, label: &str, value: &str) {
    let t = tokens(ui);
    let width = ui.available_width();
    Frame::new()
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            ui.horizontal(|ui| {
                ui.label(para_text(label, TEXT_BODY, t.text_2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(para_text(value, TEXT_BODY, t.text));
                });
            });
        });
}

/// The total row of the tax box: bold, on `surface-2`, with a rule above it in the strong ink.
pub struct Total<'a> {
    /// The label, such as "Invoice total".
    pub label: &'a str,
    /// The amount, already formatted.
    pub value: &'a str,
    /// The provenance tag after the label.
    pub tag: Option<ProvKind>,
}

/// The tax box (`.taxbox`): `rows` as (label, amount), then the `total` row when there is one.
pub fn taxbox(ui: &mut Ui, rows: &[(String, String)], total: Option<&Total<'_>>) {
    let t = tokens(ui);
    let k = kind(ui);
    let bw = border_width(k);
    Frame::new()
        .stroke(Stroke::new(bw, t.border))
        .corner_radius(RADIUS_S)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let width = ui.available_width();
            ui.set_width(width);
            for (label, value) in rows {
                tax_row(ui, label, value);
            }
            if let Some(total) = total {
                let h2 = bold_font(ui, TEXT_H2);
                let shown = Frame::new()
                    .fill(t.surface_2)
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.set_width(width - 24.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(total.label).font(h2.clone()).color(t.text));
                            if let Some(tag) = total.tag {
                                prov(ui, tag);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(total.value).font(h2).color(t.text));
                            });
                        });
                    });
                let r = shown.response.rect;
                ui.painter()
                    .hline(r.x_range(), r.top() + bw / 2.0, Stroke::new(bw, t.text_2));
            }
        });
}

/// The total in words under the tax box (`.words`).
pub fn words(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.add_space(8.0);
    ui.label(para_text(text, TEXT_META, t.text_2));
}

/// How many entries the type-ahead shows before it scrolls (`TA.list.slice(0,8)`).
const LIST_ROWS: f32 = 8.0;
/// Height of one entry: 6 + 19 + 6.
const LIST_ROW_HEIGHT: f32 = 31.0;
/// The type-ahead's least and greatest width (`.ta{min-width:340px;max-width:460px}`).
const LIST_WIDTH: (f32, f32) = (340.0, 460.0);

/// One entry of the type-ahead: the name, and the line after it in the quiet ink (`.opt .s`).
#[must_use]
pub fn split_entry(label: &str) -> (&str, &str) {
    // The screens join an entry's parts with three spaces.
    label.split_once("   ").unwrap_or((label, ""))
}

/// The type-ahead list (`.ta`): a floating card under `below`, with a caption, one line per
/// entry (the picked one on `accent-soft`) and, when `create` is given, the create line.
/// Returns where the card was drawn and the entry clicked, if any.
pub fn type_ahead(
    ui: &Ui,
    id: Id,
    below: Rect,
    caption_text: &str,
    entries: &[String],
    picked: usize,
) -> (Rect, Option<usize>) {
    let t = tokens(ui);
    let k = kind(ui);
    let mut clicked = None;
    let width = below.width().clamp(LIST_WIDTH.0, LIST_WIDTH.1);
    // No fade: a list that fades in or out leaves a ghost of itself over the field below it for a
    // few frames after the keyboard has moved on.
    let shown = Area::new(id)
        .order(Order::Foreground)
        .fade_in(false)
        .fixed_pos(pos2(below.left(), below.bottom() + 4.0))
        .constrain(true)
        .show(ui.ctx(), |ui| {
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(border_width(k), t.border_strong))
                .corner_radius(RADIUS_M)
                .shadow(shadow_2(k))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(width - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        RichText::new(caption_text)
                            .font(bold_font(ui, TEXT_CAP))
                            .color(t.text_3),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(LIST_ROWS * LIST_ROW_HEIGHT)
                        .min_scrolled_height(LIST_ROWS * LIST_ROW_HEIGHT)
                        .show(ui, |ui| {
                            for (j, label) in entries.iter().enumerate() {
                                if entry(ui, label, j == picked).clicked() {
                                    clicked = Some(j);
                                }
                            }
                        });
                });
        });
    (shown.response.rect, clicked)
}

/// One entry of the type-ahead.
fn entry(ui: &mut Ui, label: &str, hi: bool) -> Response {
    let t = tokens(ui);
    let (name, rest) = split_entry(label);
    let width = ui.available_width();
    // Click-only: an entry is not a Tab stop, so Tab leaves the field for the next one. Up, Down
    // and Enter move and pick the highlighted entry instead.
    let (row_rect, response) = ui.allocate_exact_size(vec2(width, LIST_ROW_HEIGHT), Sense::click());
    if hi {
        ui.painter().rect_filled(row_rect, RADIUS_XS, t.accent_soft);
        response.scroll_to_me(None);
    } else if response.hovered() {
        ui.painter().rect_filled(row_rect, RADIUS_XS, t.hover);
    }
    let ink = if hi { t.accent_ink } else { t.text };
    let quiet = if hi { t.accent_ink } else { t.text_3 };
    ui.painter().text(
        pos2(row_rect.left() + 8.0, row_rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(TEXT_BODY),
        ink,
    );
    if !rest.is_empty() {
        ui.painter().text(
            pos2(row_rect.right() - 8.0, row_rect.center().y),
            Align2::RIGHT_CENTER,
            rest,
            FontId::proportional(TEXT_META),
            quiet,
        );
    }
    crate::ui::a11y::name_selected(ui, &response, egui::Role::ListItem, hi, label);
    response
}
