//! Ask your books with the assistant (round s21, plan round E; mock states `ai.ask.idle`,
//! `.loading`, `.result`, `.unsupported` and `.failed`).
//!
//! The question goes through the engine's one gate (`Api::ask_books`): the tier, the feature's
//! switch and the provider's status. The provider answers with a report query, which the API
//! checks against the question, resolves and runs with the report engine. The card draws the
//! engine's `Report` and nothing the provider wrote: the chips are the report's own title and
//! dates, and every figure is a cell of the report. Enter opens the full report; Alt+D dismisses
//! the answer; saving a question as a report is not built (Ctrl+A is shown blocked).

use egui::{FontId, Frame, Key, Margin, RichText, Stroke, TextEdit, Ui};
use ledgerly_api::{
    AiFeature, AiGate, Api, AskAnswer, ProviderPlace, Report, ReportKind, ReportParams,
};
use ledgerly_core::CancelToken;

use crate::i18n::lit;
use crate::palette::PaletteTarget;
use crate::screen::KeyHint;
use crate::screens::ai_gate;
use crate::screens::report_view::{ReportSource, ReportViewScreen, engine_cell_text};
use crate::screens::wire::{Done, Latest};
use crate::shell::navigate;
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_INPUT, TEXT_META, border_width};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::chip::chip;
use crate::ui::icon::{Icon, icon};
use crate::ui::keycap::keycap;
use crate::ui::note::{NoteKind, note};
use crate::ui::prop::{Prop, PropAction};
use crate::ui::skeleton::skeleton_rows;
use crate::ui::{bold_font, format, kind, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, ScreenAction};

/// The screen's title (index.html, `SCREENS.ask`).
const TITLE: &str = "Ask your books";
/// The footer of the answer card: Ctrl+A is blocked for this reason.
const SAVE_NOT_BUILT: &str = "Saving a question as a report is not built yet";
/// The questions under "Try" (index.html `askAiRender`): one the engine answers, one it cannot.
const TRY: [&str; 2] = ["who owes us money?", "what will my profit be next quarter?"];
/// The most columns the card shows; a wider report shows its last column only.
const MAX_COLUMNS: usize = 4;
/// The most rows the card shows; the rest are in the full report.
const MAX_ROWS: usize = 12;
/// The reports an unsupported answer offers (index.html `ai.ask.unsupported`): palette keys.
const RELATED: [(&str, Icon, &str); 3] = [
    ("Profit and loss", Icon::Scale, "profit-and-loss"),
    ("Balance sheet", Icon::Scale, "balance-sheet"),
    ("Bills receivable", Icon::Clock, "receivables"),
];

/// An answer the engine ran.
#[derive(Debug, Clone)]
struct Answer {
    /// The engine's report.
    report: Report,
    /// Where the provider that wrote the query runs.
    place: ProviderPlace,
}

/// Where the screen is.
#[derive(Debug)]
enum Phase {
    /// Waiting for a question.
    Idle,
    /// The question is with the assistant and the engine; where the provider runs.
    Asking(ProviderPlace),
    /// The engine ran the query.
    Ran(Box<Answer>),
    /// No report can carry the query; why.
    Unsupported(String),
    /// The provider could not write a query, or wrote one the question does not support; why.
    Failed(String),
}

/// What the worker answered.
#[derive(Debug)]
enum Reply {
    /// The gate.
    Gate(Result<AiGate, String>),
    /// The answer to a question.
    Asked(Result<AskAnswer, String>),
}

/// The assistant half of the Ask your books screen.
#[derive(Debug)]
pub(crate) struct AiAsk {
    /// The gate, once read.
    gate: Option<Result<AiGate, String>>,
    /// The question being typed.
    text: String,
    /// Where the screen is.
    phase: Phase,
    /// A line for the person about the last action.
    notice: Option<String>,
    /// The one call in flight.
    latest: Latest<Reply>,
    /// Whether the box took focus since it could last be typed in.
    focused_once: bool,
}

