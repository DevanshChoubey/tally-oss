//! Round s21: a bill read by the OCR reader needs review field by field (mock state
//! `ai.purchase-bill.needs-review`; coordinator decision 2026-11-27: ocrs gives no confidence, so
//! each value it reads needs review and the person confirms each before the draft can post).
//!
//! Provenance (derived). The stand-in bill and its recorded lines are `s21_bill_photo`'s, each at
//! the needs-review confidence an ocrs reading carries. The draft reads the supplier, bill number,
//! date, the taxable line's amount, the party line's ledger and amount, and the printed total: 7
//! values that need review. The taxable line names no ledger, so its ledger is the person's
//! choice, made in its "Choose a ledger" select and not counted as a value to confirm (the mock
//! counts 9 for its bill and leaves the open select out). The words are the mock's.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{
    AiTier, Api, ApiError, Command, CommandResult, DraftField, FixedAnswers, OcrLine, OcrOutput,
};
use ledgerly_app::screens::purchase_bill::PurchaseBillScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use sha2::{Digest, Sha256};
use time::macros::{date, datetime};

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

/// A stand-in bill photo: a small white PNG written under the test's temporary directory.
fn bill_png(name: &str) -> (PathBuf, Vec<u8>) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let img = image::RgbImage::from_pixel(56, 76, image::Rgb([250, 247, 240]));
    img.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    (path, bytes)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

fn line(text: &str, top: u32) -> OcrLine {
    OcrLine {
        page: 1,
        text: text.to_owned(),
        bbox: [3, top, 40, 2],
        confidence_permille: ledgerly_api::NEEDS_REVIEW_PERMILLE,
    }
}

