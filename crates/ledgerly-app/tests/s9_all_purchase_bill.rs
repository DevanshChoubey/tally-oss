//! s9 Purchase from a bill photo: the image column's empty state is centred in its own column.
//!
//! Provenance: derived from the design mock (`design/mock-v2/index.html:1432`,
//! `grid-template-columns:400px ...`): the image column is 400 points wide at the left edge of the
//! page's content, so whatever stands in for the bill sits inside those 400 points. A capture on
//! 2026-11-25 showed "No bill photo" centred 90 points to the right of the column's middle, over
//! the gap between the columns.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Pos2, RawInput, Rect, vec2};
use ledgerly_app::screens::purchase_bill::{NO_IMAGE_TITLE, PAGE_TITLE, PurchaseBillScreen};
use ledgerly_app::{AppCtx, Screen};

/// The left edge and the horizontal middle of the first text shape reading `text`.
fn spot(shapes: &[ClippedShape], text: &str) -> Option<(f32, f32)> {
    fn walk(shape: &Shape, text: &str) -> Option<(f32, f32)> {
        match shape {
            Shape::Text(t) if t.galley.text() == text => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                Some((rect.left(), rect.center().x))
            }
            Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            Shape::Text(_)
            | Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => None,
        }
    }
    shapes.iter().find_map(|c| walk(&c.shape, text))
}

#[test]
fn the_empty_image_state_sits_in_the_400_point_image_column() {
    let mut screen = PurchaseBillScreen::default();
    let mut app = AppCtx::default();
    let ctx = Context::default();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let (page_left, _) = spot(&out.shapes, PAGE_TITLE).expect("the page title is drawn");
    let (_, middle) = spot(&out.shapes, NO_IMAGE_TITLE).expect("the empty state is drawn");
    out.drop_without_applying_deltas();
    assert!(
        (page_left..=page_left + 400.0).contains(&middle),
        "the empty state is centred at {middle}, outside the column {page_left}..{}",
        page_left + 400.0
    );
    let column_middle = page_left + 200.0;
    assert!(
        (middle - column_middle).abs() < 8.0,
        "centred at {middle}, the column's middle is {column_middle}"
    );
}