impl Default for AiAsk {
    fn default() -> Self {
        Self {
            gate: None,
            text: String::new(),
            phase: Phase::Idle,
            notice: None,
            latest: Latest::default(),
            focused_once: false,
        }
    }
}

impl AiAsk {
    /// Whether the gate has been read and no question is in flight.
    pub(crate) fn loaded(&self) -> bool {
        self.gate.is_some() && !matches!(self.phase, Phase::Asking(_))
    }

    /// Whether the gate lets a question run; until then the screen shows the report list.
    pub(crate) fn ready(&self) -> bool {
        matches!(self.gate, Some(Ok(AiGate::Ready(_))))
    }

    /// The key bar for where the screen is (mock `SCREENS.ask.keys` with `ai=on`).
    pub(crate) fn keys(&self) -> Vec<KeyHint> {
        match &self.phase {
            Phase::Ran(_) => vec![
                KeyHint::new("", lit(TITLE)),
                KeyHint::new("Enter", lit("Open the full report")),
                KeyHint::new("Alt+D", lit("Dismiss")),
            ],
            Phase::Asking(_) => vec![KeyHint::new("", lit(TITLE))],
            Phase::Idle | Phase::Unsupported(_) | Phase::Failed(_) => vec![
                KeyHint::new("", lit(TITLE)),
                KeyHint::new("Enter", lit("Ask")),
            ],
        }
    }

    /// Reads the gate once a worker is there.
    pub(crate) fn poll_gate(&mut self, ctx: &AppCtx) {
        self.tick();
        if let Some(worker) = ctx.api.as_ref()
            && self.gate.is_none()
            && !self.latest.is_pending()
        {
            self.latest.start(worker, |api: &mut Api| {
                Reply::Gate(api.ai_gate(AiFeature::Ask).map_err(|e| e.to_string()))
            });
        }
    }

