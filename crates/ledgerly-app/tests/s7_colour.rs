//! Card S7 (accessibility): no status is carried by colour alone.
//!
//! Audit (read from the source, 2026-11-24): each listed signal also carries a word.
//! - debit/credit: amounts read "Dr" or "Cr" (`screens/report_view/text.rs`, `voucher_entry/display.rs`).
//! - bank match: the badge reads "Matched", "Not matched" or the suggestion kind (`screens/bank/view.rs` `match_badge`).
//! - GST mismatch and review state: the badge reads the bucket word (`screens/gst/draw.rs` `bucket_badge`).
//! - errors and warnings: a note paints its text beside its colour (`ui/note.rs`).
//! - negative amounts: the side word, as above.
//!
//! The tests below read the AccessKit tree and check that the words of each badge and note kind
//! reach it, so a screen reader and a colour-blind reader get the same status. The expected words are
//! the words the code draws (derived from the source lines above, not from this test's output).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::accesskit::{Role, TreeUpdate};
use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::ui::badge::{BadgeKind, badge};
use ledgerly_app::ui::note::{NoteKind, note};

fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

fn tree(ctx: &Context, frames: usize, mut body: impl FnMut(&mut Ui)) -> TreeUpdate {
    let mut last = None;
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(input, &mut body);
        last = out.platform_output.accesskit_update.take();
        out.drop_without_applying_deltas();
    }
    last.expect("AccessKit is enabled")
}

/// Whether the tree has a label node whose text is exactly `word`. egui stores a `Label`'s text
/// as the node's value (`Response::fill_accesskit_node_from_widget_info`), not its name.
fn has_text(update: &TreeUpdate, word: &str) -> bool {
    update
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Label && node.value() == Some(word))
}

#[test]
fn every_badge_kind_reads_its_word() {
    // derived: each badge is a label with its word (`ui/badge.rs` `badge`): the words here are the
    // ones the bank, GST and masters screens pass in (`match_badge`, `bucket_badge`).
    let ctx = context();
    let cases = [
        (BadgeKind::Ok, "Matched"),
        (BadgeKind::Warn, "Not matched"),
        (BadgeKind::Bad, "Mismatch"),
        (BadgeKind::Accent, "Manual"),
        (BadgeKind::Ai, "AI suggested"),
    ];
    let update = tree(&ctx, 2, |ui| {
        for (kind, word) in cases {
            badge(ui, kind, word);
        }
    });
    for (_, word) in cases {
        assert!(
            has_text(&update, word),
            "the badge word {word:?} reaches the tree"
        );
    }
}

#[test]
fn every_note_kind_reads_its_text() {
    // derived: a note paints its text next to its colour (`ui/note.rs` `note`); the errors and
    // warnings on the GST and setup screens are notes of kind Warn and Bad.
    let ctx = context();
    let cases = [
        (NoteKind::Info, "Tax is not computed yet for this invoice."),
        (NoteKind::Ok, "Check character OK"),
        (NoteKind::Warn, "Not supported yet: rate (no rate on file)"),
        (NoteKind::Bad, "The tax engine failed: no state code"),
    ];
    let update = tree(&ctx, 2, |ui| {
        for (kind, text) in cases {
            note(ui, kind, text);
        }
    });
    for (_, text) in cases {
        assert!(
            has_text(&update, text),
            "the note text {text:?} reaches the tree"
        );
    }
}
