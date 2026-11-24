//! Drawing of the GST screen, laid out as the mock lays it out (`design/mock-v2/index.html`,
//! `SCREENS.gst` at lines 1665-1712): the page head with the Export JSON button, the tabs with their
//! keys, then per tab:
//!
//! * **GSTR-1**: four status tiles, then the `g-main` grid with the voucher table in a card on the
//!   left and "What happens next" on the right.
//! * **GSTR-3B**: the `g-main` grid with the table of the engine's working in a card on the left
//!   and "Read this before filing" on the right.
//! * **2B and IMS**: the empty state while nothing is imported, or the match table.
//!
//! The screen sums nothing (rule C4): every amount is one the engine's return or working holds,
//! and the tiles count rows the engine bucketed. Where the mock shows a figure the engine does not
//! produce, the row is left out (`design/PARITY-GAPS.md`).

use egui::{
    Align, Frame, Layout, Margin, Rect, Response, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, vec2,
};

use super::look::{self, Line3b};
use super::worker::Figures;
use super::{
    Bucket, ExceptionItem, Feed, GstCmd, GstScreen, GstTab, ImsAction, ReconRow, UiEvent,
    amount_cell, difference_cell,
};
use crate::ScreenAction;
use crate::i18n::Tr;
use crate::screens::home::draw::{split, title};
use crate::screens::wire::unsupported_text;
use crate::theme::{
    RADIUS_M, RADIUS_XS, SPACE_3, SPACE_6, TEXT_BODY, TEXT_CAP, TEXT_FIG, TEXT_H2, TEXT_META,
    border_width, shadow_1,
};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::icon::{self, Icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT};
use crate::ui::{bold_font, kind, para, tokens};
use ledgerly_core::Paise;

/// Width of the Voucher column of the GSTR-1 table.
const VOUCHER_W: f32 = 120.0;
/// Width of the Status column of the GSTR-1 table.
const STATUS_W: f32 = 150.0;
/// Width of each figure column of the GSTR-3B table.
const FIGURE_W: f32 = 84.0;
/// Width of the amount column of the figures table.
const AMOUNT_W: f32 = 130.0;
/// Gap between a label and its source chip.
const CHIP_GAP: f32 = 8.0;
/// Height of a source chip.
const CHIP_H: f32 = 16.0;
/// Space above and below a cell's text (`td{padding:7px 10px}`, index.html:150).
const ROW_PAD_Y: f32 = 7.0;

