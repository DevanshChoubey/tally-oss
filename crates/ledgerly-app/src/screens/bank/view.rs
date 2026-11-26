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
    BankScreen, BrsRow, BrsRowKind, BrsVerdict, Flow, LineState, LineStatus, PAGE_TITLE,
    SettingsRequest, SuggestionView, Tab, brs_line_height, confidence_label_in, format_flow,
    format_line_date, kind_label_in, status_label_in,
};
use crate::i18n::Tr;
use crate::screens::ai_gate::settings_note;
use crate::screens::wire::unsupported_text;
use crate::theme::{RADIUS_M, TEXT_BODY, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button, button_icon, style_combo};
use crate::ui::card::card;
use crate::ui::empty::empty_state_with_icon;
use crate::ui::format;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note, note_with_icon};
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState};
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, kind, para, tokens};
use ledgerly_core::{Outcome, VoucherId};
use ledgerly_model::MatchKind;

/// The least width of the statement lines table.
const MIN_LINES_WIDTH: f32 = 420.0;

/// Width of the card beside the statement lines.
const FOCUS_WIDTH: f32 = 300.0;

/// Height of a statement line: two lines of the narration, which wraps in its column as the
/// mock's does (index.html:1755).
const LINE_HEIGHT: f32 = 46.0;

/// One key or click the frame produced. Applied after the frame is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiEvent {
    /// The AI-off note's AI and privacy button was clicked.
    OpenSettings,
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
    /// Go to the chooser of bank accounts.
    ChangeAccount,
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
    ChangeAccount,
    Export,
}

