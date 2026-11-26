//! Review queue (mock `review`, `SCREENS.review`): every open proposal in one list, from the
//! sources the engine can list, with the sources it cannot listed by name and reason.
//!
//! The rows come from `Api::review_queue`, the same call the Home card makes, so the two never
//! disagree. The pure functions here ([`rows`], [`not_connected`], [`empty_view`]) turn that
//! answer into text; the screen only draws them and moves the selection.
//!
//! What a row opens: Enter opens the bank reconciliation of the row's bank ledger, where the
//! matcher's suggestion is listed, or, for a possible duplicate (rule D-02), the day book. It does
//! not yet select the suggestion's line.
//!
//! The sidebar count is withheld while any source is not connected ([`badge`]): bank matches and
//! duplicates are listed, the others are not, so the count stays off.
//!
//! Keys (the mock's `keys()`, `index.html` `SCREENS.review`): J and K (or the arrows) move,
//! Enter opens the row, Y or Ctrl+A accepts the highlighted row, N or Alt+D rejects it, Esc goes
//! back (the shell). Accepting a bank match sends it to `Api::confirm_bank_match`, which checks
//! it again; accepting a duplicate has nothing to post and says so. Rejecting hides the row for
//! this session only, because the engine keeps no record of a rejection.

use egui::{Event, InputState, Key, Modifiers, Ui};
use ledgerly_api::{Candidate, DuplicatePayment, ReviewItem, ReviewQueue, ReviewSource};
use ledgerly_core::{BankStatementId, LedgerId, Outcome, Paise};

use crate::i18n::Tr;
use crate::screen::{KeyHint, ScreenAction};
use crate::screens::home::draw::{split, title};
use crate::screens::wire::{Done, Latest, unsupported_text};
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

/// One row of the queue as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// The kind of proposal ("Bank match", "Draft payment" or "Draft receipt"), in the language the row was made in.
    pub kind: String,
    /// What the proposal is about: the statement line text and where it goes.
    pub what: String,
    /// Where it came from: the statement lines and the bank statement date.
    pub source: String,
    /// The matcher's confidence word, as the mock's confidence bars show it.
    pub confidence: Confidence,
    /// The amount of the line, when the proposal covers exactly one line.
    pub amount: Option<Paise>,
    /// The bank ledger the row opens; a duplicate row has none (it opens the day book).
    pub ledger: Option<LedgerId>,
    /// What the row is made of, which says what accepting it would send to the engine.
    pub origin: RowOrigin,
    /// A key that names the proposal, so a session-only rejection survives a reload of the queue.
    pub key: String,
}

/// What a queue row stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowOrigin {
    /// A bank-match suggestion: accepting sends this statement and candidate to the engine.
    Bank {
        /// The stored statement the suggestion came from.
        statement: BankStatementId,
        /// The matcher's suggestion, unchanged.
        candidate: Candidate,
    },
    /// A possible duplicate payment (rule D-02): there is nothing to accept, only two vouchers to
    /// compare in the day book.
    Duplicate,
}

/// A decision on the highlighted row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Y or Ctrl+A.
    Accept,
    /// N or Alt+D.
    Reject,
}

/// What accepting a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Send the matcher's suggestion to `Api::confirm_bank_match`; the engine re-checks it.
    Confirm {
        /// The stored statement.
        statement: BankStatementId,
        /// The suggestion.
        candidate: Box<Candidate>,
    },
    /// Nothing is sent: say this instead.
    Say(String),
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
pub fn source_label(source: ReviewSource) -> String {
    match source {
        ReviewSource::BillDraft => crate::i18n::t("review.source.bill_draft"),
        ReviewSource::PlainWordsDraft => crate::i18n::t("review.source.plain_words"),
        ReviewSource::BankSuggestion => crate::i18n::t("review.source.bank"),
        ReviewSource::NewLedgerSuggestion => crate::i18n::t("review.source.new_ledger"),
        ReviewSource::DuplicateCheck => crate::i18n::t("review.source.duplicate"),
        ReviewSource::Gstr2bExplanation => crate::i18n::t("review.source.gstr2b"),
    }
}

