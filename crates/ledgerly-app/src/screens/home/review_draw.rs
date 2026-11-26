//! Drawing of the "Waiting for your review" card, as the mock's `#homeq` (`design/mock-v2/
//! index.html` line 1066): a row per proposal with its icon, the kind in bold and what it is
//! about, where it came from, the confidence and the amount at the right.

use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui,
    pos2, text::LayoutJob, text::TextFormat, vec2,
};

use super::draw::{ghost, title_row, title_row_with};
use super::{HomeTarget, ReviewCard};
use crate::screens::review::QueueRow;
use crate::theme::{SPACE_2, TEXT_BODY, TEXT_META, border_width};
use crate::ui::card::card;
use crate::ui::conf::{self, Confidence};
use crate::ui::icon::{self, Icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::{bold_font, format, kind, para, tokens};

/// Width reserved for the confidence column.
const CONF_WIDTH: f32 = 124.0;
/// Width reserved for the amount column.
const AMOUNT_WIDTH: f32 = 96.0;
/// Width of one confidence bar and the gap between bars.
const BAR: (f32, f32, f32) = (5.0, 12.0, 2.0);

/// Draws the card. `selected` is the highlighted row; `chosen` gets the queue when a row or the
/// heading link is clicked.
pub(super) fn show(
    ui: &mut Ui,
    review: ReviewCard<'_>,
    selected: usize,
    chosen: &mut Option<HomeTarget>,
) {
    let t = tokens(ui);
    card(ui, |ui| {
        let Some(panel) = review.panel else {
            title_row(ui, &crate::i18n::t("home.waiting_review"), None);
            match review.error {
                Some(error) => note(ui, NoteKind::Bad, error),
                None => note(
                    ui,
                    NoteKind::Info,
                    &crate::i18n::t("home.loading_proposals"),
                ),
            };
            return;
        };
        let count = panel
            .count
            .map(|n| crate::i18n::tf("home.all_count", &[("count", &n.to_string())]));
        let queue_label = crate::i18n::t("home.review_queue");
        let label = count.as_deref().unwrap_or(&queue_label);
        let mut all = false;
        title_row_with(ui, &crate::i18n::t("home.waiting_review"), |ui| {
            all = ghost(ui, label, true).clicked();
        });
        if all {
            *chosen = Some(HomeTarget::ReviewQueue);
        }
        if panel.rows.is_empty() {
            note(ui, NoteKind::Info, &crate::i18n::t("home.no_proposal"));
        }
        for (i, row) in panel.rows.iter().take(super::REVIEW_ROWS).enumerate() {
            if proposal_row(ui, row, i == selected).clicked() {
                *chosen = Some(HomeTarget::ReviewQueue);
            }
        }
        if !panel.not_connected.is_empty() {
            ui.add_space(SPACE_2);
            para(
                ui,
                &crate::i18n::tf(
                    "home.sources_not_connected",
                    &[("count", &panel.not_connected.len().to_string())],
                ),
                TEXT_META,
                t.text_3,
            );
        }
        ui.add_space(SPACE_2);
        para(ui, &crate::i18n::t("home.every_row"), TEXT_META, t.text_3);
    });
}

/// The confidence at the right of a row: three bars and "High confidence".
fn confidence(ui: &mut Ui, level: Confidence) {
    let t = tokens(ui);
    let lit = conf::lit_bars(level);
    let (bar_w, bar_h, gap) = BAR;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) =
            ui.allocate_exact_size(vec2(3.0 * bar_w + 2.0 * gap, bar_h), Sense::hover());
        for i in 0..3 {
            let x = rect.left() + (bar_w + gap) * f32::from(u8::try_from(i).unwrap_or(0));
            let bar = Rect::from_min_size(pos2(x, rect.top()), vec2(bar_w, bar_h));
            let color = if i < lit {
                t.text_2
            } else {
                t.text_2.gamma_multiply(0.25)
            };
            ui.painter().rect_filled(bar, CornerRadius::same(1), color);
        }
        let word = match level {
            Confidence::Rule => conf::word_in(level, &crate::i18n::current()),
            Confidence::High | Confidence::Medium | Confidence::Low => crate::i18n::tf(
                "home.confidence",
                &[("level", &conf::word_in(level, &crate::i18n::current()))],
            ),
        };
        ui.add(
            egui::Label::new(
                RichText::new(word)
                    .font(bold_font(ui, TEXT_META))
                    .color(t.text_2),
            )
            .wrap(),
        );
    });
}

/// One proposal row.
#[allow(clippy::float_arithmetic, reason = "sharing a row between its columns")]
fn proposal_row(ui: &mut Ui, row: &QueueRow, selected: bool) -> egui::Response {
    let t = tokens(ui);
    let k = kind(ui);
    let fill = if selected {
        t.accent_soft
    } else {
        Color32::TRANSPARENT
    };
    let inner = Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let text_w = (ui.available_width() - 24.0 - CONF_WIDTH - AMOUNT_WIDTH - 16.0).max(80.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // Every row of the queue comes from the bank source today.
                icon::icon(ui, Icon::Bank, t.text_2);
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let mut job = LayoutJob::default();
                    job.append(
                        &row.kind,
                        0.0,
                        TextFormat::simple(bold_font(ui, TEXT_BODY), t.text),
                    );
                    job.append(
                        &row.what,
                        6.0,
                        TextFormat::simple(FontId::proportional(TEXT_BODY), t.text_2),
                    );
                    ui.add(egui::Label::new(job).truncate());
                    ui.label(RichText::new(&row.source).size(TEXT_META).color(t.text_3));
                });
                ui.allocate_ui_with_layout(
                    vec2(CONF_WIDTH, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| confidence(ui, row.confidence),
                );
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    if let Some(amount) = row.amount {
                        ui.label(
                            RichText::new(format::amount(amount))
                                .size(TEXT_BODY)
                                .color(t.text),
                        );
                    }
                });
            });
        });
    let rect = inner.response.rect;
    let line = Stroke::new(border_width(k), t.border);
    ui.painter().hline(rect.x_range(), rect.bottom(), line);
    if selected {
        let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
        ui.painter().rect_filled(bar, CornerRadius::ZERO, t.accent);
    }
    ui.interact(
        rect,
        ui.scope_id().with(("home-review-row", row.what.as_str())),
        Sense::click(),
    )
}
