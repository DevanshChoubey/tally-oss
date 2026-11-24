//! Drawing of the "Sales by month, before GST" card. The bars come from [`super::chart`]; this
//! file paints them as the mock's `.bars` (`design/mock-v2/index.html` lines 186 to 189): accent
//! columns with rounded tops, the open month hatched, a label under each.

use egui::{Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};

use super::chart::{Bar, Chart};
use super::draw::title_row;
use crate::theme::TEXT_CAP;
use crate::ui::card::card;
use crate::ui::note::{NoteKind, note};
use crate::ui::tokens;

/// Height of the bar area (`.bars{height:120px}`).
const AREA_HEIGHT: f32 = 120.0;
/// Space above the bars (`padding-top:8px`).
const AREA_TOP: f32 = 8.0;
/// Gap between bars (`gap:10px`).
const GAP: f32 = 10.0;
/// Widest a bar gets (`max-width:34px`).
const BAR_MAX_WIDTH: f32 = 34.0;
/// Height of the label row under a bar (11 point text and `gap:4px`).
const LABEL_ROW: f32 = 17.0;
/// The tallest bar's share of the area (`Math.round(m/mx*92)%`).
const TALLEST: f32 = 0.92;
/// Distance between hatch lines along the bar's width.
const HATCH_STEP: f32 = 10.0;

/// Draws the chart card; `chart` is `None` until the first refresh has arrived.
pub(super) fn show(ui: &mut Ui, chart: Option<&Chart>) {
    card(ui, |ui| match chart {
        Some(Chart::Bars { bars, caption }) => {
            title_row(ui, "Sales by month, before GST", Some(caption));
            draw_bars(ui, bars);
        }
        Some(Chart::Unsupported(reason)) => {
            title_row(ui, "Sales by month, before GST", None);
            note(ui, NoteKind::Info, &format!("Not supported yet. {reason}"));
        }
        None => {
            title_row(ui, "Sales by month, before GST", None);
            note(
                ui,
                NoteKind::Info,
                "Loading the monthly sales from the engine.",
            );
        }
    });
}

/// Paints `bars` side by side along the bottom of a 120 point area.
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "placing bars along a row"
)]
fn draw_bars(ui: &mut Ui, bars: &[Bar]) {
    let t = tokens(ui);
    let (area, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), AREA_HEIGHT + AREA_TOP),
        Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Image, true, "Sales by month, before GST")
    });
    if bars.is_empty() {
        return;
    }
    let count = bars.len() as f32;
    let column = ((area.width() - GAP * (count - 1.0)) / count).max(1.0);
    let base = area.bottom() - LABEL_ROW;
    let room = (AREA_HEIGHT - LABEL_ROW) * TALLEST;
    for (i, bar) in bars.iter().enumerate() {
        let left = area.left() + i as f32 * (column + GAP);
        let width = column.min(BAR_MAX_WIDTH);
        let x = left + (column - width) / 2.0;
        let height = (bar.share * room).max(0.0);
        let rect = Rect::from_min_max(pos2(x, base - height), pos2(x + width, base));
        let top = crate::theme::RADIUS_XS;
        let rounding = egui::CornerRadius {
            nw: top,
            ne: top,
            sw: 0,
            se: 0,
        };
        if bar.to_date {
            ui.painter().rect_filled(rect, rounding, t.accent_soft);
            hatch(ui, rect, t.accent);
        } else {
            ui.painter()
                .rect_filled(rect, rounding, t.accent.gamma_multiply(0.85));
        }
        ui.painter().text(
            pos2(left + column / 2.0, area.bottom()),
            egui::Align2::CENTER_BOTTOM,
            &bar.label,
            egui::FontId::proportional(TEXT_CAP),
            t.text_3,
        );
        let hit = Rect::from_min_max(pos2(left, area.top()), pos2(left + column, area.bottom()));
        let tip = format!("{}: {}", bar.label, bar.value_text);
        ui.interact(hit, response.id.with(i), Sense::hover())
            .on_hover_text(tip);
    }
}

/// Diagonal stripes of `color` across `rect` (the mock's 135 degree repeating gradient).
#[allow(clippy::float_arithmetic, reason = "placing hatch lines")]
fn hatch(ui: &Ui, rect: Rect, color: egui::Color32) {
    let painter = ui.painter().with_clip_rect(rect);
    let h = rect.height();
    let mut x = rect.left() - h;
    let mut lines: Vec<Shape> = Vec::new();
    while x < rect.right() {
        let a: Pos2 = pos2(x, rect.bottom());
        let b: Pos2 = pos2(x + h, rect.top());
        lines.push(Shape::line_segment([a, b], Stroke::new(4.0, color)));
        x += HATCH_STEP;
    }
    painter.extend(lines);
}
