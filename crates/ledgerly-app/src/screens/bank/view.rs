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
use egui_extras::Column;

use super::import;
use super::{
    BankScreen, BrsRow, BrsRowKind, Flow, LineState, LineStatus, PAGE_TITLE, SuggestionView, Tab,
    brs_line_height, confidence_label, format_flow, format_line_date, kind_label, status_label,
};
use crate::screens::wire::unsupported_text;
use crate::theme::{RADIUS_M, TEXT_BODY, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button, button_icon, style_combo};
use crate::ui::card::card;
use crate::ui::empty::empty_state_with_icon;
use crate::ui::format;
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState};
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, kind, tokens};
use ledgerly_core::{Outcome, VoucherId};
use ledgerly_model::MatchKind;

/// The sentence under the empty state, from the mock's bank empty state.
const NO_STATEMENT_SENTENCE: &str = "Download a statement from your bank's website as CSV, XLSX or MT940, then import it here. The file is read on this computer. Ledgerly never signs in to your bank.";

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
fn match_badge(line: &LineState, suggestion: Option<&SuggestionView>) -> (BadgeKind, String) {
    match line.status {
        LineStatus::Matched => (BadgeKind::Ok, "Matched".to_owned()),
        LineStatus::Skipped => (BadgeKind::Neutral, "Skipped".to_owned()),
        LineStatus::Pending => match suggestion {
            Some(s) => (suggestion_badge(s.kind), kind_label(s.kind).to_owned()),
            None => (BadgeKind::Warn, "Not matched".to_owned()),
        },
    }
}

/// What a screen reader says for a statement line: its cells as drawn, in column order, with the
/// empty ones left out.
fn line_name(cells: &[&str]) -> String {
    crate::ui::a11y::join(cells)
}

