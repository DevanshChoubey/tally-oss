//! Drawing the palette and reading its keys. This is the inner layout only: the shell draws the
//! scrim and the panel around it (`shell/mod.rs`), at [`palette_rect`], so the palette adds no
//! second frame. Sizes follow the mock's `.palette` rules (`design/mock-v2/index.html:273-286`).

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::text_edit::TextEditState;
use egui::{
    Align, Align2, Color32, Context, CornerRadius, FontId, Frame, Id, Key, Margin, Modifiers, Rect,
    Response, RichText, ScrollArea, Sense, Ui, Vec2, pos2, vec2,
};

use super::scope::{Section, empty_scope_hint};
use super::{LEGEND, Palette, PaletteChoice, PaletteMode, PaletteScope, PaletteTarget, match_span};
use crate::shell::navigate::row_icon;
use crate::theme::{RADIUS_L, RADIUS_S, TEXT_BODY, TEXT_CAP, TEXT_META, border_width};
use crate::ui::icon::{self, ICON_SIZE, Icon};
use crate::ui::keycap::{self, KEYCAP_HEIGHT};
use crate::ui::{bold_font, kind, line_height, tokens};

/// Width of the palette, in points (`.palette{width:640px}`).
pub(super) const PALETTE_WIDTH: f32 = 640.0;
/// Distance from the window's top to the panel (`.scrim{padding-top:72px}`).
const TOP_GAP: f32 = 72.0;
/// Room the panel leaves across the window, both sides together
/// (`max-width:calc(100vw - 32px)`).
const SIDE_ROOM: f32 = 32.0;
/// Most height the rows take before they scroll (`.res{max-height:420px}`).
const RESULTS_HEIGHT: f32 = 420.0;
/// Padding around the rows (`.res{padding:6px}`).
const RESULTS_PAD: i8 = 6;
/// Left and right padding of a row (`.it{padding:8px 10px}`).
const ROW_PAD_X: f32 = 10.0;
/// Top and bottom padding of a row.
const ROW_PAD_Y: f32 = 8.0;
/// Gap between a row's icon, its words and its key (`.it{gap:10px}`).
const ROW_GAP: f32 = 10.0;
/// Size of the query's text (`.pin input{font-size:16px}`).
const QUERY_SIZE: f32 = 16.0;
/// Height of the query box (`.pin input{height:28px}`).
const QUERY_HEIGHT: f32 = 28.0;
/// Size of the search icon by the query (`.ico.l`).
const QUERY_ICON: f32 = 20.0;
/// Horizontal padding of the query row, the legend row, the note and the footer, in points.
const EDGE_PAD: i8 = 14;
/// The id of the query box, so its cursor can be moved when a suggestion fills it.
const QUERY_ID: &str = "ledgerly-palette-query";

/// Where the panel sits in a window whose area is `screen`: 640 points wide, or 32 fewer than the
/// window when that is narrower, centred, 72 points from the top, down to 16 above the bottom.
#[must_use]
#[allow(clippy::float_arithmetic, reason = "placing one rectangle")]
pub fn palette_rect(screen: Rect) -> Rect {
    let width = PALETTE_WIDTH.min(screen.width() - SIDE_ROOM).max(0.0);
    let top = screen.min.y + TOP_GAP;
    let bottom = (screen.max.y - SIDE_ROOM / 2.0).max(top);
    let left = screen.center().x - width / 2.0;
    Rect::from_min_max(pos2(left, top), pos2(left + width, bottom))
}

/// A key the palette reads this frame. Escape closes; the rest move or choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyPress {
    /// Moves the selection up one row.
    Up,
    /// Moves the selection down one row.
    Down,
    /// Chooses the selected row, in the palette's own mode.
    Enter,
    /// Chooses the selected row in the other mode (Shift+Enter).
    ShiftEnter,
    /// Closes the palette without choosing.
    Escape,
}

