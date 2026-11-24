//! Review queue (mock `review`, `SCREENS.review`): every open proposal in one list, from the
//! sources the engine can list, with the sources it cannot listed by name and reason.
//!
//! The rows come from `Api::review_queue`, the same call the Home card makes, so the two never
//! disagree. The pure functions here ([`rows`], [`not_connected`], [`empty_view`]) turn that
//! answer into text; the screen only draws them and moves the selection.
//!
//! What a row opens: Enter opens the bank reconciliation of the row's bank ledger, where the
//! matcher's suggestion is listed. It does not yet select the suggestion's line, and accepting
//! happens there (`Api::confirm_bank_match`), so Y and N are not on this screen.
//!
//! The sidebar count is withheld while any source is not connected ([`badge`]): today every
//! source but bank matches is unlisted, so the count stays off.
//!
//! Keys (the mock's `keys()`, `index.html` `SCREENS.review`): J and K (or the arrows) move,
//! Enter opens the row's bank ledger, Esc goes back (the shell). Y, N and Ctrl+A are not built.

use crate::i18n::Tr;
use egui::{Key, Ui};
use ledgerly_api::{ReviewItem, ReviewQueue, ReviewSource};
use ledgerly_core::{LedgerId, Paise};

use crate::screen::{KeyHint, ScreenAction};
use crate::screens::wire::{Done, Latest};
use crate::ui::conf::{Confidence, conf};
use crate::ui::format;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::{bold_font, card::card, para};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen};

/// One row of the queue as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// The kind of proposal in English: "Bank match", "Draft payment" or "Draft receipt".
    pub kind: &'static str,
    /// The kind in the words of the language the screen is drawn in.
    pub kind_word: String,
    /// What the proposal is about: the statement line text and where it goes.
    pub what: String,
    /// Where it came from: the statement lines and the bank statement date.
    pub source: String,
    /// The matcher's confidence word, as the mock's confidence bars show it.
    pub confidence: Confidence,
    /// The amount of the line, when the proposal covers exactly one line.
    pub amount: Option<Paise>,
    /// The bank ledger the row opens.
    pub ledger: LedgerId,
}

impl QueueRow {
    /// The kind as the screen shows it: the word in the language the row was built in.
    #[must_use]
    pub fn shown_kind(&self) -> &str {
        &self.kind_word
    }
}

/// The queue as the screen shows it: its rows, its not-connected sources and its count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuePanel {
    /// The rows, in the queue's order.
    pub rows: Vec<QueueRow>,
    /// One line per source that is not connected: its name and the engine's reason.
    pub not_connected: Vec<String>,
    /// The sidebar count, when every source is connected; see [`badge`].
    pub count: Option<usize>,
}

/// The name of a source, in the words of `tr`. TODO stub: English until the Hindi keys exist.
/// The name of a source, in the words of `tr`.
#[must_use]
pub fn source_label_in(source: ReviewSource, tr: &Tr) -> String {
    tr.t(match source {
        ReviewSource::BillDraft => "review.source.bill_draft",
        ReviewSource::PlainWordsDraft => "review.source.plain_words",
        ReviewSource::BankSuggestion => "review.source.bank",
        ReviewSource::NewLedgerSuggestion => "review.source.new_ledger",
        ReviewSource::DuplicateCheck => "review.source.duplicate",
        ReviewSource::Gstr2bExplanation => "review.source.gstr2b",
    })
}

/// The name of a source, in the words the screen shows.
#[must_use]
pub fn source_label(source: ReviewSource) -> &'static str {
    match source {
        ReviewSource::BillDraft => "Bill photos",
        ReviewSource::PlainWordsDraft => "Plain-words drafts",
        ReviewSource::BankSuggestion => "Bank matches",
        ReviewSource::NewLedgerSuggestion => "New ledger suggestions",
        ReviewSource::DuplicateCheck => "Possible duplicates",
        ReviewSource::Gstr2bExplanation => "GSTR-2B explanations",
    }
}

/// The row for one queue item.
/// The row for one queue item, in English: [`row_of_in`] with the English file.
#[must_use]
pub fn row_of(item: &ReviewItem) -> QueueRow {
    row_of_in(item, &Tr::default())
}