impl GstScreen {
    /// Reads the keys, draws the screen and applies what was clicked.
    pub(super) fn draw(&mut self, ui: &mut Ui) -> ScreenAction {
        let mut action = ScreenAction::Stay;
        for cmd in read_keys(ui) {
            if matches!(self.apply(cmd), ScreenAction::Pop) {
                action = ScreenAction::Pop;
            }
        }
        if let Some((name, json)) = self.take_json_export() {
            self.notice = Some(save_json_file(&name, &json, &self.tr));
        }
        let mut events = Vec::new();
        self.draw_head(ui, &mut events);
        if let Some(error) = self.load_error() {
            note(
                ui,
                NoteKind::Bad,
                &self.tr.tf("gst.error", &[("text", error)]),
            );
            ui.add_space(SPACE_3);
        }
        if let Some(text) = &self.notice {
            note(ui, NoteKind::Info, text);
            ui.add_space(SPACE_3);
        }
        let want_scroll = self.scroll_pending;
        ScrollArea::vertical()
            .id_salt("gst-body")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.tab {
                GstTab::Gstr1 => self.draw_gstr1(ui, want_scroll, &mut events),
                GstTab::Gstr3b => self.draw_gstr3b(ui, &mut events),
                GstTab::Reconciliation => self.draw_recon(ui, want_scroll, &mut events),
            });
        self.scroll_pending = false;
        for event in events {
            self.apply_event(event);
        }
        action
    }

    /// The page head and the tabs. The mock's period button (Alt+F2) is not drawn: the screen
    /// cannot change its period yet (`design/PARITY-GAPS.md`).
    fn draw_head(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let subtitle = self.subtitle_in(&self.tr);
        let blocked = self.export_blocked_reason();
        let title = self.tr.t("gst.title");
        let export = self.tr.t("gst.export_json");
        crate::ui::page_head::page_head(ui, &title, Some(subtitle.as_str()), |ui| {
            let response = ui
                .add_enabled_ui(blocked.is_none(), |ui| {
                    button_icon(
                        ui,
                        Some(Icon::Upload),
                        &export,
                        Some("Ctrl+E"),
                        ButtonKind::Secondary,
                    )
                })
                .inner;
            let response = match &blocked {
                Some(reason) => response.on_disabled_hover_text(reason.as_str()),
                None => response,
            };
            if response.clicked() {
                events.push(UiEvent::ExportJson);
            }
        });
        let labels: Vec<String> = GstTab::ALL.iter().map(|t| t.label_in(&self.tr)).collect();
        let items: Vec<(&str, &str)> = GstTab::ALL
            .iter()
            .zip(&labels)
            .map(|(t, label)| (label.as_str(), t.key_hint()))
            .collect();
        let selected = GstTab::ALL
            .iter()
            .position(|t| *t == self.tab)
            .unwrap_or_default();
        let clicked = crate::ui::tabs::tabs_with_keys(ui, &items, selected).clicked;
        if let Some(tab) = clicked.and_then(|i| GstTab::ALL.get(i)) {
            events.push(UiEvent::Tab(*tab));
        }
    }

    // ---- GSTR-1 ------------------------------------------------------------------------------

    fn draw_gstr1(&self, ui: &mut Ui, want_scroll: bool, events: &mut Vec<UiEvent>) {
        self.draw_tiles(ui, events);
        ui.add_space(SPACE_3);
        split(
            ui,
            |ui| {
                self.draw_exceptions(ui, &self.gstr1, want_scroll, events);
                if !self.figures1.rows.is_empty() || !self.figures1.notes.is_empty() {
                    ui.add_space(SPACE_3);
                    draw_figures(ui, &self.figures1, &self.tr);
                }
            },
            |ui| self.draw_next_steps(ui),
        );
    }

    /// The four status tiles (the mock's `.kpi` in a `g4` grid). Each counts the vouchers the
    /// engine put in a bucket; the screen only counts rows (rule C4). Clicking a tile shows only
    /// that bucket in the table, and clicking it again shows every voucher.
    fn draw_tiles(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.columns(Bucket::ALL.len(), |cols| {
            for (col, bucket) in cols.iter_mut().zip(Bucket::ALL) {
                let value = match self.bucket_count(GstTab::Gstr1, bucket) {
                    Some(n) => n.to_string(),
                    None => self.tr.t("gst.not_loaded"),
                };
                let on = self.bucket_filter == Some(bucket);
                let label = bucket.label_in(&self.tr);
                let foot = self.tr.t("gst.kpi.vouchers");
                if kpi(col, &label, &value, &foot, on).clicked() {
                    events.push(UiEvent::Filter(if on { None } else { Some(bucket) }));
                }
            }
        });
    }

    /// "What happens next", with a warning above it when vouchers are held back.
    fn draw_next_steps(&self, ui: &mut Ui) {
        let held = self
            .bucket_count(GstTab::Gstr1, Bucket::NeedsCorrection)
            .unwrap_or(0);
        if held > 0 {
            let text = self
                .tr
                .tf("gst.held_back", &[("count", held.to_string().as_str())]);
            note(ui, NoteKind::Warn, &text);
            ui.add_space(SPACE_3);
        }
        let steps = keyed(&NEXT_STEPS, &self.tr);
        crate::ui::card::card(ui, |ui| {
            title(ui, &self.tr.t("gst.next_steps.title"));
            bullets(ui, &steps);
        });
    }

    /// The vouchers of a return in a card: Voucher, Detail and the engine's bucket as a badge.
    /// Click selects a row, double click or Enter opens its voucher.
    fn draw_exceptions(
        &self,
        ui: &mut Ui,
        feed: &Feed<Vec<ExceptionItem>>,
        want_scroll: bool,
        events: &mut Vec<UiEvent>,
    ) {
        table_card(ui, |ui| {
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            let [voucher, detail, status] = exception_columns(head);
            let voucher_head = self.tr.t("gst.col.voucher");
            let detail_head = self.tr.t("gst.col.detail");
            let status_head = self.tr.t("gst.col.status");
            paint_heads(
                ui,
                head,
                &[
                    (voucher, voucher_head.as_str()),
                    (detail, detail_head.as_str()),
                    (status, status_head.as_str()),
                ],
                &[],
            );
            let Feed::Loaded(items) = feed else {
                padded(ui, |ui| draw_not_ready(ui, feed, &self.tr));
                return;
            };
            let visible = self.visible_exceptions(feed);
            if visible.is_empty() {
                padded(ui, |ui| {
                    para(
                        ui,
                        &self.tr.t("gst.no_vouchers"),
                        TEXT_BODY,
                        tokens(ui).text_3,
                    );
                });
            }
            for (pos, &index) in visible.iter().enumerate() {
                let Some(item) = items.get(index) else {
                    continue;
                };
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::click());
                let state = RowState {
                    selected: pos == self.focus,
                    hovered: response.hovered(),
                    sub: false,
                };
                paint_exception(ui, rect, item, state, &self.tr);
                if state.selected && want_scroll {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    events.push(UiEvent::Focus(pos));
                }
                if response.double_clicked() {
                    events.push(UiEvent::Drill(pos));
                }
            }
        });
    }

    // ---- GSTR-3B -----------------------------------------------------------------------------

    fn draw_gstr3b(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let mut side = Vec::new();
        split(
            ui,
            |ui| self.draw_3b_table(ui),
            |ui| {
                crate::ui::card::card(ui, |ui| {
                    title(ui, &self.tr.t("gst.notes.title"));
                    let notes = keyed(&GSTR3B_NOTES, &self.tr);
                    bullets(ui, &notes);
                    ui.add_space(10.0);
                    let tab = GstTab::Reconciliation;
                    let tab_label = tab.label_in(&self.tr);
                    if button(ui, &tab_label, Some(tab.key_hint()), ButtonKind::Secondary).clicked()
                    {
                        side.push(UiEvent::Tab(tab));
                    }
                });
            },
        );
        events.extend(side);
    }

    /// The engine's GSTR-3B working as the mock's table, with the working's notes under it.
    fn draw_3b_table(&self, ui: &mut Ui) {
        table_card(ui, |ui| {
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            let [label, figures @ ..] = gstr3b_columns(head);
            let [heading, numeric @ ..] = look::GSTR3B_COLUMNS;
            let heading = look::text_in(heading, &self.tr);
            let numeric = keyed_words(&numeric, &self.tr);
            let right: Vec<(Rect, &str)> = figures
                .into_iter()
                .zip(numeric.iter().map(String::as_str))
                .collect();
            paint_heads(ui, head, &[(label, heading.as_str())], &right);
            if self.lines3b.is_empty() {
                padded(ui, |ui| draw_not_ready(ui, &self.gstr3b, &self.tr));
                return;
            }
            for line in &self.lines3b {
                paint_3b_line(ui, width, line, &self.tr);
            }
            if !self.figures3b.notes.is_empty() {
                padded(ui, |ui| {
                    let t = tokens(ui);
                    for line in &self.figures3b.notes {
                        para(ui, line, TEXT_META, t.text_3);
                    }
                });
            }
        });
    }

    // ---- 2B and IMS --------------------------------------------------------------------------

    fn draw_recon(&self, ui: &mut Ui, want_scroll: bool, events: &mut Vec<UiEvent>) {
        let rows = match &self.recon {
            Feed::Loaded(rows) => rows,
            Feed::NotLoaded => {
                self.draw_recon_empty(ui);
                return;
            }
            Feed::Unsupported(_) => {
                crate::ui::card::card(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    draw_not_ready(ui, &self.recon, &self.tr);
                });
                return;
            }
        };
        self.draw_recon_chips(ui, events);
        ui.add_space(SPACE_3);
        let visible = self.visible_recon();
        crate::ui::card::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::Grid::new("gst-recon").striped(true).show(ui, |ui| {
                for head in [
                    "gst.recon.col.match",
                    "gst.recon.col.supplier",
                    "gst.recon.col.invoice",
                    "gst.recon.col.taxable_books",
                    "gst.recon.col.taxable_2b",
                    "gst.recon.col.tax_books",
                    "gst.recon.col.tax_2b",
                    "gst.recon.col.difference",
                    "gst.recon.col.ims_action",
                ] {
                    ui.weak(self.tr.t(head));
                }
                ui.end_row();
                for (pos, &index) in visible.iter().enumerate() {
                    let Some(row) = rows.get(index) else {
                        continue;
                    };
                    let focused = pos == self.focus;
                    select_cell(
                        ui,
                        focused && want_scroll,
                        focused,
                        pos,
                        &row.state.label_in(&self.tr),
                        events,
                    );
                    ui.vertical(|ui| {
                        ui.label(row.supplier.as_str());
                        ui.weak(row.gstin.as_str());
                    });
                    ui.vertical(|ui| {
                        ui.label(row.invoice_no.as_str());
                        ui.weak(row.dates_text());
                    });
                    ui.label(amount_cell(row.taxable_books));
                    ui.label(amount_cell(row.taxable_2b));
                    ui.label(amount_cell(row.tax_books));
                    ui.label(amount_cell(row.tax_2b));
                    ui.label(difference_cell(row.difference));
                    self.draw_ims_cell(ui, index, row, events);
                    ui.end_row();
                }
            });
        });
    }

    /// The empty state of the 2B and IMS tab (the mock's `.empty` in a card, index.html:1692),
    /// without the import button: this version cannot read a 2B file.
    fn draw_recon_empty(&self, ui: &mut Ui) {
        let (heading, sentence) = look::recon_empty_in(&self.period_label, &self.tr);
        crate::ui::card::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let t = tokens(ui);
            ui.vertical_centered(|ui| {
                // `.empty{padding:48px 24px}`, less the card's own padding.
                ui.add_space(SPACE_6 + 10.0);
                let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                icon::paint(ui.painter(), rect, Icon::Upload, t.text_3);
                ui.add_space(10.0);
                ui.label(
                    RichText::new(heading)
                        .font(bold_font(ui, TEXT_H2))
                        .color(t.text),
                );
                ui.add_space(6.0);
                ui.scope(|ui| {
                    ui.set_max_width(420.0);
                    ui.label(RichText::new(sentence).size(TEXT_BODY).color(t.text_2));
                });
                ui.add_space(SPACE_6 + 10.0);
            });
        });
    }

    /// The filter chips of the match table: every bucket and the rows still needing an action.
    fn draw_recon_chips(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let Some(total) = self.item_count(GstTab::Reconciliation) else {
            return;
        };
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.bucket_filter.is_none(),
                    self.tr
                        .tf("gst.filter.all", &[("total", total.to_string().as_str())]),
                )
                .clicked()
            {
                events.push(UiEvent::Filter(None));
            }
            for bucket in Bucket::ALL {
                let n = self
                    .bucket_count(GstTab::Reconciliation, bucket)
                    .unwrap_or(0);
                let text = self.tr.tf(
                    "gst.filter.bucket",
                    &[
                        ("bucket", bucket.label_in(&self.tr).as_str()),
                        ("count", n.to_string().as_str()),
                    ],
                );
                if ui
                    .selectable_label(self.bucket_filter == Some(bucket), text)
                    .clicked()
                {
                    events.push(UiEvent::Filter(Some(bucket)));
                }
            }
            let text = self.tr.tf(
                "gst.open_actions",
                &[("count", self.open_count().to_string().as_str())],
            );
            if ui.selectable_label(self.open_only, text).clicked() {
                events.push(UiEvent::OpenOnly(!self.open_only));
            }
        });
    }

    fn draw_ims_cell(&self, ui: &mut Ui, index: usize, row: &ReconRow, events: &mut Vec<UiEvent>) {
        if !row.in_2b() {
            ui.weak(self.tr.t("gst.ask_supplier"));
            return;
        }
        let decision = self.decision(index);
        ui.horizontal(|ui| {
            for action in ImsAction::ALL {
                if ui
                    .selectable_label(decision == Some(action), action.label_in(&self.tr))
                    .clicked()
                {
                    events.push(UiEvent::Decide(index, action));
                }
            }
            if decision.is_none() {
                ui.weak(self.tr.tf(
                    "gst.suggested",
                    &[("action", row.suggested.label_in(&self.tr).as_str())],
                ));
            }
        });
    }
}

