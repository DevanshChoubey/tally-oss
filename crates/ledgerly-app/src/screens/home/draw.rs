//! Drawing of the home screen: the page head, the KPI tiles, the AI area, the chart and the
//! nudges. Every figure is text handed in by the parent module; this file lays out and paints
//! only (rule C4).
//!
//! No width is fixed. Tiles and panels split the width they are given, through `Ui::columns`, so
//! nothing clips when the window is narrow (B2).

use egui::FontId;
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Color32, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, UiBuilder, vec2,
};
use ledgerly_core::Date;

use super::{
    AiPanel, AttentionItem, AttentionKind, HomeBody, HomeTarget, KpiView, ReviewCard, figure_parts,
    main_columns, tile_columns,
};
use crate::theme::{
    RADIUS_M, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TEXT_BODY, TEXT_FIG, TEXT_H2, TEXT_META,
    border_width,
};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::card::card;
use crate::ui::conf::conf;
use crate::ui::icon::{self, Icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::skeleton::skeleton_bar;
use crate::ui::{bold_font, format, kind, para, tokens};

/// Line under the title while the figures load (the mock's `home.loading`).
const LOADING_LINE: &str =
    "Loading balances and projections. Figures appear only when the engine returns them.";
/// The AI-off note (the mock's `home.ai-off`, `index.html` line 1054).
const AI_OFF_TEXT: &str = "AI is off for this company. Plain-words entry, bill reading and suggestions are hidden. Rule checks, matching rules and every report work as usual.";
/// Placeholder for the plain-words line, which needs the assistant and runs nothing.
const ENTRY_PLACEHOLDER: &str = "Plain-words entry is not built yet. This line runs nothing.";
/// The chart's state until the engine has a monthly sales series.
const CHART_UNSUPPORTED: &str =
    "Not supported yet. The engine gives no monthly sales figures, so no bars are drawn.";
/// Placeholder for the nudges until their rules exist.
const NUDGES_PLACEHOLDER: &str = "Nudges are not built yet. Due dates and possible duplicate entries will be listed here, each with the rule that raised it.";
/// The page title of a company with no vouchers (`home.empty`).
const NEW_COMPANY_TITLE: &str = "Welcome to your new company";
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
    title: &str,
    date_line: &str,
    chosen: &mut Option<HomeTarget>,
) {
    let line = match shown {
        HomeBody::Loading => LOADING_LINE.to_owned(),
        HomeBody::NewCompany { .. } => company.to_owned(),
        HomeBody::Figures | HomeBody::NoFigures => date_line.to_owned(),
    };
    // The mock's loading and empty states have their own titles (`home.loading`: "Opening
    // <company>", `home.empty`: "Welcome to your new company", index.html lines 1043, 1046).
    let title = match shown {
        HomeBody::Loading if !company.is_empty() => format!("Opening {company}"),
        HomeBody::NewCompany { .. } => NEW_COMPANY_TITLE.to_owned(),
        HomeBody::Loading | HomeBody::Figures | HomeBody::NoFigures => title.to_owned(),
    };
    let sub = (!line.is_empty()).then_some(line.as_str());
    let with_actions = shown == HomeBody::Figures;
    page_head(ui, &title, sub, |ui| {
        if !with_actions {
            return;
        }
        // The mock's head: "Day book" with the book icon and no key, then "New sale" with the
        // sale icon and F8 (`SCREENS.home`, index.html line 1059). D still opens the day book.
        if button_icon(
            ui,
            Some(Icon::Sale),
            "New sale",
            Some("F8"),
            ButtonKind::Primary,
        )
        .clicked()
        {
            *chosen = Some(HomeTarget::SalesInvoice);
        }
        if button_icon(
            ui,
            Some(Icon::Book),
            "Day book",
            None,
            ButtonKind::Secondary,
        )
        .clicked()
        {
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
    review: ReviewCard<'_>,
    chosen: &mut Option<HomeTarget>,
) -> Vec<Rect> {
    entry_bar(ui, ai);
    // `.qe{margin-bottom:14px}`; the AI-off note has 12.
    ui.add_space(match ai {
        AiPanel::Off => SPACE_3,
        AiPanel::Placeholder => ENTRY_GAP,
    });
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
    panels(ui, ai, attention, review, chosen);
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
            ui.label(
                RichText::new(sentence.as_str())
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
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

/// The one line above the tiles: the AI-off note, or the plain-words entry line. The entry is
/// drawn in the mock's `.qe` box (strong border, radius 10, the spark icon) but says in words
/// that it is not built; it takes no typing and runs nothing.
fn entry_bar(ui: &mut Ui, ai: AiPanel) {
    match ai {
        AiPanel::Off => {
            note(ui, NoteKind::Info, AI_OFF_TEXT);
        }
        AiPanel::Placeholder => {
            let t = tokens(ui);
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(border_width(kind(ui)), t.border_strong))
                .corner_radius(RADIUS_M)
                // `.qe{padding:8px 8px 8px 12px}` around a 30 px line.
                .inner_margin(Margin {
                    left: 12,
                    right: 8,
                    top: 8,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.set_min_height(ENTRY_LINE_HEIGHT);
                        ui.spacing_mut().item_spacing.x = SPACE_2;
                        icon::icon(ui, Icon::Spark, t.text);
                        ui.label(
                            RichText::new(ENTRY_PLACEHOLDER)
                                .size(TEXT_BODY)
                                .color(t.text_3),
                        );
                    });
                });
        }
    }
}

/// Height of the entry line inside its box (`.qe input{height:30px}`).
const ENTRY_LINE_HEIGHT: f32 = 30.0;
/// Space under the entry box (`.qe` with `margin-bottom:14px`).
const ENTRY_GAP: f32 = 14.0;

/// Draws the two lower panels: the review list and the chart on the left, the nudges on the
/// right. Side by side when the width allows, one after the other otherwise.
fn panels(
    ui: &mut Ui,
    ai: AiPanel,
    attention: &[AttentionItem],
    review: ReviewCard<'_>,
    chosen: &mut Option<HomeTarget>,
) {
    // The review list and the nudges may each ask for a screen; both write to `chosen` after the
    // split, so the two closures never borrow it at the same time.
    let mut from_review = None;
    split(
        ui,
        |left| left_column(left, ai, review, &mut from_review),
        |right| right_column(right, attention, chosen),
    );
    if from_review.is_some() {
        *chosen = from_review;
    }
}

/// The rows the Home card lists before "All N" (the mock shows four).
const REVIEW_ROWS: usize = 4;

/// The left panel: the review list (not with AI off), and the sales chart state.
fn left_column(ui: &mut Ui, ai: AiPanel, review: ReviewCard<'_>, chosen: &mut Option<HomeTarget>) {
    if ai == AiPanel::Placeholder {
        card(ui, |ui| {
            title(ui, "Waiting for your review");
            review_list(ui, review, chosen);
        });
        ui.add_space(SPACE_3);
    }
    card(ui, |ui| {
        title(ui, "Sales by month, before GST");
        note(ui, NoteKind::Info, CHART_UNSUPPORTED);
    });
}

/// The Home card's list: the same rows as the Review queue, the first few, with a button to the
/// queue. The button says "All N" only when the queue's count is known (every source listed).
fn review_list(ui: &mut Ui, review: ReviewCard<'_>, chosen: &mut Option<HomeTarget>) {
    let Some(panel) = review.panel else {
        match review.error {
            Some(error) => note(ui, NoteKind::Bad, error),
            None => note(ui, NoteKind::Info, "Loading the proposals from the engine."),
        };
        return;
    };
    let label = match panel.count {
        Some(count) => format!("All {count}"),
        None => "Review queue".to_owned(),
    };
    if button(ui, &label, None, ButtonKind::Secondary).clicked() {
        *chosen = Some(HomeTarget::ReviewQueue);
    }
    if panel.rows.is_empty() {
        note(
            ui,
            NoteKind::Info,
            "No proposal is waiting from the connected sources.",
        );
    }
    for row in panel.rows.iter().take(REVIEW_ROWS) {
        review_row(ui, row);
    }
    let t = tokens(ui);
    if !panel.not_connected.is_empty() {
        note(
            ui,
            NoteKind::Info,
            &format!(
                "{} sources are not connected yet, so their proposals are not listed. The Review queue says which.",
                panel.not_connected.len()
            ),
        );
    }
    ui.label(
        RichText::new("Every row is a proposal. Nothing posts until you accept it.")
            .size(TEXT_META)
            .color(t.text_3),
    );
}

/// One proposal line on the Home card: its kind, what it is about, its confidence and amount.
fn review_row(ui: &mut Ui, row: &crate::screens::review::QueueRow) {
    let t = tokens(ui);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(row.kind).strong());
        ui.label(row.what.as_str());
        conf(ui, row.confidence);
        if let Some(amount) = row.amount {
            ui.label(format::amount(amount));
        }
    });
    ui.label(
        RichText::new(row.source.as_str())
            .size(TEXT_META)
            .color(t.text_3),
    );
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

