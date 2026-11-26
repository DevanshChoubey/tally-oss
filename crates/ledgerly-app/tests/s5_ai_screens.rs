//! Ask your books, Plain-words entry and the bill photo over a real company file (spec 6.8).
//!
//! No model or bill reader exists in this build, so each screen shows why it cannot do its job
//! for the company's stored AI tier, never a reading, a draft or a figure.
//!
//! Provenance. Mock wording is quoted from `design/mock-v2/index.html` with its line. Wording the
//! mock has no state for (a local tier with no model, no tier chosen, the cloud tier) is
//! `derived`: written out here from the facts in the module header of `screens/ai_gate.rs`
//! (no model client, no OCR engine, the cloud tier refused by `Api::set_ai_tier`). The tier is
//! stored by the test itself, so which wording applies is known before the screen runs. No
//! expected value is an output of the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{AiTier, Api};
use ledgerly_app::screens::ai_gate::{Feature, Gate, reason};
use ledgerly_app::screens::ask::{self, AskReport, AskScreen};
use ledgerly_app::screens::purchase_bill::{self, PurchaseBillScreen};
use ledgerly_app::screens::quick_entry::{self, QuickEntryScreen};
use ledgerly_app::shell::KeyHint;
use ledgerly_app::shell::navigate::NavError;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, CompanyId, Outcome, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
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
    }
}

/// A new company file with `tier` stored, or none when `tier` is `None`.
fn file_with_tier(name: &str, tier: Option<AiTier>) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    if let Some(tier) = tier {
        assert_eq!(api.set_ai_tier(tier).unwrap(), Outcome::Value(()));
    }
    drop(api);
    path
}