    fn start_ask(&mut self, worker: &ApiWorker) {
        let question = self.text.trim().to_owned();
        let Some(Ok(AiGate::Ready(info))) = &self.gate else {
            return;
        };
        if question.is_empty() || self.latest.is_pending() {
            return;
        }
        self.phase = Phase::Asking(info.place);
        self.notice = None;
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Asked(
                api.ask_books(&question, &CancelToken::new())
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn tick(&mut self) {
        let reply = match self.latest.poll() {
            None => return,
            Some(Done::Lost) => {
                self.notice = Some(lit("The company file stopped responding; reopen it."));
                self.phase = Phase::Idle;
                return;
            }
            Some(Done::Ready(reply)) => reply,
        };
        match reply {
            Reply::Gate(gate) => self.gate = Some(gate),
            Reply::Asked(Ok(AskAnswer::Ran(report))) => {
                if let Phase::Asking(place) = self.phase {
                    self.phase = Phase::Ran(Box::new(Answer {
                        report: *report,
                        place,
                    }));
                }
            }
            Reply::Asked(Ok(AskAnswer::Unsupported(reason))) => {
                self.phase = Phase::Unsupported(reason);
            }
            Reply::Asked(Ok(AskAnswer::Failed(reason))) => self.phase = Phase::Failed(reason),
            Reply::Asked(Ok(AskAnswer::Refused(gate))) => {
                self.gate = Some(Ok(gate));
                self.phase = Phase::Idle;
            }
            Reply::Asked(Err(reason)) => {
                self.notice = Some(reason);
                self.phase = Phase::Idle;
            }
        }
    }

    /// Draws the screen with the assistant (call only when [`AiAsk::ready`]).
    pub(crate) fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        crate::ui::page_head::page_head(
            ui,
            &lit(TITLE),
            Some(&lit(
                "The answer is always a report the engine runs. The assistant writes the query, never the figures.",
            )),
            |_| {},
        );
        let typing = !matches!(self.phase, Phase::Asking(_));
        let (submitted, box_focused) = self.question_box(ui, typing);
        if submitted && let Some(worker) = ctx.api.as_ref() {
            self.start_ask(worker);
        }
        ui.add_space(14.0);
        let mut action = None;
        let mut related = None;
        let mut tried = None;
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::relative(super::quick_entry::LEFT_SHARE))
            .size(egui_extras::Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| (action, related) = self.left(ui, !box_focused));
                strip.cell(|ui| tried = how_card(ui, typing));
            });
        if let Some(question) = tried
            && let Some(worker) = ctx.api.as_ref()
        {
            self.text = question;
            self.start_ask(worker);
        }
        if let Some(key) = related {
            return self.open(navigate::open_target(
                &PaletteTarget::Report(key.to_owned()),
                ctx,
            ));
        }
        match action {
            Some(PropAction::Edit) => self.open_full_report(),
            Some(PropAction::Reject) => {
                self.phase = Phase::Idle;
                self.text.clear();
                self.focused_once = false;
                self.notice = Some(lit("Answer dismissed. Nothing was stored."));
                ScreenAction::Stay
            }
            Some(PropAction::Accept) | None => ScreenAction::Stay,
        }
    }

    fn open(&mut self, result: Result<Box<dyn crate::Screen>, navigate::NavError>) -> ScreenAction {
        match result {
            Ok(screen) => ScreenAction::Push(screen),
            Err(e) => {
                self.notice = Some(e.to_string());
                ScreenAction::Stay
            }
        }
    }

    /// The full report screen for the answer, with the parameters the engine ran.
    fn open_full_report(&mut self) -> ScreenAction {
        let Phase::Ran(answer) = &self.phase else {
            return ScreenAction::Stay;
        };
        if let Some(screen) =
            ReportViewScreen::worker(ReportSource::worker(answer.report.params.clone()))
        {
            ScreenAction::Push(Box::new(screen))
        } else {
            self.notice = Some(lit("The report screen could not be opened."));
            ScreenAction::Stay
        }
    }

    /// The question box. Returns whether Enter submitted it, and whether it has focus.
    fn question_box(&mut self, ui: &mut Ui, typing: bool) -> (bool, bool) {
        let t = tokens(ui);
        let mut submitted = false;
        let mut focused = false;
        Frame::new()
            .fill(t.surface)
            .stroke(Stroke::new(border_width(kind(ui)), t.border_strong))
            .corner_radius(RADIUS_M)
            .inner_margin(Margin {
                left: 12,
                right: 8,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, Icon::Spark, t.text_2);
                    ui.add_space(8.0);
                    let width = ui.available_width() - 60.0;
                    let response = ui.add_enabled(
                        typing,
                        TextEdit::singleline(&mut self.text)
                            .hint_text(lit("Ask about your books, for example: who owes us money?"))
                            .font(FontId::proportional(TEXT_INPUT))
                            .frame(Frame::NONE)
                            .desired_width(width),
                    );
                    if typing && !self.focused_once {
                        response.request_focus();
                        self.focused_once = true;
                    }
                    submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    focused = response.has_focus();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let _ = keycap(ui, "Enter");
                    });
                });
            });
        (submitted, focused)
    }

    /// The left card for where the screen is. Returns the answer card's action and a related
    /// report clicked.
    fn left(&mut self, ui: &mut Ui, keys_free: bool) -> (Option<PropAction>, Option<&'static str>) {
        if let Some(line) = &self.notice {
            note(ui, NoteKind::Info, line);
            ui.add_space(8.0);
        }
        match &self.phase {
            Phase::Idle => {
                idle_card(ui);
                (None, None)
            }
            Phase::Asking(_) => {
                asking_card(ui);
                (None, None)
            }
            Phase::Ran(answer) => (answer_card(ui, answer, keys_free), None),
            Phase::Unsupported(reason) => (None, unsupported_card(ui, reason)),
            Phase::Failed(reason) => {
                failed_card(ui, reason);
                (None, None)
            }
        }
    }
}

/// The idle card: what to do (mock `ai.ask.idle`).
fn idle_card(ui: &mut Ui) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(lit("Type a question and press"))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
            let _ = keycap(ui, "Enter");
            ui.label(
                RichText::new(lit(". Nothing is sent or run until then."))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        });
    });
}

