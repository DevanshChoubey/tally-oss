//! Round s14: setup states that `design/PARITY-GAPS.md` listed as differing from the mock.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::epaint::ClippedShape;
use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Shape, Vec2};
use ledgerly_core::{Clock, Outcome, StateCode};
use time::OffsetDateTime;

use super::{SetupScreen, Step};
use crate::theme::{self, ThemeKind};
use crate::{AppCtx, Screen, ScreenAction};

fn raw() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        ..RawInput::default()
    }
}

fn run(
    screen: &mut SetupScreen,
    ctx: &mut AppCtx,
    frames: usize,
    mut input: impl FnMut(usize) -> RawInput,
) -> (Vec<ClippedShape>, ScreenAction) {
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);
    let mut shapes = Vec::new();
    let mut action = ScreenAction::Stay;
    for n in 0..frames {
        let out = egui_ctx.run_ui(input(n), |ui| {
            let next = screen.ui(ui, ctx);
            if !matches!(next, ScreenAction::Stay) {
                action = next;
            }
        });
        shapes = out.shapes.clone();
        out.drop_without_applying_deltas();
    }
    (shapes, action)
}

fn bad_outlines(shapes: &[ClippedShape], bad: egui::Color32) -> usize {
    fn walk(shape: &Shape, bad: egui::Color32, n: &mut usize) {
        match shape {
            Shape::Rect(r) if r.stroke.width > 0.0 && r.stroke.color == bad => *n += 1,
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, bad, n)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Text(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut n = 0;
    for c in shapes {
        walk(&c.shape, bad, &mut n);
    }
    n
}

fn press(key: Key) -> RawInput {
    let mut i = raw();
    i.events.push(Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    i
}

#[test]
fn f8_on_done_opens_a_sales_invoice() {
    // provenance: index.html:1892 `setup.done`: the "Make your first invoice" launch carries F8.
    let dir = std::env::temp_dir().join("s14_setup_done");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (api, _) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let mut ctx = AppCtx {
        books: Some(books),
        ..AppCtx::default()
    };
    let mut screen = SetupScreen {
        step: Step::Done,
        ..SetupScreen::default()
    };
    let (_, action) = run(&mut screen, &mut ctx, 3, |n| {
        if n == 2 { press(Key::F8) } else { raw() }
    });
    assert!(matches!(action, ScreenAction::Push(_)), "F8 opens a screen");
}

#[test]
fn a_state_that_disagrees_with_the_gstin_is_outlined_in_red() {
    // provenance: index.html:1952 `setup.gstin-error`: the GSTIN and State inputs carry the
    // `invalid` class (a red outline) while the state disagrees with the GSTIN's first digits.
    let mut ctx = AppCtx::default();
    let bad = theme::tokens(ThemeKind::Light).bad;
    let mut wrong = SetupScreen::default();
    wrong.step = Step::Company;
    wrong.name = "Godavari Drip & Pump Works".to_owned();
    wrong.gstin = "27ZZZZZ0000Z1ZQ".to_owned();
    wrong.gstin_seen = wrong.gstin.clone();
    wrong.state = Some(StateCode::new(24).unwrap());
    let (drawn, _) = run(&mut wrong, &mut ctx, 3, |_| raw());
    assert!(bad_outlines(&drawn, bad) >= 2, "GSTIN and State outlined");
    let mut right = SetupScreen::default();
    right.step = Step::Company;
    right.name = "Godavari Drip & Pump Works".to_owned();
    right.gstin = "27ZZZZZ0000Z1ZQ".to_owned();
    right.gstin_seen = right.gstin.clone();
    right.state = Some(StateCode::new(27).unwrap());
    let (drawn, _) = run(&mut right, &mut ctx, 3, |_| raw());
    assert_eq!(bad_outlines(&drawn, bad), 0);
}

#[test]
fn the_wizard_draws_no_breadcrumbs_over_its_card() {
    // provenance: index.html `SCREENS.setup` draws the card with no crumbs, however it was opened.
    assert!(!SetupScreen::default().shows_breadcrumbs());
}