// ---- pieces -------------------------------------------------------------------------------------

/// A card with no padding, for a table that runs to its edges (the mock's `.card` around a
/// `table.t`).
fn table_card(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let t = tokens(ui);
    let k = kind(ui);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            add_contents(ui);
        });
}

/// Contents with the mock's `p.meta` padding inside a table card (`padding:10px 16px`).
fn padded(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
            add_contents(ui);
        });
}

/// One status tile (`.kpi`): the label, the figure at the figure size and the foot in meta
/// type. `on` draws it with the accent border, as the bucket the table is showing.
fn kpi(ui: &mut Ui, label: &str, value: &str, foot: &str, on: bool) -> Response {
    let t = tokens(ui);
    let k = kind(ui);
    let edge = if on { t.accent } else { t.border };
    let inner = Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), edge))
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(16, 14))
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(label).size(TEXT_BODY).color(t.text_2));
            ui.add_space(6.0);
            ui.label(
                RichText::new(value)
                    .font(bold_font(ui, TEXT_FIG))
                    .color(t.text),
            );
            ui.add_space(2.0);
            ui.label(RichText::new(foot).size(TEXT_META).color(t.text_3));
        });
    let id = ui.scope_id().with(("gst-kpi", label));
    let response = ui.interact(inner.response.rect, id, Sense::click());
    let spoken = format!("{label}, {value} {foot}");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, spoken.as_str()));
    response
}

