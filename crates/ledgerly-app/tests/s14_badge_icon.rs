//! A badge can draw its own icon, as the mock's `badge('Missing: add HSN','b-bad','alert')`
//! (index.html:1882) draws the triangle, not the red badge's cross.
//!
//! Expected values are `derived`: the strokes `icon::paint` draws for `Icon::Alert` into the
//! badge's icon box (left padding 7, icon box 10, centred on the line), as in `s13_lock_badge`.

use egui::{Context, Event, Pos2, RawInput, Rect, Shape, Ui, Vec2};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::ui::badge::{self, BadgeKind};
use ledgerly_app::ui::icon::{self, Icon};

fn shapes_of(kind: ThemeKind, body: impl FnMut(&mut Ui)) -> Vec<Shape> {
    let ctx = Context::default();
    theme::apply(&ctx, kind);
    let input = || RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
        events: Vec::<Event>::new(),
        ..RawInput::default()
    };
    ctx.run_ui(input(), |_ui| {}).drop_without_applying_deltas();
    let mut body = body;
    let mut out = ctx.run_ui(input(), |ui| body(ui));
    out.textures_delta.clear();
    out.shapes.into_iter().map(|c| c.shape).collect()
}

#[test]
fn a_badge_with_the_alert_icon_draws_the_triangle() {
    for kind in ThemeKind::ALL {
        let mut placed = Rect::NOTHING;
        let drawn = shapes_of(kind, |ui| {
            placed =
                badge::badge_with_icon(ui, BadgeKind::Bad, "Missing: add HSN", Some(Icon::Alert))
                    .rect;
        });
        // High contrast outlines the badge, and the outline is outside the 7 point padding.
        let outline = if kind == ThemeKind::HighContrast {
            theme::border_width(ThemeKind::HighContrast)
        } else {
            0.0
        };
        let rect = Rect::from_center_size(
            Pos2::new(placed.left() + outline + 7.0 + 5.0, placed.center().y),
            Vec2::splat(10.0),
        );
        let t = theme::tokens(kind);
        let (_, fg) = badge::colors(BadgeKind::Bad, &t);
        let want = shapes_of(kind, |ui| icon::paint(ui.painter(), rect, Icon::Alert, fg));
        assert_ne!(want.len(), 0, "the alert glyph draws nothing");
        for shape in &want {
            assert!(
                drawn.contains(shape),
                "{kind:?}: alert stroke missing: {shape:?}"
            );
        }
    }
}