/// Reads this frame's keys. Esc is left to the shell unless the manual panel is open.
fn read_keys(ui: &Ui, manual_open: bool, statement_tab: bool) -> Vec<KeyCmd> {
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
        if statement_tab && i.consume_key(egui::Modifiers::COMMAND, egui::Key::E) {
            out.push(KeyCmd::Export);
        }
        if !manual_open && i.consume_key(none, egui::Key::C) {
            out.push(KeyCmd::ChangeAccount);
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

/// What a screen reader says for a statement line: its cells as drawn, in column order, with the
/// empty ones left out.
fn line_name(cells: &[&str]) -> String {
    crate::ui::a11y::join(cells)
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
        for cmd in read_keys(ui, self.manual_open, self.tab() == Tab::Statement) {
            self.run_key(cmd);
        }
        let mut events = Vec::new();
        self.draw_head(ui, &mut events);
        // index.html:1738: a file that does not add up shows the head and the working, nothing
        // else: the statement the screen held before is not shown beside a refusal.
        if let Some(working) = self.import_working().copied() {
            draw_balance_refusal(ui, &working, &self.tr);
            return;
        }
        // index.html:1737: with no statement the mock shows the head and one card, no tab strip.
        if !self.shows_empty_state() {
            self.draw_tabs(ui, &mut events);
        }
        if let Some(text) = self.import_notice() {
            note(
                ui,
                NoteKind::Bad,
                &self.tr.tf("bank.notice.not_imported", &[("text", text)]),
            );
        }
        if let Some(text) = self.export_notice() {
            let bad = text.contains("was not written");
            note(ui, if bad { NoteKind::Bad } else { NoteKind::Info }, text);
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
            KeyCmd::Export => self.choose_export_file(),
            KeyCmd::ChangeAccount => self.switch_requested = true,
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
            UiEvent::ChangeAccount => self.switch_requested = true,
            UiEvent::OpenSettings => self.settings_requested = Some(SettingsRequest::Open),
        }
    }

    /// Page head: the title, the ledger under it, and the two actions. Right-to-left layout puts
    /// the first action at the far right, so the reconciliation statement is added first.
    fn draw_head(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let sub = self.subtitle_text();
        page_head(ui, &crate::i18n::lit(PAGE_TITLE), Some(&sub), |ui| {
            if button(
                ui,
                &self.tr.t("bank.key.statement"),
                None,
                ButtonKind::Secondary,
            )
            .clicked()
            {
                events.push(UiEvent::Tab(Tab::Statement));
            }
            if self.is_worker()
                && button_icon(
                    ui,
                    Some(Icon::Upload),
                    &self.tr.t("bank.key.import"),
                    Some("Ctrl+O"),
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                events.push(UiEvent::Import);
            }
            if self.can_switch
                && button(
                    ui,
                    &self.tr.t("bank.change_account"),
                    Some("C"),
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                events.push(UiEvent::ChangeAccount);
            }
        });
    }

    /// Whether the screen is on its empty state: worker mode, the lines tab, nothing loaded and
    /// nothing loading. The reconciliation statement tab always keeps its strip, so the user can
    /// get back from it.
    fn shows_empty_state(&self) -> bool {
        self.is_worker()
            && self.tab() == Tab::Matching
            && self.line_count() == 0
            && !self.is_loading()
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
        // Two columns from the width the page has: the table takes what the card beside it leaves.
        // (A strip let the table's own minimum push the card past the window's right edge.)
        let gap = 12.0;
        let left = (ui.available_width() - FOCUS_WIDTH - gap).max(MIN_LINES_WIDTH);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.allocate_ui_with_layout(
                vec2(left, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(left);
                    self.draw_lines(ui, events);
                },
            );
            ui.allocate_ui_with_layout(
                vec2(FOCUS_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(FOCUS_WIDTH);
                    self.draw_focus(ui, events);
                },
            );
        });
    }

    /// The empty state of worker mode: no statement is stored for this ledger. It offers the
    /// import, with the file layout the import reads.
    fn draw_no_statement(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        if !self.is_worker() {
            return;
        }
        card(ui, |ui| {
            if empty_state_with_icon(
                ui,
                Some(Icon::Bank),
                &self.tr.t("bank.no_statement.title"),
                &self.tr.t("bank.no_statement.sentence"),
                Some((Some(Icon::Upload), &self.tr.t("bank.key.import"), "Ctrl+O")),
            ) {
                events.push(UiEvent::Import);
            }
            let current = self.import_layout();
            ui.horizontal(|ui| {
                ui.weak(self.tr.t("bank.layout.label"));
                style_combo(ui);
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
        // The narration takes the width the other five columns and the gaps between the columns
        // leave. (A remainder column let the table run wider than the cell it is drawn in.)
        let date = self.tr.t("bank.col.date");
        let narration = self.tr.t("bank.col.narration");
        let withdrawal = self.tr.t("bank.col.withdrawal");
        let deposit = self.tr.t("bank.col.deposit");
        let match_col = self.tr.t("bank.col.match");
        let action = self.tr.t("bank.col.action");
        let fixed = 84.0 + 88.0 + 88.0 + 112.0 + 128.0;
        let gaps = 5.0 * ui.spacing().item_spacing.x;
        let narration_w = (ui.available_width() - fixed - gaps).max(120.0);
        table::builder(ui)
            .sense(Sense::click())
            .column(Column::exact(84.0))
            .column(Column::exact(narration_w))
            .column(Column::exact(88.0))
            .column(Column::exact(88.0))
            .column(Column::exact(112.0))
            .column(Column::exact(128.0))
            .header(table::HEADER_HEIGHT, |mut row| {
                table::header(&mut row, &date, false);
                table::header(&mut row, &narration, false);
                table::header(&mut row, &withdrawal, true);
                table::header(&mut row, &deposit, true);
                table::header(&mut row, &match_col, false);
                table::header(&mut row, &action, false);
            })
            .body(|body| {
                body.rows(LINE_HEIGHT, self.line_count(), |mut row| {
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
                    let date = format_line_date(line.view.date);
                    let action = action_text(line, suggestion, &self.tr);
                    row.col(|ui| table::text(ui, &date));
                    row.col(|ui| {
                        let t = tokens(ui);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&line.view.description)
                                    .size(TEXT_BODY)
                                    .color(t.text),
                            )
                            .wrap(),
                        );
                    });
                    row.col(|ui| table::numeric(ui, withdrawal));
                    row.col(|ui| table::numeric(ui, deposit));
                    row.col(|ui| {
                        badge(ui, kind, &text);
                    });
                    row.col(|ui| table::text(ui, &action));
                    let name = line_name(&[
                        date.as_str(),
                        line.view.description.as_str(),
                        withdrawal,
                        deposit,
                        text.as_str(),
                        action.as_str(),
                    ]);
                    row.response().widget_info(|| {
                        egui::WidgetInfo::labeled(egui::accesskit::Role::Row, true, &name)
                    });
                    if row.response().clicked() {
                        events.push(UiEvent::Focus(i));
                    }
                });
            });
    }

    /// The card beside the table: the focused line, its suggestion and the actions on it.
    fn draw_focus(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        // With AI off the side is the mock's note, not a card (index.html:1830); the manual match
        // panel still opens as a card, since matching by hand needs no AI.
        if self.ai_tier == Some(crate::AiTier::Off) && !self.is_manual_open() {
            if settings_note(
                ui,
                NoteKind::Info,
                Icon::Off,
                &self.tr.t("bank.ai_off_note"),
                true,
                &self.tr,
            ) {
                events.push(UiEvent::OpenSettings);
            }
            return;
        }
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
                let verdict = super::brs_verdict(view);
                let note_text = self.brs_note();
                if verdict == BrsVerdict::Unknown {
                    statement_lines(ui, &view.rows);
                    if let Some(text) = note_text {
                        note(ui, NoteKind::Info, text);
                    }
                } else {
                    // The mock's `.g-main` row (index.html:1819): the statement over a card that
                    // says whether the engine's difference is nil.
                    #[expect(clippy::float_arithmetic, reason = "the grid's fractions of the row")]
                    let (left, right) = {
                        let room = (ui.available_width() - GRID_GAP).max(0.0);
                        let right = (room * (MAIN_PARTS - MAIN_LEFT_PARTS) / MAIN_PARTS)
                            .max(VERDICT_MIN.min(room));
                        (room - right, right)
                    };
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        StripBuilder::new(ui)
                            .size(Size::exact(left))
                            .size(Size::exact(GRID_GAP))
                            .size(Size::exact(right))
                            .horizontal(|mut strip| {
                                strip.cell(|col| {
                                    statement_lines(col, &view.rows);
                                    if let Some(text) = note_text {
                                        note(col, NoteKind::Info, text);
                                    }
                                });
                                strip.empty();
                                strip.cell(|col| draw_verdict(col, verdict, &self.tr));
                            });
                    });
                }
            }
            Outcome::Unsupported(u) => {
                note(ui, NoteKind::Info, &unsupported_text(u));
            }
        }
    }
}