/// The loading card (mock `ai.ask.loading`).
fn asking_card(ui: &mut Ui) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.horizontal(|ui| {
            icon(ui, Icon::Spark, t.text_2);
            ui.label(
                RichText::new(lit("Turning your question into a query…"))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        });
        ui.add_space(8.0);
        skeleton_rows(ui, 5, ui.available_width());
    });
}

/// "Logged in the AI activity log…" under an answer that ran nothing.
fn logged_line(ui: &mut Ui, outcome: &str) {
    let t = tokens(ui);
    ui.add_space(10.0);
    ui.label(
        RichText::new(format!(
            "{} {outcome}. {}",
            lit("Logged in the AI activity log: the question, the model, and the outcome"),
            lit("Nothing else was stored.")
        ))
        .size(TEXT_META)
        .color(t.text_3),
    );
}

/// The words with their first letter in capitals.
fn sentence(words: &str) -> String {
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The unsupported card (mock `ai.ask.unsupported`). Returns the palette key of a report clicked.
fn unsupported_card(ui: &mut Ui, reason: &str) -> Option<&'static str> {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        note(
            ui,
            NoteKind::Bad,
            &format!(
                "{} {}. {}",
                lit("Unsupported."),
                sentence(reason),
                lit("These reports show what the books hold.")
            ),
        );
        ui.add_space(12.0);
        let mut clicked = None;
        let width = (ui.available_width() - 12.0) / 3.0;
        let height = RELATED
            .iter()
            .map(|(label, _, _)| ai_gate::tile_height(ui, &lit(label), width))
            .fold(ai_gate::TILE_HEIGHT, f32::max);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (label, which, key) in RELATED {
                if ai_gate::launch_tile(ui, which, &lit(label), width, height).clicked() {
                    clicked = Some(key);
                }
            }
        });
        logged_line(ui, &lit("Unsupported"));
        clicked
    })
    .inner
}

/// The failed card (mock `ai.ask.failed`).
fn failed_card(ui: &mut Ui, reason: &str) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        note(
            ui,
            NoteKind::Bad,
            &format!(
                "{} {}. {}",
                lit("Not read."),
                sentence(reason),
                lit("Nothing ran.")
            ),
        );
        logged_line(ui, &lit("Not read"));
    });
}

/// The words for where the provider runs.
fn place_word(place: ProviderPlace) -> String {
    match place {
        ProviderPlace::ThisComputer => lit("Local model"),
        ProviderPlace::Cloud => lit("Your cloud provider"),
        ProviderPlace::Test => lit("Test provider"),
    }
}

/// The report's dates as the chip says them: a balance as at its date, a flow over its period.
fn dates_text(params: &ReportParams) -> String {
    let balance = match &params.kind {
        ReportKind::TrialBalance
        | ReportKind::BalanceSheet
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary => true,
        ReportKind::ProfitAndLoss
        | ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Audit(_) => false,
    };
    let (from, to) = (params.period.from(), params.period.to());
    if balance {
        format!("{} {}", lit("as at"), format::date(params.as_of))
    } else if from == to {
        format::date(to)
    } else {
        format!("{} {} {}", format::date(from), lit("to"), format::date(to))
    }
}

/// The answer card (mock `ai.ask.result`). Returns the action taken.
fn answer_card(ui: &mut Ui, answer: &Answer, keys_free: bool) -> Option<PropAction> {
    let save = lit(SAVE_NOT_BUILT);
    let (accept, edit, reject) = (
        lit("Save as report"),
        lit("Open the full report"),
        lit("Dismiss"),
    );
    Prop::new(&save)
        .accept(false, Some(&save))
        .labels(&accept, &edit, &reject)
        .handles_keys(keys_free)
        .show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lit("Report query")).strong());
                    ui.label(RichText::new(lit("suggested")).size(TEXT_META));
                    ui.add_space(12.0);
                    ui.label(RichText::new(lit("Your question")).size(TEXT_META));
                    ui.label(RichText::new(place_word(answer.place)).size(TEXT_META));
                });
            },
            |ui| answer_body(ui, &answer.report),
            |ui| {
                let t = tokens(ui);
                for line in [
                    "Every figure above came from the report engine.",
                    "The words above hold no figure of their own.",
                ] {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::Check, t.ok);
                        ui.label(RichText::new(lit(line)).size(TEXT_META).color(t.ok));
                    });
                }
            },
        )
}

