//! Card S8 (sample company): the welcome card "Try the sample company" builds Godavari Drip & Pump
//! Works in a throwaway file and opens it.
//!
//! Provenance: the mock, `design/mock-v2/index.html:1864` (the card's wording: "Godavari Drip &
//! Pump Works, fictional data. Nothing you do there is kept.") and the task brief (the sample is
//! made in a throwaway cache file, never in the user's documents).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::shell::Shell;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_core::{Clock, OffsetDateTime};

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

fn key(k: Key) -> Event {
    Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
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

fn base(name: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_dir_all(&base);
    base
}

fn shell_with(base: PathBuf) -> (Context, Shell) {
    let mut shell = Shell::new(Box::new(
        SetupScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).with_sample_base(base),
    ));
    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    frame(&ctx, &mut shell, vec![]);
    frame(&ctx, &mut shell, vec![]);
    (ctx, shell)
}

#[test]
fn the_sample_card_no_longer_says_no_sample_ships() {
    // provenance: index.html:1864, the card is a working choice, so it carries no "unavailable".
    let (ctx, mut shell) = shell_with(base("s8-sample-text"));
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted.iter().any(|t| t == "Try the sample company"),
        "{painted:?}"
    );
    assert!(
        !painted.iter().any(|t| t.contains("No sample data ships")),
        "{painted:?}"
    );
}

#[test]
fn choosing_the_sample_card_opens_godavari_from_a_throwaway_file() {
    // provenance: the brief (a throwaway file under the given base) and index.html:1864 (the
    // company is Godavari Drip & Pump Works).
    let base = base("s8-sample-open");
    let (ctx, mut shell) = shell_with(base.clone());
    // Down to Open (the second row), Right to the sample card beside it.
    frame(&ctx, &mut shell, vec![key(Key::ArrowDown)]);
    frame(&ctx, &mut shell, vec![key(Key::ArrowRight)]);
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted.iter().any(|t| t == "Try the sample company"),
        "{painted:?}"
    );
    frame(&ctx, &mut shell, vec![key(Key::Enter)]);
    for _ in 0..5000 {
        if shell.app_ctx_mut().books.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        frame(&ctx, &mut shell, vec![]);
    }
    frame(&ctx, &mut shell, vec![]);
    let books = shell.app_ctx_mut().books.as_ref().unwrap();
    assert_eq!(books.company.name, "Godavari Drip & Pump Works");
    assert!(
        ledgerly_sample::path_under(&base).is_file(),
        "no file under the base"
    );
}

#[test]
fn down_from_create_still_lands_on_open() {
    // provenance: the audit shot `cold.start-open-selected` (design/COMPARE.md): one arrow key from
    // Create selects "Open a Ledgerly file", whose Continue button then says so.
    let (ctx, mut shell) = shell_with(base("s8-sample-arrows"));
    frame(&ctx, &mut shell, vec![key(Key::ArrowDown)]);
    frame(&ctx, &mut shell, vec![]);
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted.iter().any(|t| t == "Open a Ledgerly file"),
        "{painted:?}"
    );
}
