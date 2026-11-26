//! Release smoke test: the first run of a fresh install, end to end and without a window.
//!
//! It starts the shell the way `main` does (`Shell::cold_start`), checks the first screen is the
//! setup welcome, types a company into the setup wizard with the keyboard (as a person would),
//! waits for the background job to write the company file under a fresh home folder, and then
//! checks the file: the audit chain verifies and AI is off. CI runs this on Linux, Windows and
//! macOS; `LEDGERLY_SMOKE_HOME` names the fresh home (default: a folder under the test target
//! directory), so a later CI step can run `ledgerly verify` on the file this test made.
//!
//! Provenance: derived from the owner's release decisions (a fresh company starts with AI off;
//! the first screen is the setup welcome, mock state `setup.welcome`; the wizard keys are the
//! ones `ui_snapshots` uses for `setup.done`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_api::{AiTier, Api};
use ledgerly_app::shell::Shell;
use ledgerly_app::theme::{self, ThemeKind};

fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Band(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn frame(ctx: &Context, shell: &mut Shell, events: Vec<Event>) -> Vec<String> {
    let input = RawInput {
        events,
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    let painted = texts(&out.shapes);
    out.drop_without_applying_deltas();
    painted
}

fn press(k: Key) -> Event {
    Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn type_text(ctx: &Context, shell: &mut Shell, text: &str) {
    frame(ctx, shell, vec![Event::Text(text.to_owned())]);
    frame(ctx, shell, vec![]);
}

fn keys(ctx: &Context, shell: &mut Shell, ks: &[Key]) {
    for k in ks {
        frame(ctx, shell, vec![press(*k)]);
        frame(ctx, shell, vec![]);
    }
}

#[test]
fn a_fresh_install_reaches_the_welcome_and_creates_a_company_with_ai_off() {
    let home = std::env::var_os("LEDGERLY_SMOKE_HOME").map_or_else(
        || PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s19_smoke_home"),
        PathBuf::from,
    );
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    ledgerly_app::screens::setup::set_home_override(std::fs::canonicalize(&home).unwrap());

    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    let mut shell = Shell::cold_start();
    frame(&ctx, &mut shell, vec![]);
    let first = frame(&ctx, &mut shell, vec![]);
    assert!(
        first.iter().any(|t| t == "Welcome to Ledgerly"),
        "the first screen is not the welcome: {first:?}"
    );

    // The sidebar names the release (it reads the crate version). A release bump updates this.
    assert!(
        first.iter().any(|t| t == "Ledgerly 1.0.0"),
        "the sidebar does not say Ledgerly 1.0.0: {first:?}"
    );

    // Welcome: Enter takes the default card (Create a company). Then the company step.
    keys(&ctx, &mut shell, &[Key::Enter]);
    type_text(&ctx, &mut shell, "Smoke Test Traders");
    keys(&ctx, &mut shell, &[Key::Tab, Key::Tab]);
    type_text(&ctx, &mut shell, "Pune");
    keys(&ctx, &mut shell, &[Key::Tab]);
    type_text(&ctx, &mut shell, "27ZZZZZ0000Z1ZQ");
    // State, GST filing, Books begin, Company file (left blank: the default under Documents),
    // then onto the step's Continue.
    keys(&ctx, &mut shell, &[Key::Tab; 6]);
    // Continue through Features, Keyboard, AI, Backup, then Finish.
    keys(&ctx, &mut shell, &[Key::Enter; 5]);

    let deadline = Instant::now() + Duration::from_secs(120);
    let docs = home.join("Documents").join("Ledgerly");
    let file = loop {
        frame(&ctx, &mut shell, vec![]);
        let found = std::fs::read_dir(&docs).ok().and_then(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "ledgerly"))
        });
        if let Some(path) = found {
            break path;
        }
        assert!(
            Instant::now() < deadline,
            "no company file appeared under {}",
            docs.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    // Let the job finish and the shell open the company, then settle a few frames.
    for _ in 0..200 {
        frame(&ctx, &mut shell, vec![]);
        std::thread::sleep(Duration::from_millis(5));
    }

    let api = Api::open(&file).unwrap();
    assert_eq!(
        api.ai_tier().unwrap(),
        Some(AiTier::Off),
        "AI must start off"
    );
    let ledgerly_core::Outcome::Value(books) = api.books().unwrap() else {
        panic!("the books of the new company are unsupported");
    };
    assert_eq!(books.company.name, "Smoke Test Traders");
}
