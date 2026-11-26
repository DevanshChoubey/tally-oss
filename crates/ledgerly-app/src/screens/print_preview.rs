//! The print preview (card S14): the pages of a PDF as the print engine laid them out, and the
//! panel that saves them.
//!
//! The pages are the engine's own (`ledgerly_api::PrintedDocument`, from the same render that
//! wrote the PDF): this draws each text, rule and fill where the engine put it and lays out
//! nothing itself. The mock's state is `profit-and-loss.print-preview` (index.html:2255): the
//! paper at the left, a card at the right with the paper, the page count and the destination,
//! and the Save as PDF and Close buttons. Every report and invoice that prints uses it.

use egui::{
    Color32, CursorIcon, FontId, Key, Layout, Modifiers, Rect, Role, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use ledgerly_api::{PageItem, PageLayout, PageSelection, Paper, PrintSetup, PrintedDocument};

use crate::theme::{RADIUS_XS, TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::card::card;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note_with_icon};
use crate::ui::{a11y, bold_font, para, para_text, tokens};

/// Width of the card at the right (the mock's `grid-template-columns:1fr 280px`).
const SIDE_WIDTH: f32 = 280.0;
/// Gap between the paper and the card (the mock's `gap:16px`).
const GAP: f32 = 16.0;
/// Gap between two pages.
const PAGE_GAP: f32 = 12.0;
/// The widest a page is drawn, so a wide window does not blow the text up.
const MAX_PAGE_WIDTH: f32 = 760.0;
/// Room kept for the scroll bar beside the pages.
const SCROLL_BAR: f32 = 14.0;
/// Height of a row of the card's table, and the width of its caption column.
const ROW_HEIGHT: f32 = 34.0;
const ROW_LABEL: f32 = 96.0;
/// The ink on the paper, in every theme: paper is white (the mock's `#1c1b19` on `#fff`).
const INK: Color32 = Color32::from_rgb(0x1c, 0x1b, 0x19);
/// Points one arrow key press scrolls the pages; a Page key scrolls ten times that.
const SCROLL_STEP: f32 = 48.0;

/// What the user did in the preview this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    /// Nothing.
    None,
    /// Save as PDF (Ctrl+P or the button).
    Save,
    /// Lay the pages out again with the settings in the card (Enter or the button).
    Update,
    /// Close the preview (Esc or the button).
    Close,
}

/// The settings as typed in the card: text the user edits, read into a [`PrintSetup`] only when
/// the preview is updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintForm {
    /// The paper chosen with the A4 and A5 tabs.
    pub paper: Paper,
    /// Copies, as typed.
    pub copies: String,
    /// The number printed on the first page, as typed.
    pub first_page: String,
    /// Pages to print, as typed; blank means every page.
    pub pages: String,
}

impl Default for PrintForm {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            copies: "1".to_owned(),
            first_page: "1".to_owned(),
            pages: String::new(),
        }
    }
}

impl PrintForm {
    /// The settings the card holds.
    ///
    /// # Errors
    /// The words for the field that is not a number or a page range, or whose number the engine
    /// refuses; the page range is checked against the document by the engine, later.
    pub fn to_setup(&self) -> Result<PrintSetup, String> {
        let copies = self
            .copies
            .trim()
            .parse::<u8>()
            .map_err(|_e| format!("Copies must be a whole number, not \"{}\".", self.copies))?;
        let first_page_number = self.first_page.trim().parse::<u32>().map_err(|_e| {
            format!(
                "First page number must be a whole number, not \"{}\".",
                self.first_page
            )
        })?;
        let pages = PageSelection::parse(&self.pages).map_err(|e| e.to_string())?;
        let setup = PrintSetup {
            paper: Some(self.paper),
            copies,
            pages,
            first_page_number,
        };
        setup.check().map_err(|e| e.to_string())?;
        Ok(setup)
    }
}

/// The paper the pages are laid on, named by size and orientation: A4 is 210 x 297 mm, which the
/// print engine lays out as 595 x 842 points, and A5 is 148 x 210 mm, 420 x 595 points. Any
/// other size is given in points.
#[must_use]
pub fn paper_label(width: i32, height: i32) -> String {
    let (short, long) = (width.min(height), width.max(height));
    let name = match (short, long) {
        (595, 842) => Some("A4"),
        (420, 595) => Some("A5"),
        _ => None,
    };
    let orientation = if width <= height {
        "portrait"
    } else {
        "landscape"
    };
    match name {
        Some(name) => format!("{name}, {orientation}"),
        None => format!("{width} x {height} points, {orientation}"),
    }
}

/// The note while the PDF is being prepared (the mock's `profit-and-loss.print-busy`).
pub fn draw_busy(ui: &mut Ui) {
    note_with_icon(ui, NoteKind::Info, Icon::Clock, |ui, ink| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new("Preparing the PDF.")
                    .font(bold_font(ui, TEXT_META))
                    .color(ink),
            );
            ui.label(para_text(
                "The preview opens when it is ready.",
                TEXT_META,
                ink,
            ));
        });
    });
    ui.add_space(12.0);
}

