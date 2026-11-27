//! Round s21, plan round C: plain-words entry with the assistant (mock states
//! `ai.quick-entry.proposal`, `.unknown-ledger`, `.rejected`, `.not-installed`,
//! `design/mock-v2/index.html` with `ai=on`).
//!
//! Provenance (derived). The sentences and the recorded model answers are written here, read by
//! the real plain-words reader behind the fixed-answer provider. "25k" is Rs 25,000.00 and
//! "5000" is Rs 5,000.00 (`nl_entry::parse_amount_text`'s documented rule). The engine balances a
//! payment as Dr payee, Cr account. The date offered is the capture clock's today, 21 Nov 2026,
//! as for any new voucher. The words are the mock's.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{AiTier, Api, ApiError, Command, CommandResult, FixedAnswers};
use ledgerly_app::screens::quick_entry::QuickEntryScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use time::macros::{date, datetime};

const RENT: &str = "paid rent 25k to Sharma Properties from Cash";
const RENT_JSON: &str = "{\"kind\":\"payment\",\"amount_text\":\"25k\",\"date_text\":null,\"party_name\":\"rent\",\"account_name\":\"Cash\",\"narration\":\"rent Sharma Properties\",\"confidence_permille\":900}";
const RAMESH: &str = "Ramesh ko 5000 cash diya";
const RAMESH_JSON: &str = "{\"kind\":\"payment\",\"amount_text\":\"5000\",\"date_text\":null,\"party_name\":\"Ramesh\",\"account_name\":\"cash\",\"narration\":\"Ramesh\",\"confidence_permille\":800}";

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: u64, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group(kind)),
        opening: SignedAmount::dr(Paise::new(100_000_000)),
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