impl BankScreen {
    /// Asks where to save the reconciliation statement and writes it there (Ctrl+E on the
    /// statement tab). Cancelling writes nothing. Host mode has no export.
    pub(super) fn choose_export_file(&mut self) {
        if !self.is_worker() {
            return;
        }
        if let Some(path) = crate::ui::dialogs::save_file(
            &self.tr.t("bank.export.dialog_title"),
            Some("bank-reconciliation-statement.csv"),
            &[
                ("CSV", &["csv"]),
                (&self.tr.t("bank.export.excel"), &["xlsx"]),
            ],
        ) {
            self.export_statement_to(path);
        }
    }

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

/// Width of the refusal's working table (`max-width:520px`, index.html:1738).
const REFUSAL_TABLE_WIDTH: f32 = 520.0;

/// The refusal for a file that does not add up (the mock's `bank.import-error`): a red note with
/// the working, every figure from the engine's [`ledgerly_api::BalanceRefusal`].
fn draw_balance_refusal(ui: &mut Ui, working: &ledgerly_api::BalanceRefusal, tr: &Tr) {
    note_with_icon(ui, NoteKind::Bad, Icon::Alert, |ui, ink| {
        ui.label(
            egui::RichText::new(tr.t("bank.refusal.title"))
                .font(bold_font(ui, crate::theme::TEXT_META))
                .color(ink),
        );
        ui.add_space(8.0);
        refusal_table(ui, working, ink, tr);
        ui.add_space(8.0);
        para(
            ui,
            &tr.t("bank.refusal.advice"),
            crate::theme::TEXT_META,
            ink,
        );
    });
}

/// How a row of the refusal's working looks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkingRow {
    /// A plain line.
    Plain,
    /// The computed closing balance, with the total rules.
    Total,
    /// The difference, in bold.
    Strong,
}

