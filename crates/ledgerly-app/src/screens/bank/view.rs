//! Drawing of the bank reconciliation screen (card S-F) with the Ledgerly component kit.
//!
//! The layout follows the mock v2 bank render (`design/mock-v2/index.html`, the `SCREENS.bank`
//! entry): a page head with its actions, two tabs, the statement lines in a table beside a card
//! for the focused line, and the reconciliation statement on its own tab. The drawing adds up
//! nothing and compares no amounts (rule C4): every figure is drawn as the state holds it.
//!
//! Mock states without an app counterpart yet (the AI suggestions and the new-ledger proposal)
//! are wave 3 work and are not drawn here.

use egui::{FontId, Frame, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_extras::{Column, Size, StripBuilder};

use super::import;
use super::{
    BankScreen, BrsRow, BrsRowKind, Flow, LineState, LineStatus, SuggestionView, Tab,
    brs_line_height, confidence_label_in, format_flow, format_line_date, kind_label_in,
    status_label_in,
};
use crate::i18n::Tr;
use crate::screens::wire::unsupported_text;
use crate::theme::{RADIUS_M, TEXT_BODY, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::empty::empty_state;
use crate::ui::format;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState};
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, kind, tokens};
use ledgerly_core::{Outcome, VoucherId};
use ledgerly_model::MatchKind;

/// Width of the card beside the statement lines.
const FOCUS_WIDTH: f32 = 340.0;

/// One key or click the frame produced. Applied after the frame is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiEvent {
    /// Focus a statement line by its index.
    Focus(usize),
    /// Accept the suggestion on a statement line by its index.
    Accept(usize),
    /// Set a statement line aside, or put it back, by its index.
    Skip(usize),
    /// Show a tab.
    Tab(Tab),
    /// Choose or take back a voucher for the manual match.
    Choose(VoucherId),
    /// Confirm the manual match.
    ConfirmManual,
    /// Open or close the manual match panel.
    ToggleManual,
    /// Pick a statement file and import it.
    Import,
    /// Choose the file layout the import reads.
    Layout(&'static str),
}

/// A key the screen handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyCmd {
    Accept,
    Skip,
    Down,
    Up,
    Manual,
    Statement,
    Toggle,
    CloseManual,
    Import,
}

/// Reads this frame's keys. Esc is left to the shell unless the manual panel is open.
fn read_keys(ui: &Ui, manual_open: bool) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.input_mut(|i| {
        let none = egui::Modifiers::NONE;
        if i.consume_key(none, egui::Key::Enter) {
            out.push(KeyCmd::Accept);
        }
        if i.consume_key(none, egui::Key::S) {
            out.push(KeyCmd::Skip);
        }
        if i.consume_key(none, egui::Key::J) || i.consume_key(none, egui::Key::ArrowDown) {
            out.push(KeyCmd::Down);
        }
        if i.consume_key(none, egui::Key::K) || i.consume_key(none, egui::Key::ArrowUp) {
            out.push(KeyCmd::Up);
        }
        if i.consume_key(none, egui::Key::M) {
            out.push(KeyCmd::Manual);
        }
        if i.consume_key(none, egui::Key::B) {
            out.push(KeyCmd::Statement);
        }
        if manual_open && i.consume_key(none, egui::Key::Space) {
            out.push(KeyCmd::Toggle);
        }
        if manual_open && i.consume_key(none, egui::Key::Escape) {
            out.push(KeyCmd::CloseManual);
        }
        if i.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
            out.push(KeyCmd::Import);
        }
    });
    out
}

/// The badge kind of a suggestion's match kind. A suggestion is never drawn as a figure.
fn suggestion_badge(kind: MatchKind) -> BadgeKind {
    match kind {
        MatchKind::Exact => BadgeKind::Ok,
        MatchKind::Rule => BadgeKind::Neutral,
        MatchKind::AiSuggested => BadgeKind::Ai,
        MatchKind::Manual => BadgeKind::Accent,
    }
}