/// The other mode: a replace becomes an open on top, and the reverse.
pub(super) fn flipped(mode: PaletteMode) -> PaletteMode {
    match mode {
        PaletteMode::OnTop => PaletteMode::Replace,
        PaletteMode::Replace => PaletteMode::OnTop,
    }
}

/// Consumes the palette's keys from `ui`, in the order they are read: Escape, the arrows, then
/// Shift+Enter before Enter. egui's plain Enter also matches a Shift+Enter, so Shift+Enter is read
/// first.
fn take_keys(ui: &mut Ui) -> Vec<KeyPress> {
    ui.input_mut(|i| {
        let mut presses = Vec::new();
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            presses.push(KeyPress::Escape);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            presses.push(KeyPress::Down);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            presses.push(KeyPress::Up);
        }
        if i.consume_key(Modifiers::SHIFT, Key::Enter) {
            presses.push(KeyPress::ShiftEnter);
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            presses.push(KeyPress::Enter);
        }
        presses
    })
}

/// Moves the query box's cursor to the end of its text, after a suggestion has filled it.
fn move_cursor_to_end(ctx: &Context, chars: usize) {
    let id = Id::unique(QUERY_ID);
    if let Some(mut state) = TextEditState::load(ctx, id) {
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(chars))));
        state.store(ctx, id);
    }
}

impl Palette {
    /// Draws the palette in `ui` while it is open and handles its keys: Up and Down move, Enter
    /// chooses in the mode the palette was opened with, Shift+Enter chooses in the other mode, and
    /// Esc closes. Returns the choice on the frame it is made. The host decides where `ui` is (a
    /// floating area near the top of the window, for example), and draws the frame around it.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<PaletteChoice> {
        self.open?;
        let mut choose_with: Option<bool> = None;
        for press in take_keys(ui) {
            match press {
                KeyPress::Escape => {
                    self.close();
                    return None;
                }
                KeyPress::Up => self.move_selection(-1),
                KeyPress::Down => self.move_selection(1),
                KeyPress::Enter => choose_with = choose_with.or(Some(false)),
                KeyPress::ShiftEnter => choose_with = Some(true),
            }
        }

        ui.spacing_mut().item_spacing = Vec2::ZERO;
        if self.input_row(ui) {
            self.selected = 0;
            self.refresh();
        }
        rule(ui);
        legend_row(ui);
        rule(ui);
        let click = self.results_area(ui);
        self.note_line(ui);
        rule(ui);
        self.footer(ui);

