//! Card S7 (accessibility): Ctrl+= / Ctrl+- / Ctrl+0 scale the whole UI.
//!
//! The shell does not handle these keys itself. egui's built-in zoom (`Options::zoom_with_keyboard`,
//! on by default) consumes them at the end of each pass, so the scale lives in the egui context and
//! holds for the session, the same as the theme (which is not persisted either; see
//! `shell/mod.rs`). These tests pin that behaviour so a change to the shell's keys shows up.
//!
//! derived: egui 0.37 `gui_zoom.rs` steps the zoom by 0.1 and clamps it to 0.2..=5.0, and Ctrl+0
//! resets it to 1.0. Hand values: 1.0 -> 1.1 (Ctrl+=), 1.1 -> 1.0 (Ctrl+-), 1.1 -> 1.0 (Ctrl+0).

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::shell::Shell;
use ledgerly_app::theme::{self, ThemeKind};

fn context() -> Context {
    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

/// Presses `key` with Ctrl held, then runs one more frame: egui applies the zoom on the frame
/// after the key event, so the scale is read once that frame has run.
fn press(ctx: &Context, shell: &mut Shell, key: Key) {
    frame(ctx, shell, Some(key));
    frame(ctx, shell, None);
}

/// One frame of the shell, with `key` pressed (Ctrl held) when given.
fn frame(ctx: &Context, shell: &mut Shell, key: Option<Key>) {
    let mut events = Vec::new();
    if let Some(key) = key {
        events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        });
    }
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

#[test]
fn ctrl_equals_steps_the_zoom_up_by_a_tenth() {
    // derived: 1.0 + 0.1 = 1.1 (egui 0.37 gui_zoom.rs zoom_in).
    let ctx = context();
    let mut shell = Shell::with_welcome();
    frame(&ctx, &mut shell, None);
    assert!((ctx.zoom_factor() - 1.0).abs() < 1e-6, "starts at 100%");
    press(&ctx, &mut shell, Key::Equals);
    assert!(
        (ctx.zoom_factor() - 1.1).abs() < 1e-6,
        "got {}",
        ctx.zoom_factor()
    );
}

#[test]
fn ctrl_minus_steps_back_and_ctrl_zero_resets() {
    // derived: 1.1 - 0.1 = 1.0 (Ctrl+-), and Ctrl+0 resets to 1.0 from any level.
    let ctx = context();
    let mut shell = Shell::with_welcome();
    press(&ctx, &mut shell, Key::Equals);
    press(&ctx, &mut shell, Key::Minus);
    assert!(
        (ctx.zoom_factor() - 1.0).abs() < 1e-6,
        "got {}",
        ctx.zoom_factor()
    );
    press(&ctx, &mut shell, Key::Equals);
    press(&ctx, &mut shell, Key::Num0);
    assert!(
        (ctx.zoom_factor() - 1.0).abs() < 1e-6,
        "got {}",
        ctx.zoom_factor()
    );
}

#[test]
fn zoom_holds_across_frames_and_is_clamped_to_five_times() {
    // derived: the clamp is 0.2..=5.0 (egui 0.37 gui_zoom.rs MAX_ZOOM_FACTOR). 40 presses from 1.0
    // reach 5.0 and stop there.
    let ctx = context();
    let mut shell = Shell::with_welcome();
    for _ in 0..40 {
        press(&ctx, &mut shell, Key::Equals);
    }
    assert!(
        (ctx.zoom_factor() - 5.0).abs() < 1e-6,
        "got {}",
        ctx.zoom_factor()
    );
    frame(&ctx, &mut shell, None);
    frame(&ctx, &mut shell, None);
    assert!(
        (ctx.zoom_factor() - 5.0).abs() < 1e-6,
        "zoom changed with no key"
    );
}