/// Draws one KPI tile and returns its click response, whose rectangle is the tile's. As the
/// mock's `.kpi`: the label in `text-2` with a chevron at the right, the figure at 26 points
/// with a small `text-3` rupee sign (`.fig small`, 16 px), then the footnote in meta `text-3`.
fn tile(ui: &mut Ui, index: usize, view: &KpiView) -> egui::Response {
    let t = tokens(ui);
    let inner = card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(view.label.as_str())
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                icon::icon(ui, Icon::Chev, t.text_2);
            });
        });
        // `.fig{margin:6px 0 2px}`.
        ui.add_space(6.0);
        if view.supported {
            // One text job, so the small sign and the digits share a baseline (each section's
            // `valign` is bottom by default).
            let (sign, digits) = figure_parts(&view.value_text);
            let mut job = LayoutJob::default();
            if let Some(sign) = sign {
                job.append(
                    sign,
                    0.0,
                    TextFormat::simple(FontId::proportional(RUPEE_SIZE), t.text_3),
                );
            }
            // `.fig small{margin-right:2px}`.
            let gap = if sign.is_some() { 2.0 } else { 0.0 };
            job.append(
                digits,
                gap,
                TextFormat::simple(bold_font(ui, TEXT_FIG), t.text),
            );
            ui.label(job);
        } else {
            ui.label(
                RichText::new(view.value_text.as_str())
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text_2),
            );
        }
        ui.add_space(2.0);
        para(ui, view.footnote_text.as_str(), TEXT_META, t.text_3);
    });
    let id = ui.scope_id().with(("home-kpi", index));
    let response = ui.interact(inner.response.rect, id, Sense::click());
    let spoken = format!(
        "{}, {}. {}",
        view.label, view.value_text, view.footnote_text
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, spoken.as_str()));
    response
}

