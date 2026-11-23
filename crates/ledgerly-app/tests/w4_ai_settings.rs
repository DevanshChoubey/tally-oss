//! Card W4 tests: the AI and privacy screen over a real company file (spec 6.8, rule A10).
//!
//! Provenance. The wording checked here is the mock's, quoted from `design/mock-v2/index.html`:
//! the option cards at lines 1905-1907, the feature rows at 1908, the log at 1915, the never list
//! at 1916 and the keys at 1920. The keys do not list Esc, which the shell draws for every screen.
//! The log and the stored tier are written to a company file by the test itself, so the expected
//! rows are the ones the test wrote (`derived`). None is an output of the code under test.

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
use ledgerly_app::screens::ai_settings::{
    AiSettingsScreen, CLOUD_STATUS, LOG_EMPTY, LOG_META, LOG_META_EMPTY, NEVER, OFF_NOTE,
    OPTION_CLOUD, OPTION_LOCAL, OPTION_OFF, UNSET_NOTE, key_hints,
};
use ledgerly_app::shell::Shell;
use ledgerly_app::shell::topbar::ai_chip;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, EventSeq, StateCode, UserId};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy, ProposalRecord};
use ledgerly_store::{Actor, AuditMeta, Event as StoreEvent, SqliteStore, Store};
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

/// A new company file, with the given AI proposal rows written to its log.
fn file_with_log(name: &str, rows: &[ProposalRecord]) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    drop(Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap());
    if !rows.is_empty() {
        let mut store = SqliteStore::open(&path).unwrap();
        for row in rows {
            let meta = AuditMeta {
                actor: Actor::User(UserId(1)),
                at: OffsetDateTime::UNIX_EPOCH,
                reason: None,
                app_version: "test".to_owned(),
            };
            store
                .commit(&[StoreEvent::AiProposalRecorded(row.clone())], &meta)
                .unwrap();
        }
    }
    path
}

fn new_file(name: &str) -> PathBuf {
    file_with_log(name, &[])
}

fn proposal() -> ProposalRecord {
    ProposalRecord {
        model: "local-small".to_owned(),
        model_version: "1".to_owned(),
        prompt_template_version: "cloud-v1".to_owned(),
        input_hash: "0123456789abcdef".to_owned(),
        output_json: "{\"a\":1}".to_owned(),
        human_edits_json: None,
        accepted: true,
    }
}

/// The app context with the company file open on a worker.
fn open(path: &Path) -> AppCtx {
    let api = Api::open(path).unwrap();
    AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    }
}

/// Every piece of text the frame drew, joined. Galleys hold the text of labels and cells.
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

/// One frame with the given keys pressed. Returns what the frame drew.
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
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _stay = screen.ui(ui, app);
    });
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    text
}

