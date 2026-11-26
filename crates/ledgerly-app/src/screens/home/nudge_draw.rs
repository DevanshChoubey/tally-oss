//! Drawing of the Nudges card and of the comparison dialog (`home.nudge-open`). The words come
//! from [`super::nudge`]; this file lays out and paints only.

use egui::{
    Align, Area, Color32, CornerRadius, Frame, Layout, Margin, Order, Rect, RichText, Sense, Shape,
    Stroke, Ui, Vec2, pos2, vec2,
};
use ledgerly_api::DuplicatePayment;

use super::nudge::{Nudge, NudgeAction, NudgeTone};
use super::{AttentionItem, AttentionKind, HomeTarget};
use crate::theme::{
    self, RADIUS_L, RADIUS_S, SPACE_2, SPACE_3, TEXT_BODY, TEXT_META, border_width,
};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::icon::{self, Icon};
use crate::ui::note::{NoteKind, colors};
use crate::ui::prov::{ProvKind, prov};
use crate::ui::{bold_font, format, kind, para, tokens};

/// What a nudge tone is drawn as.
fn note_kind(tone: NudgeTone) -> NoteKind {
    match tone {
        NudgeTone::Warn => NoteKind::Warn,
        NudgeTone::Bad => NoteKind::Bad,
        NudgeTone::Neutral => NoteKind::Info,
    }
}

/// One host-supplied item as a card: its word, its headline and its detail.
fn attention_tone(kind_of: AttentionKind) -> NudgeTone {
    match kind_of {
        AttentionKind::Pending => NudgeTone::Warn,
        AttentionKind::Due => NudgeTone::Bad,
        AttentionKind::Done => NudgeTone::Neutral,
    }
}

/// The words of a card.
struct CardText<'a> {
    tone: NudgeTone,
    icon: Icon,
    title: &'a str,
    body: &'a str,
    source: &'a str,
    /// The button's label, when the card has one.
    button: Option<String>,
}

/// Width a secondary button reading `label` needs (12 points of padding each side).
fn button_width(ui: &Ui, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(TEXT_BODY),
        Color32::WHITE,
    );
    galley.size().x + 24.0
}

/// Draws one card as the mock's `.note` with an icon, a bold title, a body, the rule that raised
/// it and a button. Returns `true` when the button was clicked.
#[allow(
    clippy::float_arithmetic,
    reason = "sharing a row between text and a button"
)]
fn card(ui: &mut Ui, text: &CardText<'_>) -> bool {
    let t = tokens(ui);
    let k = kind(ui);
    let (bg, fg, edge) = colors(note_kind(text.tone), &t);
    let outline = if k == theme::ThemeKind::HighContrast {
        Stroke::new(border_width(k), fg)
    } else {
        Stroke::new(border_width(k), edge)
    };
    let mut clicked = false;
    Frame::new()
        .fill(bg)
        .stroke(outline)
        .corner_radius(RADIUS_S)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_2;
                icon::icon(ui, text.icon, fg);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let font = bold_font(ui, TEXT_BODY);
                    ui.add(egui::Label::new(RichText::new(text.title).font(font).color(fg)).wrap());
                    if !text.body.is_empty() {
                        para(ui, text.body, TEXT_BODY, fg);
                    }
                    ui.add_space(4.0);
                    let row_w = ui.available_width();
                    let btn_w = text
                        .button
                        .as_deref()
                        .map_or(0.0, |b| button_width(ui, b) + SPACE_2);
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            vec2((row_w - btn_w).max(60.0), 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    icon::icon_sized(ui, Icon::Check, fg, 12.0);
                                    ui.label(RichText::new(text.source).size(TEXT_META).color(fg));
                                });
                            },
                        );
                        if let Some(label) = text.button.as_deref() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                clicked = button(ui, label, None, ButtonKind::Secondary).clicked();
                            });
                        }
                    });
                });
            });
        });
    clicked
}