/// The row for one queue item, in the words of `tr`.
#[must_use]
pub fn row_of_in(item: &ReviewItem, tr: &Tr) -> QueueRow {
    let candidate = &item.candidate;
    // The matcher produces only exact and rule suggestions (`ledgerly_bank::matcher`). The other
    // kinds have no matcher output, so they are shown as rule drafts rather than dropped.
    let (kind, key, level) = match candidate.kind {
        ledgerly_model::MatchKind::Exact if candidate.ambiguous => {
            ("Bank match", "review.kind.bank_match", Confidence::Low)
        }
        ledgerly_model::MatchKind::Exact => {
            ("Bank match", "review.kind.bank_match", Confidence::High)
        }
        ledgerly_model::MatchKind::Rule
        | ledgerly_model::MatchKind::AiSuggested
        | ledgerly_model::MatchKind::Manual => {
            if item.money_in {
                (
                    "Draft receipt",
                    "review.kind.draft_receipt",
                    Confidence::Rule,
                )
            } else {
                (
                    "Draft payment",
                    "review.kind.draft_payment",
                    Confidence::Rule,
                )
            }
        }
    };
    let target = match (candidate.vouchers.as_slice(), candidate.suggested_ledger) {
        ([], Some(ledger)) => tr.tf(
            "review.to_ledger",
            &[("ledger", ledger.0.to_string().as_str())],
        ),
        (vouchers, _) => vouchers
            .iter()
            .map(|v| tr.tf("review.voucher", &[("number", v.0.to_string().as_str())]))
            .collect::<Vec<_>>()
            .join(", "),
    };
    let lines = candidate
        .statement_lines
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let date = format::date_in(candidate.proposed_bank_date, tr);
    QueueRow {
        kind,
        kind_word: tr.t(key),
        what: format!("{} {}", item.description, target).trim().to_owned(),
        source: tr.tf(
            "review.source_line",
            &[("lines", lines.as_str()), ("date", date.as_str())],
        ),
        confidence: level,
        amount: item.amount,
        ledger: item.ledger,
    }
}

/// The rows of the queue, in order.
/// The rows of the queue, in order, in English.
#[must_use]
pub fn rows(queue: &ReviewQueue) -> Vec<QueueRow> {
    rows_in(queue, &Tr::default())
}

/// The rows of the queue, in order, in the words of `tr`.
#[must_use]
pub fn rows_in(queue: &ReviewQueue, tr: &Tr) -> Vec<QueueRow> {
    queue.items.iter().map(|item| row_of_in(item, tr)).collect()
}

/// One line per source that is not connected: its name, then the engine's reason.
/// One line per source that is not connected, in English: its name, then the engine's reason.
#[must_use]
pub fn not_connected(queue: &ReviewQueue) -> Vec<String> {
    not_connected_in(queue, &Tr::default())
}

/// One line per source that is not connected, in the words of `tr`: its name, then the engine's
/// reason as given.
#[must_use]
pub fn not_connected_in(queue: &ReviewQueue, tr: &Tr) -> Vec<String> {
    queue
        .unlisted
        .iter()
        .map(|u| {
            let label = source_label_in(u.source, tr);
            tr.tf(
                "review.unlisted",
                &[("label", label.as_str()), ("reason", u.reason.as_str())],
            )
        })
        .collect()
}

/// The sidebar count for the queue: the number of open proposals when every source is listed,
/// and nothing while any source is not connected (a partial count is never shown).
#[must_use]
pub fn badge(queue: &ReviewQueue) -> Option<usize> {
    queue.count()
}

/// The whole panel for a queue answer.
/// The whole panel for a queue answer, in English.
#[must_use]
pub fn panel(queue: &ReviewQueue) -> QueuePanel {
    panel_in(queue, &Tr::default())
}

/// The whole panel for a queue answer, in the words of `tr`.
#[must_use]
pub fn panel_in(queue: &ReviewQueue, tr: &Tr) -> QueuePanel {
    QueuePanel {
        rows: rows_in(queue, tr),
        not_connected: not_connected_in(queue, tr),
        count: badge(queue),
    }
}

/// What the body shows when the queue has no rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmptyView {
    /// Every source is connected and none has a proposal: the mock's empty state.
    Nothing,
    /// Nothing from the connected sources is waiting; the listed sources are not connected.
    NotConnected(Vec<String>),
}