/// A bulleted list in secondary ink (the mock's `ul.t2` with `gap:6px`).
fn bullets(ui: &mut Ui, lines: &[String]) {
    let t = tokens(ui);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            ui.add_space(6.0);
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(6.0);
            ui.label(RichText::new("\u{2022}").size(TEXT_BODY).color(t.text_2));
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(RichText::new(line.as_str()).size(TEXT_BODY).color(t.text_2))
                    .wrap(),
            );
        });
    }
}

/// Paints table headings: `left` labels from the cell's padding, `right` labels ending at it.
fn paint_heads(ui: &Ui, head: Rect, left: &[(Rect, &str)], right: &[(Rect, &str)]) {
    let t = tokens(ui);
    table::paint_head_rule(ui, head);
    let font = bold_font(ui, TEXT_META);
    for (r, label) in left {
        let place = CellPlace::Left(table::CELL_PAD_X);
        table::paint_cell_text(ui, *r, 1.0, label, font.clone(), t.text_3, place);
    }
    for (r, label) in right {
        table::paint_cell_text(ui, *r, 1.0, label, font.clone(), t.text_3, CellPlace::Right);
    }
}

/// The Voucher, Detail and Status cells of a GSTR-1 line.
#[allow(clippy::float_arithmetic, reason = "sharing a line between columns")]
fn exception_columns(line: Rect) -> [Rect; 3] {
    let voucher = Rect::from_min_size(line.min, vec2(VOUCHER_W, line.height()));
    let status = Rect::from_min_max(egui::pos2(line.right() - STATUS_W, line.top()), line.max);
    let detail = Rect::from_min_max(
        egui::pos2(voucher.right(), line.top()),
        egui::pos2(status.left(), line.bottom()),
    );
    [voucher, detail, status]
}