/// Size of the rupee sign before a tile figure (`.kpi .fig small{font-size:16px}`).
const RUPEE_SIZE: f32 = 16.0;

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

/// Lays two panels side by side as the mock's `.g-main` grid does ([`main_columns`]), or one
/// below the other when the width is too small.
#[allow(
    clippy::float_arithmetic,
    reason = "placing the right column after the left"
)]
pub(crate) fn split(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    if let Some((left_w, right_w)) = main_columns(width) {
        let top = ui.cursor().min;
        let height = ui.available_height().max(0.0);
        let column = |x: f32, w: f32| {
            UiBuilder::new()
                .max_rect(Rect::from_min_size(egui::pos2(x, top.y), vec2(w, height)))
                .layout(Layout::top_down(Align::Min))
        };
        let l = ui
            .scope_builder(column(top.x, left_w), |ui| {
                ui.set_width(left_w);
                left(ui);
            })
            .response
            .rect;
        let r = ui
            .scope_builder(column(top.x + width - right_w, right_w), |ui| {
                ui.set_width(right_w);
                right(ui);
            })
            .response
            .rect;
        let bottom = l.max.y.max(r.max.y);
        ui.advance_cursor_after_rect(Rect::from_min_max(top, egui::pos2(top.x + width, bottom)));
    } else {
        left(&mut *ui);
        ui.add_space(SPACE_3);
        right(&mut *ui);
    }
}

/// A card heading in the card title size. Called first in each panel card, it also makes the
/// card fill its column, as the mock's grid cells do.
pub(crate) fn title(ui: &mut Ui, text: &str) {
    ui.set_min_width(ui.available_width());
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_H2);
    ui.label(RichText::new(text).font(font).color(t.text));
    // `.card h2{margin-bottom:10px}`.
    ui.add_space(10.0);
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
