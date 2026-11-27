//! Round s21, plan round E: Ask your books with the assistant (mock states `ai.ask.idle`,
//! `.result`, `.unsupported` and `.failed`, `design/mock-v2/index.html` with `ai=on`).
//!
//! Provenance (derived). The questions and the recorded queries are written here. The company's
//! year starts 1 April 2026 and has no voucher, so a report opens for the whole year to
//! 31 March 2027 (`ledgerly_api::default_report_period`). Cash opens at Rs 10,00,000.00 Dr
//! (100,000,000 paise, `ledger` below) and no other ledger has an opening, so the condensed trial
//! balance shows Current Assets (Cash's primary group) at 10,00,000.00 Dr and the unmatched opening
//! as a difference line of 10,00,000.00 Cr. Bills receivable has 11 columns
//! (`ledgerly-reports` outstanding `COLUMN_LABELS`); the card shows the last, "Net outstanding",
//! and says the other 10 are in the full report. The words are the mock's.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{AiOutput, AiTier, Api, Command, FixedAnswers, ReportQuery};
use ledgerly_app::screens::ask::AskScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use time::macros::{date, datetime};

const TB: &str = "show me the trial balance";
const OWED: &str = "who owes us money?";
const FORECAST: &str = "what will my profit be next quarter?";
const LAST_WEEK: &str = "day book for last week";

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: u64, name: &str, kind: ReservedGroup, opening: i64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(Paise::new(opening)),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn query(report: &str) -> ReportQuery {
    ReportQuery {
        report: report.to_owned(),
        from: None,
        to: None,
        ledger_names: Vec::new(),
    }
}

fn provider() -> FixedAnswers {
    let mut moved = query("day_book");
    // A date the question does not say: the check refuses it.
    moved.from = Some(date!(2026 - 11 - 14));
    FixedAnswers::new()
        .with_question(TB, query("trial_balance"))
        .with_question(OWED, query("receivables"))
        .with_question(FORECAST, query("unsupported"))
        .with_question(LAST_WEEK, moved)
}

fn api(name: &str, tier: AiTier, provider: Option<FixedAnswers>) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut api = Api::create(
        &path,
        &company,
        Clock::Fixed(datetime!(2026-11-21 10:00 UTC)),
    )
    .unwrap();
    for l in [
        ledger(1, "Cash", ReservedGroup::CashInHand, 100_000_000),
        ledger(3, "Rent", ReservedGroup::IndirectExpenses, 0),
    ] {
        match api.execute(Command::CreateLedger(l)) {
            Ok(Outcome::Value(_)) => {}
            other => panic!("expected a committed ledger, got {other:?}"),
        }
    }
    api.set_ai_tier(tier).unwrap();
    if let Some(provider) = provider {
        api.attach_assistant(Box::new(provider));
    }
    api
}

