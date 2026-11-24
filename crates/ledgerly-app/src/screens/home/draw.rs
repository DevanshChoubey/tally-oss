//! Drawing of the home screen: the page head, the KPI tiles, the AI area, the chart and the
//! nudges. Every figure is text handed in by the parent module; this file lays out and paints
//! only (rule C4).
//!
//! No width is fixed. Tiles and panels split the width they are given, through `Ui::columns`, so
//! nothing clips when the window is narrow (B2).

use egui::FontId;
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};
use ledgerly_core::Date;

use super::chart::Chart;
use super::nudge::{Nudge, NudgeAction};
use super::{
    AiPanel, AttentionItem, HomeBody, HomeTarget, KpiView, ReviewCard, chart_draw, figure_parts,
    main_columns, nudge_draw, review_draw, tile_columns,
};
use crate::theme::{
    RADIUS_M, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TEXT_BODY, TEXT_FIG, TEXT_H2, TEXT_META,
    border_width,
};
use crate::ui::button::{ButtonKind, button_icon};
use crate::ui::card::card;
use crate::ui::icon::{self, Icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::skeleton::skeleton_bar;
use crate::ui::{bold_font, format, kind, para, tokens};

/// Line under the title while the figures load (the mock's `home.loading`).
const LOADING_LINE: &str =
    "Loading balances and projections. Figures appear only when the engine returns them.";
/// The AI-off note (the mock's `home.ai-off`, `index.html` line 1054).
const AI_OFF_REST: &str = "Plain-words entry, bill reading and suggestions are hidden. Rule checks, matching rules and every report work as usual.";
/// Placeholder for the plain-words line, which needs the assistant and runs nothing.
const ENTRY_PLACEHOLDER: &str = "Type an entry in plain words";
/// What the entry line says at its right, in place of the mock's "runs on this computer": no
/// assistant is connected, so nothing runs it.
const ENTRY_META: &str = "not connected in this build";
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
/// actions. The loading and new-company bodies show no actions. `new_line` is the line under
/// the new-company title ([`super::new_company_line`]).
pub(super) fn header(
    ui: &mut Ui,
    shown: HomeBody,
    company: &str,
    new_line: &str,
    title: &str,
    date_line: &str,
    chosen: &mut Option<HomeTarget>,
) {
    let line = match shown {
        HomeBody::Loading => LOADING_LINE.to_owned(),
        HomeBody::NewCompany { .. } => new_line.to_owned(),
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

/// What the lower half of the figures body shows.
pub(super) struct Lower<'a> {
    /// The AI area.
    pub(super) ai: AiPanel,
    /// The sales chart, once the first refresh has arrived.
    pub(super) chart: Option<&'a Chart>,
    /// The engine's nudges.
    pub(super) nudges: &'a [Nudge],
    /// The host's own attention items.
    pub(super) attention: &'a [AttentionItem],
    /// The review list.
    pub(super) review: ReviewCard<'a>,
    /// The review row that is highlighted.
    pub(super) selected: usize,
}

/// What the user chose in the figures body this frame.
#[derive(Debug, Default)]
pub(super) struct Chosen {
    /// A screen to open.
    pub(super) target: Option<HomeTarget>,
    /// A nudge button.
    pub(super) nudge: Option<NudgeAction>,
}

/// Draws the figures body: the AI line, the KPI tiles, then the two panels. Returns where each
/// tile was drawn, in order.
pub(super) fn figures(
    ui: &mut Ui,
    tiles: &[(KpiView, HomeTarget)],
    lower: &Lower<'_>,
    chosen: &mut Chosen,
) -> Vec<Rect> {
    entry_bar(ui, lower.ai, &mut chosen.target);
    // `.qe{margin-bottom:14px}`; the AI-off note has 12.
    ui.add_space(match lower.ai {
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
                chosen.target = Some(target.clone());
            }
        },
    );
    ui.add_space(SPACE_3);
    panels(ui, lower, chosen);
    rects
}

/// Draws the body for a company with no figures to show.
pub(super) fn no_figures(ui: &mut Ui) {
    note(ui, NoteKind::Info, NO_FIGURES_TEXT);
}

/// A note with an icon, an optional bold lead and, when `link` is given, a link at its end
/// (the mock's `.note` with `ic(..)`, `<b>` and a `.btn.ghost`). Returns `true` when the link was
/// clicked.
fn icon_note(
    ui: &mut Ui,
    kind_of: NoteKind,
    glyph: Icon,
    lead: Option<&str>,
    text: &str,
    link: Option<&str>,
) -> bool {
    let t = tokens(ui);
    let k = kind(ui);
    let (bg, fg, edge) = crate::ui::note::colors(kind_of, &t);
    let outline = if k == crate::theme::ThemeKind::HighContrast {
        Stroke::new(border_width(k), fg)
    } else {
        Stroke::new(border_width(k), edge)
    };
    let mut clicked = false;
    Frame::new()
        .fill(bg)
        .stroke(outline)
        .corner_radius(crate::theme::RADIUS_S)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_2;
                icon::icon(ui, glyph, fg);
                if let Some(lead) = lead {
                    ui.label(RichText::new(lead).font(bold_font(ui, TEXT_BODY)).color(fg));
                }
                ui.label(RichText::new(text).size(TEXT_BODY).color(fg));
                if let Some(link) = link {
                    clicked = ghost(ui, link, false).clicked();
                }
            });
        });
    clicked
}

