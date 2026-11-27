//! Round s21, plan round B: the AI and privacy screen in the later release (mock states
//! `ai.ai-settings.local` and `ai.ai-settings.cloud`, `design/mock-v2/index.html` with `ai=on`).
//!
//! Provenance (derived). The words are the mock's, quoted from its `SCREENS['ai-settings']`
//! (local model rows, provider rows, the log's columns). The provider, the key and the stored
//! draft are written to the company file or the in-memory vault by the test itself, so every
//! expected value is one the test wrote. Nothing is copied from the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{
    AiFeature, AiTier, Api, CloudSettings, KEY_SERVICE, MemoryVault, NewAiDraft, Proposal,
    VoucherDraft,
};
use ledgerly_app::screens::ai_settings::AiSettingsScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, StateCode};
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, NegativeStockPolicy,
};
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

fn api(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
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

fn frame(screen: &mut AiSettingsScreen, app: &mut AppCtx, keys: &[(Key, Modifiers)]) -> String {
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
    let out = ctx.run_ui(
        RawInput {
            events,
            ..RawInput::default()
        },
        |ui| {
            let _stay = screen.ui(ui, app);
        },
    );
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    text
}

/// Frames until the screen has read the company and drawn `wanted`.
fn settle_on(screen: &mut AiSettingsScreen, app: &mut AppCtx, wanted: &str) -> String {
    let mut text = frame(screen, app, &[]);
    for _ in 0..2000 {
        if screen.loaded() && text.contains(wanted) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("never drew {wanted:?}; last frame:\n{text}");
}

#[test]
fn the_local_panel_names_the_model_file_and_the_log_lists_assistant_activity() {
    ledgerly_app::release::set_ai_enabled(true);
    let mut api = api("s21-app-settings-local.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    api.record_ai_draft(NewAiDraft {
        feature: AiFeature::BillReading,
        provider: "Fixed answers (test provider)".to_owned(),
        model_version: "s21-1".to_owned(),
        prompt_template_version: "ocr/1".to_owned(),
        input_label: "IMG_2041.jpg".to_owned(),
        input_hash: "9f3c".repeat(16),
        proposal: Proposal::VoucherDraft(Box::new(VoucherDraft {
            kind: BaseVoucherKind::Purchase,
            date: None,
            party_name: None,
            party: None,
            invoice_number: None,
            supplier_gstin: None,
            lines: Vec::new(),
            items: Vec::new(),
            document_total: None,
            document_tax: None,
            create_ledgers: Vec::new(),
            narration: String::new(),
        })),
        confidence_permille: 930,
    })
    .unwrap();
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    let text = settle_on(&mut screen, &mut app, "Model file");
    assert!(text.contains("None chosen."), "{text}");
    assert!(text.contains("Not installed. This build has no local bill reader."));
    // The log's columns are the mock's, and the stored draft is a row of it.
    for header in ["When", "Feature", "Input", "Output", "Your decision"] {
        assert!(text.contains(header), "missing log column {header}");
    }
    assert!(text.contains("Bill reader"), "{text}");
    assert!(text.contains("IMG_2041.jpg"), "{text}");
    assert!(text.contains("Waiting in review"), "{text}");
}

#[test]
fn the_cloud_panel_shows_the_provider_and_whether_the_key_is_saved() {
    ledgerly_app::release::set_ai_enabled(true);
    let mut api = api("s21-app-settings-cloud.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::with_key(
        KEY_SERVICE,
        "company-1-Fictional Traders",
        "fictional-key-s21",
    )));
    api.set_cloud_settings(&CloudSettings {
        endpoint: Some("https://llm.example.invalid/v1/chat/completions".to_owned()),
        format: Some("chat-completions".to_owned()),
        model: Some("model-x".to_owned()),
    })
    .unwrap();
    api.set_ai_tier(AiTier::Cloud).unwrap();
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    let text = settle_on(&mut screen, &mut app, "Endpoint");
    assert!(text.contains("https://llm.example.invalid/v1/chat/completions"));
    assert!(text.contains("Chat completions"), "{text}");
    assert!(text.contains("model-x"));
    assert!(text.contains("Stored in the system keychain"));
    assert!(text.contains("Set"));
    assert!(text.contains("each document, always"));
    assert!(
        !text.contains("fictional-key-s21"),
        "the key is never drawn"
    );
}

#[test]
fn ctrl_o_on_the_local_panel_stores_the_chosen_model_file() {
    ledgerly_app::release::set_ai_enabled(true);
    let mut api = api("s21-app-settings-model.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    // A fictional 12-byte file stands in for a model; nothing is downloaded or run.
    let model = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-app-model.gguf");
    std::fs::write(&model, [7_u8; 12]).unwrap();
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "None chosen.");
    ledgerly_app::ui::dialogs::script_open_file(model.clone());
    let _ = frame(&mut screen, &mut app, &[(Key::O, Modifiers::COMMAND)]);
    let text = settle_on(&mut screen, &mut app, "(12 bytes)");
    assert!(text.contains(&model.display().to_string()), "{text}");
    assert!(!text.contains("None chosen."), "{text}");
}

/// One frame on a context that lives across frames, so a press and release make a click.
fn frame_on(
    ctx: &egui::Context,
    screen: &mut AiSettingsScreen,
    app: &mut AppCtx,
    events: Vec<Event>,
) {
    let out = ctx.run_ui(
        RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 1600.0),
            )),
            ..RawInput::default()
        },
        |ui| {
            let _stay = screen.ui(ui, app);
        },
    );
    out.drop_without_applying_deltas();
}

#[test]
fn clicking_a_feature_switch_stores_it_off_and_draws_it_off() {
    ledgerly_app::release::set_ai_enabled(true);
    let mut api = api("s21-app-settings-switch.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    let ctx = egui::Context::default();
    let mut rect = None;
    for _ in 0..2000 {
        frame_on(&ctx, &mut screen, &mut app, Vec::new());
        rect = ledgerly_app::screens::ai_settings::read_probe::<egui::Rect>(
            &ctx,
            "ai-settings.switch.2",
        );
        if screen.loaded() && rect.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        screen.features_off(),
        Some(&[][..]),
        "every feature starts on"
    );
    let at = rect.expect("the bank switch was drawn").center();
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame_on(
        &ctx,
        &mut screen,
        &mut app,
        vec![Event::PointerMoved(at), button(true)],
    );
    frame_on(&ctx, &mut screen, &mut app, vec![button(false)]);
    for _ in 0..2000 {
        frame_on(&ctx, &mut screen, &mut app, Vec::new());
        if screen.features_off() == Some(&[AiFeature::BankSuggestion][..]) {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!(
        "the bank switch never read off: {:?}",
        screen.features_off()
    );
}
