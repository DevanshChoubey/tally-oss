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

use egui::{Key, Ui};
use ledgerly_api::{ReviewItem, ReviewQueue, ReviewSource};
use ledgerly_core::{LedgerId, Paise};

use crate::screen::{KeyHint, ScreenAction};
use crate::screens::home::draw::{split, title};
use crate::screens::wire::{Done, Latest};
use crate::ui::conf::{Confidence, conf};
use crate::ui::format;
use crate::ui::icon::Icon;
use crate::ui::keycap::keycap;
use crate::ui::mock_table::{self, Cell, Width};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::{
    bold_font,
    card::{card, card_flush},
    para,
};
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen};

/// Subtitle of the page head (the mock's `review` head line).
const SUBTITLE: &str = "Every open AI proposal and rule check in one place. Decide here or in context; it is the same proposal.";
/// The mock's empty-state heading and sentence, shown only when every source is connected and
/// none has a proposal.
const EMPTY_TITLE: &str = "Nothing to review";
/// See [`EMPTY_TITLE`].
const EMPTY_SENTENCE: &str = "New bill photos, bank suggestions and 2B explanations will appear here. Ledgerly never acts on them without you.";

/// One row of the queue as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// The kind of proposal: "Bank match", "Draft payment" or "Draft receipt".
    pub kind: &'static str,
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
#[must_use]
pub fn row_of(item: &ReviewItem) -> QueueRow {
    let candidate = &item.candidate;
    // The matcher produces only exact and rule suggestions (`ledgerly_bank::matcher`). The other
    // kinds have no matcher output, so they are shown as rule drafts rather than dropped.
    let (kind, level) = match candidate.kind {
        ledgerly_model::MatchKind::Exact if candidate.ambiguous => ("Bank match", Confidence::Low),
        ledgerly_model::MatchKind::Exact => ("Bank match", Confidence::High),
        ledgerly_model::MatchKind::Rule
        | ledgerly_model::MatchKind::AiSuggested
        | ledgerly_model::MatchKind::Manual => {
            let kind = if item.money_in {
                "Draft receipt"
            } else {
                "Draft payment"
            };
            (kind, Confidence::Rule)
        }
    };
    let target = match (candidate.vouchers.as_slice(), candidate.suggested_ledger) {
        ([], Some(ledger)) => format!("to ledger {}", ledger.0),
        (vouchers, _) => vouchers
            .iter()
            .map(|v| format!("voucher {}", v.0))
            .collect::<Vec<_>>()
            .join(", "),
    };
    let lines = candidate
        .statement_lines
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    QueueRow {
        kind,
        what: format!("{} {}", item.description, target).trim().to_owned(),
        source: format!(
            "Bank statement line {lines} \u{b7} {}",
            format::date(candidate.proposed_bank_date)
        ),
        confidence: level,
        amount: item.amount,
        ledger: item.ledger,
    }
}

/// The rows of the queue, in order.
#[must_use]
pub fn rows(queue: &ReviewQueue) -> Vec<QueueRow> {
    queue.items.iter().map(row_of).collect()
}

/// One line per source that is not connected: its name, then the engine's reason.
#[must_use]
pub fn not_connected(queue: &ReviewQueue) -> Vec<String> {
    queue
        .unlisted
        .iter()
        .map(|u| format!("{}: {}", source_label(u.source), u.reason))
        .collect()
}

/// The sidebar count for the queue: the number of open proposals when every source is listed,
/// and nothing while any source is not connected (a partial count is never shown).
#[must_use]
pub fn badge(queue: &ReviewQueue) -> Option<usize> {
    queue.count()
}

