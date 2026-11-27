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

    // Ctrl+E exports the rows shown, with the decision column (the export's header is the API's).
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-app-activity.csv");
    let _removed = std::fs::remove_file(&out);
    ledgerly_app::ui::dialogs::script_save_file(out.clone());
    let _ = frame(&mut screen, &mut app, &[(Key::E, Modifiers::COMMAND)]);
    let _ = settle_on(&mut screen, &mut app, "Exported");
    let csv = std::fs::read_to_string(&out).unwrap();
    assert!(csv.starts_with("When,Feature,"), "{csv}");
    assert!(csv.contains("IMG_2041.jpg"), "{csv}");
    assert!(csv.trim_end().ends_with("Waiting in review"), "{csv}");
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
        endpoint: Some("https://llm.example.invalid/v1".to_owned()),
        format: Some("chat-completions".to_owned()),
        model: Some("model-x".to_owned()),
        bill_model: None,
        words_model: None,
    })
    .unwrap();
    api.set_ai_tier(AiTier::Cloud).unwrap();
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    let text = settle_on(&mut screen, &mut app, "Requests go to");
    assert!(
        text.contains("https://llm.example.invalid/v1/chat/completions"),
        "{text}"
    );
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

/// The provider/key form (mock `ai.ai-settings.cloud-edit`). Ctrl+Enter alters, Ctrl+A saves,
/// Alt+D deletes and Esc goes back, as TallyPrime documents those keys ("To alter a master",
/// "To accept or save a screen", delete, "To go back": help.tallysolutions.com/
/// keyboard-shortcuts-tally-prime/, read 2026-11-27). Every vault here is a `MemoryVault`; the
/// keys are fictional strings. No test touches the OS keychain or the network.
const FORM_KEY: &str = "fictional-key-form-s21";

fn cloud_app(name: &str, vault: MemoryVault, described: bool) -> (AppCtx, PathBuf) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let mut api = api(name);
    api.attach_key_vault(Box::new(vault));
    if described {
        api.set_cloud_settings(&CloudSettings {
            endpoint: Some("https://llm.example.invalid/v1".to_owned()),
            format: Some("chat-completions".to_owned()),
            model: Some("model-x".to_owned()),
            bill_model: None,
            words_model: None,
        })
        .unwrap();
    }
    let app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    (app, path)
}

