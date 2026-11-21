//! Drawing the palette and reading its keys. This is the inner layout only: the shell draws the
//! popup frame around it (`shell/mod.rs`), so the palette adds no second frame.

use egui::{
    Align, Align2, Context, FontId, Frame, Id, Key, Margin, Modifiers, Rect, Response, RichText,
    ScrollArea, Sense, Ui, pos2, vec2,
};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;

use super::scope::{Section, empty_scope_hint};
use super::{LEGEND, PLACEHOLDER, Palette, PaletteChoice, PaletteMode, PaletteScope, PaletteTarget};
use crate::theme::{RADIUS_S, SPACE_2, SPACE_3, TEXT_BODY, TEXT_CAP, TEXT_INPUT, TEXT_META};
use crate::ui::keycap::{self, KEYCAP_HEIGHT};
use crate::ui::{bold_font, tokens};

/// Width of the palette, in points (`.palette` in the mock, 640 px).
pub(super) const PALETTE_WIDTH: f32 = 640.0;
/// Most height the rows take before they scroll (`.res` in the mock, 420 px).
const RESULTS_HEIGHT: f32 = 420.0;
/// Height of a row with one line (a command with its key, or a row with no detail).
const ROW_ONE_LINE: f32 = 34.0;
/// Height of a row with a second line of detail.
const ROW_TWO_LINES: f32 = 46.0;
/// Inner left and right padding of a row, and of the key that sits at its right.
const ROW_PAD_X: f32 = 10.0;
/// Top padding of a row's first line.
const ROW_PAD_TOP: f32 = 7.0;
/// Distance from a row's top to its second line.
const SECOND_LINE_TOP: f32 = 24.0;
/// Horizontal padding of the query row, the legend row, the note and the footer, in points.
const EDGE_PAD: i8 = 14;
/// The id of the query box, so its cursor can be moved when a suggestion fills it.
const QUERY_ID: &str = "ledgerly-palette-query";

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

/// What the user clicked in the palette this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Click {
    /// A row, by its index in the results.
    Row(usize),
    /// The "Search vouchers for ..." suggestion under a query with no match.
    SearchVouchers,
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
    let id = Id::new(QUERY_ID);
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

        ui.set_width(PALETTE_WIDTH.min(ui.available_width()));
        if self.input_row(ui) {
            self.selected = 0;
            self.refresh();
        }
        legend_row(ui);
        ui.separator();
        let click = self.results_area(ui);
        self.note_line(ui);
        ui.separator();
        self.footer(ui);

        match click {
            Some(Click::Row(index)) => return self.accept_at(index),
            Some(Click::SearchVouchers) => {
                self.search_vouchers();
                move_cursor_to_end(ui.ctx(), self.query().chars().count());
                return None;
            }
            None => {}
        }
        let index = self.selected;
        choose_with.and_then(|replace| self.choose(index, replace))
    }

    /// The query box with the "Go to" label at its right. Returns whether the text changed.
    fn input_row(&mut self, ui: &mut Ui) -> bool {
        let t = tokens(ui);
        let go_width = ui
            .painter()
            .layout_no_wrap("Go to".to_owned(), FontId::proportional(TEXT_META), t.text_3)
            .size()
            .x;
        let focus = std::mem::take(&mut self.focus_pending);
        Frame::new()
            .inner_margin(Margin::symmetric(EDGE_PAD, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let width = ui.available_width() - go_width - SPACE_3;
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(Id::new(QUERY_ID))
                            .hint_text(PLACEHOLDER)
                            .frame(Frame::NONE)
                            .font(FontId::proportional(TEXT_INPUT))
                            .desired_width(width),
                    );
                    ui.label(RichText::new("Go to").size(TEXT_META).color(t.text_3));
                    if focus {
                        edit.request_focus();
                    }
                    edit.changed()
                })
                .inner
            })
            .inner
    }

    /// The rows, grouped under their headings, in a scroll area. Returns a click on a row or on a
    /// suggestion.
    fn results_area(&self, ui: &mut Ui) -> Option<Click> {
        let mut click = None;
        ScrollArea::vertical()
            .max_height(RESULTS_HEIGHT)
            .show(ui, |ui| {
                if self.shown.is_empty() {
                    click = self.empty_state(ui);
                    return;
                }
                let mut heading: Option<Section> = None;
                for (index, row) in self.shown.iter().enumerate() {
                    if heading != Some(row.section) {
                        section_heading(ui, row.section.label());
                        heading = Some(row.section);
                    }
                    let selected = index == self.selected;
                    let keycap = matches!(row.entry.target, PaletteTarget::Command(_));
                    let response = paint_line(
                        ui,
                        &row.entry.title,
                        &row.entry.detail,
                        keycap,
                        selected,
                    );
                    if selected {
                        response.scroll_to_me(Some(Align::Center));
                    }
                    if response.clicked() {
                        click = Some(Click::Row(index));
                    }
                }
            });
        click
    }

    /// What is drawn when no row is shown: the scope's hint when nothing is typed after the prefix,
    /// otherwise "No match" and, for an unprefixed query, a suggestion to search vouchers.
    fn empty_state(&self, ui: &mut Ui) -> Option<Click> {
        let (scope, body) = PaletteScope::of_query(&self.query);
        let body = body.trim();
        if body.is_empty() {
            if let Some(hint) = empty_scope_hint(scope) {
                ui.add_space(SPACE_2);
                ui.label(RichText::new(hint).size(TEXT_META).color(tokens(ui).text_3));
            }
            return None;
        }
        section_heading(ui, &format!("No match for \"{}\"", self.query.trim()));
        if scope != PaletteScope::All {
            return None;
        }
        let title = format!("Search vouchers for \"{body}\"");
        let response = paint_line(
            ui,
            &title,
            "Number, party, amount or date",
            false,
            false,
        );
        response.clicked().then_some(Click::SearchVouchers)
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

    /// The footer: the keys of [`Palette::key_hints`] that have a key, each in a keycap.
    fn footer(&self, ui: &mut Ui) {
        let t = tokens(ui);
        Frame::new()
            .fill(t.surface_2)
            .inner_margin(Margin::symmetric(EDGE_PAD, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for hint in self
                        .key_hints()
                        .iter()
                        .filter(|hint| !hint.keys.is_empty())
                    {
                        keycap::keycap(ui, &hint.keys);
                        ui.label(RichText::new(hint.label.as_str()).size(TEXT_META).color(t.text_3));
                        ui.add_space(SPACE_3);
                    }
                });
            });
    }

    /// Fills the query with `#` and the typed text, so the vouchers are searched.
    fn search_vouchers(&mut self) {
        let body = PaletteScope::of_query(&self.query).1.trim().to_owned();
        self.set_query(format!("#{body}"));
        self.focus_pending = true;
    }
}