/// The app context with the company file open on a worker and its books loaded.
fn open(path: &Path) -> AppCtx {
    let api = Api::open(path).unwrap();
    let books = match api.books().unwrap() {
        Outcome::Value(books) => books,
        Outcome::Unsupported(u) => panic!("books not readable: {u:?}"),
    };
    AppCtx {
        books: Some(books),
        api: Some(ledgerly_app::worker::ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    }
}

/// Every piece of text the frame drew, joined.
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

/// One frame with `keys` pressed. Returns what the frame drew and what the screen asked for.
fn frame(
    screen: &mut dyn Screen,
    app: &mut AppCtx,
    keys: &[(Key, Modifiers)],
) -> (String, ScreenAction) {
    let ctx = egui::Context::default();
    let events = keys
        .iter()
        .map(|&(key, modifiers)| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .collect();
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    (text, action)
}

/// Frames until the screen has read the tier. Returns the last frame's text.
fn settle<S: Screen>(screen: &mut S, app: &mut AppCtx, loaded: impl Fn(&S) -> bool) -> String {
    let mut text = frame(screen, app, &[]).0;
    for _ in 0..2000 {
        if loaded(screen) {
            // One more frame draws what the answer says.
            return frame(screen, app, &[]).0;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]).0;
    }
    panic!("the worker's answer did not arrive: {text}");
}

const ASK_OFF: &str = "AI is off for this company, so questions cannot be turned into queries. These reports answer most questions directly.";
const BILL_OFF: &str =
    "Bill reading is off for this company. Type the bill in the purchase form instead.";

#[test]
fn the_reason_follows_the_tier_for_each_feature() {
    // provenance: index.html:1169 (Ask, AI off): "AI is off for this company, so questions
    // cannot be turned into queries. These reports answer most questions directly."
    assert_eq!(reason(Feature::Ask, &Gate::Off).as_deref(), Some(ASK_OFF));
    // provenance: index.html:1430 (bill photo, AI off): "Bill reading is off for this company."
    // The mock's next sentence ("Type the bill below; the image stays attached to the voucher")
    // promises an embedded form and an attachment store this build does not have, so the second
    // sentence is derived: the purchase form is where a bill is typed.
    assert_eq!(
        reason(Feature::BillReading, &Gate::Off).as_deref(),
        Some(BILL_OFF)
    );
    // provenance: derived. The mock has no AI-off state for plain-words entry; the AI-off note
    // (index.html:1911) says plain-words entry is hidden, and payment and receipt are the voucher
    // kinds `nl_entry` drafts.
    assert_eq!(
        reason(Feature::PlainWords, &Gate::Off).as_deref(),
        Some(
            "AI is off for this company, so a sentence cannot be drafted into a voucher. Type the voucher yourself in the payment or receipt form."
        )
    );
    // provenance: derived. Local tier, no model client exists.
    assert_eq!(
        reason(Feature::Ask, &Gate::Local).as_deref(),
        Some(
            "No local model is installed on this computer, so questions cannot be turned into queries. These reports answer most questions directly."
        )
    );
    assert_eq!(
        reason(Feature::PlainWords, &Gate::Local).as_deref(),
        Some(
            "No local model is installed on this computer, so a sentence cannot be drafted into a voucher. Type the voucher yourself in the payment or receipt form."
        )
    );
    // provenance: derived. Local tier, no OCR engine produces the bill reader's input.
    assert_eq!(
        reason(Feature::BillReading, &Gate::Local).as_deref(),
        Some(
            "No bill reader is installed on this computer, so a bill photo cannot be read. Type the bill in the purchase form instead."
        )
    );
    // provenance: derived. No tier stored.
    assert_eq!(
        reason(Feature::Ask, &Gate::Unset).as_deref(),
        Some(
            "No AI tier is chosen for this company, so questions cannot be turned into queries. These reports answer most questions directly."
        )
    );
    // provenance: derived. Cloud tier stored; `Api::set_ai_tier(Cloud)` is refused for want of a
    // keychain store.
    assert_eq!(
        reason(Feature::BillReading, &Gate::Cloud).as_deref(),
        Some(
            "The cloud tier is not available in this build: the provider key must be kept in the system keychain, and no keychain store is wired in. So a bill photo cannot be read. Type the bill in the purchase form instead."
        )
    );
    // provenance: derived. While the tier is read nothing is claimed about it.
    assert_eq!(reason(Feature::Ask, &Gate::Reading), None);
    assert_eq!(
        reason(Feature::Ask, &Gate::NoCompany).as_deref(),
        Some("Open a company file to ask about your books.")
    );
    assert_eq!(
        reason(Feature::Ask, &Gate::Failed("disk gone".to_owned())).as_deref(),
        Some("The company's AI tier could not be read: disk gone")
    );
}

#[test]
fn ask_with_ai_off_offers_the_report_list() {
    let path = file_with_tier("s5_ask_off.ledgerly", Some(AiTier::Off));
    let mut app = open(&path);
    let mut screen = AskScreen::default();
    let text = settle(&mut screen, &mut app, AskScreen::loaded);
    // provenance: index.html:1167 (head), 1169 (note and button), 1170 (the six reports).
    assert!(text.contains("Ask your books"));
    assert!(text.contains(
        "The answer is always a report the engine runs. The assistant writes the query, never the figures."
    ));
    assert!(text.contains(ASK_OFF), "the off note: {text}");
    assert!(text.contains("AI and privacy"));
    assert!(text.contains("Reports"));
    for label in [
        "Outstanding receivables",
        "Balance sheet",
        "Day book",
        "GSTR-3B working",
        "Bank reconciliation",
        "Ledgers",
    ] {
        assert!(text.contains(label), "missing report {label}");
    }
    // The tier read is put in the context, so the top bar agrees.
    assert_eq!(app.ai_tier, Some(AiTier::Off));
}

#[test]
fn ask_with_the_local_tier_says_no_model_is_installed() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    let path = file_with_tier("s5_ask_local.ledgerly", Some(AiTier::Local));
    let mut app = open(&path);
    let mut screen = AskScreen::default();
    let text = settle(&mut screen, &mut app, AskScreen::loaded);
    // provenance: derived (see the_reason_follows_the_tier_for_each_feature).
    assert!(text.contains("No local model is installed on this computer"));
    assert!(!text.contains("AI is off"));
    assert!(text.contains("Outstanding receivables"));
}