/// Frames until the screen is idle (nothing in flight) and has drawn `wanted`.
fn settle_idle_on(screen: &mut AiSettingsScreen, app: &mut AppCtx, wanted: &str) -> String {
    let mut text = frame(screen, app, &[]);
    for _ in 0..2000 {
        if screen.loaded() && !screen.busy() && text.contains(wanted) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("never drew {wanted:?}; last frame:\n{text}");
}

#[test]
fn ctrl_enter_opens_the_form_with_the_key_field_empty_and_esc_closes_it() {
    ledgerly_app::release::set_ai_enabled(true);
    let vault = MemoryVault::with_key(KEY_SERVICE, "company-1-Fictional Traders", FORM_KEY);
    let (mut app, _path) = cloud_app("s21-app-form-open.ledgerly", vault, true);
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "Off");
    let _ = frame(&mut screen, &mut app, &[(Key::Num3, Modifiers::NONE)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Requests go to");

    let _ = frame(&mut screen, &mut app, &[(Key::Enter, Modifiers::COMMAND)]);
    let text = settle_idle_on(&mut screen, &mut app, "Change provider or key");
    assert!(screen.editing_provider());
    assert!(text.contains("Leave empty to keep the saved key"), "{text}");
    assert!(
        text.contains("model-x"),
        "the form starts from the stored provider: {text}"
    );
    assert!(
        !text.contains(FORM_KEY),
        "the key is never filled back or drawn"
    );

    let _ = frame(&mut screen, &mut app, &[(Key::Escape, Modifiers::NONE)]);
    assert!(!screen.editing_provider(), "Esc closes the form");
    let text = settle_idle_on(&mut screen, &mut app, "Stored in the system keychain");
    assert!(!text.contains("Leave empty to keep the saved key"));
}

#[test]
fn ctrl_a_saves_the_provider_and_puts_the_key_in_the_vault_only() {
    ledgerly_app::release::set_ai_enabled(true);
    let (mut app, path) = cloud_app("s21-app-form-save.ledgerly", MemoryVault::default(), false);
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "Off");
    let _ = frame(&mut screen, &mut app, &[(Key::Num3, Modifiers::NONE)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Not described yet");

    let _ = frame(&mut screen, &mut app, &[(Key::Enter, Modifiers::COMMAND)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Change provider or key");
    screen.fill_provider_form("messages", "", ["", "", "model-y"], FORM_KEY);
    let _ = frame(&mut screen, &mut app, &[(Key::A, Modifiers::COMMAND)]);
    let text = settle_idle_on(&mut screen, &mut app, "Stored in the system keychain");
    assert!(!screen.editing_provider(), "a save closes the form");
    // Messages (Anthropic API) with no endpoint and no bill or plain-words model: the owner's
    // defaults (2026-11-27) are shown as the API resolves them.
    assert!(
        text.contains("https://api.anthropic.com/v1/messages"),
        "{text}"
    );
    assert!(text.contains("Messages (Anthropic API)"), "{text}");
    assert!(text.contains("claude-sonnet-5-5"), "{text}");
    assert!(text.contains("claude-haiku-5-5"), "{text}");
    assert!(text.contains("model-y"), "{text}");
    assert!(text.contains("Set"), "{text}");
    assert!(!text.contains(FORM_KEY), "the key is never drawn");

    drop(screen);
    drop(app);
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(FORM_KEY.len())
            .any(|w| w == FORM_KEY.as_bytes()),
        "the company file must never hold the key"
    );
}

#[test]
fn an_http_endpoint_is_refused_and_the_form_stays_open() {
    ledgerly_app::release::set_ai_enabled(true);
    let (mut app, _path) = cloud_app("s21-app-form-http.ledgerly", MemoryVault::default(), false);
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "Off");
    let _ = frame(&mut screen, &mut app, &[(Key::Num3, Modifiers::NONE)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Not described yet");
    let _ = frame(&mut screen, &mut app, &[(Key::Enter, Modifiers::COMMAND)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Change provider or key");
    screen.fill_provider_form(
        "chat-completions",
        "http://llm.example.invalid/v1",
        ["", "", "model-x"],
        FORM_KEY,
    );
    let _ = frame(&mut screen, &mut app, &[(Key::A, Modifiers::COMMAND)]);
    let text = settle_idle_on(&mut screen, &mut app, "must start with https://");
    assert!(
        screen.editing_provider(),
        "a refused save keeps the form open"
    );
    assert!(!text.contains(FORM_KEY), "the key is never drawn");
    // Nothing was stored: closing the form shows the provider still undescribed, no key.
    let _ = frame(&mut screen, &mut app, &[(Key::Escape, Modifiers::NONE)]);
    let text = settle_idle_on(&mut screen, &mut app, "Not described yet");
    assert!(text.contains("Not set"), "{text}");
}

#[test]
fn alt_d_in_the_form_deletes_the_saved_key() {
    ledgerly_app::release::set_ai_enabled(true);
    let vault = MemoryVault::with_key(KEY_SERVICE, "company-1-Fictional Traders", FORM_KEY);
    let (mut app, _path) = cloud_app("s21-app-form-delete.ledgerly", vault, true);
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "Off");
    let _ = frame(&mut screen, &mut app, &[(Key::Num3, Modifiers::NONE)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Stored in the system keychain");
    let _ = frame(&mut screen, &mut app, &[(Key::Enter, Modifiers::COMMAND)]);
    let _ = settle_idle_on(&mut screen, &mut app, "Change provider or key");
    let _ = frame(&mut screen, &mut app, &[(Key::D, Modifiers::ALT)]);
    let text = settle_idle_on(&mut screen, &mut app, "Not set");
    assert!(!screen.editing_provider(), "a delete closes the form");
    assert!(
        text.contains("No key in the system keychain for this company"),
        "{text}"
    );
}

#[test]
fn with_ai_out_of_the_release_ctrl_enter_opens_no_form() {
    ledgerly_app::release::set_ai_enabled(false);
    let (mut app, _path) = cloud_app("s21-app-form-off.ledgerly", MemoryVault::default(), true);
    let mut screen = AiSettingsScreen::default();
    let _ = settle_on(&mut screen, &mut app, "Off");
    let text = frame(&mut screen, &mut app, &[(Key::Enter, Modifiers::COMMAND)]);
    assert!(!screen.editing_provider());
    assert!(!text.contains("Change provider or key"), "{text}");
}