/// The working's rows, from the engine's figures: label, amount, look, and whether the label
/// carries the engine tag.
fn refusal_rows(
    w: &ledgerly_api::BalanceRefusal,
    tr: &Tr,
) -> Vec<(String, String, WorkingRow, bool)> {
    vec![
        (
            tr.t("bank.refusal.opening"),
            format::amount(w.opening),
            WorkingRow::Plain,
            false,
        ),
        (
            tr.tf("bank.refusal.lines_net", &[("n", &w.lines.to_string())]),
            format::amount(w.lines_net),
            WorkingRow::Plain,
            true,
        ),
        (
            tr.t("bank.refusal.computed"),
            format::amount(w.computed_closing),
            WorkingRow::Total,
            false,
        ),
        (
            tr.t("bank.refusal.stated"),
            format::amount(w.stated_closing),
            WorkingRow::Plain,
            false,
        ),
        (
            tr.t("bank.refusal.difference"),
            format::amount(w.difference),
            WorkingRow::Strong,
            false,
        ),
    ]
}

/// The working as a `table.t` in the note's ink: rules in the border colour, the computed line
/// with the total's rules.
fn refusal_table(ui: &mut Ui, w: &ledgerly_api::BalanceRefusal, ink: egui::Color32, tr: &Tr) {
    let width = ui.available_width().min(REFUSAL_TABLE_WIDTH);
    for (label, amount, look, engine) in refusal_rows(w, tr) {
        let height = match look {
            WorkingRow::Total => table::T_TOTAL_HEIGHT,
            WorkingRow::Plain | WorkingRow::Strong => table::T_ROW_HEIGHT,
        };
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let (rule, bold) = match look {
            WorkingRow::Plain => {
                table::paint_cell_rule(ui, rect);
                (1.0, false)
            }
            WorkingRow::Total => {
                table::paint_total_rules(ui, rect);
                (3.0, true)
            }
            WorkingRow::Strong => {
                table::paint_cell_rule(ui, rect);
                (1.0, true)
            }
        };
        let font = if bold {
            bold_font(ui, TEXT_BODY)
        } else {
            FontId::proportional(TEXT_BODY)
        };
        table::paint_cell_text(
            ui,
            rect,
            rule,
            &label,
            font.clone(),
            ink,
            CellPlace::Left(table::CELL_PAD_X),
        );
        if engine {
            table::paint_engine_tag(ui, rect, &label, &font);
        }
        table::paint_cell_text(ui, rect, rule, &amount, font, ink, CellPlace::Right);
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
                let height = statement_line_height(ui, row, width);
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                paint_statement_line(ui, rect, row);
            }
        });
}

/// The height of statement line `row` when the line is `width` wide: its usual height, or taller
/// when the label does not fit beside the amount column on one line and wraps onto more.
fn statement_line_height(ui: &Ui, row: &BrsRow, width: f32) -> f32 {
    let usual = brs_line_height(row.kind);
    let galley = statement_label_galley(ui, row, width);
    (galley.size().y + 2.0 * LABEL_PAD_Y).max(usual)
}

/// Vertical padding above and below a wrapped statement label.
const LABEL_PAD_Y: f32 = 8.0;

/// The depth a statement line's label is indented to.
fn statement_depth(kind: BrsRowKind) -> u8 {
    match kind {
        BrsRowKind::Uncleared => 1,
        BrsRowKind::Item => 2,
        BrsRowKind::Books
        | BrsRowKind::Expected
        | BrsRowKind::Statement
        | BrsRowKind::Difference => 0,
    }
}

/// The font of a statement line: bold for the books, expected and difference lines.
fn statement_font(ui: &Ui, kind: BrsRowKind) -> FontId {
    match kind {
        BrsRowKind::Books | BrsRowKind::Expected | BrsRowKind::Difference => {
            bold_font(ui, TEXT_BODY)
        }
        BrsRowKind::Uncleared | BrsRowKind::Item | BrsRowKind::Statement => {
            FontId::proportional(TEXT_BODY)
        }
    }
}