/// The whole panel for a queue answer.
#[must_use]
pub fn panel(queue: &ReviewQueue) -> QueuePanel {
    QueuePanel {
        rows: rows(queue),
        not_connected: not_connected(queue),
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

/// The empty state for a queue with no rows. The mock's wording is used only when no source is
/// left unlisted, since a missing list is not the same as an empty one.
#[must_use]
pub fn empty_view(queue: &ReviewQueue) -> EmptyView {
    if queue.unlisted.is_empty() {
        EmptyView::Nothing
    } else {
        EmptyView::NotConnected(not_connected(queue))
    }
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

    /// Advances the load by one step: shows a finished answer, then sends a waiting request.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.latest.poll() {
            Some(Done::Ready(Ok(queue))) => {
                self.error = None;
                self.panel = Some(panel(&queue));
                self.empty = if queue.items.is_empty() {
                    Some(empty_view(&queue))
                } else {
                    None
                };
                self.selected = self.selected.min(queue.items.len().saturating_sub(1));
            }
            Some(Done::Ready(Err(e))) => self.error = Some(e),
            Some(Done::Lost) => {
                self.error = Some(
                    "The background worker stopped, so the queue could not be read.".to_owned(),
                );
            }
            None => {}
        }
        if !self.want {
            return;
        }
        self.want = false;
        let Some(handle) = worker else {
            self.error = Some("No company is open, so there is no queue to show.".to_owned());
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
            self.error = Some("No company is open, so the bank ledger cannot open.".to_owned());
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
    /// Draws the queue as the mock does: a table of proposals beside the "How decisions work"
    /// card, or the empty state.
    fn draw_panel(&mut self, ui: &mut Ui, panel: &QueuePanel) {
        if panel.rows.is_empty() {
            self.draw_empty(ui);
            return;
        }
        let mut clicked = None;
        split(
            ui,
            |ui| {
                card_flush(ui, |ui| clicked = self.draw_rows(ui, panel));
            },
            |ui| {
                draw_how_decisions_work(ui);
                // The sources the engine cannot list are named under the card, so a short list is
                // never taken for a complete one.
                if !panel.not_connected.is_empty() {
                    ui.add_space(crate::theme::SPACE_3);
                    card(ui, |ui| {
                        title(ui, "Not connected yet");
                        para(
                            ui,
                            "These proposals are not listed. Nothing is shown for them here, and nothing is posted.",
                            crate::theme::TEXT_BODY,
                            crate::ui::tokens(ui).text_2,
                        );
                        for line in &panel.not_connected {
                            para(
                                ui,
                                line,
                                crate::theme::TEXT_META,
                                crate::ui::tokens(ui).text_2,
                            );
                        }
                    });
                }
            },
        );
        if let Some(index) = clicked {
            self.selected = index;
        }
    }

    /// The proposals table: an icon, the proposal, its source, the confidence and the amount.
    fn draw_rows(&self, ui: &mut Ui, panel: &QueuePanel) -> Option<usize> {
        let widths = [
            Width::Fixed(46.0),
            Width::Rest,
            Width::Fixed(150.0),
            Width::Fixed(112.0),
            Width::Fixed(112.0),
        ];
        let head = ["", "Proposal", "Source", "Confidence", "Amount"];
        let rows: Vec<Vec<Cell<'_>>> = panel
            .rows
            .iter()
            .map(|row| {
                let proposal: Cell<'_> = Box::new(move |ui| {
                    let t = crate::ui::tokens(ui);
                    ui.label(
                        egui::RichText::new(row.kind)
                            .font(bold_font(ui, crate::theme::TEXT_BODY))
                            .color(t.text),
                    );
                    ui.add(
                        egui::Label::new(crate::ui::para_text(
                            &row.what,
                            crate::theme::TEXT_BODY,
                            t.text_2,
                        ))
                        .wrap(),
                    );
                });
                let icon: Cell<'_> = Box::new(|ui| {
                    let t = crate::ui::tokens(ui);
                    ui.add_space(2.0);
                    crate::ui::icon::icon(ui, Icon::Bank, t.text_2);
                });
                let confidence: Cell<'_> = Box::new(move |ui| {
                    conf(ui, row.confidence);
                });
                let amount: Cell<'_> = Box::new(move |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let t = crate::ui::tokens(ui);
                        match row.amount {
                            Some(amount) => ui.label(
                                egui::RichText::new(format::amount(amount))
                                    .font(egui::FontId::proportional(crate::theme::TEXT_BODY))
                                    .color(t.text),
                            ),
                            None => ui.label(
                                egui::RichText::new("none")
                                    .font(egui::FontId::proportional(crate::theme::TEXT_BODY))
                                    .color(t.text_3),
                            ),
                        };
                    });
                });
                vec![
                    icon,
                    proposal,
                    mock_table::muted(&row.source),
                    confidence,
                    amount,
                ]
            })
            .collect();
        let (_, lines) =
            mock_table::table_rows(ui, &widths, Some(&head), rows, Some(self.selected));
        let mut clicked = None;
        for (index, line) in lines.into_iter().enumerate() {
            let response = ui.interact(
                line,
                ui.scope_id().with(("review-row", index)),
                egui::Sense::click(),
            );
            if response.clicked() {
                clicked = Some(index);
            }
        }
        clicked
    }

    /// The empty state: when no source is left unlisted, the mock's; otherwise it says what the
    /// connected sources hold and names the ones that are not connected.
    fn draw_empty(&self, ui: &mut Ui) {
        let (title, sentence, lines): (&str, &str, Vec<String>) = match self.empty.clone() {
            Some(EmptyView::NotConnected(lines)) => (
                "Nothing waiting from the connected sources",
                "Bank matches appear here when a statement is imported. These sources are not connected, so nothing is shown for them:",
                lines,
            ),
            Some(EmptyView::Nothing) | None => (EMPTY_TITLE, EMPTY_SENTENCE, Vec::new()),
        };
        card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let t = crate::ui::tokens(ui);
            ui.vertical_centered(|ui| {
                ui.add_space(crate::theme::SPACE_6);
                crate::ui::icon::icon_sized(ui, Icon::Inbox, t.text_3, 36.0);
                ui.add_space(10.0);
                draw_empty_heading(ui, title);
                ui.add_space(crate::theme::SPACE_4 / 2.0);
                ui.scope(|ui| {
                    ui.set_max_width(520.0_f32.min(ui.available_width()));
                    ui.add(
                        egui::Label::new(crate::ui::para_text(
                            sentence,
                            crate::theme::TEXT_BODY,
                            t.text_2,
                        ))
                        .wrap(),
                    );
                    for line in &lines {
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(crate::ui::para_text(
                                line,
                                crate::theme::TEXT_META,
                                t.text_3,
                            ))
                            .wrap(),
                        );
                    }
                });
                ui.add_space(crate::theme::SPACE_6);
            });
        });
    }
}