/// The action column of a statement line.
fn action_text(line: &LineState, suggestion: Option<&SuggestionView>) -> String {
    match line.status {
        LineStatus::Matched => "Matched by you".to_owned(),
        LineStatus::Skipped => "Set aside".to_owned(),
        LineStatus::Pending => match suggestion {
            Some(s) => format!("Matches {}", s.voucher_label),
            None => "Match by hand".to_owned(),
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
        // index.html:1737: with no statement the mock shows the head and one card, no tab strip.
        if !self.shows_empty_state() {
            self.draw_tabs(ui, &mut events);
        }
        if let Some(text) = self.import_notice() {
            note(
                ui,
                NoteKind::Bad,
                &format!("Not imported. {text} Nothing was imported."),
            );
        }
        match self.tab() {
            Tab::Matching => self.draw_matching(ui, &mut events),
            Tab::Statement => self.draw_statement(ui),
        }
        if let Some(err) = self.last_error() {
            note(ui, NoteKind::Bad, &format!("Error: {err}"));
        }
        if let Some(text) = self.notice() {
            note(ui, NoteKind::Bad, &format!("Error: {text}"));
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
        }
    }

    /// Page head: the title, the ledger under it, and the two actions. Right-to-left layout puts
    /// the first action at the far right, so the reconciliation statement is added first.
    fn draw_head(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        let sub = self.subtitle_text();
        page_head(ui, PAGE_TITLE, Some(&sub), |ui| {
            if button(ui, "Reconciliation statement", None, ButtonKind::Secondary).clicked() {
                events.push(UiEvent::Tab(Tab::Statement));
            }
            if self.is_worker()
                && button_icon(
                    ui,
                    Some(Icon::Upload),
                    "Import statement",
                    Some("Ctrl+O"),
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                events.push(UiEvent::Import);
            }
            if self.can_switch
                && button(ui, "Change account", Some("C"), ButtonKind::Secondary).clicked()
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
        let response = tabs(ui, &["All lines", "Reconciliation statement"], selected);
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
                ui.weak("Loading the statement and its suggestions.");
            } else {
                self.draw_no_statement(ui, events);
            }
            return;
        }
        ui.weak(format!(
            "{} of {} statement lines are not matched. Enter accepts the suggestion, S skips, M matches by hand, J and K move.",
            self.pending_count(),
            self.line_count()
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
                "No statement for this account yet",
                NO_STATEMENT_SENTENCE,
                Some((Some(Icon::Upload), "Import statement", "Ctrl+O")),
            ) {
                events.push(UiEvent::Import);
            }
            let current = self.import_layout();
            ui.horizontal(|ui| {
                ui.weak("File layout");
                style_combo(ui);
                egui::ComboBox::from_id_salt("bank-import-layout")
                    .selected_text(import::layout_label(current))
                    .show_ui(ui, |ui| {
                        for (id, label) in import::LAYOUTS {
                            if ui.selectable_label(id == current, label).clicked() {
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
        let fixed = 84.0 + 88.0 + 88.0 + 112.0 + 128.0;
        let gaps = 5.0 * ui.spacing().item_spacing.x;
        let narration = (ui.available_width() - fixed - gaps).max(120.0);
        table::builder(ui)
            .sense(Sense::click())
            .column(Column::exact(84.0))
            .column(Column::exact(narration))
            .column(Column::exact(88.0))
            .column(Column::exact(88.0))
            .column(Column::exact(112.0))
            .column(Column::exact(128.0))
            .header(table::HEADER_HEIGHT, |mut row| {
                table::header(&mut row, "Date", false);
                table::header(&mut row, "Narration (as imported)", false);
                table::header(&mut row, "Withdrawal", true);
                table::header(&mut row, "Deposit", true);
                table::header(&mut row, "Match", false);
                table::header(&mut row, "Action", false);
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
                    let (kind, text) = match_badge(line, suggestion);
                    let date = format_line_date(line.view.date);
                    let action = action_text(line, suggestion);
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
                    ui.label(status_label(line.status));
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
            ui.horizontal(|ui| {
                badge(ui, suggestion_badge(s.kind), kind_label(s.kind));
            });
            ui.label(format!(
                "{}{}",
                if together {
                    "Match together: "
                } else {
                    "Match: "
                },
                s.voucher_label
            ));
            ui.weak(format!(
                "{}. Confidence {}.",
                s.reason,
                confidence_label(s.confidence)
            ));
            ui.horizontal(|ui| {
                if button(ui, "Match", Some("Enter"), ButtonKind::Primary).clicked() {
                    events.push(UiEvent::Accept(index));
                }
                if button(ui, "Skip", Some("S"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::Skip(index));
                }
            });
        } else {
            ui.weak("No suggestion. Press M to match it by hand.");
            ui.horizontal(|ui| {
                if button(ui, "Match by hand", Some("M"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::ToggleManual);
                }
                if button(ui, "Skip", Some("S"), ButtonKind::Secondary).clicked() {
                    events.push(UiEvent::Skip(index));
                }
            });
        }
    }

    /// The manual match panel: the open vouchers as choices, then the confirm button.
    fn draw_manual(&self, ui: &mut Ui, events: &mut Vec<UiEvent>) {
        ui.strong("Match by hand");
        ui.weak(
            "Choose the vouchers for the focused statement line (Space), then Enter. The engine checks the amounts before the bank date is set. Esc closes.",
        );
        let candidates = self.manual_candidates();
        if candidates.is_empty() {
            ui.label("Every open voucher is matched.");
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
        if button(ui, "Match by hand", Some("Enter"), ButtonKind::Primary).clicked() {
            events.push(UiEvent::ConfirmManual);
        }
    }

    /// The reconciliation statement tab: the engine's rows as given, or the reason it is missing.
    fn draw_statement(&self, ui: &mut Ui) {
        match self.brs() {
            Outcome::Value(view) => {
                ui.strong(format!(
                    "Reconciliation statement as of {}",
                    format_line_date(view.as_of)
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
        match import::pick_file(self.import_layout()) {
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

#[cfg(test)]
mod a11y_names {
    use super::*;

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