/// The match badge of a statement line: the status when it is not waiting, else the kind of the
/// suggestion on it, else "Not matched".
fn match_badge(
    line: &LineState,
    suggestion: Option<&SuggestionView>,
    tr: &Tr,
) -> (BadgeKind, String) {
    match line.status {
        LineStatus::Matched => (BadgeKind::Ok, status_label_in(line.status, tr)),
        LineStatus::Skipped => (BadgeKind::Neutral, status_label_in(line.status, tr)),
        LineStatus::Pending => match suggestion {
            Some(s) => (suggestion_badge(s.kind), kind_label_in(s.kind, tr)),
            None => (BadgeKind::Warn, status_label_in(line.status, tr)),
        },
    }
}

/// The action column of a statement line.
fn action_text(line: &LineState, suggestion: Option<&SuggestionView>, tr: &Tr) -> String {
    match line.status {
        LineStatus::Matched => tr.t("bank.action.matched_by_you"),
        LineStatus::Skipped => tr.t("bank.action.set_aside"),
        LineStatus::Pending => match suggestion {
            Some(s) => tr.tf(
                "bank.action.matches",
                &[("voucher", s.voucher_label.as_str())],
            ),
            None => tr.t("bank.action.by_hand"),
        },
    }
}

impl BankScreen {
    /// Draws the whole screen for one frame, and applies the keys and clicks it produced.
    pub(super) fn draw(&mut self, ui: &mut Ui) {
        for cmd in read_keys(ui, self.manual_open) {
            self.run_key(cmd);
        }
        let mut events = Vec::new();
        self.draw_head(ui, &mut events);
        self.draw_tabs(ui, &mut events);
        if let Some(text) = self.import_notice() {
            note(
                ui,
                NoteKind::Bad,
                &self.tr.tf("bank.notice.not_imported", &[("text", text)]),
            );
        }
        match self.tab() {
            Tab::Matching => self.draw_matching(ui, &mut events),
            Tab::Statement => self.draw_statement(ui),
        }
        if let Some(err) = self.last_error() {
            note(
                ui,
                NoteKind::Bad,
                &self
                    .tr
                    .tf("bank.notice.error", &[("text", &err.to_string())]),
            );
        }
        if let Some(text) = self.notice() {
            note(
                ui,
                NoteKind::Bad,
                &self.tr.tf("bank.notice.error", &[("text", text)]),
            );
        }
        for event in events {
            self.run_event(event);
        }
    }