/// Draws the Nudges card's contents: the engine's nudges, then the host's items, then the
/// footer. `go` gets the action of a clicked button.
pub(super) fn list(
    ui: &mut Ui,
    nudges: &[Nudge],
    attention: &[AttentionItem],
    chosen: &mut Option<HomeTarget>,
    go: &mut Option<NudgeAction>,
) {
    let t = tokens(ui);
    if nudges.is_empty() && attention.is_empty() {
        ui.label(
            RichText::new(crate::i18n::t("home.nudge.nothing"))
                .size(TEXT_BODY)
                .color(t.text_2),
        );
    }
    ui.spacing_mut().item_spacing.y = SPACE_2;
    for n in nudges {
        let text = CardText {
            tone: n.tone,
            icon: n.icon,
            title: &n.title,
            body: &n.body,
            source: &n.source,
            button: Some(n.action_label.clone()),
        };
        if card(ui, &text) {
            *go = Some(n.action.clone());
        }
    }
    for item in attention {
        let text = CardText {
            tone: attention_tone(item.kind),
            icon: Icon::Info,
            title: &item.headline,
            body: &item.detail,
            source: &item.kind.word_in(&crate::i18n::current()),
            button: item.target.as_ref().map(|_| crate::i18n::t("common.open")),
        };
        if card(ui, &text) {
            chosen.clone_from(&item.target);
        }
    }
    ui.add_space(SPACE_2);
    ui.label(
        RichText::new(crate::i18n::t("home.nudge.footer"))
            .size(TEXT_META)
            .color(t.text_3),
    );
}

/// What the user did in the comparison dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DialogChoice {
    /// Nothing yet.
    None,
    /// "Investigate": open the day book.
    Investigate,
    /// A click on the scrim.
    Close,
}

/// Width of the dialog (`style="width:640px"`).
const DIALOG_WIDTH: f32 = 640.0;
/// Distance from the window's top to the dialog (`.scrim{padding-top:72px}`).
const DIALOG_TOP: f32 = 72.0;
/// Width of the row-label column.
const LABEL_COL: f32 = 104.0;

/// "PY/0304", or "payment of 28 Oct 2026" for a payment without a number.
fn column_name(p: &ledgerly_api::PaymentRef) -> String {
    p.number.clone().unwrap_or_else(|| {
        crate::i18n::tf(
            "home.cmp.payment_of",
            &[("date", &format::date_in(p.date, &crate::i18n::current()))],
        )
    })
}

/// A table cell of exactly `size`: egui shrinks an allocated child to its content, which would
/// collapse the columns, so the child is given a minimum size.
fn fixed(ui: &mut Ui, size: Vec2, layout: Layout, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(size, layout, |cell| {
        cell.set_min_size(size);
        add(cell);
    });
}

/// Draws one table row: the label, then the two vouchers' cells.
fn row(
    ui: &mut Ui,
    label: &str,
    cell_w: f32,
    first: impl FnOnce(&mut Ui),
    second: impl FnOnce(&mut Ui),
) {
    let t = tokens(ui);
    ui.separator();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        fixed(
            ui,
            vec2(LABEL_COL, 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(RichText::new(label).size(TEXT_BODY).color(t.text_2));
            },
        );
        fixed(
            ui,
            vec2(cell_w, 22.0),
            Layout::left_to_right(Align::Center),
            first,
        );
        fixed(
            ui,
            vec2(cell_w, 22.0),
            Layout::left_to_right(Align::Center),
            second,
        );
    });
}

/// The amount cell: the engine's amount and the "books" tag, right-aligned in its column.
fn amount_cell(ui: &mut Ui, amount: ledgerly_core::Paise, width: f32) {
    let t = tokens(ui);
    fixed(
        ui,
        vec2(width - 16.0, 22.0),
        Layout::right_to_left(Align::Center),
        |ui| {
            prov(ui, ProvKind::Books);
            ui.label(
                RichText::new(format::amount(amount))
                    .size(TEXT_BODY)
                    .color(t.text),
            );
        },
    );
}