/// The chips, the sentence and the engine's table.
fn answer_body(ui: &mut Ui, report: &Report) {
    let t = tokens(ui);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(lit("I ran:")).size(TEXT_META).color(t.text_3));
        let _ = chip(ui, &report.title);
        let _ = chip(ui, &dates_text(&report.params));
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(format!(
            "{} {}",
            lit("The table is the engine's result for this query."),
            lit("The assistant wrote no figure.")
        ))
        .size(TEXT_BODY)
        .color(t.text),
    );
    ui.add_space(8.0);
    let shown: Vec<usize> = if report.columns.len() <= MAX_COLUMNS {
        (0..report.columns.len()).collect()
    } else {
        report.columns.len().checked_sub(1).into_iter().collect()
    };
    egui::Grid::new("ask-answer")
        .num_columns(shown.len() + 1)
        .spacing([24.0, 6.0])
        .striped(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new(lit("Particulars"))
                    .size(TEXT_META)
                    .color(t.text_2),
            );
            for &c in &shown {
                if let Some(head) = report.columns.get(c) {
                    ui.label(RichText::new(&head.label).size(TEXT_META).color(t.text_2));
                }
            }
            ui.end_row();
            for row in report.rows.iter().take(MAX_ROWS) {
                ui.horizontal(|ui| {
                    ui.add_space(f32::from(row.depth) * 12.0);
                    ui.label(RichText::new(&row.label).size(TEXT_BODY).color(t.text));
                });
                for &c in &shown {
                    let text = row.cells.get(c).map_or_else(String::new, engine_cell_text);
                    numeric(ui, &text, false);
                }
                ui.end_row();
            }
            if !report.totals.is_empty() {
                ui.label(RichText::new(lit("Total")).font(bold_font(ui, TEXT_BODY)));
                for &c in &shown {
                    let text = report
                        .totals
                        .get(c)
                        .map_or_else(String::new, engine_cell_text);
                    numeric(ui, &text, true);
                }
                ui.end_row();
            }
        });
    let more_rows = report.rows.len().saturating_sub(MAX_ROWS);
    let more_columns = report.columns.len().saturating_sub(shown.len());
    for (count, words) in [
        (more_columns, lit("more columns are in the full report.")),
        (more_rows, lit("more rows are in the full report.")),
    ] {
        if count > 0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{count} {words}"))
                    .size(TEXT_META)
                    .color(t.text_3),
            );
        }
    }
}

/// A figure, right-aligned in its cell.
fn numeric(ui: &mut Ui, text: &str, bold: bool) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let rich = if bold {
            RichText::new(text).font(bold_font(ui, TEXT_BODY))
        } else {
            RichText::new(text).size(TEXT_BODY)
        };
        ui.label(rich);
    });
}

/// "How this works" with the Try questions (mock `askAiRender`). Returns a question clicked.
fn how_card(ui: &mut Ui, typing: bool) -> Option<String> {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.label(
            RichText::new(lit("How this works"))
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(8.0);
        for (n, line) in [
            "Your model turns your words into a ReportQuery: a report, the dates you said, and the ledger you named.",
            "The engine checks it and runs it, like any report.",
            "The words here hold no figure. Every figure is in the engine's table.",
            "Every question goes into the AI activity log.",
        ]
        .iter()
        .enumerate()
        {
            ui.label(
                RichText::new(format!("{}. {}", n + 1, lit(line)))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        }
        ui.add_space(10.0);
        ui.separator();
        ui.label(
            RichText::new(lit("Try"))
                .font(bold_font(ui, TEXT_BODY))
                .color(t.text),
        );
        ui.add_space(4.0);
        if !typing {
            ui.disable();
        }
        let mut clicked = None;
        for question in TRY {
            let words = lit(question);
            if button(ui, &words, None, ButtonKind::Secondary).clicked() {
                clicked = Some(words);
            }
        }
        clicked
    })
    .inner
}
