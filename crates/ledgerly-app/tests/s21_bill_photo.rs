//! Round s21, plan round D: a bill photo to a draft purchase on the Purchase from bill photo
//! screen (mock states `purchase-bill.choose`, `.reading`, `.ledger-lines`, `.rejected`,
//! `.error` and `.accepted`, `design/mock-v2/index.html` with `ai=on`).
//!
//! Provenance (derived). The bill is a stand-in PNG written here; the fixed-answer provider holds
//! the OCR lines recorded for it (keyed by the SHA-256 of its bytes), and the real bill reader
//! (`ledgerly_assist::ocr::read_bill`) drafts them: "Taxable Value 68,000.00" is Rs 68,000.00 on a
//! line with no ledger (the bill names no purchase ledger) and "Total 68,000.00" is the printed
//! total. With Purchases chosen for that line the engine's voucher is Purchases Dr 68,000.00,
//! Rupa Polymers Cr 68,000.00. The company has no GST registration, so no tax is computed.

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
    AiTier, Api, ApiError, Command, CommandResult, FixedAnswers, OcrLine, OcrOutput,
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
        group: group(kind),
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
        confidence_permille: 940,
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

    /// Types the path into the file field and reads it with Enter.
    fn read_file(&mut self, path: &Path) {
        let _ = self.settle_on("Read a bill");
        let _ = self.frame(vec![Event::Text(path.display().to_string())]);
        self.key(Key::Enter, Modifiers::NONE);
    }
}

#[test]
fn a_png_bill_is_read_into_a_draft_whose_open_line_asks_for_a_ledger() {
    let (path, bytes) = bill_png("s21-bill-read.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-read.ledgerly", Some(provider)));
    h.read_file(&path);
    let text = h.settle_on("Draft purchase");
    assert!(text.contains("Rupa Polymers"), "{text}");
    assert!(text.contains("RP/5521"), "{text}");
    assert!(text.contains("68,000.00"), "{text}");
    assert!(text.contains("Choose a ledger"), "{text}");
    assert!(text.contains("No ledger matches 'Taxable Value'"), "{text}");
    // No engine figure while the engine has made no voucher.
    assert!(
        text.contains("Shown when the engine makes the voucher"),
        "{text}"
    );
    // The log names the file, not its folder.
    let rows = h.log_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].input_label, "s21-bill-read.png");
}

#[test]
fn choosing_the_purchase_ledger_lets_the_engine_balance_and_accept_post_it() {
    let (path, bytes) = bill_png("s21-bill-accept.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-accept.ledgerly", Some(provider)));
    h.read_file(&path);
    let _ = h.settle_on("Choose a ledger");
    h.screen.choose_ledger(0, LedgerId(4));
    let text = h.settle_on("Dr equals Cr (engine).");
    assert!(text.contains("Purchases"), "{text}");
    assert!(text.contains("Engine total"), "{text}");
    h.key(Key::A, Modifiers::CTRL);
    let text = h.settle_on("posted");
    assert!(text.contains("Accepted"), "{text}");
}

#[test]
fn reject_posts_nothing_records_it_and_offers_another_bill() {
    let (path, bytes) = bill_png("s21-bill-reject.png");
    let provider = FixedAnswers::new().with_document(recorded(&bytes));
    let mut h = Harness::new(api("s21-pb-reject.ledgerly", Some(provider)));
    h.read_file(&path);
    let _ = h.settle_on("Draft purchase");
    h.key(Key::D, Modifiers::ALT);
    let text = h.settle_on("Draft rejected. Nothing was posted.");
    assert!(text.contains("Read another"), "{text}");
    let rows = h.log_rows();
    assert!(
        rows.iter()
            .any(|r| matches!(r.decision, ledgerly_api::AiDecision::Rejected(None))),
        "{rows:?}"
    );
}

#[test]
fn a_missing_file_says_what_failed_and_reads_nothing() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-no-such-bill.png");
    let _removed = std::fs::remove_file(&path);
    let mut h = Harness::new(api("s21-pb-missing.ledgerly", Some(FixedAnswers::new())));
    h.read_file(&path);
    let text = h.settle_on("Could not read s21-no-such-bill.png");
    assert!(text.contains("Choose another file"), "{text}");
    assert_eq!(h.log_rows().len(), 0);
}

#[test]
fn a_pdf_is_refused_as_a_file_type_this_build_does_not_read() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-bill.pdf");
    std::fs::write(&path, b"%PDF-1.7 fictional").unwrap();
    let mut h = Harness::new(api("s21-pb-pdf.ledgerly", Some(FixedAnswers::new())));
    h.read_file(&path);
    let text = h.settle_on("Could not read s21-bill.pdf");
    assert!(text.contains("PNG"), "{text}");
    assert_eq!(h.log_rows().len(), 0);
}