fn committed(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn api(name: &str, provider: Option<FixedAnswers>) -> Api {
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
        ledger(1, "Cash", ReservedGroup::CashInHand),
        ledger(3, "Rent", ReservedGroup::IndirectExpenses),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    api.set_ai_tier(AiTier::Local).unwrap();
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
    screen: QuickEntryScreen,
    app: AppCtx,
}

impl Harness {
    fn new(api: Api) -> Self {
        ledgerly_app::release::set_ai_enabled(true);
        ledgerly_app::shell::navigate::set_clock_override(date!(2026 - 11 - 21), 15);
        Self {
            ctx: egui::Context::default(),
            screen: QuickEntryScreen::default(),
            app: AppCtx {
                api: Some(ApiWorker::spawn(api).unwrap()),
                ..AppCtx::default()
            },
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> String {
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
                let _stay = self.screen.ui(ui, &mut self.app);
            },
        );
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

    /// The AI activity log rows, read through the screen's own worker.
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

    fn type_sentence(&mut self, sentence: &str) {
        let _ = self.settle_on("Type an entry, as you would say it");
        let _ = self.frame(vec![Event::Text(sentence.to_owned())]);
        self.key(Key::Enter, Modifiers::NONE);
    }
}

#[test]
fn with_no_model_to_run_the_box_is_off_and_the_voucher_forms_are_offered() {
    let mut h = Harness::new(api("s21-qe-none.ledgerly", None));
    let text = h.settle_on("No local model can run.");
    assert!(text.contains("Type it myself"), "{text}");
}

#[test]
fn a_sentence_becomes_a_draft_the_engine_balances_and_accept_posts_it() {
    let provider = FixedAnswers::new().with_text(RENT, RENT_JSON);
    let mut h = Harness::new(api("s21-qe-rent.ledgerly", Some(provider)));
    h.type_sentence(RENT);
    let text = h.settle_on("Draft payment");
    assert!(text.contains("Rent"), "{text}");
    assert!(text.contains("25,000.00"), "{text}");
    assert!(text.contains("Dr equals Cr (engine)."), "{text}");
    assert!(text.contains("21 Nov 2026"), "{text}");
    assert!(text.contains("rent Sharma Properties"), "{text}");
    h.key(Key::A, Modifiers::CTRL);
    let text = h.settle_on("posted");
    assert!(text.contains("Accepted"), "{text}");
}

#[test]
fn reject_posts_nothing_and_says_so() {
    let provider = FixedAnswers::new().with_text(RENT, RENT_JSON);
    let mut h = Harness::new(api("s21-qe-reject.ledgerly", Some(provider)));
    h.type_sentence(RENT);
    let _ = h.settle_on("Draft payment");
    h.key(Key::D, Modifiers::ALT);
    let text = h.settle_on("Nothing was posted.");
    assert!(text.contains("Rejected"), "{text}");
    // The rejection is recorded in the company's AI activity log, not only drawn.
    let rows = h.log_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(
        matches!(rows[0].decision, ledgerly_api::AiDecision::Rejected(None)),
        "{:?}",
        rows[0]
    );
}

#[test]
fn an_unknown_name_blocks_the_draft_and_no_ledger_is_invented() {
    let provider = FixedAnswers::new().with_text(RAMESH, RAMESH_JSON);
    let mut h = Harness::new(api("s21-qe-ramesh.ledgerly", Some(provider)));
    h.type_sentence(RAMESH);
    let text = h.settle_on("No party or ledger named \"Ramesh\".");
    assert!(text.contains("5,000.00"), "{text}");
    // Accept is blocked: Ctrl+A posts nothing.
    h.key(Key::A, Modifiers::CTRL);
    let text = h.frame(Vec::new());
    assert!(!text.contains("posted"), "{text}");
}

#[test]
fn the_draft_shows_the_account_balance_now_and_after_from_the_engine() {
    // provenance: index.html:1745 ("Saptagiri now" and "After (engine)"). derived: Cash opens at
    // Rs 10,00,000.00 Dr (100,000,000 paise, `ledger` above); the payment of Rs 25,000.00 leaves
    // Rs 9,75,000.00 Dr. Both figures come from the trial balance (`Api::balance_change`).
    let provider = FixedAnswers::new().with_text(RENT, RENT_JSON);
    let mut h = Harness::new(api("s21-qe-balance.ledgerly", Some(provider)));
    h.type_sentence(RENT);
    let text = h.settle_on("Draft payment");
    assert!(text.contains("Cash now"), "{text}");
    assert!(text.contains("10,00,000.00"), "{text}");
    assert!(text.contains("After"), "{text}");
    assert!(text.contains("9,75,000.00"), "{text}");
}

#[test]
fn the_draft_says_its_date_is_after_the_lock_date() {
    // provenance: index.html:1749 ("21 Nov 2026 is after the lock date."). derived: the books are
    // locked to 31 Oct 2026 here and the date offered is 21 Nov 2026.
    let provider = FixedAnswers::new().with_text(RENT, RENT_JSON);
    let mut books = api("s21-qe-lock.ledgerly", Some(provider));
    committed(books.execute(Command::LockPeriod(date!(2026 - 10 - 31))));
    let mut h = Harness::new(books);
    h.type_sentence(RENT);
    let text = h.settle_on("Draft payment");
    assert!(
        text.contains("21 Nov 2026 is after the lock date."),
        "{text}"
    );
}

#[test]
fn try_offers_sentences_the_reader_can_draft() {
    // provenance: index.html:1750 with ai=on: "paid rent 25k to Sharma Properties from Saptagiri"
    // and "Ramesh ko 5000 cash diya" under "Try".
    let mut h = Harness::new(api("s21-qe-try.ledgerly", Some(FixedAnswers::new())));
    let text = h.settle_on("Type an entry, as you would say it");
    assert!(text.contains("Try"), "{text}");
    assert!(
        text.contains("paid rent 25k to Sharma Properties from Saptagiri"),
        "{text}"
    );
    assert!(text.contains("Ramesh ko 5000 cash diya"), "{text}");
}
