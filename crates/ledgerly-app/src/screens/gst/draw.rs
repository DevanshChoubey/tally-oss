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
    Bucket, ExceptionItem, Feed, GstCmd, GstScreen, GstTab, ImsAction, MatchState, ReconRow,
    UiEvent, amount_cell, date_cell, section_bucket,
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
/// Width of the Records column of the GSTR-1 section table.
const RECORDS_W: f32 = 64.0;
/// Width of the Status column of the GSTR-1 section table (a badge and "Needs correction").
const SECTION_STATUS_W: f32 = 136.0;
/// Width of the Taxable and Tax columns of the GSTR-1 section table.
const SECTION_AMOUNT_W: f32 = 96.0;
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
        self.draw_period_picker(ui, &mut events);
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

    /// The page head and the tabs. The period button names the period on show and opens the
    /// period picker, as Alt+F2 does.
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
            let period = button_icon(
                ui,
                Some(Icon::Cal),
                &self.period_label,
                Some("Alt+F2"),
                ButtonKind::Secondary,
            );
            if period.clicked() {
                events.push(UiEvent::PeriodPicker);
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

    /// The open period picker: the financial year's twelve months, the highlighted one filled.
    /// Click or Enter chooses; Esc closes. The screen reloads for the chosen month.
    fn draw_period_picker(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let Some(at) = self.picker else {
            return;
        };
        crate::ui::card::card(ui, |ui| {
            title(ui, "Return period");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for (i, label) in self.picker_labels().iter().enumerate() {
                    let kind = if i == at {
                        ButtonKind::Primary
                    } else {
                        ButtonKind::Secondary
                    };
                    if button(ui, label, None, kind).clicked() {
                        events.push(UiEvent::ChoosePeriod(i));
                    }
                }
            });
            ui.add_space(6.0);
            para(
                ui,
                "J / K move, Enter chooses, Esc closes. The return is built again for the month you choose.",
                TEXT_META,
                tokens(ui).text_3,
            );
        });
        ui.add_space(SPACE_3);
    }

    /// A feed that is not loaded, or is unsupported, is one card: the heading and the reason.
    fn draw_feed_card<T>(&self, ui: &mut Ui, name: &str, feed: &Feed<T>) {
        crate::ui::card::card(ui, |ui| {
            title(ui, &format!("{name}, {}", self.period_label));
            draw_not_ready(ui, feed, &self.tr);
        });
    }

    // ---- GSTR-1 ------------------------------------------------------------------------------

    fn draw_gstr1(&self, ui: &mut Ui, want_scroll: bool, events: &mut Vec<UiEvent>) {
        if !matches!(self.gstr1, Feed::Loaded(_)) {
            self.draw_feed_card(ui, "GSTR-1", &self.gstr1);
            return;
        }
        self.draw_tiles(ui, events);
        ui.add_space(SPACE_3);
        let mut side = Vec::new();
        split(
            ui,
            |ui| {
                // With the section table on show, the voucher table behind it is folded away
                // until asked for, so the default view is the mock's (tiles, sections, card).
                let vouchers = !self.sections_loaded || self.vouchers_shown();
                if self.sections_loaded {
                    self.draw_sections(ui, events);
                    ui.add_space(SPACE_3);
                    self.draw_voucher_fold(ui, events);
                }
                if vouchers {
                    if self.sections_loaded {
                        ui.add_space(SPACE_3);
                        title(ui, "Vouchers behind the sections");
                    }
                    self.draw_exceptions(ui, &self.gstr1, want_scroll, events);
                }
                if !self.figures1.rows.is_empty() || !self.figures1.notes.is_empty() {
                    ui.add_space(SPACE_3);
                    draw_figures(ui, &self.figures1, &self.tr);
                }
            },
            |ui| self.draw_side_card(ui, &mut side),
        );
        events.extend(side);
    }

    /// The button that unfolds or folds the voucher table behind the section table.
    fn draw_voucher_fold(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let label = match (self.vouchers_shown(), self.item_count(GstTab::Gstr1)) {
            (true, _) => self.tr.lit("Hide vouchers"),
            (false, Some(n)) => self.tr.tf("gst.show_vouchers_n", &[("n", &n.to_string())]),
            (false, None) => self.tr.lit("Show vouchers"),
        };
        if button(ui, &label, None, ButtonKind::Secondary).clicked() {
            events.push(UiEvent::ToggleVouchers);
        }
    }

    /// The four status tiles (the mock's `.kpi` in a `g4` grid). Each counts the vouchers the
    /// engine put in a bucket; the screen only counts rows (rule C4). Clicking a tile shows only
    /// that bucket in the table, and clicking it again shows every voucher.
    fn draw_tiles(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.columns(Bucket::ALL.len(), |cols| {
            for (col, bucket) in cols.iter_mut().zip(Bucket::ALL) {
                // The mock's tiles count sections; with no section table, vouchers.
                let (count, unit) = if self.sections_loaded {
                    (self.section_count(bucket), self.tr.lit("sections"))
                } else {
                    (
                        self.bucket_count(GstTab::Gstr1, bucket),
                        self.tr.lit("vouchers"),
                    )
                };
                let value = match count {
                    Some(n) => n.to_string(),
                    None => self.tr.t("gst.not_loaded"),
                };
                let on = self.bucket_filter == Some(bucket);
                if kpi(col, &self.tr.lit(bucket.label()), &value, &unit, on).clicked() {
                    events.push(UiEvent::Filter(if on { None } else { Some(bucket) }));
                }
            }
        });
    }

    /// The section table: one row per table of the return with its records, taxable value, tax
    /// and the engine's status (the mock's `#g1`). The amounts are the engine's; `none` where a
    /// table has no such figure.
    #[allow(
        clippy::too_many_lines,
        reason = "one drawing pass of the screen, a line for each part of it"
    )]
    fn draw_sections(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        table_card(ui, |ui| {
            let t = tokens(ui);
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            let cols = section_columns(head);
            paint_heads(
                ui,
                head,
                &[
                    (cols[0], &self.tr.lit("Section")),
                    (cols[4], &self.tr.lit("Status")),
                ],
                &[],
            );
            paint_heads(
                ui,
                head,
                &[],
                &[
                    (cols[1], &self.tr.lit("Records")),
                    (cols[2], &self.tr.lit("Taxable")),
                    (cols[3], &self.tr.lit("Tax")),
                ],
            );
            let font = egui::FontId::proportional(TEXT_BODY);
            let left = CellPlace::Left(table::CELL_PAD_X);
            for (pos, section) in self.sections.iter().enumerate() {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::click());
                if response.double_clicked() {
                    events.push(UiEvent::OpenSection(pos));
                } else if response.clicked() {
                    events.push(UiEvent::Section(pos));
                }
                let state = RowState {
                    selected: pos == self.section_focus,
                    hovered: response.hovered(),
                    sub: false,
                };
                table::paint_row(ui, rect, state);
                let cols = section_columns(rect);
                let label = if section.code == "held" {
                    section.name.to_owned()
                } else {
                    format!("{} {}", section.code, section.name)
                };
                let records = section.records.to_string();
                let taxable = amount_cell(section.taxable);
                let tax = amount_cell(section.tax);
                table::paint_cell_text(ui, cols[0], 1.0, &label, font.clone(), t.text, left);
                let right = CellPlace::Right;
                table::paint_cell_text(ui, cols[1], 1.0, &records, font.clone(), t.text, right);
                table::paint_cell_text(ui, cols[2], 1.0, &taxable, font.clone(), t.text, right);
                table::paint_cell_text(ui, cols[3], 1.0, &tax, font.clone(), t.text, right);
                let bucket = section_bucket(section.status);
                let cell = cols[4].shrink2(vec2(table::CELL_PAD_X, 0.0));
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(cell)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                child.spacing_mut().item_spacing = vec2(4.0, 0.0);
                badge(
                    &mut child,
                    bucket_badge(bucket),
                    &crate::i18n::lit(bucket.label()),
                );
                crate::ui::a11y::name(
                    ui,
                    &response,
                    egui::accesskit::Role::Row,
                    &crate::ui::a11y::join(&[
                        label.as_str(),
                        records.as_str(),
                        taxable.as_str(),
                        tax.as_str(),
                        bucket.label(),
                    ]),
                );
            }
            if let Some(line) = &self.agreement {
                padded(ui, |ui| {
                    para(ui, line, TEXT_META, t.text_3);
                });
            }
        });
    }

    /// The card beside the section table. With a section that needs correction selected it names
    /// what is wrong, in the engine's words, and offers the fix (the mock's `gstr1-correction`
    /// card, index.html:1688); otherwise it is "What happens next".
    fn draw_side_card(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let Some(section) = self.selected_needs_correction() else {
            self.draw_next_steps(ui);
            return;
        };
        let t = tokens(ui);
        crate::ui::card::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                icon::icon(ui, Icon::Alert, t.warn);
                title(ui, &self.side_card_title());
            });
            if let Some(reason) = &section.note {
                para(ui, reason, TEXT_BODY, t.text);
                ui.add_space(6.0);
            }
            let text = format!(
                "The {} table cannot be complete without it, so Export JSON is refused until it \
                 is fixed. Nothing is guessed.",
                section.name
            );
            para(ui, &text, TEXT_BODY, t.text_2);
            if self.side_card_offers_the_item_master() {
                ui.add_space(SPACE_3);
                let fix = button_icon(
                    ui,
                    None,
                    "Fix the item master",
                    Some("Enter"),
                    ButtonKind::Primary,
                );
                if fix.clicked() {
                    events.push(UiEvent::OpenSection(self.section_focus));
                }
            }
        });
    }

    /// "What happens next", with a warning above it when vouchers are held back.
    fn draw_next_steps(&self, ui: &mut Ui) {
        let held = self
            .bucket_count(GstTab::Gstr1, Bucket::NeedsCorrection)
            .unwrap_or(0);
        // The section table already marks the section that needs correction, as the mock does.
        if held > 0 && !self.sections_loaded {
            let text = format!(
                "{held} voucher(s) need correction. The JSON is held back until they are fixed; \
                 the reason is in the table."
            );
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
                crate::ui::a11y::name(
                    ui,
                    &response,
                    egui::accesskit::Role::Row,
                    &exception_name(item),
                );
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
        if !matches!(self.gstr3b, Feed::Loaded(_)) {
            self.draw_feed_card(ui, "GSTR-3B working", &self.gstr3b);
            return;
        }
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
        if let Some(reason) = &self.import_error {
            Self::draw_import_error(ui, reason, events);
            ui.add_space(SPACE_3);
        }
        let rows = match &self.recon {
            Feed::Loaded(rows) => rows,
            Feed::NotLoaded => {
                if self.import_error.is_none() {
                    self.draw_recon_empty(ui, events);
                }
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
        if rows.is_empty() {
            match &self.recon_file {
                // After a refused file the mock shows the refusal alone, not the empty card.
                None if self.import_error.is_some() => {}
                None => self.draw_recon_empty(ui, events),
                Some(name) => {
                    let text = format!("The 2B file {name} holds no documents.");
                    note(ui, NoteKind::Info, &text);
                }
            }
            return;
        }
        let visible = self.visible_recon();
        let selected = self.focused_index();
        let mut side = Vec::new();
        split(
            ui,
            |ui| self.draw_recon_table(ui, rows, &visible, want_scroll, events),
            |ui| {
                if let Some((index, row)) = selected.and_then(|i| rows.get(i).map(|r| (i, r))) {
                    self.draw_recon_side(ui, index, row, &mut side);
                }
            },
        );
        events.extend(side);
    }

    /// The table of the 2B documents: Supplier, Invoice, 2B, Books, Engine status, IMS action.
    fn draw_recon_table(
        &self,
        ui: &mut Ui,
        rows: &[ReconRow],
        visible: &[usize],
        want_scroll: bool,
        events: &mut Vec<UiEvent>,
    ) {
        table_card(ui, |ui| {
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            let c = recon_columns(head);
            paint_heads(
                ui,
                head,
                &[
                    (c[0], "Supplier"),
                    (c[1], "Invoice"),
                    (c[4], "Engine status"),
                    (c[5], "IMS action"),
                ],
                &[(c[2], "2B"), (c[3], "Books")],
            );
            for (pos, &index) in visible.iter().enumerate() {
                let Some(row) = rows.get(index) else {
                    continue;
                };
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, RECON_ROW_H), Sense::click());
                let state = RowState {
                    selected: pos == self.focus,
                    hovered: response.hovered(),
                    sub: false,
                };
                self.paint_recon_row(ui, rect, row, self.decision(index), state);
                crate::ui::a11y::name(
                    ui,
                    &response,
                    egui::accesskit::Role::Row,
                    &crate::ui::a11y::join(&[
                        row.supplier.as_str(),
                        row.invoice_no.as_str(),
                        amount_cell(row.total_2b).as_str(),
                        amount_cell(row.total_books).as_str(),
                        status_text(row).as_str(),
                        self.decision(index).map_or("No action", ImsAction::label),
                    ]),
                );
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

    fn paint_recon_row(
        &self,
        ui: &mut Ui,
        rect: Rect,
        row: &ReconRow,
        decision: Option<ImsAction>,
        state: RowState,
    ) {
        let t = tokens(ui);
        table::paint_row(ui, rect, state);
        let c = recon_columns(rect);
        let font = egui::FontId::proportional(TEXT_BODY);
        let meta = egui::FontId::proportional(TEXT_META);
        paint_wrapped(ui, c[0], 0.0, &row.supplier, font.clone(), t.text);
        paint_wrapped(ui, c[1], 0.0, &row.invoice_no, font.clone(), t.text);
        paint_wrapped(
            ui,
            c[1],
            LINE_H,
            &date_cell(row.portal_date),
            meta.clone(),
            t.text_3,
        );
        let right = CellPlace::Right;
        table::paint_cell_text(
            ui,
            c[2],
            1.0,
            &amount_cell(row.total_2b),
            font.clone(),
            t.text,
            right,
        );
        let (books_text, books_color) = match row.total_books {
            Some(_) => (amount_cell(row.total_books), t.text),
            None => ("none".to_owned(), t.text_3),
        };
        table::paint_cell_text(ui, c[3], 1.0, &books_text, font.clone(), books_color, right);
        cell_row(ui, c[4], |ui| {
            badge(ui, status_kind(row), &status_text(row));
        });
        let locked = self.filed_on.is_some();
        let badges = Rect::from_min_size(c[5].min + vec2(0.0, 12.0), vec2(c[5].width(), 20.0));
        match decision {
            Some(action) => cell_row(ui, badges, |ui| {
                badge(ui, action_kind(action), &crate::i18n::lit(action.label()));
            }),
            None => paint_wrapped(
                ui,
                c[5],
                0.0,
                &crate::i18n::lit("No action"),
                font,
                t.text_2,
            ),
        }
        if locked {
            if decision.is_some() {
                let second = badges.translate(vec2(0.0, 22.0));
                cell_row(ui, second, |ui| {
                    badge(ui, BadgeKind::Neutral, &crate::i18n::lit("Final"));
                });
            }
        } else {
            let text = crate::i18n::tf(
                "gst.suggested_row",
                &[("action", &crate::i18n::lit(row.suggested.label()))],
            );
            paint_wrapped(ui, c[5], LINE_H, &text, meta, t.text_3);
        }
    }

    /// The card beside the table: the selected document, what the engine found and the three
    /// actions. When GSTR-3B was marked filed it is the lock bar instead.
    fn draw_recon_side(
        &self,
        ui: &mut Ui,
        index: usize,
        row: &ReconRow,
        events: &mut Vec<UiEvent>,
    ) {
        let t = tokens(ui);
        if let Some(filed) = self.filed_on {
            let (lead, rest) = look::lock_words(&self.period_label, &date_long(filed));
            lock_bar_card(ui, &lead, &rest);
            return;
        }
        crate::ui::card::card(ui, |ui| {
            title(ui, &format!("{} \u{00B7} {}", row.supplier, row.invoice_no));
            let line = match (row.state, row.total_2b, row.total_books, row.difference) {
                (MatchState::ValueDiffers, Some(b2), Some(bk), Some(d)) => format!(
                    "2B shows {}; your books show {}. Difference {} (engine).",
                    amount_cell(Some(b2)),
                    amount_cell(Some(bk)),
                    amount_cell(Some(d))
                ),
                _ => status_text(row),
            };
            para(ui, &line, TEXT_BODY, t.text_2);
            ui.add_space(SPACE_3);
            ui.horizontal(|ui| {
                for action in ImsAction::ALL {
                    let key = match action {
                        ImsAction::Accept => "A",
                        ImsAction::Reject => "R",
                        ImsAction::Pending => "P",
                    };
                    if button(
                        ui,
                        &crate::i18n::lit(action.label()),
                        Some(key),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        events.push(UiEvent::Decide(index, action));
                    }
                }
            });
            ui.add_space(SPACE_3);
            para(
                ui,
                "IMS actions are yours. Ledgerly records them and lists them for you to apply on the portal.",
                TEXT_META,
                t.text_3,
            );
            ui.add_space(SPACE_3);
            if button(ui, "Mark GSTR-3B filed", None, ButtonKind::Secondary).clicked() {
                events.push(UiEvent::MarkFiled);
            }
            ui.add_space(4.0);
            para(
                ui,
                "Mark it once you have filed on the portal. The period's IMS actions then lock.",
                TEXT_META,
                t.text_3,
            );
        });
    }

    /// The empty state of the 2B and IMS tab (the mock's `recon-empty`, index.html:1692).
    fn draw_recon_empty(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let (heading, sentence) = look::recon_empty(&self.period_label);
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
                ui.add_space(SPACE_3);
                let import = button_icon(
                    ui,
                    Some(Icon::Upload),
                    "Import 2B JSON",
                    Some("Ctrl+O"),
                    ButtonKind::Primary,
                );
                if import.clicked() {
                    events.push(UiEvent::ImportFile);
                }
                ui.add_space(SPACE_6 + 10.0);
            });
        });
    }

    /// The refusal of a 2B file (the mock's `recon-error`, index.html:1693): what the engine
    /// said, that nothing changed, and a button to choose another file.
    fn draw_import_error(ui: &mut Ui, reason: &str, events: &mut Vec<UiEvent>) {
        crate::ui::note::note_with_icon(ui, NoteKind::Bad, Icon::Alert, |ui, ink| {
            ui.label(
                RichText::new(look::import_refusal(reason))
                    .font(bold_font(ui, TEXT_META))
                    .color(ink),
            );
            para(
                ui,
                "Nothing was imported or changed. Download the file again for this company's GSTIN.",
                TEXT_META,
                ink,
            );
            ui.add_space(8.0);
            let again = button_icon(
                ui,
                Some(Icon::Upload),
                "Choose another file",
                None,
                ButtonKind::Secondary,
            );
            if again.clicked() {
                events.push(UiEvent::ImportFile);
            }
        });
    }
}