/// The badge of an engine bucket: its colour and icon say the same as its word.
fn bucket_badge(bucket: Bucket) -> BadgeKind {
    match bucket {
        Bucket::Ready => BadgeKind::Ok,
        Bucket::NotRelevant => BadgeKind::Neutral,
        Bucket::Mismatch => BadgeKind::Bad,
        Bucket::NeedsCorrection => BadgeKind::Warn,
    }
}

/// One GSTR-1 voucher: its title, the engine's detail and its bucket as a badge.
fn paint_exception(ui: &mut Ui, rect: Rect, item: &ExceptionItem, state: RowState, tr: &Tr) {
    let t = tokens(ui);
    table::paint_row(ui, rect, state);
    let [voucher, detail, status] = exception_columns(rect);
    let font = egui::FontId::proportional(TEXT_BODY);
    let left = CellPlace::Left(table::CELL_PAD_X);
    table::paint_cell_text(ui, voucher, 1.0, &item.title, font.clone(), t.text, left);
    table::paint_cell_text(ui, detail, 1.0, &item.detail, font, t.text_2, left);
    let cell = status.shrink2(vec2(table::CELL_PAD_X, 0.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(cell)
            .layout(Layout::left_to_right(Align::Center)),
    );
    // The table zeroes item spacing; the badge spaces its icon and word with it.
    child.spacing_mut().item_spacing = vec2(4.0, 0.0);
    let bucket = item.bucket.label_in(tr);
    badge(&mut child, bucket_badge(item.bucket), &bucket);
}

/// The Table cell and the five figure cells of a GSTR-3B line.
#[allow(clippy::float_arithmetic, reason = "sharing a line between columns")]
fn gstr3b_columns(line: Rect) -> [Rect; 6] {
    let figures_w = FIGURE_W * 5.0;
    let label = Rect::from_min_max(
        line.min,
        egui::pos2(line.right() - figures_w, line.bottom()),
    );
    let cell = |i: f32| {
        let x = label.right() + FIGURE_W * i;
        Rect::from_min_max(
            egui::pos2(x, line.top()),
            egui::pos2(x + FIGURE_W, line.bottom()),
        )
    };
    [label, cell(0.0), cell(1.0), cell(2.0), cell(3.0), cell(4.0)]
}

/// One line of the GSTR-3B table: a sub-heading, or a row of the engine's figures with its
/// source chip after the label, or under it when the two do not fit on one line.
#[allow(clippy::float_arithmetic, reason = "placing the label and its chip")]
fn paint_3b_line(ui: &mut Ui, width: f32, line: &Line3b, tr: &Tr) {
    let t = tokens(ui);
    let font = egui::FontId::proportional(TEXT_BODY);
    match line {
        Line3b::Sub(label) => {
            let label = look::text_in(label, tr);
            let (rect, _) = ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::hover());
            let state = RowState {
                selected: false,
                hovered: false,
                sub: true,
            };
            table::paint_row(ui, rect, state);
            let bold = bold_font(ui, TEXT_BODY);
            let place = CellPlace::Left(table::CELL_PAD_X);
            table::paint_cell_text(ui, rect, 1.0, &label, bold, t.text, place);
        }
        Line3b::Row {
            label,
            source,
            indent,
            taxable,
            tax,
        } => {
            let label_text = look::text_in(label, tr);
            let source_text = source.map(|s| look::text_in(s, tr));
            let pad = table::CELL_PAD_X + if *indent { table::indent(1) } else { 0.0 };
            let probe = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(width, T_ROW_HEIGHT));
            let [label_cell, ..] = gstr3b_columns(probe);
            let wrap = (label_cell.width() - pad - table::CELL_PAD_X).max(40.0);
            let galley = ui.painter().layout(label_text, font.clone(), t.text, wrap);
            let text = galley.size();
            let one_row = galley.rows.len() <= 1;
            let chip_w = source_text.as_deref().map_or(0.0, |s| chip_width(ui, s));
            // The chip follows the label on its line when both fit, else it goes under it.
            let chip_inline = one_row && text.x + CHIP_GAP + chip_w <= wrap;
            let chip_below = source.is_some() && !chip_inline;
            let content = text.y
                + if chip_below {
                    CHIP_GAP / 2.0 + CHIP_H
                } else {
                    0.0
                };
            let height = (content + 2.0 * ROW_PAD_Y).max(T_ROW_HEIGHT);
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
            let state = RowState {
                selected: false,
                hovered: response.hovered(),
                sub: false,
            };
            table::paint_row(ui, rect, state);
            let [label_rect, figures @ ..] = gstr3b_columns(rect);
            let top = rect.top() + (rect.height() - 1.0 - content) / 2.0;
            let text_pos = egui::pos2(label_rect.left() + pad, top);
            ui.painter()
                .with_clip_rect(label_rect)
                .galley(text_pos, galley, t.text);
            if let Some(s) = source_text.as_deref() {
                let chip_rect = if chip_inline {
                    Rect::from_min_size(
                        egui::pos2(
                            text_pos.x + text.x + CHIP_GAP,
                            top + (text.y - CHIP_H) / 2.0,
                        ),
                        vec2(chip_w, CHIP_H),
                    )
                } else {
                    Rect::from_min_size(
                        egui::pos2(text_pos.x, top + text.y + CHIP_GAP / 2.0),
                        vec2(chip_w, CHIP_H),
                    )
                };
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(chip_rect)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                source_chip(&mut child, s);
            }
            paint_3b_figures(ui, figures, *taxable, tax);
        }
    }
}