    /// Applies a key: the same action the matching button or click would take.
    fn run_key(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Accept => {
                if self.tab() == Tab::Matching {
                    let _ = if self.is_manual_open() {
                        self.confirm_manual()
                    } else {
                        self.accept_focused()
                    };
                }
            }
            KeyCmd::Skip => {
                if self.tab() == Tab::Matching && !self.is_manual_open() {
                    let _ = self.skip_focused();
                }
            }
            // Moving the focus is for the Matching tab only: the Statement tab shows no focus, so
            // a move there would change which line Enter accepts without the user seeing it.
            KeyCmd::Down => {
                if self.tab() == Tab::Matching {
                    if self.is_manual_open() {
                        self.move_manual_cursor(1);
                    } else {
                        self.focus_next();
                    }
                }
            }
            KeyCmd::Up => {
                if self.tab() == Tab::Matching {
                    if self.is_manual_open() {
                        self.move_manual_cursor(-1);
                    } else {
                        self.focus_prev();
                    }
                }
            }
            KeyCmd::Manual => {
                if self.tab() == Tab::Matching {
                    self.toggle_manual();
                }
            }
            KeyCmd::Statement => {
                let next = match self.tab() {
                    Tab::Matching => Tab::Statement,
                    Tab::Statement => Tab::Matching,
                };
                self.set_tab(next);
            }
            KeyCmd::Toggle => {
                if let Some(id) = self.manual_cursor_voucher() {
                    let _ = self.toggle_voucher(id);
                }
            }
            KeyCmd::CloseManual => self.close_manual(),
            KeyCmd::Import => self.choose_statement_file(),
        }
    }

    /// Applies one event from this frame.
    fn run_event(&mut self, event: UiEvent) {
        let last = self.line_count().saturating_sub(1);
        match event {
            UiEvent::Focus(i) => self.set_focus(i.min(last)),
            UiEvent::Accept(i) => {
                self.set_focus(i.min(last));
                let _ = self.accept_focused();
            }
            UiEvent::Skip(i) => {
                self.set_focus(i.min(last));
                let _ = self.skip_focused();
            }
            UiEvent::Tab(tab) => self.set_tab(tab),
            UiEvent::Choose(id) => {
                let _ = self.toggle_voucher(id);
            }
            UiEvent::ConfirmManual => {
                let _ = self.confirm_manual();
            }
            UiEvent::ToggleManual => self.toggle_manual(),
            UiEvent::Import => self.choose_statement_file(),
            UiEvent::Layout(id) => self.set_import_layout(id),
        }
    }

    /// Page head: the title, the ledger under it, and the two actions. Right-to-left layout puts
    /// the first action at the far right, so the reconciliation statement is added first.
    fn draw_head(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let sub = self.ledger_name().to_owned();
        let title = self.tr.t("bank.page_title");
        let statement = self.tr.t("bank.key.statement");
        let import = self.tr.t("bank.key.import");
        page_head(ui, &title, Some(&sub), |ui| {
            if button(ui, &statement, None, ButtonKind::Secondary).clicked() {
                events.push(UiEvent::Tab(Tab::Statement));
            }
            if self.is_worker()
                && button(ui, &import, Some("Ctrl+O"), ButtonKind::Secondary).clicked()
            {
                events.push(UiEvent::Import);
            }
        });
    }

    /// The two tabs. The first is the statement lines, the second the reconciliation statement.
    fn draw_tabs(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let selected = match self.tab() {
            Tab::Matching => 0,
            Tab::Statement => 1,
        };
        let all_lines = self.tr.t("bank.key.all_lines");
        let statement = self.tr.t("bank.key.statement");
        let response = tabs(ui, &[all_lines.as_str(), statement.as_str()], selected);
        if let Some(i) = response.clicked {
            let tab = match i {
                0 => Tab::Matching,
                _ => Tab::Statement,
            };
            events.push(UiEvent::Tab(tab));
        }
    }

    /// The matching tab. With no statement loaded it shows the empty state and never a count.
    fn draw_matching(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        if self.line_count() == 0 {
            if self.is_loading() {
                ui.weak(self.tr.t("bank.loading"));
            } else {
                self.draw_no_statement(ui, events);
            }
            return;
        }
        ui.weak(self.tr.tf(
            "bank.count",
            &[
                ("pending", self.pending_count().to_string().as_str()),
                ("total", self.line_count().to_string().as_str()),
            ],
        ));
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(420.0))
            .size(Size::exact(FOCUS_WIDTH))
            .horizontal(|mut strip| {
                strip.cell(|ui| self.draw_lines(ui, events));
                strip.cell(|ui| self.draw_focus(ui, events));
            });
    }

    /// The empty state of worker mode: no statement is stored for this ledger. It offers the
    /// import, with the file layout the import reads.
    fn draw_no_statement(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        if !self.is_worker() {
            return;
        }
        card(ui, |ui| {
            let import = self.tr.t("bank.key.import");
            if empty_state(
                ui,
                &self.tr.t("bank.no_statement.title"),
                &self.tr.t("bank.no_statement.sentence"),
                Some((import.as_str(), "Ctrl+O")),
            ) {
                events.push(UiEvent::Import);
            }
            let current = self.import_layout();
            ui.horizontal(|ui| {
                ui.weak(self.tr.t("bank.layout.label"));
                egui::ComboBox::from_id_salt("bank-import-layout")
                    .selected_text(import::layout_label_in(current, &self.tr))
                    .show_ui(ui, |ui| {
                        for (id, key) in import::LAYOUTS {
                            if ui.selectable_label(id == current, self.tr.t(key)).clicked() {
                                events.push(UiEvent::Layout(id));
                            }
                        }
                    });
            });
        });
    }

    /// The statement lines as a table, one row per line, the focused row selected.
    fn draw_lines(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let date = self.tr.t("bank.col.date");
        let narration = self.tr.t("bank.col.narration");
        let withdrawal = self.tr.t("bank.col.withdrawal");
        let deposit = self.tr.t("bank.col.deposit");
        let match_col = self.tr.t("bank.col.match");
        let action = self.tr.t("bank.col.action");
        table::builder(ui)
            .sense(Sense::click())
            .column(Column::exact(96.0))
            .column(Column::remainder().at_least(150.0))
            .column(Column::exact(104.0))
            .column(Column::exact(104.0))
            .column(Column::exact(128.0))
            .column(Column::exact(150.0))
            .header(table::HEADER_HEIGHT, |mut row| {
                table::header(&mut row, &date, false);
                table::header(&mut row, &narration, false);
                table::header(&mut row, &withdrawal, true);
                table::header(&mut row, &deposit, true);
                table::header(&mut row, &match_col, false);
                table::header(&mut row, &action, false);
            })
            .body(|body| {
                body.rows(table::ROW_HEIGHT, self.line_count(), |mut row| {
                    let i = row.index();
                    let Some(line) = self.line_at(i) else {
                        return;
                    };
                    let suggestion = self.suggestion_for(line.view.line_no);
                    row.set_selected(i == self.focus_index());
                    let amount = format::amount(line.view.amount);
                    let (withdrawal, deposit) = match line.view.flow {
                        Flow::Out => (amount.as_str(), ""),
                        Flow::In => ("", amount.as_str()),
                    };
                    let (kind, text) = match_badge(line, suggestion, &self.tr);
                    row.col(|ui| table::text(ui, &format_line_date(line.view.date)));
                    row.col(|ui| table::text(ui, &line.view.description));
                    row.col(|ui| table::numeric(ui, withdrawal));
                    row.col(|ui| table::numeric(ui, deposit));
                    row.col(|ui| {
                        badge(ui, kind, &text);
                    });
                    row.col(|ui| table::text(ui, &action_text(line, suggestion, &self.tr)));
                    if row.response().clicked() {
                        events.push(UiEvent::Focus(i));
                    }
                });
            });
    }

    /// The card beside the table: the focused line, its suggestion and the actions on it.
    fn draw_focus(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        card(ui, |ui| {
            let Some(line) = self.line_at(self.focus_index()) else {
                return;
            };
            ui.strong(line.view.description.as_str());
            ui.weak(format!(
                "{} · {}",
                format_line_date(line.view.date),
                format_flow(line.view.amount, line.view.flow)
            ));
            ui.add_space(8.0);
            match line.status {
                LineStatus::Pending => self.draw_pending(ui, events, self.focus_index()),
                LineStatus::Matched | LineStatus::Skipped => {
                    ui.label(status_label_in(line.status, &self.tr));
                }
            }
            if self.is_manual_open() {
                ui.separator();
                self.draw_manual(ui, events);
            }
        });
    }

    /// The actions for a waiting line: accept its suggestion, match it by hand, or set it aside.
    fn draw_pending(&self, ui: &mut Ui, events: &mut Vec<UiEvent>, index: usize) {
        let Some(line) = self.line_at(index) else {
            return;
        };
        if let Some(s) = self.suggestion_for(line.view.line_no) {
            let together = s.lines.len() > 1 || s.vouchers.len() > 1;
            let kind = kind_label_in(s.kind, &self.tr);
            let match_word = self.tr.t(if together {
                "bank.match.together"
            } else {
                "bank.match.single"
            });
            let confidence = confidence_label_in(s.confidence, &self.tr);
            let match_button = self.tr.t("bank.key.match");
            let skip = self.tr.t("bank.key.skip");
            ui.horizontal(|ui| {
                badge(ui, suggestion_badge(s.kind), &kind);
            });
            ui.label(format!("{match_word}{}", s.voucher_label));
            ui.weak(self.tr.tf(
                "bank.pending.confidence",
                &[
                    ("reason", s.reason.as_str()),
                    ("level", confidence.as_str()),
                ],
            ));
            ui.horizontal(|ui| {
                if button(ui, &match_button, Some("Enter"), ButtonKind::Primary).clicked() {
                    events.push(UiEvent::Accept(index));
                }
                if button(ui, &skip, Some("S"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::Skip(index));
                }
            });
        } else {
            ui.weak(self.tr.t("bank.pending.no_suggestion"));
            let by_hand = self.tr.t("bank.key.match_by_hand");
            let skip = self.tr.t("bank.key.skip");
            ui.horizontal(|ui| {
                if button(ui, &by_hand, Some("M"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::ToggleManual);
                }
                if button(ui, &skip, Some("S"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::Skip(index));
                }
            });
        }
    }

    /// The manual match panel: the open vouchers as choices, then the confirm button.
    fn draw_manual(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.strong(self.tr.t("bank.manual.title"));
        ui.weak(self.tr.t("bank.manual.help"));
        let candidates = self.manual_candidates();
        if candidates.is_empty() {
            ui.label(self.tr.t("bank.manual.all_matched"));
        }
        for (j, v) in candidates.iter().enumerate() {
            let mut chosen = self.is_voucher_chosen(v.voucher);
            let text = format!(
                "{}{}  {}  {}",
                if j == self.manual_cursor_index() {
                    "> "
                } else {
                    "  "
                },
                format_line_date(v.date),
                v.label,
                format_flow(v.amount, v.flow),
            );
            if ui.checkbox(&mut chosen, text).changed() {
                events.push(UiEvent::Choose(v.voucher));
            }
        }
        let by_hand = self.tr.t("bank.key.match_by_hand");
        if button(ui, &by_hand, Some("Enter"), ButtonKind::Primary).clicked() {
            events.push(UiEvent::ConfirmManual);
        }
    }

    /// The reconciliation statement tab: the engine's rows as given, or the reason it is missing.
    fn draw_statement(&self, ui: &mut Ui) {
        match self.brs() {
            Outcome::Value(view) => {
                ui.strong(self.tr.tf(
                    "bank.statement.as_of",
                    &[("date", format_line_date(view.as_of).as_str())],
                ));
                statement_lines(ui, &view.rows);
                if let Some(text) = self.brs_note() {
                    note(ui, NoteKind::Info, text);
                }
            }
            Outcome::Unsupported(u) => {
                note(ui, NoteKind::Info, &unsupported_text(u));
            }
        }
    }
}

