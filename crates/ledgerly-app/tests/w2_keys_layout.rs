//! Wave 2 item S-J: the keyboard map's cards sit side by side in three columns, as the mock's
//! `keys.classic` and `keys.standard` states show them (`design/mock-v2/captures/states/light/
//! keys.classic.webp`): no card overlaps another and none runs past the content.
//!
//! Provenance: derived. The content widths are the home tests' (`w2_home.rs`): 1024 points in a
//! 1280 px window and 768 in a 1024 px one (window less the 208-point sidebar and 2 x 24 margins).
#![allow(
    clippy::unwrap_used,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap; layout checks add points"
)]

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::keymap_map::KeymapScreen;
use ledgerly_app::{AppCtx, Screen};

const WINDOW_HEIGHT: f32 = 2000.0;

/// Draws the map three frames in a window `content_width` points wide (pressing `view` first:
/// `Num1` classic, `Num2` standard) and returns the card rectangles of the last frame.
fn drawn_cards(content_width: f32, view: Key) -> Vec<Rect> {
    let ctx = egui::Context::default();
    let mut screen = KeymapScreen::default();
    let mut app = AppCtx::default();
    for frame in 0..3 {
        let events = if frame == 0 {
            vec![Event::Key {
                key: view,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]
        } else {
            Vec::new()
        };
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(content_width, WINDOW_HEIGHT),
            )),
            events,
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            ui.set_max_width(content_width);
            let _action = screen.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    }
    screen.card_rects().to_vec()
}

fn assert_cards_apart(content_width: f32, view: Key) {
    let cards = drawn_cards(content_width, view);
    // derived: five groups (four from the mock, then "Other actions"), one card each.
    assert_eq!(cards.len(), 5, "{cards:?}");
    for (i, a) in cards.iter().enumerate() {
        assert!(
            a.right() <= content_width + 0.5,
            "card {i} runs past the content ({content_width}): {a:?}"
        );
        for (j, b) in cards.iter().enumerate().skip(i + 1) {
            assert!(
                !a.shrink(0.5).intersects(b.shrink(0.5)),
                "cards {i} and {j} overlap at width {content_width}: {a:?} {b:?}"
            );
        }
    }
}

#[test]
fn classic_cards_do_not_overlap_at_1280() {
    assert_cards_apart(1024.0, Key::Num1);
}

#[test]
fn standard_cards_do_not_overlap_at_1280() {
    assert_cards_apart(1024.0, Key::Num2);
}

#[test]
fn standard_cards_do_not_overlap_at_1024() {
    assert_cards_apart(768.0, Key::Num2);
}