fn drawn_text(shapes: &[ClippedShape]) -> String {
    fn walk(shape: &Shape, out: &mut String) {
        if let Shape::Text(text) = shape {
            out.push_str(text.galley.text());
            out.push('\n');
        } else if let Shape::Vec(shapes) = shape {
            for shape in shapes {
                walk(shape, out);
            }
        }
    }
    let mut out = String::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

struct Harness {
    ctx: egui::Context,
    screen: AskScreen,
    app: AppCtx,
    pushed: bool,
}

impl Harness {
    fn new(api: Api) -> Self {
        ledgerly_app::release::set_ai_enabled(true);
        ledgerly_app::shell::navigate::set_clock_override(date!(2026 - 11 - 21), 15);
        let app = AppCtx {
            api: Some(ApiWorker::spawn(api).unwrap()),
            ..AppCtx::default()
        };
        Self {
            ctx: egui::Context::default(),
            screen: AskScreen::default(),
            app,
            pushed: false,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> String {
        let mut pushed = false;
        let out = self.ctx.run_ui(
            RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 1000.0),
                )),
                ..RawInput::default()
            },
            |ui| {
                pushed = matches!(self.screen.ui(ui, &mut self.app), ScreenAction::Push(_));
            },
        );
        self.pushed |= pushed;
        let text = drawn_text(&out.shapes);
        out.drop_without_applying_deltas();
        text
    }

    fn settle_on(&mut self, wanted: &str) -> String {
        let mut text = self.frame(Vec::new());
        for _ in 0..2000 {
            if text.contains(wanted) {
                return text;
            }
            std::thread::sleep(Duration::from_millis(2));
            text = self.frame(Vec::new());
        }
        panic!("never drew {wanted:?}; last frame:\n{text}");
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let _ = self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
    }

    fn ask(&mut self, question: &str) {
        let _ = self.settle_on("Type a question and press");
        let _ = self.frame(vec![Event::Text(question.to_owned())]);
        self.key(Key::Enter, Modifiers::NONE);
    }

    fn log_rows(&self) -> Vec<ledgerly_api::AiLogRow> {
        let mut ticket = self
            .app
            .api
            .as_ref()
            .unwrap()
            .run(|api: &mut Api| api.ai_log_rows().unwrap());
        for _ in 0..2000 {
            if let ledgerly_app::worker::Poll::Ready(rows) = ticket.poll() {
                return rows;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("the worker never answered");
    }
}

#[test]
fn ready_the_box_waits_and_try_offers_questions() {
    // provenance: the mock's `ai.ask.idle` (index.html, askAiRender).
    let mut h = Harness::new(api(
        "s21-ask-idle.ledgerly",
        AiTier::Local,
        Some(provider()),
    ));
    let text = h.settle_on("Type a question and press");
    assert!(text.contains("How this works"), "{text}");
    assert!(text.contains("Try"), "{text}");
    assert!(text.contains(OWED), "{text}");
    assert!(text.contains(FORECAST), "{text}");
    assert!(!text.contains("I ran:"), "{text}");
}

#[test]
fn a_question_shows_the_engines_report_and_the_query_from_its_parameters() {
    let mut h = Harness::new(api("s21-ask-tb.ledgerly", AiTier::Local, Some(provider())));
    h.ask(TB);
    let text = h.settle_on("I ran:");
    assert!(text.contains("Trial Balance"), "{text}");
    assert!(text.contains("as at 31 Mar 2027"), "{text}");
    assert!(text.contains("Debit"), "{text}");
    assert!(text.contains("Credit"), "{text}");
    // Condensed, as every report but a group's or a ledger's opens: Cash sits in its primary
    // group, and the unmatched opening is the explicit difference line (spec invariant 4).
    assert!(text.contains("Current Assets"), "{text}");
    assert!(text.contains("10,00,000.00 Dr"), "{text}");
    assert!(text.contains("Difference in opening balances"), "{text}");
    assert!(
        text.contains("The table is the engine's result for this query."),
        "{text}"
    );
    assert!(
        text.contains("Every figure above came from the report engine."),
        "{text}"
    );
    let rows = h.log_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].output, AiOutput::Answered);
}

#[test]
fn a_wide_report_shows_its_last_column_and_says_how_many_more_the_full_report_has() {
    let mut h = Harness::new(api(
        "s21-ask-owed.ledgerly",
        AiTier::Local,
        Some(provider()),
    ));
    h.ask(OWED);
    let text = h.settle_on("I ran:");
    assert!(text.contains("Bills receivable"), "{text}");
    assert!(text.contains("Net outstanding"), "{text}");
    assert!(
        text.contains("10 more columns are in the full report."),
        "{text}"
    );
}

#[test]
fn enter_opens_the_full_report_and_alt_d_dismisses_the_answer() {
    let mut h = Harness::new(api(
        "s21-ask-keys.ledgerly",
        AiTier::Local,
        Some(provider()),
    ));
    h.ask(TB);
    let _ = h.settle_on("I ran:");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.pushed, "Enter opens the full report");
    h.key(Key::D, Modifiers::ALT);
    let text = h.settle_on("Answer dismissed. Nothing was stored.");
    assert!(text.contains("Type a question and press"), "{text}");
    assert!(!text.contains("I ran:"), "{text}");
}

#[test]
fn a_forecast_is_unsupported_and_offers_reports() {
    let mut h = Harness::new(api(
        "s21-ask-forecast.ledgerly",
        AiTier::Local,
        Some(provider()),
    ));
    h.ask(FORECAST);
    let text = h.settle_on("Unsupported.");
    assert!(
        text.contains("the assistant does not estimate figures"),
        "{text}"
    );
    for report in ["Profit and loss", "Balance sheet", "Bills receivable"] {
        assert!(text.contains(report), "{report}: {text}");
    }
    assert!(text.contains("Logged in the AI activity log"), "{text}");
}

#[test]
fn a_query_with_a_date_the_question_does_not_say_is_not_read() {
    let mut h = Harness::new(api(
        "s21-ask-moved.ledgerly",
        AiTier::Local,
        Some(provider()),
    ));
    h.ask(LAST_WEEK);
    let text = h.settle_on("Not read.");
    assert!(
        text.contains(
            "the report query states a date or a ledger name that the question does not say"
        ),
        "{text}"
    );
    assert!(text.contains("Nothing ran."), "{text}");
}

#[test]
fn with_ai_off_for_the_company_the_report_list_stays() {
    let mut h = Harness::new(api("s21-ask-off.ledgerly", AiTier::Off, Some(provider())));
    let text = h.settle_on("Reports");
    assert!(text.contains("Outstanding receivables"), "{text}");
    assert!(!text.contains("Type a question and press"), "{text}");
}