/// Draws the comparison of one possible duplicate: a scrim over the window and the dialog of the
/// mock's `nudgeDialog` (`design/mock-v2/index.html` line 1091), so far as the engine gives the
/// facts. The bank-line row, the bank check and the buttons "Adjust rule" and "Not a duplicate"
/// are not drawn: the engine has no bank matching for these payments and no way to record that a
/// pair is not a duplicate (`design/PARITY-GAPS.md`, "Home").
#[allow(
    clippy::float_arithmetic,
    reason = "placing the dialog and sharing its columns"
)]
pub(super) fn dialog(ui: &Ui, pair: &DuplicatePayment) -> DialogChoice {
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    let k = kind(ui);
    let t = tokens(ui);
    let scrim_clicked = Area::new(ui.scope_id().with("home-nudge-scrim"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(&ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, theme::scrim(k));
            response.clicked()
        })
        .inner;
    let width = DIALOG_WIDTH.min(screen.width() - 2.0 * SPACE_3);
    let left = screen.center().x - width / 2.0;
    let mut choice = DialogChoice::None;
    Area::new(ui.scope_id().with("home-nudge-dialog"))
        .order(Order::Foreground)
        .fixed_pos(pos2(left, screen.top() + DIALOG_TOP))
        .show(&ctx, |ui| {
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(border_width(k), t.border_strong))
                .corner_radius(RADIUS_L)
                .shadow(theme::shadow_2(k))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(width - 24.0);
                    choice = proposal(ui, pair);
                });
        });
    if choice == DialogChoice::None && scrim_clicked {
        choice = DialogChoice::Close;
    }
    choice
}

/// The proposal frame inside the dialog: head band, body, checks and the footer.
#[allow(
    clippy::too_many_lines,
    reason = "one frame of drawing code, read top to bottom"
)]
fn proposal(ui: &mut Ui, pair: &DuplicatePayment) -> DialogChoice {
    let t = tokens(ui);
    let k = kind(ui);
    let mut choice = DialogChoice::None;
    let frame = Frame::new()
        .fill(t.surface)
        .corner_radius(RADIUS_S)
        .show(ui, |ui| {
            Frame::new()
                .fill(t.ai_soft)
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        icon::icon(ui, Icon::Spark, t.ai_ink);
                        ui.label(
                            RichText::new(crate::i18n::t("home.cmp.possible"))
                                .font(bold_font(ui, TEXT_BODY))
                                .color(t.ai_ink),
                        );
                        ui.label(
                            RichText::new(crate::i18n::t("home.cmp.suggested"))
                                .size(TEXT_BODY)
                                .color(t.ai_ink),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(crate::i18n::t("home.cmp.deterministic"))
                                    .font(bold_font(ui, TEXT_META))
                                    .color(t.ai_ink),
                            );
                            icon::icon_sized(ui, Icon::Check, t.ai_ink, 12.0);
                            let both = crate::i18n::tf(
                                "home.cmp.and",
                                &[
                                    ("first", &column_name(&pair.first)),
                                    ("second", &column_name(&pair.second)),
                                ],
                            );
                            crate::ui::chip::chip_parts(
                                ui,
                                crate::ui::chip::Chip {
                                    icon: Some(Icon::Book),
                                    text: &both,
                                    ..crate::ui::chip::Chip::default()
                                },
                            );
                        });
                    });
                });
            Frame::new()
                .inner_margin(Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    body(ui, pair);
                });
            ui.separator();
            Frame::new()
                .fill(t.surface_2)
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(crate::i18n::t("home.cmp.rule"))
                                .size(TEXT_META)
                                .color(t.text_2),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if button_icon(
                                ui,
                                Some(Icon::Check),
                                &crate::i18n::t("home.cmp.investigate"),
                                Some("Ctrl+A"),
                                ButtonKind::Primary,
                            )
                            .clicked()
                            {
                                choice = DialogChoice::Investigate;
                            }
                        });
                    });
                });
        });
    paint_edges(ui, frame.response.rect, &t, k);
    choice
}

