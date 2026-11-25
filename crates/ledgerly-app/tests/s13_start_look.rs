//! The Start screen drawn to the mock's `SCREENS.start` (design/mock-v2/index.html, "start
//! screen, when the app opens with recent companies").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap"
)]

use egui::{Context, RawInput, Rect, pos2, vec2};
use ledgerly_app::recent::RecentCompany;
use ledgerly_app::screens::start::StartScreen;
use ledgerly_app::{AppCtx, Screen};

fn entry() -> RecentCompany {
    RecentCompany {
        path: "/books/c1.ledgerly".into(),
        name: "Godavari Drip & Pump Works".to_owned(),
        detail: "Partnership firm · Maharashtra".to_owned(),
        financial_year: "2026-27".to_owned(),
        last_opened: "21 Nov 2026".to_owned(),
    }
}

/// The right edge of the text `label` and the right edge of the widest filled rectangle (the
/// card) in one frame.
fn edges(label: &str) -> (f32, f32) {
    fn walk(shape: &egui::Shape, label: &str, text: &mut f32, card: &mut f32) {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                *text = t.pos.x + t.galley.size().x;
            }
            egui::Shape::Rect(r) if r.fill.a() > 0 => *card = card.max(r.rect.right()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, label, text, card)),
            egui::Shape::Text(_)
            | egui::Shape::Rect(_)
            | egui::Shape::Noop
            | egui::Shape::Circle(_)
            | egui::Shape::Ellipse(_)
            | egui::Shape::LineSegment { .. }
            | egui::Shape::Path(_)
            | egui::Shape::Band(_)
            | egui::Shape::Mesh(_)
            | egui::Shape::QuadraticBezier(_)
            | egui::Shape::CubicBezier(_)
            | egui::Shape::Callback(_) => {}
        }
    }
    let ctx = Context::default();
    ledgerly_app::theme::apply(&ctx, ledgerly_app::theme::ThemeKind::Light);
    let mut screen = StartScreen::new(
        std::path::PathBuf::from("/nonexistent-recent-dir"),
        vec![entry()],
    );
    let mut app = AppCtx::default();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let (mut text, mut card) = (0.0, 0.0);
    for clipped in &out.shapes {
        walk(&clipped.shape, label, &mut text, &mut card);
    }
    out.drop_without_applying_deltas();
    (text, card)
}

#[test]
fn saved_on_this_computer_sits_at_the_right_of_the_card() {
    // provenance: index.html, SCREENS.start: the heading row is `.spread` (title at the left, the
    // meta text at the right), and the meta text is followed by a check icon, so the text ends
    // one icon plus its gap (about 20 points) inside the card's padded right edge. The card is
    // 760 wide and padded 18, so the text must end within 60 points of the card's right edge,
    // and well right of its middle.
    let (text, card) = edges("Saved on this computer");
    assert!(text > 0.0, "the text was not painted");
    assert!(card - text < 60.0, "text ends {text}, card ends {card}");
}