#[test]
fn ask_reports_open_real_screens() {
    let path = file_with_tier("s5_ask_open.ledgerly", Some(AiTier::Off));
    let app = open(&path);
    // provenance: index.html:1170, the labels in the mock's order.
    let labels: Vec<&str> = AskReport::ALL.iter().map(|r| r.label()).collect();
    assert_eq!(
        labels,
        [
            "Outstanding receivables",
            "Balance sheet",
            "Day book",
            "GSTR-3B working",
            "Bank reconciliation",
            "Ledgers"
        ]
    );
    // provenance: derived. Balance sheet opens the balance sheet report, whose crumb title is
    // "Balance sheet" (report_view `title`, index.html:1572); the masters screen is "Masters".
    assert_eq!(
        AskReport::BalanceSheet
            .open(&app)
            .unwrap()
            .title(&ledgerly_app::i18n::Tr::default()),
        "Balance sheet"
    );
    assert_eq!(
        AskReport::Ledgers
            .open(&app)
            .unwrap()
            .title(&ledgerly_app::i18n::Tr::default()),
        "Masters"
    );
    // provenance: derived. The test company has no GST registration and no bank ledger, so those
    // two say why (navigate::gst_screen, navigate::bank_entry_screen).
    match AskReport::Gstr3b.open(&app) {
        Err(NavError::Unsupported { reason, .. }) => {
            assert_eq!(reason, "the company has no GST registration");
        }
        Err(other) => panic!("unexpected error {other}"),
        Ok(screen) => panic!(
            "opened {}",
            screen.title(&ledgerly_app::i18n::Tr::default())
        ),
    }
    match AskReport::BankReconciliation.open(&app) {
        Err(NavError::Unsupported { reason, .. }) => {
            assert_eq!(reason, "the company has no bank ledger");
        }
        Err(other) => panic!("unexpected error {other}"),
        Ok(screen) => panic!(
            "opened {}",
            screen.title(&ledgerly_app::i18n::Tr::default())
        ),
    }
}

#[test]
fn ask_keys_list_only_what_works() {
    // provenance: index.html:1199 gives [['','Ask your books'],['Enter','Ask'],['Esc','Back']] for
    // the other states. Enter asks nothing in this build, so only the name is listed; Esc is the
    // shell's.
    assert_eq!(ask::key_hints(), vec![KeyHint::new("", "Ask your books")]);
}

#[test]
fn plain_words_with_ai_off_points_to_the_voucher_forms() {
    let path = file_with_tier("s5_qe_off.ledgerly", Some(AiTier::Off));
    let mut app = open(&path);
    let mut screen = QuickEntryScreen::default();
    let text = settle(&mut screen, &mut app, QuickEntryScreen::loaded);
    // provenance: index.html:1542 (head), 1545 (the rules card).
    assert!(text.contains("Plain-words entry"));
    assert!(text.contains(
        "Type it the way you would say it. You get a draft voucher; the engine balances it; you accept."
    ));
    assert!(text.contains("What the assistant may do here"));
    assert!(text.contains("Copy amounts you typed (25k, 1 lakh) exactly. It never works one out."));
    assert!(text.contains("Pick ledgers that already exist. It never creates one without you."));
    assert!(text.contains("Suggest a date from a matching bank line."));
    // provenance: derived (see the_reason_follows_the_tier_for_each_feature).
    assert!(text.contains("AI is off for this company, so a sentence cannot be drafted"));
    // provenance: index.html:1554, the rejected state's "Type it myself" F5; F6 is the receipt
    // key of the classic preset (spec 7).
    assert!(text.contains("Payment"));
    assert!(text.contains("Receipt"));
    // No sample draft is drawn.
    assert!(!text.contains("Sharma Properties"));
    assert!(!text.contains("Draft payment"));
}

#[test]
fn plain_words_keys_open_payment_and_receipt() {
    let path = file_with_tier("s5_qe_keys.ledgerly", Some(AiTier::Local));
    let mut app = open(&path);
    let mut screen = QuickEntryScreen::default();
    let _ = settle(&mut screen, &mut app, QuickEntryScreen::loaded);
    // provenance: index.html:1554 (F5 type it myself); classic preset F5 Payment, F6 Receipt.
    assert_eq!(
        quick_entry::key_hints(),
        vec![
            KeyHint::new("", "Plain-words entry"),
            KeyHint::new("F5", "Payment"),
            KeyHint::new("F6", "Receipt"),
        ]
    );
    let (_, action) = frame(&mut screen, &mut app, &[(Key::F5, Modifiers::NONE)]);
    match action {
        ScreenAction::Push(screen) => assert_eq!(
            screen.page_title(&ledgerly_app::i18n::Tr::default()),
            "Payment"
        ),
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("F5 opened no payment")
        }
    }
    let (_, action) = frame(&mut screen, &mut app, &[(Key::F6, Modifiers::NONE)]);
    match action {
        ScreenAction::Push(screen) => assert_eq!(
            screen.page_title(&ledgerly_app::i18n::Tr::default()),
            "Receipt"
        ),
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("F6 opened no receipt")
        }
    }
}