/// The OCR lines recorded for the stand-in bill.
fn recorded(bytes: &[u8]) -> OcrOutput {
    OcrOutput {
        doc_hash: hex(bytes),
        lines: vec![
            line("Supplier: Rupa Polymers", 10),
            line("Invoice No: RP/5521", 16),
            line("Date: 17/11/2026", 20),
            line("Taxable Value 68,000.00", 50),
            line("Total 68,000.00", 60),
        ],
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
    committed(api.execute(Command::CreateLedger(ledger(
        4,
        "Purchases",
        ReservedGroup::PurchaseAccounts,
    ))));
    let mut supplier = ledger(5, "Rupa Polymers", ReservedGroup::SundryCreditors);
    supplier.party = Some(ledgerly_model::PartyDetails {
        registration: ledgerly_model::RegistrationType::Unregistered,
        gstin: None,
        pan: None,
        state: Some(StateCode::new(29).unwrap()),
        address: Address::default(),
    });
    committed(api.execute(Command::CreateLedger(supplier)));
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
    screen: PurchaseBillScreen,
    app: AppCtx,
}

impl Harness {
    fn new(api: Api) -> Self {
        ledgerly_app::release::set_ai_enabled(true);
        ledgerly_app::shell::navigate::set_clock_override(date!(2026 - 11 - 21), 15);
        Self {
            ctx: egui::Context::default(),
            screen: PurchaseBillScreen::default(),
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

    /// One frame with `events`; returns the labels of the widgets that gained keyboard focus.
    fn focus_frame(&mut self, events: Vec<Event>) -> Vec<String> {
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
        let gained = out
            .platform_output
            .events
            .iter()
            .filter_map(|e| match e {
                egui::output::OutputEvent::FocusGained(info) => info.label.clone(),
                egui::output::OutputEvent::Clicked(_)
                | egui::output::OutputEvent::DoubleClicked(_)
                | egui::output::OutputEvent::TripleClicked(_)
                | egui::output::OutputEvent::TextSelectionChanged(_)
                | egui::output::OutputEvent::ValueChanged(_) => None,
            })
            .collect();
        out.drop_without_applying_deltas();
        gained
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

    /// Types the path into the file field and reads it with Enter.
    fn read_file(&mut self, path: &Path) {
        let _ = self.settle_on("Read a bill");
        let _ = self.frame(vec![Event::Text(path.display().to_string())]);
        self.key(Key::Enter, Modifiers::NONE);
    }
}

#[test]
fn every_value_read_by_the_ocr_reader_shows_needs_review_and_blocks_the_accept() {
    let (path, bytes) = bill_png("s21-review-read.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-review-read.ledgerly", Some(provider)));
    h.read_file(&path);
    let text = h.settle_on("Draft purchase");
    assert!(text.contains("needs review"), "{text}");
    assert!(text.contains("Confirm"), "{text}");
    assert!(
        text.contains("7 values read from the bill need review: confirm each against the photo."),
        "{text}"
    );
    h.screen.choose_ledger(0, LedgerId(4));
    let text = h.settle_on("7 values read from the bill need review");
    assert!(!text.contains("Dr equals Cr (engine)."), "{text}");
    h.key(Key::A, Modifiers::CTRL);
    let text = h.frame(Vec::new());
    assert!(!text.contains("posted"), "{text}");
}

#[test]
fn confirming_each_value_lets_the_engine_balance_and_accept_logs_the_confirmations() {
    let (path, bytes) = bill_png("s21-review-ok.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-review-ok.ledgerly", Some(provider)));
    h.read_file(&path);
    let _ = h.settle_on("Choose a ledger");
    h.screen.choose_ledger(0, LedgerId(4));
    let _ = h.settle_on("7 values read from the bill need review");
    for field in [
        DraftField::Date,
        DraftField::PartyName,
        DraftField::InvoiceNumber,
        DraftField::LineAmount(0),
        DraftField::LineLedger(1),
        DraftField::LineAmount(1),
    ] {
        h.screen.confirm_field(field);
    }
    let text = h.settle_on("1 value read from the bill needs review");
    assert!(text.contains("you confirmed"), "{text}");
    h.screen.confirm_field(DraftField::DocumentTotal);
    let text = h.settle_on("Dr equals Cr (engine).");
    assert!(!text.contains("need review"), "{text}");
    h.key(Key::A, Modifiers::CTRL);
    let _ = h.settle_on("posted");
    let mut ticket = h.app.api.as_ref().unwrap().run(|api: &mut Api| {
        api.ai_activity()
            .unwrap()
            .into_iter()
            .find_map(|e| {
                if let ledgerly_api::AiActivityKind::DraftAccepted {
                    human_edits_json, ..
                } = e.activity.kind
                {
                    human_edits_json
                } else {
                    None
                }
            })
            .unwrap_or_default()
    });
    let logged = loop {
        if let ledgerly_app::worker::Poll::Ready(logged) = ticket.poll() {
            break logged;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(logged.contains("confirmed"), "{logged}");
}

/// A bill that prints the supplier's GSTIN and its total tax: both are read and need review, so
/// both are drawn with a Confirm button. Every value the count names can be confirmed on the
/// screen; none is waiting where the person cannot reach it. GSTIN 29ABCDE1234F1ZW is the a3
/// test vector; the tax line is drawn as 0.00 for a bill with no tax.
#[test]
fn a_read_gstin_and_printed_tax_are_shown_with_a_confirm_each() {
    let (path, bytes) = bill_png("s21-review-gstin.png");
    let mut ocr = recorded(&bytes);
    ocr.lines.push(line("GSTIN 29ABCDE1234F1ZW", 30));
    ocr.lines.push(line("Total Tax 0.00", 56));
    let provider = FixedAnswers::new().with_document(ocr);
    let mut h = Harness::new(api("s21-pb-review-gstin.ledgerly", Some(provider)));
    h.read_file(&path);
    let text = h.settle_on("Draft purchase");
    assert!(text.contains("Supplier GSTIN"), "{text}");
    assert!(text.contains("29ABCDE1234F1ZW"), "{text}");
    assert!(text.contains("Printed tax"), "{text}");
    assert!(
        text.contains("9 values read from the bill need review: confirm each against the photo."),
        "{text}"
    );
    assert_eq!(text.matches("\u{2713} Confirm").count(), 9, "{text}");
}

/// The mock's note under the lines (`billNeedsReview`), and its keyboard path: Tab reaches a
/// Confirm button and Space confirms the value.
#[test]
fn tab_reaches_a_confirm_button_and_space_confirms_the_value() {
    let (path, bytes) = bill_png("s21-review-keys.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-review-keys.ledgerly", Some(provider)));
    h.read_file(&path);
    let text = h.settle_on("7 values read from the bill need review");
    assert!(
        text.contains("The OCR reader gives no confidence, so nothing it read is kept until you confirm it against the photo. Tab to a Confirm button; Space confirms."),
        "{text}"
    );
    let tab = Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut reached = false;
    for _ in 0..40 {
        let gained = h.focus_frame(vec![tab.clone()]);
        if gained.iter().any(|l| l.contains("Confirm")) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab never reached a Confirm button");
    h.key(Key::Space, Modifiers::NONE);
    let text = h.settle_on("6 values read from the bill need review");
    assert!(text.contains("you confirmed"), "{text}");
}