/// The body: the sentence, the two vouchers side by side and the check on the locked period.
#[allow(
    clippy::float_arithmetic,
    reason = "sharing the table's width between two columns"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one table of drawing code, read top to bottom"
)]
fn body(ui: &mut Ui, pair: &DuplicatePayment) {
    let t = tokens(ui);
    let tr = crate::i18n::current();
    let days = pair.days_apart;
    let sentence = crate::i18n::tf(
        if days == 1 {
            "home.cmp.sentence_one"
        } else {
            "home.cmp.sentence_many"
        },
        &[("days", &days.to_string())],
    );
    ui.label(RichText::new(sentence).size(TEXT_BODY).color(t.text_2));
    ui.add_space(SPACE_2);
    let cell_w = (ui.available_width() - LABEL_COL) / 2.0;
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(LABEL_COL);
        for p in [&pair.first, &pair.second] {
            fixed(
                ui,
                vec2(cell_w, 22.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(column_name(p))
                            .size(TEXT_BODY)
                            .color(t.text_2),
                    );
                },
            );
        }
    });
    let plain = |text: String| {
        move |ui: &mut Ui| {
            let t = tokens(ui);
            ui.label(RichText::new(text).size(TEXT_BODY).color(t.text));
        }
    };
    let first_locked = pair.first.locked;
    let second_locked = pair.second.locked;
    let date_cell = |p: &ledgerly_api::PaymentRef, locked: bool| {
        let text = format::date_in(p.date, &crate::i18n::current());
        move |ui: &mut Ui| {
            let t = tokens(ui);
            ui.label(RichText::new(text).size(TEXT_BODY).color(t.text));
            if locked {
                badge(ui, BadgeKind::Neutral, &crate::i18n::t("home.cmp.locked"));
            }
        }
    };
    row(
        ui,
        &tr.t("home.cmp.date"),
        cell_w,
        date_cell(&pair.first, first_locked),
        date_cell(&pair.second, second_locked),
    );
    row(
        ui,
        &tr.t("home.cmp.ledger"),
        cell_w,
        plain(pair.ledger.clone()),
        plain(pair.ledger.clone()),
    );
    let amount = pair.amount;
    row(
        ui,
        &tr.t("home.cmp.amount"),
        cell_w,
        move |ui| amount_cell(ui, amount, cell_w),
        move |ui| amount_cell(ui, amount, cell_w),
    );
    row(
        ui,
        &tr.t("home.cmp.narration"),
        cell_w,
        plain(pair.narration.clone()),
        plain(pair.narration.clone()),
    );
    let bank_cell = |p: &ledgerly_api::PaymentRef| {
        let text = p.bank_date.map_or_else(
            || crate::i18n::t("home.cmp.unmatched"),
            |d| {
                let tr = crate::i18n::current();
                tr.tf("home.cmp.statement", &[("date", &format::date_in(d, &tr))])
            },
        );
        plain(text)
    };
    row(
        ui,
        &tr.t("home.cmp.bank_date"),
        cell_w,
        bank_cell(&pair.first),
        bank_cell(&pair.second),
    );
    ui.separator();
    ui.add_space(SPACE_2);
    ui.horizontal(|ui| {
        if pair.first.bank_date.is_some() && pair.second.bank_date.is_some() {
            icon::icon_sized(ui, Icon::Check, t.ok, 14.0);
            ui.label(
                RichText::new(tr.t("home.cmp.both_banked"))
                    .size(TEXT_BODY)
                    .color(t.ok),
            );
        } else {
            icon::icon_sized(ui, Icon::Info, t.text_2, 14.0);
            ui.label(
                RichText::new(tr.t("home.cmp.not_banked"))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        }
    });
    for p in [&pair.first, &pair.second] {
        if p.locked {
            ui.add_space(SPACE_2);
            ui.horizontal(|ui| {
                icon::icon_sized(ui, Icon::Alert, t.warn, 14.0);
                ui.label(
                    RichText::new(tr.tf("home.cmp.locked_note", &[("name", &column_name(p))]))
                        .size(TEXT_BODY)
                        .color(t.warn),
                );
            });
        }
    }
}

/// Paints the left bar and the dashed outline of an open proposal, as `ui::prop` does.
fn paint_edges(ui: &Ui, rect: Rect, t: &theme::Tokens, k: theme::ThemeKind) {
    let bar = if k == theme::ThemeKind::HighContrast {
        6.0
    } else {
        4.0
    };
    let rim = Stroke::new(border_width(k), t.ai);
    let painter = ui.painter();
    painter.rect_filled(
        Rect::from_min_max(rect.min, pos2(rect.min.x + bar, rect.max.y)),
        CornerRadius::ZERO,
        t.ai,
    );
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(Shape::dashed_line(&corners, rim, 6.0, 4.0));
}
