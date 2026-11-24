//! Theme fixes found by comparing the dark and high-contrast captures with the mock's
//! (`design/mock-v2/captures/states/{dark,hc}/`).
//!
//! Provenance. The mock sets `--bw:1px` in `:root` (`design/mock-v2/index.html:25`) and
//! `--bw:2px` in `:root[data-theme="hc"]` (index.html:56); every border named below uses
//! `var(--bw)` at the line cited next to it. The tests ask what width and token a shape was
//! painted with, never what a screen computed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap and panic; widths are compared in points"
)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::ui::button::{self, ButtonKind};
use ledgerly_app::ui::icon::Icon;
use ledgerly_app::ui::keycap;
use ledgerly_app::ui::option_card::{self, OptionCard};
use ledgerly_app::ui::switch;
use ledgerly_app::ui::table::{self, RowState};
use ledgerly_app::ui::tabs;

/// One rectangle: where, its fill, its stroke width and stroke colour.
struct PaintedRect {
    /// The shape's place in paint order.
    order: usize,
    rect: Rect,
    fill: Color32,
    width: f32,
    color: Color32,
}

/// One straight line: its two ends, width and colour.
struct Line {
    /// The shape's place in paint order.
    order: usize,
    a: Pos2,
    b: Pos2,
    width: f32,
    color: Color32,
}

/// One text galley: its text, its colour and the opacity egui paints it at.
struct Galley {
    content: String,
    color: Color32,
    opacity: f32,
}

/// What a frame painted.
#[derive(Default)]
struct Painted {
    rects: Vec<PaintedRect>,
    lines: Vec<Line>,
    texts: Vec<Galley>,
    /// Shapes walked so far.
    seen: usize,
}

impl Painted {
    fn walk(&mut self, shape: &Shape) {
        let order = self.seen;
        self.seen += 1;
        match shape {
            Shape::Rect(r) => self.rects.push(PaintedRect {
                order,
                rect: r.rect,
                fill: r.fill,
                width: r.stroke.width,
                color: r.stroke.color,
            }),
            Shape::LineSegment { points, stroke } => self.lines.push(Line {
                order,
                a: points[0],
                b: points[1],
                width: stroke.width,
                color: stroke.color,
            }),
            Shape::Text(t) => self.texts.push(Galley {
                content: t.galley.text().to_owned(),
                color: t.fallback_color,
                opacity: t.opacity_factor,
            }),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| self.walk(s)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
}

/// Draws `body` for two frames in a context with `kind` applied (the first binds the fonts) and
/// returns what the second painted.
fn paint(kind: ThemeKind, mut body: impl FnMut(&mut Ui)) -> Painted {
    let ctx = egui::Context::default();
    theme::apply(&ctx, kind);
    let input = || RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
        ..RawInput::default()
    };
    ctx.run_ui(input(), |ui| body(ui))
        .drop_without_applying_deltas();
    let out = ctx.run_ui(input(), |ui| body(ui));
    let mut painted = Painted::default();
    for ClippedShape { shape, .. } in &out.shapes {
        painted.walk(shape);
    }
    painted
}

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn keycap_border_is_the_theme_border_width() {
    // provenance: derived from index.html:75 `kbd{border:var(--bw) solid var(--border)}`; --bw is 1 px in light
    // and dark (index.html:25) and 2 px in high contrast (index.html:56).
    for (kind, bw) in [
        (ThemeKind::Light, 1.0),
        (ThemeKind::Dark, 1.0),
        (ThemeKind::HighContrast, 2.0),
    ] {
        let t = theme::tokens(kind);
        let painted = paint(kind, |ui| {
            keycap::keycap(ui, "F8");
        });
        let outline = painted
            .rects
            .iter()
            .find(|r| r.width > 0.0 && r.color == t.border)
            .expect("the keycap is outlined in the border token");
        assert!(
            same(outline.width, bw),
            "{kind:?}: keycap border {} px, mock {bw} px",
            outline.width
        );
        assert!(outline.rect.height() > 0.0);
    }
}

/// The horizontal lines a `table.t` helper paints into a 400 x 34 point line at the origin.
fn table_lines(kind: ThemeKind, draw: fn(&Ui, Rect)) -> Vec<Line> {
    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(400.0, 34.0));
    paint(kind, |ui| draw(ui, rect))
        .lines
        .into_iter()
        .filter(|l| same(l.a.y, l.b.y))
        .collect()
}

#[test]
fn table_rules_are_the_theme_border_width() {
    // provenance: derived from index.html:149 `table.t th{border-bottom:var(--bw) solid var(--border)}`,
    // index.html:150 `table.t td{border-bottom:var(--bw) solid var(--border)}` and index.html:156
    // `tr.total td{border-top:var(--bw) solid var(--text-2)}`; --bw is 1 px, 2 px in high contrast
    // (index.html:25, 56).
    for (kind, bw) in [
        (ThemeKind::Light, 1.0),
        (ThemeKind::Dark, 1.0),
        (ThemeKind::HighContrast, 2.0),
    ] {
        let t = theme::tokens(kind);
        let row = table_lines(kind, |ui, r| table::paint_row(ui, r, unselected_row()));
        let rule = row.iter().find(|l| l.color == t.border).expect("row rule");
        assert!(
            same(rule.width, bw),
            "{kind:?}: row rule {} px, mock {bw} px",
            rule.width
        );

        let head = table_lines(kind, table::paint_head_rule);
        let rule = head
            .iter()
            .find(|l| l.color == t.border)
            .expect("head rule");
        assert!(
            same(rule.width, bw),
            "{kind:?}: head rule {} px, mock {bw} px",
            rule.width
        );

        let cell = table_lines(kind, table::paint_cell_rule);
        let rule = cell
            .iter()
            .find(|l| l.color == t.border)
            .expect("cell rule");
        assert!(
            same(rule.width, bw),
            "{kind:?}: cell rule {} px, mock {bw} px",
            rule.width
        );

        let total = table_lines(kind, table::paint_total_rules);
        let top = total
            .iter()
            .filter(|l| l.color == t.text_2)
            .min_by(|a, b| a.a.y.total_cmp(&b.a.y))
            .expect("total top rule");
        assert!(
            same(top.width, bw),
            "{kind:?}: total top rule {} px, mock {bw} px",
            top.width
        );
        // The rule stays inside the line's rectangle.
        assert!(
            top.a.y - top.width / 2.0 >= -1e-3,
            "{kind:?}: top rule leaves the line"
        );
    }
}