/// Frames until `done` holds, so the worker's answer has been taken. Returns the last frame's
/// text. Fails the test when the answer never comes.
fn settle(
    screen: &mut AiSettingsScreen,
    app: &mut AppCtx,
    keys: &[(Key, Modifiers)],
    done: impl Fn(&AiSettingsScreen) -> bool,
) -> String {
    let mut text = frame(screen, app, keys);
    for _ in 0..2000 {
        if done(screen) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("the worker's answer did not arrive");
}

const NONE: Modifiers = Modifiers::NONE;

/// The stored tiers and every event that changed them, from the company file.
fn ai_settings(path: &Path) -> Vec<(String, String, String)> {
    let store = SqliteStore::open(path).unwrap();
    store
        .events(EventSeq(1), 1000)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            if let StoreEvent::SettingChanged { key, before, after } = record.event {
                Some((key, before, after))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn the_three_states_render_the_mock_wording_and_keys() {
    let path = new_file("w4_states.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();

    // Nothing chosen yet: the three options, the note that nothing is sent, and no log rows.
    let unset = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    for (title, body) in [OPTION_OFF, OPTION_LOCAL, OPTION_CLOUD] {
        assert!(unset.contains(title), "missing option title {title}");
        assert!(unset.contains(body), "missing option body {body}");
    }
    assert!(unset.contains(UNSET_NOTE));
    assert!(unset.contains(LOG_EMPTY));
    assert!(unset.contains(LOG_META_EMPTY));
    assert!(!unset.contains("Ctrl+E exports it"));
    assert_eq!(screen.stored(), None);

    // On this computer (mock state ai-settings.local, index.html:1906 and 1907): the local panel.
    let local = settle(&mut screen, &mut app, &[(Key::Num2, NONE)], |s| {
        s.stored() == Some(AiTier::Local)
    });
    assert!(local.contains("Not installed. This build has no local bill reader."));
    assert!(!local.contains("Nothing: off"));
    assert!(local.contains("Narration, date, amount, on this computer"));
    assert!(local.contains("Not built yet"));
    assert_eq!(
        screen.panel(),
        ledgerly_app::screens::ai_settings::Panel::Local
    );
    assert_eq!(screen.keys(), key_hints(false), "the keys without a log");

    // Off (mock state ai-settings.off, index.html:1905 and the off note): every feature is off.
    let off = settle(&mut screen, &mut app, &[(Key::Num1, NONE)], |s| {
        s.stored() == Some(AiTier::Off)
    });
    assert!(off.contains(OFF_NOTE), "the off note is missing");
    assert!(off.contains("Nothing: off"));
    assert!(!off.contains("Not installed. This build has no local bill reader."));

    // Cloud (mock state ai-settings.cloud, index.html:1907): the engine refuses it, and the
    // panel says why. The stored tier stays Off.
    let cloud = settle(&mut screen, &mut app, &[(Key::Num3, NONE)], |s| {
        s.notice().is_some()
    });
    assert!(cloud.contains("Cloud, your own key"));
    assert!(cloud.contains(CLOUD_STATUS));
    assert!(cloud.contains("keychain"), "the engine's reason is shown");
    assert_eq!(screen.stored(), Some(AiTier::Off));
    assert!(cloud.contains("Not built yet"));
}

#[test]
fn the_never_list_is_the_mock_list_on_every_state() {
    // index.html:1916: the five lines of "What AI may never do here".
    let path = new_file("w4_never.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains("What AI may never do here"));
    for line in NEVER {
        assert!(text.contains(line), "missing: {line}");
    }
    assert_eq!(NEVER.len(), 5);
}

#[test]
fn choosing_a_tier_is_kept_in_the_file_and_the_chip_follows_it() {
    let path = new_file("w4_chip.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert_eq!(ai_chip(app.ai_tier), "AI: not set up");

    frame(&mut screen, &mut app, &[(Key::Num2, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Local)
    });
    assert_eq!(app.ai_tier, Some(AiTier::Local));
    assert_eq!(ai_chip(app.ai_tier), "AI on this computer");

    frame(&mut screen, &mut app, &[(Key::Num1, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Off)
    });
    assert_eq!(ai_chip(app.ai_tier), "AI off");

    // Read back from the file after the company is closed and opened again.
    drop(app);
    let reopened = Api::open(&path).unwrap();
    assert_eq!(reopened.ai_tier().unwrap(), Some(AiTier::Off));
    assert_eq!(
        ai_settings(&path)
            .last()
            .map(|(_, _, after)| after.as_str()),
        Some("off")
    );
}

#[test]
fn the_top_bar_chip_reads_the_stored_tier_when_the_shell_opens_the_file() {
    let path = new_file("w4_shell_chip.ledgerly");
    {
        let mut api = Api::open(&path).unwrap();
        api.set_ai_tier(AiTier::Local).unwrap();
    }
    let mut shell = Shell::new(Box::new(AiSettingsScreen::default()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(Api::open(&path).unwrap()).unwrap());
    for _ in 0..2000 {
        shell.sync_books();
        if shell.app_ctx_mut().ai_tier.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(shell.app_ctx_mut().ai_tier, Some(AiTier::Local));
    assert_eq!(ai_chip(shell.app_ctx_mut().ai_tier), "AI on this computer");
}

#[test]
fn the_log_shows_the_file_rows_and_never_sample_rows() {
    let path = file_with_log("w4_log_rows.ledgerly", &[proposal()]);
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains(LOG_META), "the log's meta line with Ctrl+E");
    assert!(text.contains("local-small"));
    assert!(text.contains("{\"a\":1}"));
    assert!(text.contains("Accepted"));
    assert!(
        text.contains("0123456789ab…"),
        "the hash is shortened for the table"
    );
    // The mock's sample rows (index.html:1896-1900) must not appear for a real log.
    for sample in [
        "IMG_2041",
        "Sharma Properties",
        "5 statement lines",
        "Draft payment",
    ] {
        assert!(!text.contains(sample), "sample row leaked: {sample}");
    }
    assert!(
        screen
            .keys()
            .iter()
            .any(|k| k.keys == "Ctrl+E" && k.label == "Export log")
    );
}

#[test]
fn an_empty_log_says_so_and_offers_no_export() {
    let path = new_file("w4_log_empty.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains(LOG_EMPTY));
    assert!(!text.contains("Ctrl+E exports it"));
    assert_eq!(screen.log().len(), 0);
    assert!(!screen.keys().iter().any(|k| k.keys == "Ctrl+E"));
    // Ctrl+E with no rows does nothing: no save dialog, no notice.
    frame(&mut screen, &mut app, &[(Key::E, Modifiers::COMMAND)]);
    assert_eq!(screen.notice(), None);
}

#[test]
fn the_key_hints_match_the_mock_keys() {
    // index.html:1920: 1 Off, 2 On this computer, 3 Cloud, Ctrl+E Export log. The shell adds Esc.
    let without = key_hints(false);
    let labels: Vec<(&str, &str)> = without
        .iter()
        .map(|k| (k.keys.as_str(), k.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("", "AI and privacy"),
            ("1", "Off"),
            ("2", "On this computer"),
            ("3", "Cloud"),
        ]
    );
    assert_eq!(key_hints(true).len(), 5);
}

#[test]
fn no_cloud_key_reaches_the_company_file() {
    let path = new_file("w4_no_key_file.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    frame(&mut screen, &mut app, &[(Key::Num2, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Local)
    });
    frame(&mut screen, &mut app, &[(Key::Num3, NONE)]);
    settle(&mut screen, &mut app, &[], |s| s.notice().is_some());
    drop(app);

    // Only the tier is written. The cloud choice is refused, so no key-like text ever is.
    let changes = ai_settings(&path);
    assert_eq!(
        changes.len(),
        1,
        "only the local choice is stored: {changes:?}"
    );
    assert!(changes.iter().all(|(key, _, _)| key == "ai.tier"));
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(3).any(|w| w == b"sk-"));
    assert!(!bytes.windows(8).any(|w| w == b"keychain"));
}