        let index = self.selected;
        let choice = match click {
            Some(row) => self.accept_at(row),
            None => choose_with.and_then(|replace| self.choose(index, replace)),
        };
        if self.take_cursor_to_end() {
            move_cursor_to_end(ui.ctx(), self.query().chars().count());
        }
        choice
    }

    /// The search icon, the query box and the "Go to" label at its right (`.pin`). Returns whether
    /// the text changed.
    #[allow(clippy::float_arithmetic, reason = "sharing the row's width")]
    fn input_row(&mut self, ui: &mut Ui) -> bool {
        let t = tokens(ui);
        let go_width = ui
            .painter()
            .layout_no_wrap(
                self.tr.t("palette.go_to"),
                FontId::proportional(TEXT_META),
                t.text_3,
            )
            .size()
            .x;
        let focus = std::mem::take(&mut self.focus_pending);
        Frame::new()
            .inner_margin(Margin::symmetric(EDGE_PAD, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = ROW_GAP;
                    let (spot, _) =
                        ui.allocate_exact_size(vec2(QUERY_ICON, QUERY_HEIGHT), Sense::hover());
                    let glyph = Rect::from_center_size(spot.center(), Vec2::splat(QUERY_ICON));
                    icon::paint(ui.painter(), glyph, Icon::Search, t.text);
                    let width = ui.available_width() - go_width - ROW_GAP;
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(Id::unique(QUERY_ID))
                            .hint_text(
                                RichText::new(self.tr.t("palette.placeholder"))
                                    .size(QUERY_SIZE)
                                    .color(t.text_3),
                            )
                            .frame(Frame::NONE)
                            .margin(Margin::ZERO)
                            .font(FontId::proportional(QUERY_SIZE))
                            .text_color(t.text)
                            .align(Align2::LEFT_CENTER)
                            .min_size(vec2(0.0, QUERY_HEIGHT))
                            .desired_width(width),
                    );
                    ui.label(
                        RichText::new(self.tr.t("palette.go_to"))
                            .size(TEXT_META)
                            .color(t.text_3),
                    );
                    if focus {
                        edit.request_focus();
                    }
                    edit.changed()
                })
                .inner
            })
            .inner
    }

    /// The rows, grouped under their headings, in a scroll area. Returns the index of a clicked
    /// row.
    fn results_area(&self, ui: &mut Ui) -> Option<usize> {
        let mut click = None;
        let pad = f32::from(RESULTS_PAD);
        ScrollArea::vertical()
            .max_height(RESULTS_HEIGHT)
            .show(ui, |ui| {
                ui.add_space(pad);
                if self.shown.is_empty() {
                    self.empty_state(ui);
                    return;
                }
                let typed = PaletteScope::of_query(&self.query).1.trim();
                let mut heading: Option<Section> = None;
                for (index, row) in self.shown.iter().enumerate() {
                    if heading != Some(row.section) {
                        let label = match row.section {
                            Section::NoMatch => self
                                .tr
                                .tf("palette.no_match_for", &[("query", self.query.trim())]),
                            Section::Recent
                            | Section::Prefixes
                            | Section::Reports
                            | Section::Ledgers
                            | Section::Vouchers
                            | Section::Commands
                            | Section::Settings
                            | Section::Ask
                            | Section::Entry => row.section.label_in(&self.tr),
                        };
                        padded(ui, |ui| section_heading(ui, &label));
                        heading = Some(row.section);
                    }
                    let selected = index == self.selected;
                    let keycap = matches!(row.entry.target, PaletteTarget::Command(_));
                    let line = Line {
                        icon: row_icon(&row.entry.target),
                        title: &row.entry.title,
                        mark: match_span(&row.entry.title, typed),
                        detail: &row.entry.detail,
                        keycap,
                    };
                    let response = padded(ui, |ui| paint_line(ui, &line, selected));
                    if selected {
                        response.scroll_to_me(Some(Align::Center));
                    }
                    if response.clicked() {
                        click = Some(index);
                    }
                }
                ui.add_space(pad);
            });
        click
    }

    /// What is drawn when no row is shown: the scope's hint when nothing is typed after the prefix;
    /// "No match" for a prefixed query; nothing for a plain query whose search is still out (its
    /// no-match rows come once the search is over).
    fn empty_state(&self, ui: &mut Ui) {
        let (scope, body) = PaletteScope::of_query(&self.query);
        if body.trim().is_empty() {
            if let Some(hint) = empty_scope_hint(scope) {
                let ink = tokens(ui).text_3;
                padded(ui, |ui| {
                    ui.add_space(ROW_PAD_Y);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD_X);
                        ui.label(
                            RichText::new(hint)
                                .size(TEXT_META)
                                .line_height(Some(line_height(TEXT_META)))
                                .color(ink),
                        );
                    });
                    ui.add_space(ROW_PAD_Y);
                });
            }
            return;
        }
        if scope != PaletteScope::All {
            padded(ui, |ui| {
                section_heading(
                    ui,
                    &self
                        .tr
                        .tf("palette.no_match_for", &[("query", self.query.trim())]),
                );
            });
        }
    }

    /// The line under the rows, when the palette has one (for example, that no company is open).
    fn note_line(&self, ui: &mut Ui) {
        if let Some(note) = self.note() {
            let t = tokens(ui);
            Frame::new()
                .inner_margin(Margin::symmetric(EDGE_PAD, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(note).size(TEXT_META).color(t.text_2));
                });
        }
    }

    /// The footer (`.pfoot`): each of [`Palette::footer_hints`], its keys in keycaps 5 points
    /// apart and then its words, 14 points between hints, on the raised fill. Its lower corners
    /// follow the panel's.
    #[allow(
        clippy::float_arithmetic,
        reason = "the corner inside the panel's border"
    )]
    fn footer(&self, ui: &mut Ui) {
        let t = tokens(ui);
        let inner = f32::from(RADIUS_L) - border_width(kind(ui));
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a radius of a few points"
        )]
        let corner = inner.round().max(0.0) as u8;
        Frame::new()
            .fill(t.surface_2)
            .corner_radius(CornerRadius {
                nw: 0,
                ne: 0,
                sw: corner,
                se: corner,
            })
            .inner_margin(Margin::symmetric(EDGE_PAD, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (n, (keys, label)) in self.footer_hints().into_iter().enumerate() {
                        if n > 0 {
                            ui.add_space(14.0);
                        }
                        for key in keys {
                            keycap::keycap(ui, key);
                            ui.add_space(5.0);
                        }
                        ui.label(
                            RichText::new(label)
                                .size(TEXT_META)
                                .line_height(Some(line_height(TEXT_META)))
                                .color(t.text_3),
                        );
                    }
                });
            });
    }
}