#[test]
fn bill_photo_with_ai_off_says_reading_is_off() {
    let path = file_with_tier("s5_pb_off.ledgerly", Some(AiTier::Off));
    let mut app = open(&path);
    let mut screen = PurchaseBillScreen::default();
    let text = settle(&mut screen, &mut app, PurchaseBillScreen::loaded);
    // provenance: index.html:1422 (page title), 1430 (the note and its button).
    assert!(text.contains("Purchase from a bill photo"));
    assert!(text.contains(BILL_OFF), "the off note: {text}");
    assert!(text.contains("AI and privacy"));
    // No sample bill, reading or total is drawn.
    assert!(!text.contains("RUPA POLYMERS"));
    assert!(!text.contains("Engine total"));
    assert_eq!(
        screen.page_title(&ledgerly_app::i18n::Tr::default()),
        purchase_bill::PAGE_TITLE
    );
}

#[test]
fn bill_photo_local_tier_and_f9() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    let path = file_with_tier("s5_pb_local.ledgerly", Some(AiTier::Local));
    let mut app = open(&path);
    let mut screen = PurchaseBillScreen::default();
    let text = settle(&mut screen, &mut app, PurchaseBillScreen::loaded);
    // provenance: derived (see the_reason_follows_the_tier_for_each_feature).
    assert!(text.contains("No bill reader is installed on this computer"));
    // provenance: index.html:1422 subtitle "F9 purchase"; classic preset F9 Purchase (spec 7).
    assert_eq!(
        purchase_bill::key_hints(),
        vec![
            KeyHint::new("", "Bill photo"),
            KeyHint::new("F9", "Purchase")
        ]
    );
    let (_, action) = frame(&mut screen, &mut app, &[(Key::F9, Modifiers::NONE)]);
    match action {
        ScreenAction::Push(_) => {}
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("F9 opened no purchase")
        }
    }
}

#[test]
fn no_tier_chosen_is_said_and_not_taken_for_off() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    let path = file_with_tier("s5_unset.ledgerly", None);
    let mut app = open(&path);
    let mut screen = AskScreen::default();
    let text = settle(&mut screen, &mut app, AskScreen::loaded);
    // provenance: derived (see the_reason_follows_the_tier_for_each_feature).
    assert!(text.contains("No AI tier is chosen for this company"));
    assert!(!text.contains("AI is off"));
    assert_eq!(app.ai_tier, None);
}

#[test]
fn a_tier_changed_after_opening_is_read_again() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    let path = file_with_tier("s5_reread.ledgerly", Some(AiTier::Off));
    let mut app = open(&path);
    let mut screen = AskScreen::default();
    let text = settle(&mut screen, &mut app, AskScreen::loaded);
    assert!(text.contains(ASK_OFF));
    // AI and privacy stores Local on the worker and notes the change, as its screen does.
    let ticket = app
        .api
        .as_ref()
        .unwrap()
        .run(|api| api.set_ai_tier(AiTier::Local).map_err(|e| e.to_string()));
    let mut ticket = ticket;
    let mut stored = None;
    for _ in 0..2000 {
        if let ledgerly_app::worker::Poll::Ready(result) = ticket.poll() {
            stored = Some(result);
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(stored, Some(Ok(Outcome::Value(()))));
    ledgerly_app::shell::submit::note_changed();
    // provenance: derived (see the_reason_follows_the_tier_for_each_feature): the Local wording.
    let mut text = frame(&mut screen, &mut app, &[]).0;
    for _ in 0..2000 {
        if text.contains("No local model is installed") {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(&mut screen, &mut app, &[]).0;
    }
    assert!(
        text.contains("No local model is installed"),
        "the new tier was not read: {text}"
    );
    assert_eq!(app.ai_tier, Some(AiTier::Local));
}

#[test]
fn bill_photo_names_its_key_bar_and_keeps_purchase_selected() {
    let screen = PurchaseBillScreen::default();
    // provenance: index.html:1422, the crumbs Home, Purchase, From bill photo: the sidebar's
    // Purchase entry stays selected. The key bar name is derived: the mock's "Draft purchase"
    // (index.html:1455) names a draft that never exists here, so the bar names the screen.
    assert_eq!(
        screen.sidebar_title(&ledgerly_app::i18n::Tr::default()),
        "Purchase"
    );
    assert_eq!(
        screen.key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Bill photo"
    );
}
