//! The "Locked" badge on an invoice date draws the lock glyph, as the mock does
//! (`badge('Locked','b-bad','lock')`, index.html:1359), not the red badge's cross.
//!
//! Expected values are `derived`: the lock icon's strokes are the ones `icon::paint` draws for
//! `Icon::Lock` into the badge's icon box (left padding 7, icon box 10, centred on the line).

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
fn locked_badge_draws_the_lock_glyph() {
    for kind in ThemeKind::ALL {
        let (left, y) = (100.0_f32, 50.0_f32);
        let drawn = shapes_of(kind, |ui| {
            badge::paint_badge_with_icon(ui, left, y, BadgeKind::Bad, "Locked", Some(Icon::Lock));
        });
        // The icon box: 7 points of padding, a 10 point square centred on `y`.
        let rect = Rect::from_center_size(Pos2::new(left + 7.0 + 5.0, y), Vec2::splat(10.0));
        let t = theme::tokens(kind);
        let (_, fg) = badge::colors(BadgeKind::Bad, &t);
        let want = shapes_of(kind, |ui| icon::paint(ui.painter(), rect, Icon::Lock, fg));
        assert_ne!(want.len(), 0, "the lock glyph draws nothing");
        for shape in &want {
            assert!(
                drawn.contains(shape),
                "{kind:?}: lock stroke missing: {shape:?}"
            );
        }
    }
}