/// The card beside the table (the mock's "How decisions work"), limited to the keys this screen
/// has: Y, N and Ctrl+A are not built, and accepting happens on the bank screen.
fn draw_how_decisions_work(ui: &mut Ui) {
    let t = crate::ui::tokens(ui);
    card(ui, |ui| {
        title(ui, "How decisions work");
        let lines: [(&[&str], &str); 3] = [
            (
                &["Enter"],
                "Open in context: the bank reconciliation of the proposal's account, where the matcher's suggestion is listed.",
            ),
            (&["J", "K"], "Next and previous."),
            (
                &["Esc"],
                "Back. Nothing is posted from this list; accepting a match happens in the bank reconciliation.",
            ),
        ];
        for (keys, what) in lines {
            ui.horizontal_top(|ui| {
                ui.allocate_ui(egui::vec2(72.0, 20.0), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for key in keys {
                            keycap(ui, key);
                        }
                    });
                });
                ui.add(
                    egui::Label::new(crate::ui::para_text(what, crate::theme::TEXT_BODY, t.text))
                        .wrap(),
                );
            });
            ui.add_space(6.0);
        }
        para(
            ui,
            "Amounts in this list are the bank line as imported, never a total computed here.",
            crate::theme::TEXT_META,
            t.text_2,
        );
    });
}

/// A bold heading inside a card.
fn draw_empty_heading(ui: &mut Ui, text: &str) {
    ui.label(bold_font_text(ui, text));
}

impl Screen for ReviewScreen {
    fn title(&self) -> String {
        "Review queue".to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        None
    }

    fn keys(&self) -> Vec<KeyHint> {
        [
            ("", "Review queue"),
            ("J / K", "Move"),
            ("Enter", "Open to edit"),
            ("Esc", "Back"),
        ]
        .iter()
        .map(|(keys, label)| KeyHint::new(*keys, *label))
        .collect()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.panel.is_none() && self.error.is_none() && !self.latest.is_pending() {
            self.want = true;
        }
        self.tick(ctx.api.as_ref());
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
        page_head(ui, "Review queue", Some(SUBTITLE), |_| {});
        if let Some(error) = self.error.clone() {
            note(ui, NoteKind::Bad, &error);
        }
        match self.panel.clone() {
            // An error is on screen already; "loading" beside it would say the opposite.
            None if self.error.is_some() => {}
            None => {
                note(ui, NoteKind::Info, "Loading the queue from the engine.");
            }
            Some(panel) => self.draw_panel(ui, &panel),
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
