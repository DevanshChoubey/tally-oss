//! The one-step import screen reads like the mock's `SCREENS.import` (index.html lines 2430-2470):
//! Enter runs the import, the key bar says so, and the harness can tell the states apart by the
//! text the screen shows.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::import::{DirectState, ImportScreen};
use ledgerly_app::{AppCtx, Screen};
use time::macros::date;

fn enter() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

fn frame(s: &mut ImportScreen, input: RawInput) {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let out = ctx.run_ui(input, |ui| {
        let _action = s.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn enter_runs_the_import_with_the_form_as_filled() {
    // provenance: index.html:2455 and 2463: the primary button reads "Import the company" with the
    // Enter keycap, and the key bar offers Enter. With the company name empty the import refuses
    // before it asks for any worker (the mock's `validation-error` text, index.html:2442).
    let mut s = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    frame(&mut s, enter());
    assert_eq!(
        *s.direct_state(),
        DirectState::Failed("enter the name of the company in the other app".to_owned())
    );
}

#[test]
fn the_key_bar_offers_enter_to_import_and_esc_to_go_back() {
    // provenance: index.html:2463 (the empty-keys entry is the bar's name): [['','Import from another app'],['Enter','Import the company'],
    // ['Esc','Back']].
    let s = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    let keys: Vec<(String, String)> = s.keys().into_iter().map(|k| (k.keys, k.label)).collect();
    assert_eq!(
        keys,
        [
            ("Enter".to_owned(), "Import the company".to_owned()),
            ("Esc".to_owned(), "Back".to_owned()),
        ]
    );
}

#[test]
fn the_cursor_label_names_what_the_screen_shows() {
    // provenance: derived; the label is the first line of the state's text (the mock's note
    // text), so a harness can pin a state by it.
    let mut s = ImportScreen::new(date!(2026 - 11 - 21)).unwrap();
    let app = AppCtx::default();
    assert_eq!(
        s.cursor_label(&app).as_deref(),
        Some("No company is open yet.")
    );
    s.set_direct_form("localhost", "9000", "");
    s.start_direct_import(None);
    assert_eq!(
        s.cursor_label(&app).as_deref(),
        Some("Error: enter the name of the company in the other app")
    );
}
