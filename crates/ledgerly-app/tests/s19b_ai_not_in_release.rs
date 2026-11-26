//! Release 1.0.0 carries no AI assistant (owner decision of 2026-11-26). With the release switch
//! off, which is how the app ships, the AI choices are visible but disabled, and no route leads to
//! an AI-only screen. These tests pin that UI; the tests of the AI screens themselves turn the
//! switch on for their own thread (`release::set_ai_enabled`).
//!
//! Provenance: derived from the owner's decision above and from the mock's `AI_IN_RELEASE=false`
//! states `ai-settings.off` and `setup.ai` in `design/mock-v2/index.html`: Off selected, the Local
//! and Cloud cards disabled with a "Not in this release" tag, the line "AI arrives in a later
//! version. This company stays Off.", no Plain-words entry in the sidebar, the search box reading
//! "Search, jump or type an entry", the chip reading "AI off". The stored tier in the company file
//! is written by the test itself (`derived`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{AiTier, Api};
use ledgerly_app::keymap::classic;
use ledgerly_app::palette::{PaletteScope, PaletteTarget, RunCommand, legend, placeholder_key};
use ledgerly_app::release::{AI_IN_THIS_RELEASE, ai_enabled, effective_tier};
use ledgerly_app::screens::ai_settings::{AiSettingsScreen, key_hints};
use ledgerly_app::shell::navigate::{host_entries, screen_entries, sidebar};
use ledgerly_app::shell::topbar::ai_chip;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, EventSeq, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_store::{Event as StoreEvent, SqliteStore, Store};
use time::OffsetDateTime;
use time::macros::date;

const NOT_IN_RELEASE: &str = "Not in this release";
const LATER_NOTE: &str = "AI arrives in a later version. This company stays Off.";

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

/// A company file written by an older build with the given tier stored.
fn file_with_tier(name: &str, tier: AiTier) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    api.set_ai_tier(tier).unwrap();
    drop(api);
    path
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

fn frame(screen: &mut AiSettingsScreen, app: &mut AppCtx, keys: &[Key]) -> String {
    let ctx = egui::Context::default();
    let events = keys
        .iter()
        .map(|&key| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
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

#[test]
fn the_release_switch_is_off_in_this_release() {
    const { assert!(!AI_IN_THIS_RELEASE) };
    assert!(!ai_enabled());
}

#[test]
fn every_company_shows_ai_off_whatever_tier_its_file_stores() {
    for stored in [
        None,
        Some(AiTier::Off),
        Some(AiTier::Local),
        Some(AiTier::Cloud),
    ] {
        assert_eq!(effective_tier(stored), Some(AiTier::Off), "{stored:?}");
    }
    // The chip says "AI off", also before a company is open (no tier known).
    assert_eq!(
        ai_chip(effective_tier(None), &ledgerly_app::i18n::Tr::default()),
        "AI off"
    );
}

#[test]
fn a_file_stored_with_local_opens_and_shows_off_with_the_other_cards_disabled() {
    let path = file_with_tier("s19b_stored_local.ledgerly", AiTier::Local);
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(Api::open(&path).unwrap()).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    for _ in 0..2000 {
        if screen.loaded() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        let _text = frame(&mut screen, &mut app, &[]);
    }
    assert!(screen.loaded(), "the company file did not load");
    let text = frame(&mut screen, &mut app, &[]);
    assert_eq!(screen.stored(), Some(AiTier::Off));
    assert_eq!(app.ai_tier, Some(AiTier::Off));
    assert_eq!(text.matches(NOT_IN_RELEASE).count(), 2, "{text}");
    assert!(text.contains(LATER_NOTE), "{text}");
    assert!(text.contains("On this computer"), "{text}");
    assert!(text.contains("Cloud, your own key"), "{text}");
    // The Local model panel is not shown for a company that stays Off.
    assert!(!text.contains("bill reader"), "{text}");
}

#[test]
fn the_keys_for_local_and_cloud_change_nothing() {
    let path = file_with_tier("s19b_keys.ledgerly", AiTier::Off);
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(Api::open(&path).unwrap()).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = AiSettingsScreen::default();
    for _ in 0..200 {
        let _text = frame(&mut screen, &mut app, &[]);
        if screen.loaded() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    for key in [Key::Num2, Key::Num3, Key::Num1] {
        let _text = frame(&mut screen, &mut app, &[key]);
    }
    std::thread::sleep(Duration::from_millis(200));
    let _text = frame(&mut screen, &mut app, &[]);
    assert_eq!(screen.stored(), Some(AiTier::Off));
    drop(app);
    let store = SqliteStore::open(&path).unwrap();
    let changes = store
        .events(EventSeq(1), 1000)
        .unwrap()
        .into_iter()
        .filter(|record| matches!(&record.event, StoreEvent::SettingChanged { key, .. } if key == "ai.tier"))
        .count();
    // The one change is the test's own set_ai_tier(Off) when it made the file.
    assert_eq!(changes, 1);
}

#[test]
fn the_key_bar_lists_off_and_no_local_or_cloud_key() {
    let keys: Vec<String> = key_hints(false).iter().map(|h| h.keys.clone()).collect();
    assert_eq!(keys, ["", "1"]);
}

#[test]
fn the_sidebar_has_no_plain_words_entry() {
    for group in sidebar() {
        for entry in group.entries {
            assert_ne!(entry.label, "Plain-words entry");
            assert!(
                !matches!(&entry.target, PaletteTarget::Report(key)
                    if ["quick-entry", "ask", "purchase-bill"].contains(&key.as_str())),
                "{}",
                entry.label
            );
        }
    }
}

#[test]
fn the_palette_has_no_route_to_an_ai_only_screen() {
    let keymap = classic();
    let mut rows = host_entries(&keymap);
    rows.extend(screen_entries(None));
    for row in rows {
        assert!(
            !matches!(&row.target, PaletteTarget::Run(RunCommand::ReadBillPhoto)),
            "{}",
            row.title
        );
        assert!(
            !matches!(&row.target, PaletteTarget::Report(key)
                if ["quick-entry", "ask", "purchase-bill"].contains(&key.as_str())),
            "{}",
            row.title
        );
    }
}

#[test]
fn question_mark_and_plus_are_plain_text_and_the_legend_drops_ask_and_entry() {
    assert_eq!(
        PaletteScope::of_query("?top debtors"),
        (PaletteScope::All, "?top debtors")
    );
    assert_eq!(
        PaletteScope::of_query("+paid rent"),
        (PaletteScope::All, "+paid rent")
    );
    assert_eq!(
        PaletteScope::of_query("#0412"),
        (PaletteScope::Vouchers, "0412")
    );
    assert_eq!(
        legend().iter().map(|(key, _)| *key).collect::<Vec<_>>(),
        ["#", "@", ">"]
    );
    assert_eq!(placeholder_key(), "palette.placeholder_no_ai");
    let tr = ledgerly_app::i18n::Tr::default();
    let words = tr.t(placeholder_key());
    assert_eq!(words, "Search reports, ledgers, vouchers or commands");
    assert!(!words.contains('?') && !words.contains("ask"));
    assert_eq!(
        tr.t("topbar.search_text_no_ai"),
        "Search, jump or type an entry"
    );
}