/// The row for one queue item.
#[must_use]
pub fn row_of(item: &ReviewItem) -> QueueRow {
    let candidate = &item.candidate;
    // The matcher produces only exact and rule suggestions (`ledgerly_bank::matcher`). The other
    // kinds have no matcher output, so they are shown as rule drafts rather than dropped.
    let (kind, level) = match candidate.kind {
        ledgerly_model::MatchKind::Exact if candidate.ambiguous => {
            (crate::i18n::t("review.kind.bank_match"), Confidence::Low)
        }
        ledgerly_model::MatchKind::Exact => {
            (crate::i18n::t("review.kind.bank_match"), Confidence::High)
        }
        ledgerly_model::MatchKind::Rule
        | ledgerly_model::MatchKind::AiSuggested
        | ledgerly_model::MatchKind::Manual => {
            let kind = if item.money_in {
                crate::i18n::t("review.kind.draft_receipt")
            } else {
                crate::i18n::t("review.kind.draft_payment")
            };
            (kind, Confidence::Rule)
        }
    };
    let target = match (candidate.vouchers.as_slice(), candidate.suggested_ledger) {
        ([], Some(ledger)) => {
            crate::i18n::tf("review.to_ledger", &[("ledger", &ledger.0.to_string())])
        }
        (vouchers, _) => vouchers
            .iter()
            .map(|v| crate::i18n::tf("review.voucher", &[("number", &v.0.to_string())]))
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
        source: crate::i18n::tf(
            "review.source_line",
            &[
                ("lines", &lines),
                (
                    "date",
                    &format::date_in(candidate.proposed_bank_date, &crate::i18n::current()),
                ),
            ],
        ),
        confidence: level,
        amount: item.amount,
        ledger: Some(item.ledger),
        origin: RowOrigin::Bank {
            statement: item.statement,
            candidate: item.candidate.clone(),
        },
        key: format!(
            "bank:{}:{:?}:{:?}",
            item.statement.0, candidate.statement_lines, candidate.vouchers
        ),
    }
}

/// The name of a payment in the queue's words: its number, or "Voucher N" ("voucher N" inside a
/// sentence) when it has none.
fn payment_name(p: &ledgerly_api::PaymentRef, starts_sentence: bool) -> String {
    p.number.clone().unwrap_or_else(|| {
        let key = if starts_sentence {
            "review.voucher_cap"
        } else {
            "review.voucher"
        };
        crate::i18n::tf(key, &[("number", &p.id.0.to_string())])
    })
}

/// The row for one possible duplicate payment (rule D-02).
#[must_use]
pub fn duplicate_row(pair: &DuplicatePayment) -> QueueRow {
    let tr = crate::i18n::current();
    QueueRow {
        kind: crate::i18n::t("gst.match.possible_duplicate"),
        what: crate::i18n::tf(
            "review.dup.what",
            &[
                ("second", &payment_name(&pair.second, true)),
                ("second_date", &format::date_in(pair.second.date, &tr)),
                ("first", &payment_name(&pair.first, false)),
                ("first_date", &format::date_in(pair.first.date, &tr)),
                ("ledger", &pair.ledger.clone()),
            ],
        ),
        source: crate::i18n::tf(
            if pair.days_apart == 1 {
                "review.dup.source_one"
            } else {
                "review.dup.source_many"
            },
            &[("days", &pair.days_apart.to_string())],
        ),
        confidence: Confidence::Rule,
        amount: Some(pair.amount),
        ledger: None,
        origin: RowOrigin::Duplicate,
        key: format!("duplicate:{}:{}", pair.first.id.0, pair.second.id.0),
    }
}

/// What accepting `row` does.
#[must_use]
pub fn accept_verdict(row: &QueueRow) -> Verdict {
    match &row.origin {
        RowOrigin::Bank {
            statement,
            candidate,
        } => Verdict::Confirm {
            statement: *statement,
            candidate: Box::new(candidate.clone()),
        },
        RowOrigin::Duplicate => Verdict::Say(crate::i18n::t("review.dup.nothing_to_accept")),
    }
}

