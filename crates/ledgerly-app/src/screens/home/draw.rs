//! Drawing of the home screen: the page head, the KPI tiles, the AI area, the chart and the
//! nudges. Every figure is text handed in by the parent module; this file lays out and paints
//! only (rule C4).
//!
//! No width is fixed. Tiles and panels split the width they are given, through `Ui::columns`, so
//! nothing clips when the window is narrow (B2).

use egui::{Color32, Rect, RichText, Sense, Ui};
use ledgerly_core::Date;

use super::{
    AiPanel, AttentionItem, AttentionKind, HomeBody, HomeTarget, KpiView, panel_row_is_wide,
    tile_columns,
};
use crate::theme::{SPACE_2, SPACE_3, SPACE_4, SPACE_6, TEXT_BODY, TEXT_FIG, TEXT_H2, TEXT_META};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::skeleton::skeleton_bar;
use crate::ui::{bold_font, format, tokens};

/// Line under the title while the figures load (the mock's `home.loading`).
const LOADING_LINE: &str =
    "Loading balances and projections. Figures appear only when the engine returns them.";
/// The AI-off note (the mock's `home.ai-off`, `index.html` line 1054).
const AI_OFF_TEXT: &str = "AI is off for this company. Plain-words entry, bill reading and suggestions are hidden. Rule checks, matching rules and every report work as usual.";
/// Placeholder for the plain-words line, which needs the assistant and runs nothing.
const ENTRY_PLACEHOLDER: &str = "Plain-words entry is not built yet. This line runs nothing.";
/// Placeholder for the review list, which needs the assistant and runs nothing.
const REVIEW_PLACEHOLDER: &str = "Not built yet. Proposals from the assistant will be listed here, and nothing posts until you accept one.";
/// The chart's state until the engine has a monthly sales series.
const CHART_UNSUPPORTED: &str = "Not supported yet. The engine gives no monthly sales figures, so no bars are drawn.";
/// Placeholder for the nudges until their rules exist.
const NUDGES_PLACEHOLDER: &str = "Nudges are not built yet. Due dates and possible duplicate entries will be listed here, each with the rule that raised it.";
/// Shown when no company is open.
const NO_FIGURES_TEXT: &str = "No figures yet. Open a company to see them.";
/// Under the welcome card, as the mock's empty state says (`home.empty`, `index.html` line 1054).
const WELCOME_NOTE: &str =
    "Totals stay hidden until there is data. Ledgerly never shows a zero it did not compute.";
/// Relative widths of the bars in a skeleton KPI card.
const KPI_BAR_WIDTHS: [f32; 3] = [0.5, 0.8, 0.4];
/// Relative widths of the bars in a skeleton panel; the first `rows` are used.
const PANEL_BAR_WIDTHS: [f32; 6] = [0.9, 0.7, 0.85, 0.6, 0.75, 0.5];

/// Draws the page head: the title, the company line and, when there are figures, the two
/// actions. The loading and new-company bodies show no actions.
pub(super) fn header(
    ui: &mut Ui,
    shown: HomeBody,
    company: &str,
    subtitle: &str,
    chosen: &mut Option<HomeTarget>,
) {
    let line = match shown {
        HomeBody::Loading => LOADING_LINE.to_owned(),
        HomeBody::NewCompany { .. } => company.to_owned(),
        HomeBody::Figures | HomeBody::NoFigures => super::sub_line(company, subtitle),
    };
    let sub = (!line.is_empty()).then_some(line.as_str());
    let with_actions = shown == HomeBody::Figures;
    page_head(ui, super::PAGE_TITLE, sub, |ui| {
        if !with_actions {
            return;
        }
        if button(ui, "New sale", Some("F8"), ButtonKind::Primary).clicked() {
            *chosen = Some(HomeTarget::SalesInvoice);
        }
        if button(ui, "Day book", Some("D"), ButtonKind::Secondary).clicked() {
            *chosen = Some(HomeTarget::DayBook);
        }
    });
}

/// Draws the last refresh's error, when there is one.
pub(super) fn notice(ui: &mut Ui, text: Option<&str>) {
    if let Some(text) = text {
        note(ui, NoteKind::Bad, text);
        ui.add_space(SPACE_2);
    }
}

