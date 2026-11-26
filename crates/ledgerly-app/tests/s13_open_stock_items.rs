//! The provisional banner's "Open stock items" button opens the stock items master, as the mock's
//! button does (index.html:1661 and 2237: `data-go` = `hashFor('masters','items')`), not another
//! report.
//!
//! Expected values are `derived`: the mock's target is the masters screen on its stock items tab,
//! whose cursor label with no row chosen is the kind's plural, "Stock items".
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::Duration;

use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_api::ReportKind;
use ledgerly_app::shell::navigate::report_screen;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

#[test]
fn open_stock_items_pushes_the_stock_items_master() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s13_open_stock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (api, _path) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = report_screen(ReportKind::ProfitAndLoss, &books).unwrap();
    let mut ctx = AppCtx {
        books: Some(books),
        api: Some(worker),
        ..AppCtx::default()
    };
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);
    let input = || RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
        ..RawInput::default()
    };
    screen.open_stock_items();
    let mut pushed = None;
    for _ in 0..4000 {
        let mut out = egui_ctx.run_ui(input(), |ui| {
            if let ScreenAction::Push(next) = screen.ui(ui, &mut ctx) {
                pushed = Some(next);
            }
        });
        out.textures_delta.clear();
        if pushed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let next = pushed.expect("the button opens a screen");
    assert_eq!(next.title(&ledgerly_app::i18n::Tr::default()), "Masters");
    assert_eq!(next.cursor_label(&ctx).as_deref(), Some("Stock items"));
}