/// Height of a row of the 2B table: two lines, as the mock's rows with their meta line.
const RECON_ROW_H: f32 = 70.0;
/// Width of the IMS action column.
const RECON_IMS_W: f32 = 89.0;
/// Width of the Engine status column.
const RECON_STATUS_W: f32 = 138.0;
/// Width of the 2B and Books amount columns.
const RECON_AMOUNT_W: f32 = 98.0;
/// Width of the Invoice column.
const RECON_INVOICE_W: f32 = 86.0;

/// The Supplier, Invoice, 2B, Books, Engine status and IMS action cells of a line.
#[allow(clippy::float_arithmetic, reason = "sharing a line between columns")]
fn recon_columns(line: Rect) -> [Rect; 6] {
    let cell = |from: f32, to: f32| {
        Rect::from_min_max(egui::pos2(from, line.top()), egui::pos2(to, line.bottom()))
    };
    let ims = cell(line.right() - RECON_IMS_W, line.right());
    let status = cell(ims.left() - RECON_STATUS_W, ims.left());
    let books = cell(status.left() - RECON_AMOUNT_W, status.left());
    let portal = cell(books.left() - RECON_AMOUNT_W, books.left());
    let invoice = cell(portal.left() - RECON_INVOICE_W, portal.left());
    let supplier = cell(line.left(), invoice.left());
    [supplier, invoice, portal, books, status, ims]
}