/// A hairline across the panel in the border colour, between its parts.
fn rule(ui: &mut Ui) {
    let t = tokens(ui);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), border_width(kind(ui))),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0.0, t.border);
}

/// Draws `add` inset by the rows' padding at the left and right (`.res{padding:6px}`).
fn padded<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .inner_margin(Margin::symmetric(RESULTS_PAD, 0))
        .show(ui, add)
        .inner
}

/// The legend under the query box (`.modes`): each prefix in a keycap and what it finds, 6 points
/// between entries (`index.html:1137`).
fn legend_row(ui: &mut Ui) {
    let t = tokens(ui);
    Frame::new()
        .inner_margin(Margin::symmetric(EDGE_PAD, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (n, (key, label)) in LEGEND.into_iter().enumerate() {
                    if n > 0 {
                        ui.add_space(6.0);
                    }
                    keycap::keycap(ui, key);
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(label)
                            .size(TEXT_META)
                            .line_height(Some(line_height(TEXT_META)))
                            .color(t.text_3),
                    );
                }
            });
        });
}

/// A heading above a group of rows (`.sec`): caption size, semibold, muted, padded 8 10 4.
fn section_heading(ui: &mut Ui, label: &str) {
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_CAP);
    ui.add_space(ROW_PAD_Y);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD_X);
        ui.label(
            RichText::new(label)
                .font(font)
                .line_height(Some(line_height(TEXT_CAP)))
                .color(t.text_3),
        );
    });
    ui.add_space(4.0);
}

/// What one row shows.
struct Line<'a> {
    /// The icon first.
    icon: Icon,
    /// The first line.
    title: &'a str,
    /// The part of `title` the query matched, as byte offsets, drawn marked.
    mark: Option<(usize, usize)>,
    /// The second line, or the key at the right when `keycap` is set.
    detail: &'a str,
    /// Whether `detail` is a key, drawn in a keycap at the right (a command's key).
    keycap: bool,
}

