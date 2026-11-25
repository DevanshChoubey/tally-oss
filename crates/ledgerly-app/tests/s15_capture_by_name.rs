#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

//! Card S15: the capture harness clicks a widget by its accessible name, never by a fixed point.
//!
//! derived: egui gives every button and text field the name it was built with (the text of the
//! button, or the name the screen passes) once AccessKit is on; the harness reads those names at
//! the end of a pass and aims the click at the middle of the named widget's area.

use egui::{Context, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::ui::capture::{
    FindError, NamedRect, collect_named, find_named, find_named_start,
};

/// One pass with two buttons at known places and a third named like the first, when asked.
fn run(twin: bool) -> (Vec<NamedRect>, Vec<Rect>) {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
        ..RawInput::default()
    };
    let mut named = Vec::new();
    let mut rects = Vec::new();
    // The first pass only lays the widgets out; the second is the one read.
    for _ in 0..2 {
        named.clear();
        rects.clear();
        let output = ctx.run_ui(input.clone(), |ui| {
            ui.set_min_size(Vec2::new(600.0, 400.0));
            rects.push(ui.button("Compare").rect);
            rects.push(ui.button("Dismiss").rect);
            if twin {
                rects.push(ui.button("Compare").rect);
            }
            named = collect_named(ui.ctx());
        });
        output.drop_without_applying_deltas();
    }
    (named, rects)
}

#[test]
fn a_button_is_found_by_its_name_at_the_middle_of_its_area() {
    let (named, rects) = run(false);
    let at = find_named(&named, "Dismiss").unwrap();
    assert_eq!(at, rects[1].center());
    let at = find_named(&named, "Compare").unwrap();
    assert_eq!(at, rects[0].center());
}

#[test]
fn a_name_nothing_has_is_refused_and_the_names_there_are_listed() {
    let (named, _) = run(false);
    let Err(FindError::NotFound { name, known }) = find_named(&named, "Nothing") else {
        panic!("expected NotFound");
    };
    assert_eq!(name, "Nothing");
    assert!(known.contains(&"Compare".to_owned()), "{known:?}");
    assert!(known.contains(&"Dismiss".to_owned()), "{known:?}");
}

#[test]
fn a_name_two_widgets_share_is_refused_as_ambiguous() {
    let (named, _) = run(true);
    assert_eq!(
        find_named(&named, "Compare"),
        Err(FindError::Ambiguous {
            name: "Compare".to_owned(),
            count: 2
        })
    );
}

#[test]
fn a_widget_can_be_aimed_at_by_its_left_end_so_a_list_over_its_middle_is_missed() {
    let (named, rects) = run(false);
    let at = find_named_start(&named, "Dismiss").unwrap();
    // derived: the point is inside the widget, at its vertical middle, near its left edge.
    assert!(rects[1].contains(at));
    assert_eq!(at.y, rects[1].center().y);
    assert!(at.x - rects[1].left() < 20.0, "{at:?} {:?}", rects[1]);
    assert!(at.x < rects[1].center().x);
}