fn unselected_row() -> RowState {
    RowState {
        selected: false,
        hovered: false,
        sub: false,
    }
}

#[test]
fn a_disabled_button_is_drawn_in_its_tokens_at_full_opacity() {
    // provenance: derived from index.html:131 `.btn[disabled]{opacity:1;background:var(--sunken);
    // border-color:var(--border);color:var(--text-3)}`: the disabled look is the tokens, not a
    // fade. egui fades a disabled Ui to `disabled_alpha` (one half), which in high contrast turns
    // the label from #E6E6E6 into a mid grey.
    for kind in ThemeKind::ALL {
        let t = theme::tokens(kind);
        let painted = paint(kind, |ui| {
            ui.add_enabled_ui(false, |ui| {
                button::button(ui, "Export JSON", Some("Ctrl+E"), ButtonKind::Secondary);
            });
        });
        let label = painted
            .texts
            .iter()
            .find(|g| g.content == "Export JSON")
            .expect("the label is drawn");
        assert_eq!(label.color, t.text_3, "{kind:?}: label colour");
        assert!(
            same(label.opacity, 1.0),
            "{kind:?}: label drawn at opacity {}",
            label.opacity
        );
    }
}

#[test]
fn an_off_switch_is_the_strong_border_colour() {
    // provenance: derived from index.html:324 `.switch{background:var(--border-strong)}` and index.html:326
    // `.switch[aria-checked="true"]{background:var(--accent)}`. In dark the plain border token
    // (#34322E) on the surface (#1C1B19) all but hides an off switch.
    for kind in ThemeKind::ALL {
        let t = theme::tokens(kind);
        for (on, expected) in [(false, t.border_strong), (true, t.accent)] {
            let painted = paint(kind, |ui| {
                switch::switch(ui, on, egui::Sense::hover());
            });
            let track = painted
                .rects
                .iter()
                .find(|r| same(r.rect.width(), switch::SWITCH_WIDTH))
                .expect("the switch track is drawn");
            assert_eq!(track.fill, expected, "{kind:?}: switch on={on}");
        }
    }
}

#[test]
fn the_selected_tab_underline_lies_over_the_rule() {
    // provenance: derived from index.html:139 `.tabs{border-bottom:var(--bw) solid var(--border)}`, index.html:140
    // `.tabs button{border-bottom:2px solid transparent;margin-bottom:-1px}` and index.html:141
    // `.tabs button[aria-selected="true"]{border-bottom-color:var(--accent)}`. The button reaches
    // 1 px into the rule, and as the later box it is painted over it: the accent's lower point
    // covers the rule's top row, and the rule's rows below that stay `border`. In high contrast
    // (--bw 2 px, index.html:56) the mock shows 2 px of accent, then 1 px of white.
    for kind in ThemeKind::ALL {
        let t = theme::tokens(kind);
        let painted = paint(kind, |ui| {
            tabs::tabs(ui, &["GSTR-1", "GSTR-3B"], 0);
        });
        let rule = painted
            .lines
            .iter()
            .find(|l| l.color == t.border && same(l.a.y, l.b.y))
            .expect("the rule under the tabs");
        let rule_top = rule.a.y - rule.width / 2.0;
        let underline = painted
            .rects
            .iter()
            .find(|r| r.fill == t.accent)
            .expect("the selected tab's underline");
        assert!(
            underline.order > rule.order,
            "{kind:?}: the underline is painted under the rule"
        );
        assert!(
            same(underline.rect.height(), 2.0),
            "{kind:?}: underline {} px, mock 2 px",
            underline.rect.height()
        );
        assert!(
            same(underline.rect.max.y, rule_top + 1.0),
            "{kind:?}: underline ends at {}, the rule's top row ends at {}",
            underline.rect.max.y,
            rule_top + 1.0
        );
    }
}

#[test]
fn a_chosen_option_card_edge_is_the_border_width_and_the_inset() {
    // provenance: derived from index.html:319 `.opt-card{border:var(--bw) solid var(--border)}` and index.html:321
    // `[aria-checked="true"]{border-color:var(--accent);box-shadow:inset 0 0 0 1px var(--accent)}`:
    // the accent edge is --bw plus 1 px, so 2 px, or 3 px in high contrast (--bw 2 px,
    // index.html:56).
    for (kind, edge) in [
        (ThemeKind::Light, 2.0),
        (ThemeKind::Dark, 2.0),
        (ThemeKind::HighContrast, 3.0),
    ] {
        let t = theme::tokens(kind);
        let painted = paint(kind, |ui| {
            let card = OptionCard::new(Icon::Upload, "Cloud", "Your own key").selected(true);
            option_card::option_card(ui, egui::Id::unique("card"), &card, 0.0);
        });
        let outline = painted
            .rects
            .iter()
            .find(|r| r.width > 0.0 && r.color == t.accent)
            .expect("the chosen card is edged in the accent");
        assert!(
            same(outline.width, edge),
            "{kind:?}: card edge {} px, mock {edge} px",
            outline.width
        );
    }
}