/// The keys of the preview this frame: Ctrl+P saves, Enter updates, Esc closes.
fn read_keys(ui: &Ui) -> PreviewAction {
    ui.ctx().input_mut(|i| {
        if i.consume_key(Modifiers::COMMAND, Key::P) {
            PreviewAction::Save
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            PreviewAction::Update
        } else if i.consume_key(Modifiers::NONE, Key::Escape) {
            PreviewAction::Close
        } else {
            PreviewAction::None
        }
    })
}

/// Draws the preview in the space left under the page head and returns what was asked.
pub fn show(
    ui: &mut Ui,
    doc: &PrintedDocument,
    form: &mut PrintForm,
    error: Option<&str>,
) -> PreviewAction {
    let mut action = read_keys(ui);
    let room = ui.available_size();
    let paper_width = (room.x - SIDE_WIDTH - GAP).max(240.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            vec2(paper_width, room.y),
            Layout::top_down(egui::Align::Min),
            |ui| draw_pages(ui, doc, paper_width),
        );
        ui.add_space(GAP);
        ui.allocate_ui_with_layout(
            vec2(SIDE_WIDTH, room.y),
            Layout::top_down(egui::Align::Min),
            |ui| {
                let side = draw_side(ui, doc, form, error);
                if side != PreviewAction::None {
                    action = side;
                }
            },
        );
    });
    action
}

/// The pages, one under the other, in a scroll area that the arrow and Page keys also move.
fn draw_pages(ui: &mut Ui, doc: &PrintedDocument, width: f32) {
    let page_width = (width - SCROLL_BAR).clamp(200.0, MAX_PAGE_WIDTH);
    let total = doc.pages.len();
    let keys = ui.ctx().input_mut(|i| {
        let mut moved = 0.0_f32;
        for (key, by) in [
            (Key::ArrowDown, -SCROLL_STEP),
            (Key::ArrowUp, SCROLL_STEP),
            (Key::PageDown, -SCROLL_STEP * 10.0),
            (Key::PageUp, SCROLL_STEP * 10.0),
        ] {
            if i.consume_key(Modifiers::NONE, key) {
                moved += by;
            }
        }
        moved
    });
    ScrollArea::vertical()
        .id_salt("print-preview-pages")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if keys != 0.0 {
                ui.scroll_with_delta(vec2(0.0, keys));
            }
            ui.spacing_mut().item_spacing.y = PAGE_GAP;
            for (n, page) in doc.pages.iter().enumerate() {
                draw_page(ui, page, page_width, n + 1, total);
            }
        });
}

/// `v` points as an `f32` (layout values are small whole numbers).
fn pt(v: i32) -> f32 {
    f32::from(i16::try_from(v).unwrap_or(i16::MAX))
}

/// One page, white, scaled to `width`, with every item where the engine put it.
fn draw_page(ui: &mut Ui, page: &PageLayout, width: f32, number: usize, total: usize) {
    let t = tokens(ui);
    let scale = width / pt(page.width).max(1.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, pt(page.height) * scale), Sense::hover());
    a11y::name(
        ui,
        &response,
        Role::Image,
        &format!("Page {number} of {total}"),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_XS, Color32::WHITE);
    painter.rect_stroke(
        rect,
        RADIUS_XS,
        Stroke::new(1.0, t.border_strong),
        StrokeKind::Inside,
    );
    let at = |x: i32, y: i32| pos2(rect.min.x + pt(x) * scale, rect.min.y + pt(y) * scale);
    for item in &page.items {
        match item {
            PageItem::Text {
                x,
                baseline,
                size,
                bold,
                text,
                width: box_width,
            } => {
                let font = if *bold {
                    bold_font(ui, pt(*size) * scale)
                } else {
                    FontId::proportional(pt(*size) * scale)
                };
                let galley = painter.layout_no_wrap(text.clone(), font, INK);
                // The engine's width is Helvetica's; the face here differs a little. The text is
                // centred in the engine's box so a right-aligned figure still ends where the
                // engine ended it, and is never wider than the box.
                let wanted = pt(*box_width) * scale;
                let have = galley.size().x;
                let (galley, have) = if have > wanted && wanted > 0.0 {
                    let font = if *bold {
                        bold_font(ui, pt(*size) * scale * wanted / have)
                    } else {
                        FontId::proportional(pt(*size) * scale * wanted / have)
                    };
                    let fitted = painter.layout_no_wrap(text.clone(), font, INK);
                    let w = fitted.size().x;
                    (fitted, w)
                } else {
                    (galley, have)
                };
                let left = at(*x, 0).x + (wanted - have) / 2.0;
                let top = at(0, *baseline).y - galley.size().y * 0.8;
                painter.galley(pos2(left, top), galley, INK);
            }
            PageItem::Rule { x1, x2, y } => {
                painter.line_segment(
                    [at(*x1, *y), at(*x2, *y)],
                    Stroke::new((0.5 * scale).max(0.75), INK),
                );
            }
            PageItem::Fill { x, y, w, h } => {
                painter.rect_filled(Rect::from_min_max(at(*x, *y), at(x + w, y + h)), 0.0, INK);
            }
        }
    }
}