impl BankScreen {
    /// Asks the user for a statement file and queues it for the import. The file is read here,
    /// on the UI thread; the engine parses and checks it on the worker. A cancelled dialog does
    /// nothing. Host mode has no import.
    pub(super) fn choose_statement_file(&mut self) {
        if !self.is_worker() {
            return;
        }
        match import::pick_file(self.import_layout(), &self.tr) {
            Ok(Some(picked)) => {
                self.set_import_layout(picked.layout);
                self.queue_import(picked.bytes, picked.layout);
            }
            Ok(None) => {}
            Err(text) => self.import_notice = Some(text),
        }
    }
}

/// Width of the amount column of the reconciliation statement (as the balance sheet's).
const BRS_AMOUNT_WIDTH: f32 = 200.0;

/// The reconciliation statement's lines in a card, drawn to the mock's `table.t` (index.html
/// lines 1741-1745): no heading line, each row styled by its [`BrsRowKind`].
fn statement_lines(ui: &mut Ui, rows: &[BrsRow]) {
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
            let width = ui.available_width();
            for row in rows {
                let height = brs_line_height(row.kind);
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                paint_statement_line(ui, rect, row);
            }
        });
}

/// One line of the reconciliation statement: its fill and rules, the label and the amount.
fn paint_statement_line(ui: &Ui, rect: Rect, row: &BrsRow) {
    let t = tokens(ui);
    let plain = RowState {
        selected: false,
        hovered: false,
        sub: false,
    };
    let (rule, bold, depth) = match row.kind {
        BrsRowKind::Books => {
            table::paint_row(ui, rect, RowState { sub: true, ..plain });
            (1.0, true, 0)
        }
        BrsRowKind::Uncleared => {
            table::paint_row(ui, rect, plain);
            (1.0, false, 1)
        }
        BrsRowKind::Expected => {
            table::paint_total_rules(ui, rect);
            (3.0, true, 0)
        }
        BrsRowKind::Statement => {
            table::paint_row(ui, rect, plain);
            (1.0, false, 0)
        }
        BrsRowKind::Difference => {
            table::paint_row(ui, rect, plain);
            (1.0, true, 0)
        }
    };
    let font = if bold {
        bold_font(ui, TEXT_BODY)
    } else {
        FontId::proportional(TEXT_BODY)
    };
    let label = Rect::from_min_max(
        rect.min,
        pos2(
            (rect.right() - BRS_AMOUNT_WIDTH).max(rect.left()),
            rect.bottom(),
        ),
    );
    table::paint_cell_text(
        ui,
        label,
        rule,
        &row.label,
        font.clone(),
        t.text,
        CellPlace::Left(table::indent(depth)),
    );
    table::paint_cell_text(
        ui,
        rect,
        rule,
        &format::amount(row.amount),
        font,
        t.text,
        CellPlace::Right,
    );
}