/// The empty state for a queue with no rows, in the words of `tr`. The mock's wording is used only
/// when no source is left unlisted, since a missing list is not the same as an empty one.
#[must_use]
pub fn empty_view_in(queue: &ReviewQueue, tr: &Tr) -> EmptyView {
    if queue.unlisted.is_empty() {
        EmptyView::Nothing
    } else {
        EmptyView::NotConnected(not_connected_in(queue, tr))
    }
}

/// The empty state for a queue with no rows. The mock's wording is used only when no source is
/// left unlisted, since a missing list is not the same as an empty one.
/// The empty state for a queue with no rows, in English. The mock's wording is used only when no
/// source is left unlisted, since a missing list is not the same as an empty one.
#[must_use]
pub fn empty_view(queue: &ReviewQueue) -> EmptyView {
    empty_view_in(queue, &Tr::default())
}

/// The Review queue screen.
#[derive(Debug, Default)]
pub struct ReviewScreen {
    /// The queue call running on the worker.
    latest: Latest<Result<ReviewQueue, String>>,
    /// The last answer, as a panel.
    panel: Option<QueuePanel>,
    /// The queue's empty state, when it has no rows.
    empty: Option<EmptyView>,
    /// Why the queue could not be read, when it could not.
    error: Option<String>,
    /// The highlighted row.
    selected: usize,
    /// A request to load (or reload) the queue, not yet sent to the worker.
    want: bool,
}

impl ReviewScreen {
    /// Asks for the queue to be read again by the worker. The next [`ReviewScreen::tick`] sends it.
    pub fn request_load(&mut self) {
        self.want = true;
    }

    /// The panel of the last answer, when there is one.
    #[must_use]
    pub fn panel(&self) -> Option<&QueuePanel> {
        self.panel.as_ref()
    }

    /// Why the last load failed, when it did.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The highlighted row index.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Moves the highlight by `delta` rows, stopping at the ends.
    pub fn move_by(&mut self, delta: isize) {
        let Some(panel) = &self.panel else {
            return;
        };
        let last = panel.rows.len().saturating_sub(1);
        let next = self.selected.saturating_add_signed(delta).min(last);
        self.selected = next;
    }

    /// The bank ledger that Enter would open (the highlighted row's ledger), if any row exists.
    #[must_use]
    pub fn opens(&self) -> Option<LedgerId> {
        self.panel
            .as_ref()
            .and_then(|p| p.rows.get(self.selected))
            .map(|r| r.ledger)
    }

    /// Advances the load by one step, in English: [`ReviewScreen::tick_in`] with the English file.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        self.tick_in(worker, &Tr::default());
    }

    /// Advances the load by one step: shows a finished answer, then sends a waiting request. The
    /// lines the screen itself says are in the words of `tr`.
    pub fn tick_in(&mut self, worker: Option<&ApiWorker>, tr: &Tr) {
        match self.latest.poll() {
            Some(Done::Ready(Ok(queue))) => {
                self.error = None;
                self.panel = Some(panel_in(&queue, tr));
                self.empty = if queue.items.is_empty() {
                    Some(empty_view_in(&queue, tr))
                } else {
                    None
                };
                self.selected = self.selected.min(queue.items.len().saturating_sub(1));
            }
            Some(Done::Ready(Err(e))) => self.error = Some(e),
            Some(Done::Lost) => {
                self.error = Some(tr.t("review.worker_stopped"));
            }
            None => {}
        }
        if !self.want {
            return;
        }
        self.want = false;
        let Some(handle) = worker else {
            self.error = Some(tr.t("review.no_company"));
            return;
        };
        self.latest
            .start(handle, |api| api.review_queue().map_err(|e| e.to_string()));
    }

    /// The Enter action: opens the highlighted row's bank ledger on the screen stack.
    fn open_selected(&mut self, ctx: &AppCtx) -> ScreenAction {
        let Some(ledger) = self.opens() else {
            return ScreenAction::Stay;
        };
        let Some(books) = ctx.books.as_ref() else {
            self.error = Some(ctx.tr.t("review.no_company_bank"));
            return ScreenAction::Stay;
        };
        match crate::shell::navigate::bank_screen(books, ledger) {
            Ok(screen) => ScreenAction::Push(Box::new(screen)),
            Err(e) => {
                self.error = Some(e.to_string());
                ScreenAction::Stay
            }
        }
    }
}

