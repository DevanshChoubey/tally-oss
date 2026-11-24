//! s9 GST returns: with the section table loaded, the default view is the mock's (tiles, section
//! table and the "What happens next" card). The voucher table that sits behind the sections is
//! folded away until the person asks for it.
//!
//! Provenance: derived from the design mock (`design/mock-v2/index.html`, `SCREENS.gst` and
//! `design/mock-v2/captures/states/light/gst.gstr1.webp`): the GSTR-1 tab shows the four tiles, the
//! six-row section table and the side card, with no voucher table under them. The engine has
//! vouchers behind each section, so the app keeps them one click away.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Pos2, RawInput, Rect, vec2};
use ledgerly_api::{Bucket as ApiBucket, Gstr1Section};
use ledgerly_app::screens::gst::{Bucket, ExceptionItem, Feed, GstScreen, GstTab};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Paise, VoucherId};

fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Band(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn loaded() -> GstScreen {
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    s.set_sections(vec![Gstr1Section {
        code: "4A",
        name: "B2B invoices",
        records: 1,
        taxable: Some(Paise::ZERO),
        tax: None,
        status: ApiBucket::Ready,
        note: None,
    }]);
    s.set_gstr1(Feed::Loaded(vec![ExceptionItem {
        bucket: Bucket::Ready,
        title: "Voucher 22".to_owned(),
        detail: "Its figures are in the return.".to_owned(),
        voucher: Some(VoucherId(22)),
    }]));
    s
}

fn painted(s: &mut GstScreen) -> Vec<String> {
    let ctx = Context::default();
    let mut app = AppCtx::default();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _ = s.ui(ui, &mut app);
    });
    let all = texts(&out.shapes);
    out.drop_without_applying_deltas();
    all
}

#[test]
fn the_voucher_table_is_folded_by_default_when_the_section_table_is_shown() {
    let mut s = loaded();
    assert!(!s.vouchers_shown(), "folded until asked");
    let text = painted(&mut s).join("\n");
    assert!(
        text.contains("4A B2B invoices"),
        "the section table shows: {text}"
    );
    assert!(
        !text.contains("Voucher 22"),
        "no voucher rows in the default view: {text}"
    );
    assert!(
        text.contains("Show vouchers"),
        "a way to unfold is offered: {text}"
    );
}

#[test]
fn unfolding_shows_the_vouchers_behind_the_sections() {
    let mut s = loaded();
    s.show_vouchers(true);
    let text = painted(&mut s).join("\n");
    assert!(text.contains("Vouchers behind the sections"), "{text}");
    assert!(text.contains("Voucher 22"), "{text}");
}

#[test]
fn without_a_section_table_the_voucher_table_is_the_only_view() {
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    s.set_gstr1(Feed::Loaded(vec![ExceptionItem {
        bucket: Bucket::Ready,
        title: "Voucher 22".to_owned(),
        detail: "x".to_owned(),
        voucher: Some(VoucherId(22)),
    }]));
    let text = painted(&mut s).join("\n");
    assert!(text.contains("Voucher 22"), "{text}");
}