/// The taxable value (blank where the table has none) and the four tax heads of a GSTR-3B row,
/// each as the engine's working holds it.
fn paint_3b_figures(ui: &Ui, cells: [Rect; 5], taxable: Option<Paise>, tax: &[Paise; 4]) {
    let t = tokens(ui);
    let font = egui::FontId::proportional(TEXT_BODY);
    let taxable_text = taxable.map_or_else(String::new, |p| amount_cell(Some(p)));
    let texts = std::iter::once(taxable_text).chain(tax.iter().map(|p| amount_cell(Some(*p))));
    for (r, text) in cells.into_iter().zip(texts) {
        table::paint_cell_text(ui, r, 1.0, &text, font.clone(), t.text, CellPlace::Right);
    }
}

/// Width of a source chip reading `text`.
#[allow(clippy::float_arithmetic, reason = "a chip's padding")]
fn chip_width(ui: &Ui, text: &str) -> f32 {
    let font = bold_font(ui, TEXT_CAP);
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
        .size()
        .x
        + 10.0
}

/// A chip saying where a row's figures come from (the mock's `prov('books', ...)`), in the
/// colours of figures read from the books.
fn source_chip(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    let (bg, fg) = crate::ui::prov::colors(crate::ui::prov::ProvKind::Books, &t);
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_XS)
        .inner_margin(Margin::symmetric(5, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(bold_font(ui, TEXT_CAP)).color(fg));
        });
}