/// Draws the first-steps body for a company with no vouchers (the mock's `home.empty`): the
/// welcome card with its book icon and three actions, then the note that totals stay hidden.
pub(super) fn welcome(ui: &mut Ui, begins: Date, chosen: &mut Option<HomeTarget>) {
    let t = tokens(ui);
    let sentence = format!(
        "Your books start on {}. Enter opening balances, bring data from another app, or start with today's first voucher.",
        format::date(begins)
    );
    card(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(SPACE_6);
            icon::icon_sized(ui, Icon::Book, t.text_3, 32.0);
            ui.add_space(SPACE_2);
            title(ui, "No vouchers yet");
            ui.add_space(SPACE_2);
            ui.label(
                RichText::new(sentence.as_str())
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
            ui.add_space(SPACE_4);
            ui.horizontal(|ui| {
                // The three buttons are centred as a row.
                let widths = [160.0_f32, 200.0, 190.0];
                let total: f32 = widths.iter().sum::<f32>() + 2.0 * SPACE_2;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                if button_icon(
                    ui,
                    Some(Icon::Users),
                    "Opening balances",
                    None,
                    ButtonKind::Primary,
                )
                .clicked()
                {
                    *chosen = Some(HomeTarget::Masters);
                }
                if button_icon(
                    ui,
                    Some(Icon::Upload),
                    "Import from another app",
                    None,
                    ButtonKind::Secondary,
                )
                .clicked()
                {
                    *chosen = Some(HomeTarget::ImportFromAnotherApp);
                }
                if button_icon(
                    ui,
                    Some(Icon::Sale),
                    "First sales invoice",
                    Some("F8"),
                    ButtonKind::Secondary,
                )
                .clicked()
                {
                    *chosen = Some(HomeTarget::SalesInvoice);
                }
            });
            ui.add_space(SPACE_6);
        });
    });
    ui.add_space(SPACE_3);
    icon_note(ui, NoteKind::Info, Icon::Info, None, WELCOME_NOTE, None);
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
/// that nothing runs it in this build; it takes no typing.
fn entry_bar(ui: &mut Ui, ai: AiPanel, chosen: &mut Option<HomeTarget>) {
    match ai {
        AiPanel::Off => {
            if icon_note(
                ui,
                NoteKind::Info,
                Icon::Off,
                Some("AI is off for this company."),
                AI_OFF_REST,
                Some("AI and privacy"),
            ) {
                *chosen = Some(HomeTarget::AiSettings);
            }
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
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(ENTRY_META).size(TEXT_META).color(t.text_3));
                            icon::icon_sized(ui, Icon::Off, t.text_3, 14.0);
                        });
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
fn panels(ui: &mut Ui, lower: &Lower<'_>, chosen: &mut Chosen) {
    // The two columns each write to their own slot after the split, so the closures never
    // borrow `chosen` at the same time.
    let mut from_review = None;
    let mut from_nudges = Chosen::default();
    split(
        ui,
        |left| {
            if lower.ai == AiPanel::Placeholder {
                review_draw::show(left, lower.review, lower.selected, &mut from_review);
                left.add_space(SPACE_3);
            }
            chart_draw::show(left, lower.chart);
        },
        |right| {
            card(right, |ui| {
                title(ui, "Nudges");
                nudge_draw::list(
                    ui,
                    lower.nudges,
                    lower.attention,
                    &mut from_nudges.target,
                    &mut from_nudges.nudge,
                );
            });
        },
    );
    chosen.target = from_review.or(from_nudges.target).or(chosen.target.take());
    chosen.nudge = from_nudges.nudge;
}

/// A card heading with a caption at its right (`.spread` with `h2` and `.meta`).
pub(super) fn title_row(ui: &mut Ui, text: &str, caption: Option<&str>) {
    title_row_with(ui, text, |ui| {
        if let Some(caption) = caption {
            let t = tokens(ui);
            ui.label(RichText::new(caption).size(TEXT_META).color(t.text_3));
        }
    });
}

/// A card heading with whatever `right` draws at its right end.
pub(super) fn title_row_with(ui: &mut Ui, text: &str, right: impl FnOnce(&mut Ui)) {
    ui.set_min_width(ui.available_width());
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_H2);
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).font(font).color(t.text));
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    // `.card h2{margin-bottom:10px}`.
    ui.add_space(10.0);
}

/// A link without a frame: the label and, when `chevron` is set, a small arrow after it
/// (the mock's `.btn.ghost`). Returns the click response.
pub(super) fn ghost(ui: &mut Ui, label: &str, chevron: bool) -> egui::Response {
    let t = tokens(ui);
    let font = bold_font(ui, TEXT_BODY);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, t.text);
    let extra = if chevron { 22.0 } else { 0.0 };
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 16.0 + extra, 32.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, t.hover);
    }
    let ink = t.text;
    ui.painter().galley(
        pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    if chevron {
        let icon_rect = Rect::from_min_size(
            pos2(rect.right() - 8.0 - 16.0, rect.center().y - 8.0),
            vec2(16.0, 16.0),
        );
        icon::paint(ui.painter(), icon_rect, Icon::Chev, ink);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    response
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