/// The words shown after a rejection of `row`.
#[must_use]
pub fn reject_notice(row: &QueueRow) -> String {
    match row.origin {
        RowOrigin::Bank { .. } | RowOrigin::Duplicate => crate::i18n::t("review.rejected"),
    }
}

/// The rows of the queue, in order.
#[must_use]
pub fn rows(queue: &ReviewQueue) -> Vec<QueueRow> {
    queue
        .items
        .iter()
        .map(row_of)
        .chain(queue.duplicates.iter().map(duplicate_row))
        .collect()
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
    /// The accept that is running on the worker, with the text of the row it is for.
    confirm: Latest<Result<Outcome<()>, String>>,
    /// Keys of the rows rejected in this session; they stay hidden when the queue is read again.
    dismissed: Vec<String>,
    /// What the last decision did, with its tone.
    notice: Option<(NoteKind, String)>,
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

    /// Applies a decision to the highlighted row. Accepting a bank match sends it to the engine
    /// through `worker`; rejecting hides the row for this session.
    pub fn decide(&mut self, decision: Decision, worker: Option<&ApiWorker>) {
        let Some(row) = self
            .panel
            .as_ref()
            .and_then(|p| p.rows.get(self.selected))
            .cloned()
        else {
            return;
        };
        match decision {
            Decision::Reject => {
                self.dismissed.push(row.key.clone());
                if let Some(panel) = self.panel.as_mut() {
                    panel.rows.retain(|r| r.key != row.key);
                }
                self.clamp_selection();
                self.notice = Some((NoteKind::Info, reject_notice(&row)));
            }
            Decision::Accept => match accept_verdict(&row) {
                Verdict::Say(text) => self.notice = Some((NoteKind::Info, text)),
                Verdict::Confirm {
                    statement,
                    candidate,
                } => {
                    let Some(handle) = worker else {
                        self.notice =
                            Some((NoteKind::Bad, crate::i18n::t("review.no_company_accept")));
                        return;
                    };
                    self.notice = Some((NoteKind::Info, crate::i18n::t("review.accepting")));
                    self.confirm.start(handle, move |api| {
                        api.confirm_bank_match(statement, &candidate)
                            .map(|out| out.map(|_| ()))
                            .map_err(|e| e.to_string())
                    });
                }
            },
        }
    }

    /// The line the last decision left, when there is one.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(_, text)| text.as_str())
    }

    /// Keeps the highlight on a row after rows were removed.
    fn clamp_selection(&mut self) {
        let last = self
            .panel
            .as_ref()
            .map_or(0, |p| p.rows.len().saturating_sub(1));
        self.selected = self.selected.min(last);
    }

    /// The bank ledger that Enter would open (the highlighted row's ledger), if any row exists.
    #[must_use]
    pub fn opens(&self) -> Option<LedgerId> {
        self.panel
            .as_ref()
            .and_then(|p| p.rows.get(self.selected))
            .and_then(|r| r.ledger)
    }

    /// Advances the load by one step: shows a finished answer, then sends a waiting request.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.latest.poll() {
            Some(Done::Ready(Ok(queue))) => {
                self.error = None;
                let mut shown = panel(&queue);
                shown.rows.retain(|r| !self.dismissed.contains(&r.key));
                self.panel = Some(shown);
                self.empty = Some(empty_view(&queue));
                self.clamp_selection();
            }
            Some(Done::Ready(Err(e))) => self.error = Some(e),
            Some(Done::Lost) => {
                self.error = Some(crate::i18n::t("review.worker_stopped"));
            }
            None => {}
        }
        match self.confirm.poll() {
            Some(Done::Ready(Ok(Outcome::Value(())))) => {
                crate::shell::submit::note_changed();
                self.notice = Some((NoteKind::Ok, crate::i18n::t("review.accepted")));
                self.want = true;
            }
            Some(Done::Ready(Ok(Outcome::Unsupported(u)))) => {
                self.notice = Some((NoteKind::Warn, unsupported_text(&u)));
            }
            Some(Done::Ready(Err(e))) => {
                self.notice = Some((
                    NoteKind::Bad,
                    crate::i18n::tf("review.not_accepted", &[("error", &e)]),
                ));
            }
            Some(Done::Lost) => {
                self.notice = Some((
                    NoteKind::Bad,
                    crate::i18n::t("review.worker_stopped_accept"),
                ));
            }
            None => {}
        }
        if !self.want {
            return;
        }
        self.want = false;
        let Some(handle) = worker else {
            self.error = Some(crate::i18n::t("review.no_company"));
            return;
        };
        self.latest.start(handle, |api| {
            api.review_queue_on(crate::shell::navigate::today())
                .map_err(|e| e.to_string())
        });
    }

    /// The Enter action: opens the highlighted row. A bank match opens its bank ledger; a
    /// possible duplicate opens the day book, where both payments can be compared.
    fn open_selected(&mut self, ctx: &AppCtx) -> ScreenAction {
        let Some(row) = self
            .panel
            .as_ref()
            .and_then(|p| p.rows.get(self.selected))
            .cloned()
        else {
            return ScreenAction::Stay;
        };
        let opened = match (row.ledger, ctx.books.as_ref()) {
            (Some(ledger), Some(books)) => crate::shell::navigate::bank_screen(books, ledger)
                .map(|s| Box::new(s) as Box<dyn Screen>),
            (None, _) => {
                crate::shell::navigate::open_report(ledgerly_api::ReportKind::DayBook, ctx)
            }
            (Some(_), None) => {
                self.error = Some(crate::i18n::t("review.no_company_bank"));
                return ScreenAction::Stay;
            }
        };
        match opened {
            Ok(screen) => ScreenAction::Push(screen),
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
                        title(ui, &crate::i18n::t("review.not_connected_title"));
                        para(
                            ui,
                            &crate::i18n::t("review.not_connected_note"),
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
    #[allow(
        clippy::too_many_lines,
        reason = "one drawing pass of the screen, a line for each part of it"
    )]
    fn draw_rows(&self, ui: &mut Ui, panel: &QueuePanel) -> Option<usize> {
        let widths = [
            Width::Fixed(46.0),
            Width::Rest,
            Width::Fixed(150.0),
            Width::Fixed(112.0),
            Width::Fixed(112.0),
        ];
        let head = [
            String::new(),
            crate::i18n::t("review.col.proposal"),
            crate::i18n::t("review.col.source"),
            crate::i18n::t("review.col.confidence"),
            crate::i18n::t("invoice.col.amount"),
        ];
        let rows: Vec<Vec<Cell<'_>>> = panel
            .rows
            .iter()
            .map(|row| {
                let proposal: Cell<'_> = Box::new(move |ui| {
                    let t = crate::ui::tokens(ui);
                    ui.label(
                        egui::RichText::new(&row.kind)
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
                let glyph = match row.origin {
                    RowOrigin::Bank { .. } => Icon::Bank,
                    RowOrigin::Duplicate => Icon::Alert,
                };
                let icon: Cell<'_> = Box::new(move |ui| {
                    let t = crate::ui::tokens(ui);
                    ui.add_space(2.0);
                    crate::ui::icon::icon(ui, glyph, t.text_2);
                });
                let confidence: Cell<'_> = Box::new(move |ui| {
                    conf(ui, row.confidence, &crate::i18n::current());
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
                                egui::RichText::new(crate::i18n::lit("none"))
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
        let (_, lines) = mock_table::table_rows(
            ui,
            &widths,
            Some(&head.each_ref().map(String::as_str)),
            rows,
            Some(self.selected),
        );
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
        let (title, sentence, lines): (String, String, Vec<String>) = match self.empty.clone() {
            Some(EmptyView::NotConnected(lines)) => (
                crate::i18n::t("review.waiting_connected"),
                crate::i18n::t("review.empty_not_connected"),
                lines,
            ),
            Some(EmptyView::Nothing) | None => (
                crate::i18n::t("review.empty_title"),
                crate::i18n::t("review.empty_sentence"),
                Vec::new(),
            ),
        };
        card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let t = crate::ui::tokens(ui);
            ui.vertical_centered(|ui| {
                ui.add_space(crate::theme::SPACE_6);
                crate::ui::icon::icon_sized(ui, Icon::Inbox, t.text_3, 36.0);
                ui.add_space(10.0);
                draw_empty_heading(ui, &title);
                ui.add_space(crate::theme::SPACE_4 / 2.0);
                ui.scope(|ui| {
                    ui.set_max_width(520.0_f32.min(ui.available_width()));
                    ui.add(
                        egui::Label::new(crate::ui::para_text(
                            &sentence,
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

/// The card beside the table (the mock's "How decisions work"), worded for what each key does
/// today.
fn draw_how_decisions_work(ui: &mut Ui) {
    let t = crate::ui::tokens(ui);
    card(ui, |ui| {
        title(ui, &crate::i18n::t("review.how.title"));
        let lines: [(&[&str], String); 5] = [
            (&["Y", "Ctrl+A"], crate::i18n::t("review.how.accept")),
            (&["Enter"], crate::i18n::t("review.how.open")),
            (&["N", "Alt+D"], crate::i18n::t("review.how.reject")),
            (&["J", "K"], crate::i18n::t("review.how.next_previous")),
            (&["Esc"], crate::i18n::t("review.how.back")),
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
                    egui::Label::new(crate::ui::para_text(&what, crate::theme::TEXT_BODY, t.text))
                        .wrap(),
                );
            });
            ui.add_space(6.0);
        }
        para(
            ui,
            &crate::i18n::t("review.how.amounts"),
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
    fn title(&self, tr: &Tr) -> String {
        tr.t("sidebar.review_queue")
    }

    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        None
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        [
            ("", tr.t("sidebar.review_queue")),
            ("J / K", tr.t("keys.move")),
            ("Y", tr.t("gst.ims.accept")),
            ("Enter", tr.t("review.key.open")),
            ("N", tr.t("gst.ims.reject")),
            ("Esc", tr.t("chrome.back")),
        ]
        .into_iter()
        .map(|(keys, label)| KeyHint::new(keys, label))
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
        let (down, up, enter, accept, reject) = ui.input_mut(|i| {
            (
                i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::Enter),
                consume_exact(i, Key::Y, Modifiers::NONE)
                    | consume_exact(i, Key::A, Modifiers::CTRL),
                consume_exact(i, Key::N, Modifiers::NONE)
                    | consume_exact(i, Key::D, Modifiers::ALT),
            )
        });
        if accept {
            self.decide(Decision::Accept, ctx.api.as_ref());
        }
        if reject {
            self.decide(Decision::Reject, ctx.api.as_ref());
        }
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
            &crate::i18n::t("sidebar.review_queue"),
            Some(&crate::i18n::t("review.subtitle")),
            |_| {},
        );
        if let Some(error) = self.error.clone() {
            note(ui, NoteKind::Bad, &error);
        }
        if let Some((kind, text)) = self.notice.clone() {
            note(ui, kind, &text);
            ui.add_space(crate::theme::SPACE_3);
        }
        match self.panel.clone() {
            // An error is on screen already; "loading" beside it would say the opposite.
            None if self.error.is_some() => {}
            None => {
                note(ui, NoteKind::Info, &crate::i18n::t("review.loading"));
            }
            Some(panel) => self.draw_panel(ui, &panel),
        }
        action
    }
}

/// Consumes the press of exactly `key` with exactly `modifiers` held, and reports whether there
/// was one (egui's `consume_key` also accepts extra Alt or Shift).
fn consume_exact(input: &mut InputState, key: Key, modifiers: Modifiers) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            Event::Key { key: pressed_key, modifiers: held, pressed: true, .. }
                if *pressed_key == key && held.matches_exact(modifiers)
        )
    });
    input.events.len() != before
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
            duplicates: Vec::new(),
            unlisted: Vec::new(),
        };
        assert_eq!(empty_view(&connected), EmptyView::Nothing);
    }
}