/// The figures a return holds, one per line, in a card table, and the notes about what is not
/// drawn.
fn draw_figures(ui: &mut Ui, figures: &Figures, tr: &Tr) {
    table_card(ui, |ui| {
        let t = tokens(ui);
        let width = ui.available_width();
        let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
        let (label, amount) = figure_columns(head);
        let from_return = look::text_in("From the return", tr);
        let amount_head = look::text_in("Amount", tr);
        paint_heads(
            ui,
            head,
            &[(label, from_return.as_str())],
            &[(amount, amount_head.as_str())],
        );
        let font = egui::FontId::proportional(TEXT_BODY);
        for figure in &figures.rows {
            let (rect, response) =
                ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::hover());
            let state = RowState {
                selected: false,
                hovered: response.hovered(),
                sub: false,
            };
            table::paint_row(ui, rect, state);
            let (l, a) = figure_columns(rect);
            let place = CellPlace::Left(table::CELL_PAD_X);
            table::paint_cell_text(ui, l, 1.0, &figure.label, font.clone(), t.text, place);
            let text = amount_cell(Some(figure.amount));
            table::paint_cell_text(ui, a, 1.0, &text, font.clone(), t.text, CellPlace::Right);
        }
        if !figures.notes.is_empty() {
            padded(ui, |ui| {
                for line in &figures.notes {
                    para(ui, line, TEXT_META, t.text_3);
                }
            });
        }
    });
}

/// The label and amount cells of a figures line.
fn figure_columns(line: Rect) -> (Rect, Rect) {
    let amount = Rect::from_min_max(egui::pos2(line.right() - AMOUNT_W, line.top()), line.max);
    let label = Rect::from_min_max(line.min, egui::pos2(amount.left(), line.bottom()));
    (label, amount)
}

/// The first cell of a match row: click selects, double click opens, and the selected row
/// scrolls into view when `scroll` is set.
fn select_cell(
    ui: &mut Ui,
    scroll: bool,
    focused: bool,
    pos: usize,
    label: &str,
    events: &mut Vec<UiEvent>,
) {
    let r = ui.selectable_label(focused, label);
    if r.clicked() {
        events.push(UiEvent::Focus(pos));
    }
    if r.double_clicked() {
        events.push(UiEvent::Drill(pos));
    }
    if scroll {
        r.scroll_to_me(None);
    }
}

