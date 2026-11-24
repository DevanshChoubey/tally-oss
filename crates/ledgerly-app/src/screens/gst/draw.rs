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
    amount_cell, difference_cell, section_bucket,
};
use crate::ScreenAction;
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
            self.notice = Some(save_json_file(&name, &json));
        }
        let mut events = Vec::new();
        self.draw_head(ui, &mut events);
        if let Some(error) = self.load_error() {
            note(ui, NoteKind::Bad, &format!("Error: {error}"));
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
        let subtitle = self.subtitle();
        let blocked = self.export_blocked_reason();
        crate::ui::page_head::page_head(ui, "GST returns", Some(subtitle.as_str()), |ui| {
            let response = ui
                .add_enabled_ui(blocked.is_none(), |ui| {
                    button_icon(
                        ui,
                        Some(Icon::Upload),
                        "Export JSON",
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
        let items: Vec<(&str, &str)> = GstTab::ALL
            .iter()
            .map(|t| (t.label(), t.key_hint()))
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

    // ---- GSTR-1 ------------------------------------------------------------------------------

    fn draw_gstr1(&self, ui: &mut Ui, want_scroll: bool, events: &mut Vec<UiEvent>) {
        self.draw_tiles(ui, events);
        ui.add_space(SPACE_3);
        let mut side = Vec::new();
        split(
            ui,
            |ui| {
                if self.sections_loaded {
                    self.draw_sections(ui, events);
                    ui.add_space(SPACE_3);
                    title(ui, "Vouchers behind the sections");
                }
                self.draw_exceptions(ui, &self.gstr1, want_scroll, events);
                if !self.figures1.rows.is_empty() || !self.figures1.notes.is_empty() {
                    ui.add_space(SPACE_3);
                    draw_figures(ui, &self.figures1);
                }
            },
            |ui| self.draw_side_card(ui, &mut side),
        );
        events.extend(side);
    }

    /// The four status tiles (the mock's `.kpi` in a `g4` grid). Each counts the vouchers the
    /// engine put in a bucket; the screen only counts rows (rule C4). Clicking a tile shows only
    /// that bucket in the table, and clicking it again shows every voucher.
    fn draw_tiles(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.columns(Bucket::ALL.len(), |cols| {
            for (col, bucket) in cols.iter_mut().zip(Bucket::ALL) {
                // The mock's tiles count sections; with no section table, vouchers.
                let (count, unit) = if self.sections_loaded {
                    (self.section_count(bucket), "sections")
                } else {
                    (self.bucket_count(GstTab::Gstr1, bucket), "vouchers")
                };
                let value = match count {
                    Some(n) => n.to_string(),
                    None => "Not loaded".to_owned(),
                };
                let on = self.bucket_filter == Some(bucket);
                if kpi(col, bucket.label(), &value, unit, on).clicked() {
                    events.push(UiEvent::Filter(if on { None } else { Some(bucket) }));
                }
            }
        });
    }

    /// The section table: one row per table of the return with its records, taxable value, tax
    /// and the engine's status (the mock's `#g1`). The amounts are the engine's; `none` where a
    /// table has no such figure.
    fn draw_sections(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        table_card(ui, |ui| {
            let t = tokens(ui);
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            let cols = section_columns(head);
            paint_heads(ui, head, &[(cols[0], "Section"), (cols[4], "Status")], &[]);
            paint_heads(
                ui,
                head,
                &[],
                &[(cols[1], "Records"), (cols[2], "Taxable"), (cols[3], "Tax")],
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
                badge(&mut child, bucket_badge(bucket), bucket.label());
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
        if held > 0 {
            let text = format!(
                "{held} voucher(s) need correction. The JSON is held back until they are fixed; \
                 the reason is in the table."
            );
            note(ui, NoteKind::Warn, &text);
            ui.add_space(SPACE_3);
        }
        crate::ui::card::card(ui, |ui| {
            title(ui, "What happens next");
            bullets(ui, &NEXT_STEPS);
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
            paint_heads(
                ui,
                head,
                &[(voucher, "Voucher"), (detail, "Detail"), (status, "Status")],
                &[],
            );
            let Feed::Loaded(items) = feed else {
                padded(ui, |ui| draw_not_ready(ui, feed));
                return;
            };
            let visible = self.visible_exceptions(feed);
            if visible.is_empty() {
                padded(ui, |ui| {
                    para(ui, "No vouchers here.", TEXT_BODY, tokens(ui).text_3);
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
                paint_exception(ui, rect, item, state);
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
        let mut side = Vec::new();
        split(
            ui,
            |ui| self.draw_3b_table(ui),
            |ui| {
                crate::ui::card::card(ui, |ui| {
                    title(ui, "Read this before filing");
                    bullets(ui, &GSTR3B_NOTES);
                    ui.add_space(10.0);
                    let tab = GstTab::Reconciliation;
                    if button(ui, tab.label(), Some(tab.key_hint()), ButtonKind::Secondary)
                        .clicked()
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
            let right: Vec<(Rect, &str)> = figures.into_iter().zip(numeric).collect();
            paint_heads(ui, head, &[(label, heading)], &right);
            if self.lines3b.is_empty() {
                padded(ui, |ui| draw_not_ready(ui, &self.gstr3b));
                return;
            }
            for line in &self.lines3b {
                paint_3b_line(ui, width, line);
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
                    draw_not_ready(ui, &self.recon);
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
                    "Match",
                    "Supplier",
                    "Invoice",
                    "Taxable, books",
                    "Taxable, 2B",
                    "Tax, books",
                    "Tax, 2B",
                    "Difference",
                    "IMS action",
                ] {
                    ui.weak(head);
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
                        row.state.label(),
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
                .selectable_label(self.bucket_filter.is_none(), format!("All {total}"))
                .clicked()
            {
                events.push(UiEvent::Filter(None));
            }
            for bucket in Bucket::ALL {
                let n = self
                    .bucket_count(GstTab::Reconciliation, bucket)
                    .unwrap_or(0);
                let text = format!("{} {n}", bucket.label());
                if ui
                    .selectable_label(self.bucket_filter == Some(bucket), text)
                    .clicked()
                {
                    events.push(UiEvent::Filter(Some(bucket)));
                }
            }
            let text = format!("Open actions {}", self.open_count());
            if ui.selectable_label(self.open_only, text).clicked() {
                events.push(UiEvent::OpenOnly(!self.open_only));
            }
        });
    }

    fn draw_ims_cell(&self, ui: &mut Ui, index: usize, row: &ReconRow, events: &mut Vec<UiEvent>) {
        if !row.in_2b() {
            ui.weak("Ask supplier to file");
            return;
        }
        let decision = self.decision(index);
        ui.horizontal(|ui| {
            for action in ImsAction::ALL {
                if ui
                    .selectable_label(decision == Some(action), action.label())
                    .clicked()
                {
                    events.push(UiEvent::Decide(index, action));
                }
            }
            if decision.is_none() {
                ui.weak(format!(
                    "suggested: {}, not confirmed",
                    row.suggested.label()
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
fn bullets(ui: &mut Ui, lines: &[&str]) {
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
            ui.add(egui::Label::new(RichText::new(*line).size(TEXT_BODY).color(t.text_2)).wrap());
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
fn paint_exception(ui: &mut Ui, rect: Rect, item: &ExceptionItem, state: RowState) {
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
    badge(&mut child, bucket_badge(item.bucket), item.bucket.label());
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
fn paint_3b_line(ui: &mut Ui, width: f32, line: &Line3b) {
    let t = tokens(ui);
    let font = egui::FontId::proportional(TEXT_BODY);
    match line {
        Line3b::Sub(label) => {
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
        Line3b::Row {
            label,
            source,
            indent,
            taxable,
            tax,
        } => {
            let pad = table::CELL_PAD_X + if *indent { table::indent(1) } else { 0.0 };
            let probe = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(width, T_ROW_HEIGHT));
            let [label_cell, ..] = gstr3b_columns(probe);
            let wrap = (label_cell.width() - pad - table::CELL_PAD_X).max(40.0);
            let galley = ui
                .painter()
                .layout((*label).to_owned(), font.clone(), t.text, wrap);
            let text = galley.size();
            let one_row = galley.rows.len() <= 1;
            let chip_w = source.map_or(0.0, |s| chip_width(ui, s));
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
            if let Some(s) = source {
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
fn draw_figures(ui: &mut Ui, figures: &Figures) {
    table_card(ui, |ui| {
        let t = tokens(ui);
        let width = ui.available_width();
        let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
        let (label, amount) = figure_columns(head);
        paint_heads(
            ui,
            head,
            &[(label, "From the return")],
            &[(amount, "Amount")],
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
fn draw_not_ready<T>(ui: &mut Ui, feed: &Feed<T>) {
    let t = tokens(ui);
    match feed {
        Feed::Loaded(_) => {}
        Feed::NotLoaded => {
            para(ui, "Not loaded for this period yet.", TEXT_BODY, t.text_3);
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
    "Fix anything marked Needs correction or Mismatch. Enter opens the voucher to fix.",
    "Export JSON (Ctrl+E) writes the GSTR-1 JSON for the GST portal's offline tool. The engine \
     writes it only when every voucher is Ready or Not relevant.",
    "Upload and file on the GST portal yourself. Ledgerly does not file.",
    "Ledgerly does not yet record a return as filed or lock its period.",
];

/// The "Read this before filing" lines of GSTR-3B. They follow the engine's module notes in
/// `ledgerly-gst/src/gstr3b.rs` (what is worked out, which conditions are not checked, and the
/// reverse-charge rule), not the mock's text, which the engine does not match.
const GSTR3B_NOTES: [&str; 5] = [
    "Ledgerly does not file GSTR-3B. It works out tables 3.1(a) and 4(A)(5) from your books.",
    "Table 4(A)(5) is the credit your books could claim. It is not checked against GSTR-2B, and \
     the conditions of CGST Act s.16(2) and s.16(4) are not checked.",
    "Tax on blocked or ineligible purchase lines is shown apart and is never added to 4(A)(5).",
    "Purchases under reverse charge are in neither 3.1(a) nor 4(A)(5).",
    "Set-off against the liability and the credit accepted in IMS are not worked out yet, so no \
     cash payable figure is shown.",
];

/// Asks where to save the GSTR-1 JSON and writes it there. Cancelling writes nothing. The dialog
/// is the user's own choice of place; nothing is uploaded.
fn save_json_file(name: &str, json: &str) -> String {
    let Some(path) =
        crate::ui::dialogs::save_file("Save GSTR-1 JSON", Some(name), &[("JSON", &["json"])])
    else {
        return "Export cancelled. Nothing was written.".to_owned();
    };
    match std::fs::write(&path, json) {
        Ok(()) => format!("Saved the GSTR-1 JSON to {}.", path.display()),
        Err(error) => format!("Could not write {}: {error}.", path.display()),
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