/// Draws the figures body: the AI line, the KPI tiles, then the two panels. Returns where each
/// tile was drawn, in order.
pub(super) fn figures(
    ui: &mut Ui,
    tiles: &[(KpiView, HomeTarget)],
    ai: AiPanel,
    attention: &[AttentionItem],
    chosen: &mut Option<HomeTarget>,
) -> Vec<Rect> {
    entry_bar(ui, ai);
    ui.add_space(SPACE_3);
    let per_row = tile_columns(ui.available_width());
    let mut rects = Vec::new();
    grid(
        ui,
        tiles,
        per_row,
        &mut |col: &mut Ui, index: usize, item: &(KpiView, HomeTarget)| {
            let (view, target) = item;
            let response = tile(col, index, view);
            rects.push(response.rect);
            if response.clicked() {
                *chosen = Some(target.clone());
            }
        },
    );
    ui.add_space(SPACE_3);
    panels(ui, ai, attention, chosen);
    rects
}

/// Draws the body for a company with no figures to show.
pub(super) fn no_figures(ui: &mut Ui) {
    note(ui, NoteKind::Info, NO_FIGURES_TEXT);
}

/// Draws the first-steps body for a company with no vouchers (the mock's `home.empty`): the
/// welcome card with the two actions, then the note that totals stay hidden.
pub(super) fn welcome(ui: &mut Ui, begins: Date, chosen: &mut Option<HomeTarget>) {
    let t = tokens(ui);
    let sentence = format!(
        "Your books start on {}. Enter opening balances, or start with today's first voucher.",
        format::date(begins)
    );
    card(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(SPACE_6);
            title(ui, "No vouchers yet");
            ui.add_space(SPACE_2);
            ui.label(RichText::new(sentence.as_str()).size(TEXT_BODY).color(t.text_2));
            ui.add_space(SPACE_4);
            ui.horizontal(|ui| {
                if button(ui, "Opening balances", None, ButtonKind::Primary).clicked() {
                    *chosen = Some(HomeTarget::Masters);
                }
                if button(ui, "First sales invoice", Some("F8"), ButtonKind::Secondary).clicked() {
                    *chosen = Some(HomeTarget::SalesInvoice);
                }
            });
            ui.add_space(SPACE_6);
        });
    });
    ui.add_space(SPACE_3);
    note(ui, NoteKind::Info, WELCOME_NOTE);
}

/// Draws the loading body (the mock's `home.loading`): four skeleton KPI cards and two skeleton
/// panels. No figure is drawn.
pub(super) fn skeleton(ui: &mut Ui) {
    let per_row = tile_columns(ui.available_width());
    let placeholders = [(); 4];
    grid(
        ui,
        &placeholders,
        per_row,
        &mut |col: &mut Ui, _index: usize, _item: &()| {
            card(col, |col| {
                let width = col.available_width();
                for fraction in KPI_BAR_WIDTHS {
                    skeleton_bar(col, width * fraction);
                    col.add_space(SPACE_2);
                }
            });
        },
    );
    ui.add_space(SPACE_3);
    split(
        ui,
        |left| skeleton_panel(left, 6),
        |right| skeleton_panel(right, 5),
    );
}

/// The one line above the tiles: the AI-off note, or the placeholder for the plain-words entry.
fn entry_bar(ui: &mut Ui, ai: AiPanel) {
    let text = match ai {
        AiPanel::Off => AI_OFF_TEXT,
        AiPanel::Placeholder => ENTRY_PLACEHOLDER,
    };
    note(ui, NoteKind::Info, text);
}

/// Draws the two lower panels: the review list and the chart on the left, the nudges on the
/// right. Side by side when the width allows, one after the other otherwise.
fn panels(ui: &mut Ui, ai: AiPanel, attention: &[AttentionItem], chosen: &mut Option<HomeTarget>) {
    split(
        ui,
        |left| left_column(left, ai),
        |right| right_column(right, attention, chosen),
    );
}

/// The left panel: the review list placeholder (not with AI off), and the sales chart state.
fn left_column(ui: &mut Ui, ai: AiPanel) {
    if ai == AiPanel::Placeholder {
        card(ui, |ui| {
            title(ui, "Waiting for your review");
            note(ui, NoteKind::Info, REVIEW_PLACEHOLDER);
        });
        ui.add_space(SPACE_3);
    }
    card(ui, |ui| {
        title(ui, "Sales by month, before GST");
        note(ui, NoteKind::Info, CHART_UNSUPPORTED);
    });
}