/// Says why a feed shows no table.
fn draw_not_ready<T>(ui: &mut Ui, feed: &Feed<T>, tr: &Tr) {
    let t = tokens(ui);
    match feed {
        Feed::Loaded(_) => {}
        Feed::NotLoaded => {
            para(ui, &tr.t("gst.not_loaded_period"), TEXT_BODY, t.text_3);
        }
        Feed::Unsupported(u) => {
            para(ui, &unsupported_text(u), TEXT_BODY, t.bad);
        }
    }
}

/// The guidance of the GSTR-1 tab. Each line says what the screen does and nothing it does
/// not: there is no mark-as-filed and no portal upload. Sources: the `Api::gstr1_json` refusal
/// (`ledgerly-api/src/gst.rs`), the GST portal's returns page
/// (<https://www.gst.gov.in/help/returns>, read 2026-11-22), and the module notes of
/// `ledgerly-gst/src/gstr3b.rs`, which say that Ledgerly does not file.
const NEXT_STEPS: [&str; 4] = [
    "gst.next_steps.fix",
    "gst.next_steps.export",
    "gst.next_steps.upload",
    "gst.next_steps.not_filed",
];

/// The "Read this before filing" lines of GSTR-3B. They follow the engine's module notes in
/// `ledgerly-gst/src/gstr3b.rs` (what is worked out, which conditions are not checked, and the
/// reverse-charge rule), not the mock's text, which the engine does not match.
const GSTR3B_NOTES: [&str; 5] = [
    "gst.notes.not_filed",
    "gst.notes.table4",
    "gst.notes.blocked",
    "gst.notes.reverse_charge",
    "gst.notes.set_off",
];

/// The translations of translation keys, in order.
fn keyed(keys: &[&str], tr: &Tr) -> Vec<String> {
    keys.iter().map(|key| tr.t(key)).collect()
}

/// The GSTR-3B words of `words` in the words of `tr`, in order (see [`look::text_in`]).
fn keyed_words(words: &[&str], tr: &Tr) -> Vec<String> {
    words.iter().map(|word| look::text_in(word, tr)).collect()
}

/// Asks where to save the GSTR-1 JSON and writes it there. Cancelling writes nothing. The dialog
/// is the user's own choice of place; nothing is uploaded.
fn save_json_file(name: &str, json: &str, tr: &Tr) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(tr.t("gst.export.filter"), &["json"])
        .save_file()
    else {
        return tr.t("gst.export.cancelled");
    };
    let shown = path.display().to_string();
    match std::fs::write(&path, json) {
        Ok(()) => tr.tf("gst.export.saved", &[("path", shown.as_str())]),
        Err(error) => tr.tf(
            "gst.export.failed",
            &[
                ("path", shown.as_str()),
                ("error", error.to_string().as_str()),
            ],
        ),
    }
}

/// Reads the workspace keys. Nothing is read while a text field has the keyboard.
fn read_keys(ui: &Ui) -> Vec<GstCmd> {
    use egui::{Key, Modifiers};
    let ctx = ui.ctx();
    if ctx.egui_wants_keyboard_input() {
        return Vec::new();
    }
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        for tab in GstTab::ALL {
            if i.consume_key(Modifiers::NONE, tab.key()) {
                out.push(GstCmd::SwitchTab(tab));
            }
        }
        // Both keys are consumed even when the first one fired.
        let j = i.consume_key(Modifiers::NONE, Key::J);
        let down = i.consume_key(Modifiers::NONE, Key::ArrowDown);
        if j || down {
            out.push(GstCmd::Next);
        }
        let k = i.consume_key(Modifiers::NONE, Key::K);
        let up = i.consume_key(Modifiers::NONE, Key::ArrowUp);
        if k || up {
            out.push(GstCmd::Prev);
        }
        for action in ImsAction::ALL {
            if i.consume_key(Modifiers::NONE, action.key()) {
                out.push(GstCmd::Decide(action));
            }
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            out.push(GstCmd::Open);
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            out.push(GstCmd::Close);
        }
        if i.consume_key(Modifiers::CTRL, Key::E) {
            out.push(GstCmd::ExportJson);
        }
    });
    out
}