/// The legend under the query box: each prefix and what it finds (`index.html:1138`).
fn legend_row(ui: &mut Ui) {
    let t = tokens(ui);
    Frame::new()
        .inner_margin(Margin::symmetric(EDGE_PAD, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (key, label) in LEGEND {
                    keycap::keycap(ui, key);
                    ui.label(RichText::new(label).size(TEXT_META).color(t.text_3));
                    ui.add_space(SPACE_2);
                }
            });
        });
}

/// A heading above a group of rows, in the caption size and the muted ink.
fn section_heading(ui: &mut Ui, label: &str) {
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_CAP);
    ui.add_space(SPACE_2);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD_X);
        ui.label(RichText::new(label).font(font).color(t.text_3));
    });
}

/// One clickable row: `title` on the first line; `detail` on a second line, or at the right as a
/// keycap when `keycap` is set (a command shows its key there). The selected row is filled with
/// the accent.
fn paint_line(
    ui: &mut Ui,
    title: &str,
    detail: &str,
    keycap: bool,
    selected: bool,
) -> Response {
    let key_at_right = keycap && !detail.is_empty();
    let two_lines = !key_at_right && !detail.is_empty();
    let height = if two_lines { ROW_TWO_LINES } else { ROW_ONE_LINE };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let t = tokens(ui);
    let key_width = keycap::width(ui, detail);
    let painter = ui.painter();
    let (title_ink, detail_ink) = if selected {
        (t.accent_ink, t.accent_ink)
    } else {
        (t.text, t.text_3)
    };
    if selected {
        painter.rect_filled(rect, RADIUS_S, t.accent_soft);
    }
    let top = rect.left_top() + vec2(ROW_PAD_X, ROW_PAD_TOP);
    painter.text(
        top,
        Align2::LEFT_TOP,
        title,
        FontId::proportional(TEXT_BODY),
        title_ink,
    );
    if two_lines {
        painter.text(
            top + vec2(0.0, SECOND_LINE_TOP),
            Align2::LEFT_TOP,
            detail,
            FontId::proportional(TEXT_META),
            detail_ink,
        );
    }
    if key_at_right {
        let key_rect = Rect::from_min_size(
            pos2(
                rect.right() - ROW_PAD_X - key_width,
                rect.center().y - KEYCAP_HEIGHT / 2.0,
            ),
            vec2(key_width, KEYCAP_HEIGHT),
        );
        keycap::paint(painter, key_rect, detail, t.surface_2, t.border, t.text_2);
    }
    response
}