impl ReviewScreen {
    /// Draws the rows (or the empty state) and the not-connected sources.
    fn draw_panel(&mut self, ui: &mut Ui, panel: &QueuePanel, tr: &Tr) {
        card(ui, |ui| {
            if panel.rows.is_empty() {
                let lines = match self.empty.clone() {
                    Some(EmptyView::NotConnected(lines)) => {
                        draw_empty_heading(ui, &tr.t("review.waiting_connected"));
                        lines
                    }
                    Some(EmptyView::Nothing) | None => {
                        draw_empty_heading(ui, &tr.t("review.empty_title"));
                        vec![tr.t("review.empty_sentence")]
                    }
                };
                for line in lines {
                    para(
                        ui,
                        &line,
                        crate::theme::TEXT_BODY,
                        crate::ui::tokens(ui).text_2,
                    );
                }
                return;
            }
            for (index, row) in panel.rows.iter().enumerate() {
                let selected = index == self.selected;
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(selected, row.kind_word.as_str())
                        .clicked()
                    {
                        self.selected = index;
                    }
                    ui.label(&row.what);
                    conf(ui, row.confidence, tr);
                    if let Some(amount) = row.amount {
                        ui.label(format::amount(amount));
                    }
                });
                ui.label(&row.source);
            }
        });
        // With no rows the empty state above already names the sources, so the card is not repeated.
        if !panel.rows.is_empty() && !panel.not_connected.is_empty() {
            ui.add_space(crate::theme::SPACE_3);
            card(ui, |ui| {
                draw_empty_heading(ui, &tr.t("review.not_connected_title"));
                para(
                    ui,
                    &tr.t("review.not_connected_note"),
                    crate::theme::TEXT_BODY,
                    crate::ui::tokens(ui).text_2,
                );
                for line in &panel.not_connected {
                    para(
                        ui,
                        line,
                        crate::theme::TEXT_BODY,
                        crate::ui::tokens(ui).text_2,
                    );
                }
            });
        }
    }
}

/// A bold heading inside a card.
fn draw_empty_heading(ui: &mut Ui, text: &str) {
    ui.label(bold_font_text(ui, text));
}

impl Screen for ReviewScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("review.title")
    }

    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        None
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        vec![
            KeyHint::new("", tr.t("review.title")),
            KeyHint::new("J / K", tr.t("review.key.move")),
            KeyHint::new("Enter", tr.t("review.key.open")),
            KeyHint::new("Esc", tr.t("review.key.back")),
        ]
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        let tr = ctx.tr.clone();
        if self.panel.is_none() && self.error.is_none() && !self.latest.is_pending() {
            self.want = true;
        }
        self.tick_in(ctx.api.as_ref(), &tr);
        if self.latest.is_pending() {
            ui.ctx().request_repaint();
        }
        let (down, up, enter) = ui.input(|i| {
            (
                i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::Enter),
            )
        });
        if down {
            self.move_by(1);
        }
        if up {
            self.move_by(-1);
        }
        let action = if enter {
            self.open_selected(ctx)
        } else {
            ScreenAction::Stay
        };
        page_head(
            ui,
            &tr.t("review.title"),
            Some(&tr.t("review.subtitle")),
            |_| {},
        );
        if let Some(error) = self.error.clone() {
            note(ui, NoteKind::Bad, &error);
        }
        match self.panel.clone() {
            None => {
                note(ui, NoteKind::Info, &tr.t("review.loading"));
            }
            Some(panel) => self.draw_panel(ui, &panel, &tr),
        }
        action
    }
}

/// A bold label for a card heading.
fn bold_font_text(ui: &Ui, text: &str) -> egui::RichText {
    egui::RichText::new(text).font(bold_font(ui, crate::theme::TEXT_H2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_wording_is_used_only_when_nothing_is_unlisted() {
        // provenance: derived. A queue with no items and no unlisted source is the mock's empty
        // state; one unlisted source means the empty wording would claim more than the engine lists.
        let connected = ReviewQueue {
            items: Vec::new(),
            unlisted: Vec::new(),
        };
        assert_eq!(empty_view(&connected), EmptyView::Nothing);
    }
}