/// A captioned single-line field of the card; `bad` marks it and says why underneath.
fn field(ui: &mut Ui, label: &str, text: &mut String, width: f32, hint: &str, bad: Option<&str>) {
    let t = tokens(ui);
    ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        ui.label(para_text(label, TEXT_META, t.text_2));
        let edit = egui::TextEdit::singleline(text)
            .desired_width(width)
            .hint_text(hint);
        let response = a11y::text_edit(ui, label, edit);
        if bad.is_some() {
            ui.painter().rect_stroke(
                response.rect,
                RADIUS_XS,
                Stroke::new(1.5, t.bad),
                StrokeKind::Outside,
            );
        }
        if let Some(why) = bad {
            ui.add_space(2.0);
            para(ui, why, TEXT_META, t.bad);
        }
    });
}

/// The settings of the card: paper, copies, first page number and pages to print (the mock's
/// `profit-and-loss.print-config`).
fn draw_settings(ui: &mut Ui, form: &mut PrintForm, error: Option<&str>) {
    let inner = SIDE_WIDTH - 34.0;
    let t = tokens(ui);
    let paper = usize::from(form.paper == Paper::A5);
    ui.label(para_text("Paper", TEXT_META, t.text_2));
    let picked = crate::ui::tabs::tab_strip(ui, &["A4", "A5"], Some(paper));
    match picked.clicked {
        Some(0) => form.paper = Paper::A4,
        Some(1) => form.paper = Paper::A5,
        Some(_) | None => {}
    }
    ui.add_space(6.0);
    let half = (inner - 8.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        field(ui, "Copies", &mut form.copies, half, "", None);
        field(
            ui,
            "First page number",
            &mut form.first_page,
            half,
            "",
            None,
        );
    });
    ui.add_space(6.0);
    field(ui, "Pages to print", &mut form.pages, inner, "All", error);
    ui.add_space(8.0);
}

/// The card at the right: the settings, the pages, the destination and the buttons.
fn draw_side(
    ui: &mut Ui,
    doc: &PrintedDocument,
    form: &mut PrintForm,
    error: Option<&str>,
) -> PreviewAction {
    let t = tokens(ui);
    let mut action = PreviewAction::None;
    card(ui, |ui| {
        ui.set_width(SIDE_WIDTH - 34.0);
        ui.label(
            egui::RichText::new("Print preview")
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(10.0);
        draw_settings(ui, form, error);
        let inner = SIDE_WIDTH - 34.0;
        let shown = if doc.copies > 1 {
            format!("{}, in {} copies", doc.pages.len(), doc.copies)
        } else {
            doc.pages.len().to_string()
        };
        for (label, value) in [("Pages", shown), ("Destination", "PDF file".to_owned())] {
            // One row of the mock's `table.t`: the caption, the value and a hairline under it.
            ui.allocate_ui_with_layout(
                vec2(inner, ROW_HEIGHT),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(10.0);
                    ui.allocate_ui_with_layout(
                        vec2(ROW_LABEL, ROW_HEIGHT),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(ROW_LABEL);
                            ui.label(para_text(label, TEXT_BODY, t.text_2));
                        },
                    );
                    ui.label(para_text(&value, TEXT_BODY, t.text));
                },
            );
            let y = ui.cursor().min.y;
            let x0 = ui.min_rect().min.x;
            ui.painter()
                .hline(x0..=x0 + inner, y, Stroke::new(1.0, t.border));
        }
        ui.add_space(8.0);
        para(
            ui,
            "Paper printing is not built yet. The PDF is written to a folder you choose.",
            TEXT_META,
            t.text_3,
        );
        ui.add_space(10.0);
        // The mock's `.actions`: the buttons wrap when the card is too narrow for all three.
        ui.horizontal_wrapped(|ui| {
            if button(ui, "Update preview", Some("Enter"), ButtonKind::Secondary).clicked() {
                action = PreviewAction::Update;
            }
            if button_icon(
                ui,
                Some(Icon::Upload),
                "Save as PDF",
                Some("Ctrl+P"),
                ButtonKind::Primary,
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
            {
                action = PreviewAction::Save;
            }
            if button(ui, "Close", Some("Esc"), ButtonKind::Secondary).clicked() {
                action = PreviewAction::Close;
            }
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_is_named_by_the_size_the_engine_laid_out() {
        // provenance: derived. The print crate lays A4 (210 x 297 mm) out as 595 x 842 points
        // and A5 (148 x 210 mm) as 420 x 595: millimetres x 72 / 25.4, rounded to the nearest
        // point (210 -> 595.28, 297 -> 841.89, 148 -> 419.53, and 210 -> 595.28).
        assert_eq!(paper_label(595, 842), "A4, portrait");
        assert_eq!(paper_label(842, 595), "A4, landscape");
        assert_eq!(paper_label(420, 595), "A5, portrait");
        assert_eq!(paper_label(300, 400), "300 x 400 points, portrait");
    }
}
