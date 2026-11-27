//! S22: nothing is cut off on the right at the window sizes people use.
//!
//! The owner's screenshot of 1.0.0 (a window about 2000 points wide, the Payment voucher of the
//! sample company) shows the Mode field, the Group column of the particulars and the right end of
//! the top bar running past the right edge. This lays the real shell out at the minimum window
//! size and at common screen sizes, at scale factors 1, 1.25, 1.5 and 2, and checks that every
//! painted text and frame ends inside the window.
//!
//! Provenance: derived. The minimum window is the one `shell::run` asks for (800 x 520 points);
//! the sizes are common monitor resolutions (1366x768, 1920x1080, 2560x1440) and the owner's
//! window (about 2000 x 1000). A shape is inside when its right edge is at most the window's right
//! edge (half a point allowed for anti-aliasing).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::epaint::ClippedShape;
use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Shape, vec2};
use ledgerly_api::Api;
use ledgerly_app::shell::submit;
use ledgerly_app::shell::{GatewayScreen, Shell};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::Clock;
use time::OffsetDateTime;

fn sample(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn input(screen: Rect, ppp: f32, events: Vec<Event>) -> RawInput {
    let mut raw = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    raw.viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(ppp);
    raw
}

fn frame(ctx: &egui::Context, shell: &mut Shell, raw: RawInput) -> Vec<ClippedShape> {
    let mut out = ctx.run_ui(raw, |ui| shell.show(ui));
    out.textures_delta.clear();
    out.shapes
}

/// Every painted text with its rect, and every filled or stroked rectangle.
fn painted(shapes: &[ClippedShape]) -> Vec<(String, Rect)> {
    fn walk(shape: &Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            Shape::Vec(items) => items.iter().for_each(|s| walk(s, out)),
            Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect())),
            Shape::Rect(r) => out.push(("<frame>".to_owned(), r.visual_bounding_rect())),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

/// The shell on the sample company's Payment voucher (F5), laid out at `size` points with
/// `ppp` physical pixels per point, after the books have arrived.
fn payment_shapes(name: &str, size: (f32, f32), ppp: f32) -> (Rect, Vec<ClippedShape>) {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(size.0, size.1));
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(sample(name)).unwrap());
    submit::note_changed();
    let start = Instant::now();
    while shell.app_ctx_mut().books.is_none() {
        let _shapes = frame(&ctx, &mut shell, input(screen, ppp, Vec::new()));
        assert!(start.elapsed() < Duration::from_secs(10), "books arrive");
        std::thread::sleep(Duration::from_millis(5));
    }
    let f5 = Event::Key {
        key: Key::F5,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let _shapes = frame(&ctx, &mut shell, input(screen, ppp, vec![f5]));
    let mut shapes = Vec::new();
    for _ in 0..6 {
        shapes = frame(&ctx, &mut shell, input(screen, ppp, Vec::new()));
    }
    assert_eq!(shell.top_key_bar_name(), "Payment", "F5 opened the payment");
    (screen, shapes)
}

fn assert_fits(name: &str, size: (f32, f32), ppp: f32) {
    let (screen, shapes) = payment_shapes(name, size, ppp);
    let shown = painted(&shapes);
    for label in ["Mode", "Group", "Amount", "Narration"] {
        assert!(
            shown.iter().any(|(t, _)| t == label),
            "{size:?} at {ppp}: '{label}' is painted"
        );
    }
    let over: Vec<String> = shown
        .iter()
        .filter(|(_, r)| r.is_finite() && r.max.x > screen.max.x + 0.5)
        .map(|(t, r)| format!("'{t}' ends at x={:.1}", r.max.x))
        .collect();
    assert!(
        over.is_empty(),
        "{size:?} points at {ppp}x: past the right edge {:.0}: {over:?}",
        screen.max.x
    );
    // The top bar (48 points high): no two words sit on top of each other, so a chip never runs
    // under the search chip or the right side.
    let top: Vec<&(String, Rect)> = shown
        .iter()
        .filter(|(t, r)| t != "<frame>" && r.is_finite() && r.max.y <= 48.0)
        .collect();
    for (i, (a, ra)) in top.iter().enumerate() {
        for (b, rb) in top.iter().skip(i + 1) {
            assert!(
                !ra.shrink(0.5).intersects(rb.shrink(0.5)),
                "{size:?} at {ppp}: top bar words '{a}' and '{b}' overlap"
            );
        }
    }
}

#[test]
fn the_payment_fits_the_minimum_window() {
    assert_fits("s22-fit-min.ledgerly", (800.0, 520.0), 1.0);
}

#[test]
fn the_payment_fits_1366_by_768() {
    assert_fits("s22-fit-1366.ledgerly", (1366.0, 768.0), 1.0);
}

#[test]
fn the_payment_fits_1920_by_1080() {
    assert_fits("s22-fit-1920.ledgerly", (1920.0, 1080.0), 1.0);
}

#[test]
fn the_payment_fits_the_owners_2000_wide_window() {
    assert_fits("s22-fit-2000.ledgerly", (2000.0, 1000.0), 1.0);
}

#[test]
fn the_payment_fits_2560_by_1440() {
    assert_fits("s22-fit-2560.ledgerly", (2560.0, 1440.0), 1.0);
}

#[test]
fn the_payment_fits_scaled_screens() {
    // 2560x1440 pixels at 1.25 is 2048x1152 points; 1920x1080 at 1.5 is 1280x720; 2560x1440 at 2
    // is 1280x720; 2000x1000 at 1.25 is 1600x800.
    assert_fits("s22-fit-125.ledgerly", (2048.0, 1152.0), 1.25);
    assert_fits("s22-fit-150.ledgerly", (1280.0, 720.0), 1.5);
    assert_fits("s22-fit-200.ledgerly", (1280.0, 720.0), 2.0);
    assert_fits("s22-fit-2000-125.ledgerly", (1600.0, 800.0), 1.25);
}

#[test]
fn the_payment_fits_after_the_window_shrinks() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(sample("s22-fit-shrink.ledgerly")).unwrap());
    submit::note_changed();
    let wide = Rect::from_min_size(Pos2::ZERO, vec2(2560.0, 1440.0));
    let start = Instant::now();
    while shell.app_ctx_mut().books.is_none() {
        let _shapes = frame(&ctx, &mut shell, input(wide, 1.0, Vec::new()));
        assert!(start.elapsed() < Duration::from_secs(10), "books arrive");
        std::thread::sleep(Duration::from_millis(5));
    }
    let f5 = Event::Key {
        key: Key::F5,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let _shapes = frame(&ctx, &mut shell, input(wide, 1.0, vec![f5]));
    for _ in 0..4 {
        let _shapes = frame(&ctx, &mut shell, input(wide, 1.0, Vec::new()));
    }
    for size in [(2000.0, 1000.0), (1366.0, 768.0), (800.0, 520.0)] {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(size.0, size.1));
        let mut shapes = Vec::new();
        for _ in 0..4 {
            shapes = frame(&ctx, &mut shell, input(screen, 1.0, Vec::new()));
        }
        let over: Vec<String> = painted(&shapes)
            .iter()
            .filter(|(_, r)| r.is_finite() && r.max.x > screen.max.x + 0.5)
            .map(|(t, r)| format!("'{t}' ends at x={:.1}", r.max.x))
            .collect();
        assert!(over.is_empty(), "after shrinking to {size:?}: {over:?}");
    }
}