/// One text line height of the wrapped cells of a 2B row.
const LINE_H: f32 = 18.0;

/// Paints `text` wrapped to the cell's width, the block centred in the row and pushed down by
/// `skip` points (so a second block can sit under the first).
#[allow(clippy::float_arithmetic, reason = "placing wrapped text in a cell")]
fn paint_wrapped(
    ui: &Ui,
    cell: Rect,
    skip: f32,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) {
    let width = cell.width() - 2.0 * table::CELL_PAD_X;
    let galley = ui
        .painter()
        .layout(text.to_owned(), font, color, width.max(1.0));
    let y = cell.top() + 12.0 + skip;
    ui.painter().with_clip_rect(cell).galley(
        egui::pos2(cell.left() + table::CELL_PAD_X, y),
        galley,
        color,
    );
}

/// Lays `add` out left to right in `cell`, centred, with the table's cell padding.
fn cell_row(ui: &mut Ui, cell: Rect, add: impl FnOnce(&mut Ui)) {
    let inner = cell.shrink2(vec2(table::CELL_PAD_X, 0.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing = vec2(4.0, 0.0);
    add(&mut child);
}

/// The words of the Engine status badge: the engine's state, and how it paired the document.
fn status_text(row: &ReconRow) -> String {
    let word = match row.state {
        MatchState::ValueDiffers => "Differs",
        MatchState::DateDiffers => "Date differs",
        MatchState::Matched
        | MatchState::NotInBooks
        | MatchState::PossibleDuplicate
        | MatchState::NotIn2b
        | MatchState::NotComparable => row.state.label(),
    };
    let word = crate::i18n::lit(word);
    match row.level {
        Some(level) => format!("{word} \u{00B7} {level}"),
        None => word,
    }
}

fn status_kind(row: &ReconRow) -> BadgeKind {
    match row.state {
        MatchState::Matched => BadgeKind::Ok,
        MatchState::ValueDiffers
        | MatchState::DateDiffers
        | MatchState::PossibleDuplicate
        | MatchState::NotComparable => BadgeKind::Warn,
        MatchState::NotInBooks | MatchState::NotIn2b => BadgeKind::Bad,
    }
}

fn action_kind(action: ImsAction) -> BadgeKind {
    match action {
        ImsAction::Accept => BadgeKind::Ok,
        ImsAction::Reject => BadgeKind::Bad,
        ImsAction::Pending => BadgeKind::Warn,
    }
}

/// `20 Nov 2026`, for a sentence.
fn date_long(date: ledgerly_core::Date) -> String {
    crate::ui::format::date(date)
}

/// The lock bar (`.lockbar`): the lock icon, the lead in bold and the rest, on the soft red.
fn lock_bar_card(ui: &mut Ui, lead: &str, rest: &str) {
    let t = tokens(ui);
    Frame::new()
        .fill(t.bad_soft)
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                icon::icon(ui, Icon::Lock, t.bad);
                let mut job = egui::text::LayoutJob::default();
                job.wrap.max_width = ui.available_width();
                let line = crate::ui::line_height(TEXT_META);
                job.append(
                    lead,
                    0.0,
                    egui::TextFormat {
                        font_id: bold_font(ui, TEXT_META),
                        color: t.bad,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                job.append(
                    &format!(" {rest}"),
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(TEXT_META),
                        color: t.bad,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                ui.add(egui::Label::new(job).wrap());
            });
        });
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
    // The mock's `.kpi` is a plain `div`, not a `.card` (index.html:180, 1675): no fill, border or
    // shadow. The accent border shows for the bucket the table is filtered to, and on hover
    // (`.kpi:hover{border-color:var(--accent)}`).
    let inner = Frame::new()
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(16, 14))
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
    if on || response.hovered() {
        ui.painter().rect_stroke(
            inner.response.rect,
            RADIUS_M,
            Stroke::new(border_width(k), t.accent),
            egui::StrokeKind::Inside,
        );
    }
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

/// The Section, Records, Taxable, Tax and Status cells of a GSTR-1 section line.
#[allow(clippy::float_arithmetic, reason = "sharing a line between columns")]
fn section_columns(line: Rect) -> [Rect; 5] {
    let status = Rect::from_min_max(
        egui::pos2(line.right() - SECTION_STATUS_W, line.top()),
        line.max,
    );
    let tax = Rect::from_min_max(
        egui::pos2(status.left() - SECTION_AMOUNT_W, line.top()),
        egui::pos2(status.left(), line.bottom()),
    );
    let taxable = Rect::from_min_max(
        egui::pos2(tax.left() - SECTION_AMOUNT_W, line.top()),
        egui::pos2(tax.left(), line.bottom()),
    );
    let records = Rect::from_min_max(
        egui::pos2(taxable.left() - RECORDS_W, line.top()),
        egui::pos2(taxable.left(), line.bottom()),
    );
    let label = Rect::from_min_max(line.min, egui::pos2(records.left(), line.bottom()));
    [label, records, taxable, tax, status]
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

/// What a screen reader says for a GSTR-1 row: its title, its detail, then its bucket word.
fn exception_name(item: &ExceptionItem) -> String {
    crate::ui::a11y::join(&[
        item.title.as_str(),
        item.detail.as_str(),
        item.bucket.label(),
    ])
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

/// The 6.1 total row: the label wraps inside its column, as the other rows do, and the row grows
/// to fit.
fn paint_3b_total(ui: &mut Ui, width: f32, label: &str, taxable: Paise, tax: &[Paise]) {
    let t = tokens(ui);
    let bold = bold_font(ui, TEXT_BODY);
    // The label wraps inside its column, as the other rows do, and the row grows to fit.
    let probe = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(width, T_ROW_HEIGHT));
    let [probe_label, ..] = gstr3b_columns(probe);
    let wrap = (probe_label.width() - 2.0 * table::CELL_PAD_X).max(40.0);
    let galley = ui
        .painter()
        .layout(label.to_owned(), bold.clone(), t.text, wrap);
    let height = (galley.size().y + 2.0 * table::CELL_PAD_X).max(T_ROW_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let state = RowState {
        selected: false,
        hovered: false,
        sub: true,
    };
    table::paint_row(ui, rect, state);
    let [label_rect, figures @ ..] = gstr3b_columns(rect);
    let text_pos = egui::pos2(
        label_rect.left() + table::CELL_PAD_X,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, t.text);
    let texts = std::iter::once(amount_cell(Some(taxable)))
        .chain(tax.iter().map(|p| amount_cell(Some(*p))));
    for (r, text) in figures.into_iter().zip(texts) {
        table::paint_cell_text(ui, r, 1.0, &text, bold.clone(), t.text, CellPlace::Right);
    }
}

/// A sub-heading row.
fn paint_3b_sub(ui: &mut Ui, width: f32, label: &str) {
    let t = tokens(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::hover());
    let state = RowState {
        selected: false,
        hovered: false,
        sub: true,
    };
    table::paint_row(ui, rect, state);
    let bold = bold_font(ui, TEXT_BODY);
    let place = CellPlace::Left(table::CELL_PAD_X);
    table::paint_cell_text(ui, rect, 1.0, label, bold, t.text, place);
}

/// A sub-heading with its source chip after it.
#[allow(clippy::float_arithmetic, reason = "placing the label and its chip")]
fn paint_3b_sub_sourced(ui: &mut Ui, width: f32, label: &str, source: &str) {
    let t = tokens(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::hover());
    let state = RowState {
        selected: false,
        hovered: false,
        sub: true,
    };
    table::paint_row(ui, rect, state);
    let bold = bold_font(ui, TEXT_BODY);
    let place = CellPlace::Left(table::CELL_PAD_X);
    table::paint_cell_text(ui, rect, 1.0, label, bold.clone(), t.text, place);
    let text_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), bold, t.text)
        .size()
        .x;
    let chip_rect = Rect::from_min_size(
        egui::pos2(
            rect.left() + table::CELL_PAD_X + text_w + CHIP_GAP,
            rect.top() + (T_ROW_HEIGHT - CHIP_H) / 2.0,
        ),
        vec2(chip_width(ui, source), CHIP_H),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(chip_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    source_chip(&mut child, source);
}

/// One line of the GSTR-3B table: a sub-heading, or a row of the engine's figures with its
/// source chip after the label, or under it when the two do not fit on one line.
fn paint_3b_line(ui: &mut Ui, width: f32, line: &Line3b, tr: &Tr) {
    match line {
        Line3b::Sub(label) => paint_3b_sub(ui, width, label),
        Line3b::SubSourced { label, source } => paint_3b_sub_sourced(ui, width, label, source),
        Line3b::Total {
            label,
            taxable,
            tax,
        } => paint_3b_total(ui, width, label, *taxable, tax),
        Line3b::Row {
            label,
            source,
            indent,
            taxable,
            tax,
        } => paint_3b_row(ui, width, label, *source, *indent, *taxable, Some(tax), tr),
        Line3b::ValueOnly {
            label,
            source,
            value,
        } => paint_3b_row(ui, width, label, *source, false, Some(*value), None, tr),
    }
}

/// One figure row of the GSTR-3B table: the label (indented under a sub-heading when `indent`),
/// its provenance chip, the taxable value and the four tax heads (blank when `tax` is `None`).
#[allow(
    clippy::too_many_arguments,
    reason = "the parts of one row, as the look gives them"
)]
#[allow(clippy::float_arithmetic, reason = "placing the label and its chip")]
fn paint_3b_row(
    ui: &mut Ui,
    width: f32,
    label: &str,
    source: Option<&str>,
    indent: bool,
    taxable: Option<Paise>,
    tax: Option<&[Paise; 4]>,
    tr: &Tr,
) {
    let t = tokens(ui);
    let font = egui::FontId::proportional(TEXT_BODY);
    let label_text = look::text_in(label, tr);
    let source_text = source.map(|s| look::text_in(s, tr));
    let pad = table::CELL_PAD_X + if indent { table::indent(1) } else { 0.0 };
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
    paint_3b_figures(ui, figures, taxable, tax);
}

/// The taxable value (blank where the table has none) and the four tax heads of a GSTR-3B row,
/// each as the engine's working holds it.
fn paint_3b_figures(ui: &Ui, cells: [Rect; 5], taxable: Option<Paise>, tax: Option<&[Paise; 4]>) {
    let t = tokens(ui);
    let font = egui::FontId::proportional(TEXT_BODY);
    let taxable_text = taxable.map_or_else(String::new, |p| amount_cell(Some(p)));
    let tax_texts: [String; 4] = match tax {
        Some(heads) => heads.map(|p| amount_cell(Some(p))),
        None => Default::default(),
    };
    let texts = std::iter::once(taxable_text).chain(tax_texts);
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
/// `ledgerly-gst/src/gstr3b.rs` and `ledgerly-gst/src/setoff.rs` (what is worked out, which
/// conditions are not checked, the reverse-charge rule and the order of set-off), not the mock's
/// text, which the engine does not match. Set-off: CGST Act s.49(5) and Rule 88A (read
/// 2026-11-26); cess credit pays cess only (Compensation Act s.11(2)).
const GSTR3B_NOTES: [&str; 6] = [
    "Ledgerly does not file GSTR-3B. It works out tables 3.1(a), 3.1(c), 3.1(d), 3.1(e), \
     4(A)(3), 4(A)(5) and the payment of tax (6.1) from your books and the 2B file.",
    "Table 4(A)(5) is the credit your books could claim. It is not checked against GSTR-2B, and \
     the conditions of CGST Act s.16(2) and s.16(4) are not checked.",
    "Only credit you accepted in IMS pays tax. Records with no action are shown apart and are \
     not used.",
    "Set-off: IGST credit pays IGST first, then CGST and SGST. CGST credit pays CGST, then IGST. \
     SGST credit pays SGST, then IGST. Cess credit pays cess only.",
    "Credit left in your electronic credit ledger from earlier months is not known to Ledgerly, \
     so the cash figure assumes none.",
    "Purchases under reverse charge are in 3.1(d) and 4(A)(3), not in 3.1(a) or 4(A)(5), and tax \
     on blocked or ineligible purchase lines is never added to 4(A)(3) or 4(A)(5).",
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
    let Some(path) = crate::ui::dialogs::save_file(
        &tr.t("gst.export.dialog_title"),
        Some(name),
        &[("JSON", &["json"])],
    ) else {
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
        if i.consume_key(Modifiers::COMMAND, Key::O) {
            out.push(GstCmd::ImportFile);
        }
        if i.consume_key(Modifiers::ALT, Key::F2) {
            out.push(GstCmd::PickPeriod);
        }
    });
    out
}

#[cfg(test)]
mod a11y_names {
    use super::*;

    #[test]
    fn a_gstr1_row_reads_its_title_detail_and_bucket() {
        // derived: paint_exception draws the title, the detail and the bucket word, in that
        // column order (Bucket::label gives "Mismatch", gst.rs).
        let item = ExceptionItem {
            bucket: Bucket::Mismatch,
            title: "INV-12".to_owned(),
            detail: "Tax differs by 2.00".to_owned(),
            voucher: None,
        };
        assert_eq!(
            exception_name(&item),
            "INV-12, Tax differs by 2.00, Mismatch"
        );
    }
}