/// The label of `row` laid out to wrap before the amount column, in a line `width` wide.
fn statement_label_galley(ui: &Ui, row: &BrsRow, width: f32) -> std::sync::Arc<egui::Galley> {
    let room = (width - BRS_AMOUNT_WIDTH - table::indent(statement_depth(row.kind))).max(40.0);
    ui.painter().layout(
        row.label.clone(),
        statement_font(ui, row.kind),
        tokens(ui).text,
        room,
    )
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
        BrsRowKind::Item => {
            table::paint_row(ui, rect, plain);
            (1.0, false, 2)
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
    // The label wraps before the amount column; its lines are centred in the row.
    let galley = statement_label_galley(ui, row, rect.width());
    let y = rect.top() + (rect.height() - rule - galley.size().y) / 2.0;
    ui.painter().with_clip_rect(rect).galley(
        pos2(rect.left() + table::indent(depth), y),
        galley,
        t.text,
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

/// Gap between the statement and its card (the mock's grid gap).
const GRID_GAP: f32 = 16.0;
/// The mock's `.g-main` columns, `minmax(0,1.55fr) minmax(320px,1fr)`.
const MAIN_LEFT_PARTS: f32 = 1.55;
/// The two columns' parts together.
const MAIN_PARTS: f32 = 2.55;
/// The card's least width.
const VERDICT_MIN: f32 = 320.0;

/// The card beside the statement: Reconciled with a check, or Not yet reconciled, and one
/// sentence. The words say only what the engine's Difference line says.
fn draw_verdict(ui: &mut Ui, verdict: BrsVerdict, tr: &Tr) {
    let t = tokens(ui);
    let (title, glyph, text) = match verdict {
        BrsVerdict::Reconciled => (
            tr.t("bank.verdict.ok_title"),
            Some(Icon::Check),
            tr.t("bank.verdict.ok_text"),
        ),
        BrsVerdict::NotYet | BrsVerdict::Unknown => (
            tr.t("bank.verdict.not_title"),
            None,
            tr.t("bank.verdict.not_text"),
        ),
    };
    card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        crate::ui::mock_table::panel_title(ui, glyph, &title);
        para(ui, &text, TEXT_BODY, t.text_2);
    });
}

#[cfg(test)]
mod a11y_names {
    use super::*;

    #[test]
    fn a_long_uncleared_line_wraps_and_grows_the_row_instead_of_running_under_the_amount() {
        // derived: the mock shows "... Kaveri Packaging Pvt Ltd . cheque 004512" whole. In a
        // statement 400 points wide the label has 400 - 200 (amount column) - 16 (indent)
        // = 184 points, far less than the 70-odd characters need at body size, so the label
        // wraps onto two or more lines and the row is taller than the usual 34 points.
        let ctx = egui::Context::default();
        let row = BrsRow {
            label:
                "12 Nov 2026 \u{b7} PY/0321 \u{b7} Kaveri Packaging Pvt Ltd \u{b7} cheque 004512"
                    .to_owned(),
            amount: ledgerly_core::Paise::new(1_250_000),
            kind: BrsRowKind::Item,
        };
        let usual = crate::screens::bank::brs_line_height(row.kind);
        let mut taller = false;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            taller = statement_line_height(ui, &row, 400.0) > usual;
        })
        .drop_without_applying_deltas();
        assert!(taller, "the long label must wrap and make the row taller");
        let short = BrsRow {
            label: "Cheque 1".to_owned(),
            ..row
        };
        let mut same = false;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            same = (statement_line_height(ui, &short, 400.0) - usual).abs() < f32::EPSILON;
        })
        .drop_without_applying_deltas();
        assert!(same, "a short label keeps the usual height");
    }

    #[test]
    fn a_statement_line_reads_its_cells_and_leaves_out_the_empty_ones() {
        // derived: the columns are Date, Narration, Withdrawal, Deposit, Match, Action; a
        // withdrawal leaves Deposit empty, and an empty cell is not spoken.
        let name = line_name(&[
            "03-11-2026",
            "NEFT RAMESH",
            "1,200.00",
            "",
            "Not matched",
            "Set aside",
        ]);
        assert_eq!(
            name,
            "03-11-2026, NEFT RAMESH, 1,200.00, Not matched, Set aside"
        );
    }
}