/// The right panel: the nudges. Each host item is a line with its word; with none, the card says
/// the rules are not built.
fn right_column(ui: &mut Ui, attention: &[AttentionItem], chosen: &mut Option<HomeTarget>) {
    card(ui, |ui| {
        title(ui, "Nudges");
        if attention.is_empty() {
            note(ui, NoteKind::Info, NUDGES_PLACEHOLDER);
        }
        for item in attention {
            attention_row(ui, item, chosen);
        }
    });
}

/// One nudge line: its word, the headline (a button when it has a target) and the detail.
fn attention_row(ui: &mut Ui, item: &AttentionItem, chosen: &mut Option<HomeTarget>) {
    let t = tokens(ui);
    let color = attention_color(ui, item.kind);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(item.kind.word()).color(color).strong());
        match &item.target {
            Some(target) => {
                if ui
                    .add(egui::Button::new(item.headline.as_str()).frame(false))
                    .clicked()
                {
                    *chosen = Some(target.clone());
                }
            }
            None => {
                ui.label(RichText::new(item.headline.as_str()).strong());
            }
        }
    });
    if !item.detail.is_empty() {
        ui.label(
            RichText::new(item.detail.as_str())
                .size(TEXT_META)
                .color(t.text_3),
        );
    }
}

/// The skeleton of one lower panel, with `rows` bars.
fn skeleton_panel(ui: &mut Ui, rows: usize) {
    card(ui, |ui| {
        let width = ui.available_width();
        for fraction in PANEL_BAR_WIDTHS.iter().take(rows) {
            skeleton_bar(ui, width * fraction);
            ui.add_space(SPACE_2);
        }
    });
}

/// Draws one KPI tile and returns its click response, whose rectangle is the tile's.
fn tile(ui: &mut Ui, index: usize, view: &KpiView) -> egui::Response {
    let t = tokens(ui);
    let label_font = bold_font(ui, TEXT_META);
    let figure_font = if view.supported {
        bold_font(ui, TEXT_FIG)
    } else {
        bold_font(ui, TEXT_H2)
    };
    let figure_color = if view.supported { t.text } else { t.text_2 };
    let inner = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(view.label.as_str())
                .font(label_font)
                .color(t.text_2),
        );
        ui.label(
            RichText::new(view.value_text.as_str())
                .font(figure_font)
                .color(figure_color),
        );
        ui.label(
            RichText::new(view.footnote_text.as_str())
                .size(TEXT_META)
                .color(t.text_3),
        );
    });
    let id = ui.id().with(("home-kpi", index));
    let response = ui.interact(inner.response.rect, id, Sense::click());
    let spoken = format!(
        "{}, {}. {}",
        view.label, view.value_text, view.footnote_text
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, spoken.as_str()));
    response
}

/// Lays `items` out in rows of `per_row` columns. `draw` gets the index of each item in the whole
/// list, so the tiles get distinct ids.
fn grid<T>(ui: &mut Ui, items: &[T], per_row: usize, draw: &mut impl FnMut(&mut Ui, usize, &T)) {
    let per_row = per_row.max(1);
    for (row_no, row) in items.chunks(per_row).enumerate() {
        let first = row_no * per_row;
        ui.columns(per_row, |cols| {
            for (col, (offset, item)) in cols.iter_mut().zip(row.iter().enumerate()) {
                draw(col, first + offset, item);
            }
        });
    }
}

/// Lays two panels side by side when the width allows, or one below the other otherwise.
fn split(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    if panel_row_is_wide(ui.available_width()) {
        ui.columns(2, |cols| {
            let mut columns = cols.iter_mut();
            if let (Some(first), Some(second)) = (columns.next(), columns.next()) {
                left(first);
                right(second);
            }
        });
    } else {
        left(&mut *ui);
        ui.add_space(SPACE_3);
        right(&mut *ui);
    }
}

/// A card heading in the card title size.
fn title(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_H2);
    ui.label(RichText::new(text).font(font).color(t.text));
}

/// Colour of the word before a nudge. Taken from the active visuals, so the three themes stay
/// consistent; the word carries the meaning.
fn attention_color(ui: &Ui, kind: AttentionKind) -> Color32 {
    match kind {
        AttentionKind::Pending => ui.visuals().warn_fg_color,
        AttentionKind::Due => ui.visuals().error_fg_color,
        AttentionKind::Done => ui.visuals().text_color(),
    }
}
