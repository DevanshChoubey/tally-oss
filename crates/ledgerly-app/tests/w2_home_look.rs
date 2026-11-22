//! Home drawn to the mock (`design/mock-v2/index.html`, `SCREENS.home`): the line height of
//! running text, the lower row's column split and the tile figure's small rupee sign.

use egui::{Align, Layout, RawInput, Rect, pos2, vec2};
use ledgerly_app::screens::home::{figure_parts, main_columns};
use ledgerly_app::theme::{self, TEXT_BODY, TEXT_META, ThemeKind};
use ledgerly_app::ui::{line_height, para};

#[test]
fn running_text_uses_the_mocks_line_height() {
    // provenance: derived from the mock's `body{line-height:1.45}` (index.html line 27): 13 px
    // body text is 13 x 1.45 = 18.85, 12 px meta text 17.4; egui lays rows on whole points, so
    // 19 and 17. Catches text drawn at the font's own tighter row height.
    assert!((line_height(TEXT_BODY) - 19.0).abs() < f32::EPSILON);
    assert!((line_height(TEXT_META) - 17.0).abs() < f32::EPSILON);
}

fn para_rect(width: f32, text: &str) -> Rect {
    let ctx = egui::Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    let mut rect = Rect::NOTHING;
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        ui.allocate_ui_with_layout(
            vec2(width, 400.0),
            Layout::top_down_justified(Align::Min),
            |ui| {
                rect = para(ui, text, TEXT_BODY, egui::Color32::BLACK).rect;
            },
        );
    });
    out.drop_without_applying_deltas();
    rect
}

#[test]
fn a_paragraph_is_one_line_height_per_row() {
    // provenance: derived from the same rule: one row is 19 points, two rows 38. Catches a
    // paragraph that ignores the line height, or one that does not wrap.
    let one = para_rect(600.0, "Short line.");
    assert!(
        (one.height() - 19.0).abs() < 0.5,
        "one row: {}",
        one.height()
    );
    let two = para_rect(
        200.0,
        "A sentence long enough to need a second row at two hundred points.",
    );
    assert!(two.height() >= 38.0 - 0.5, "wrapped: {}", two.height());
    assert!(
        ((two.height() / 19.0).round() * 19.0 - two.height()).abs() < 0.5,
        "rows are whole line heights: {}",
        two.height()
    );
}

#[test]
fn a_paragraph_in_a_justified_column_is_not_stretched() {
    // provenance: derived. The mock's text is ragged right (no `text-align:justify`). egui
    // justifies a label in a justified layout, which spreads the words of every full row; a
    // ragged paragraph's first row is narrower than the column. Catches the stretched rows seen
    // in the 2026-11-22 Home capture.
    let rect = para_rect(
        200.0,
        "GST cash payable: the engine gives the GSTR-3B working but no payable after set-off.",
    );
    assert!(rect.width() <= 200.0 + 0.5);
    // A justified paragraph fills the column exactly on every row but the last; the rect of a
    // ragged one is the widest row, which for this text falls short of the full width.
    assert!(
        rect.width() < 199.5,
        "rows were stretched to {}",
        rect.width()
    );
}

#[test]
fn the_lower_row_splits_like_the_mocks_main_grid() {
    // provenance: derived from the mock's `.g-main{grid-template-columns:minmax(0,1.55fr)
    // minmax(320px,1fr)}` and `.grid{gap:12px}` (index.html lines 138, 136). At 1048 points:
    // 1036 to share, left 1036 x 1.55 / 2.55 = 629.73, right 406.27. At 720 the right column
    // would be 282.35, under its 320 floor, so it takes 320 and the left 388. Below 720 the
    // panels stack (WIDE_MIN_WIDTH), and there is no split.
    let (l, r) = main_columns(1048.0).unwrap_or_default();
    assert!((l - 629.73).abs() < 0.01, "left {l}");
    assert!((r - 406.27).abs() < 0.01, "right {r}");
    let (l, r) = main_columns(720.0).unwrap_or_default();
    assert!(
        (l - 388.0).abs() < 0.01 && (r - 320.0).abs() < 0.01,
        "{l} {r}"
    );
    assert_eq!(main_columns(719.0), None);
}

#[test]
fn a_tile_figure_draws_its_rupee_sign_apart() {
    // provenance: derived from the mock's tile, `<div class="fig"><small>₹</small>...`, with
    // `.kpi .fig small{font-size:16px}` (index.html lines 184, 1061). The sign is split off for
    // drawing only; the digits are the engine's text unchanged. Words have no sign.
    assert_eq!(
        figure_parts("\u{20B9}25,000.00"),
        (Some("\u{20B9}"), "25,000.00")
    );
    assert_eq!(
        figure_parts("Not supported yet"),
        (None, "Not supported yet")
    );
}