/// One clickable row (`.it`): the icon, then `title` with `detail` on a second line in the muted
/// meta size, or `detail` at the right in a keycap for a command. Padded 8 10, 10 points between
/// parts; the selected row is filled with the soft accent and inked with the accent.
#[allow(clippy::float_arithmetic, reason = "laying out one row")]
fn paint_line(ui: &mut Ui, line: &Line<'_>, selected: bool) -> Response {
    let key_at_right = line.keycap && !line.detail.is_empty();
    let two_lines = !key_at_right && !line.detail.is_empty();
    let title_h = line_height(TEXT_BODY);
    let detail_h = line_height(TEXT_META);
    let words_h = if two_lines {
        title_h + detail_h
    } else {
        title_h
    };
    let height = 2.0 * ROW_PAD_Y + words_h.max(KEYCAP_HEIGHT);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let t = tokens(ui);
    let key_width = keycap::width(ui, line.detail);
    let painter = ui.painter();
    let (title_ink, detail_ink) = if selected {
        (t.accent_ink, t.accent_ink)
    } else {
        (t.text, t.text_3)
    };
    if selected {
        painter.rect_filled(rect, RADIUS_S, t.accent_soft);
    }
    let mid = rect.center().y;
    let glyph = Rect::from_min_size(
        pos2(rect.left() + ROW_PAD_X, mid - ICON_SIZE / 2.0),
        Vec2::splat(ICON_SIZE),
    );
    icon::paint(painter, glyph, line.icon, title_ink);
    let x = glyph.right() + ROW_GAP;
    let top = mid - words_h / 2.0;
    let mark_ink = Mark {
        fill: t.accent_soft,
        ink: t.accent_ink,
    };
    paint_title(painter, pos2(x, top), title_h, line, title_ink, mark_ink);
    if two_lines {
        paint_centred(
            painter,
            pos2(x, top + title_h),
            detail_h,
            line.detail,
            TEXT_META,
            detail_ink,
        );
    }
    if key_at_right {
        let key_rect = Rect::from_min_size(
            pos2(
                rect.right() - ROW_PAD_X - key_width,
                mid - KEYCAP_HEIGHT / 2.0,
            ),
            vec2(key_width, KEYCAP_HEIGHT),
        );
        keycap::paint(
            painter,
            key_rect,
            line.detail,
            t.surface_2,
            t.border,
            t.text_2,
        );
    }
    response
}

/// The colours of the marked part of a title (`mark` in the mock: the soft accent behind the
/// accent ink).
#[derive(Clone, Copy)]
struct Mark {
    /// The fill behind the marked letters.
    fill: Color32,
    /// The ink of the marked letters.
    ink: Color32,
}

/// Paints a row's title from `left_top`, centred in a line `line_h` points high, with the part
/// the query matched marked.
#[allow(clippy::float_arithmetic, reason = "centring one line")]
fn paint_title(
    painter: &egui::Painter,
    left_top: egui::Pos2,
    line_h: f32,
    line: &Line<'_>,
    ink: Color32,
    mark: Mark,
) {
    let font = FontId::proportional(TEXT_BODY);
    let plain = TextFormat::simple(font.clone(), ink);
    let marked = TextFormat {
        background: mark.fill,
        color: mark.ink,
        ..TextFormat::simple(font, mark.ink)
    };
    let mut job = LayoutJob::default();
    let title = line.title;
    let parts = line.mark.and_then(|(start, end)| {
        Some((
            title.get(..start)?,
            title.get(start..end)?,
            title.get(end..)?,
        ))
    });
    match parts {
        Some((before, hit, after)) => {
            job.append(before, 0.0, plain.clone());
            job.append(hit, 0.0, marked);
            job.append(after, 0.0, plain);
        }
        None => job.append(title, 0.0, plain),
    }
    let galley = painter.layout_job(job);
    let top = left_top.y + (line_h - galley.size().y) / 2.0;
    painter.galley(pos2(left_top.x, top), galley, ink);
}

/// Paints `text` at `size` from `left_top`, centred in a line `line_h` points high.
#[allow(clippy::float_arithmetic, reason = "centring one line")]
fn paint_centred(
    painter: &egui::Painter,
    left_top: egui::Pos2,
    line_h: f32,
    text: &str,
    size: f32,
    ink: Color32,
) {
    painter.text(
        pos2(left_top.x, left_top.y + line_h / 2.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        ink,
    );
}
