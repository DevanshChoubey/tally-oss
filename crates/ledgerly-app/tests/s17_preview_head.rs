//! S17: the print preview of a saved voucher names it as the entry screen does ("Sales invoice
//! SI/0412"), says what was saved, and keeps the plain voucher view as it was.
//!
//! Provenance: the mock's `sales-invoice.print-preview` (index.html `addPrintPreview`: crumb
//! "Sales invoice", head "Sales invoice SI/0412", subtitle "Saved · Shree Ganesh Traders · 14 Nov
//! 2026"). The sample company is the mock's: SI/0412 is its sales invoice, dated 14 Nov 2026.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::Api;
use ledgerly_app::screens::report_view::VoucherViewScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, VoucherId};
use time::OffsetDateTime;
fn sample(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

fn frame(
    ctx: &egui::Context,
    screen: &mut VoucherViewScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> bool {
    let mut left = false;
    let out = ctx.run_ui(input, |ui| {
        left = !matches!(screen.ui(ui, app), ScreenAction::Stay);
    });
    out.drop_without_applying_deltas();
    left
}

fn until(
    ctx: &egui::Context,
    screen: &mut VoucherViewScreen,
    app: &mut AppCtx,
    done: impl Fn(&VoucherViewScreen) -> bool,
) {
    for _ in 0..2000 {
        let _left = frame(ctx, screen, app, RawInput::default());
        if done(screen) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the screen did not settle within the bound");
}

/// ones that print), and its number.
/// The id of the first voucher numbered with `prefix` in the sample that has a GST block (the
/// ones that print), and its number.
fn voucher_with(api: &Api, prefix: &str) -> (VoucherId, String) {
    let books = api.books().unwrap().value().unwrap();
    let v = books
        .vouchers
        .values()
        .find(|v| v.gst.is_some() && v.number.as_deref().is_some_and(|n| n.starts_with(prefix)))
        .unwrap_or_else(|| panic!("the sample has no voucher numbered {prefix}"));
    (v.id, v.number.clone().unwrap())
}

#[test]
fn the_preview_of_a_sales_invoice_is_headed_with_the_type_and_number() {
    // provenance: derived from the mock (see the module docs). Before Ctrl+P the screen is the
    // voucher view: crumb "SI/0412", head "Voucher SI/0412" (index.html:1574). With the preview
    // open: crumb "Sales invoice", head "Sales invoice SI/0412", subtitle "Saved · Shree Ganesh
    // Traders · 14 Nov 2026". After Esc it is the voucher view again.
    let api = sample("s17_head.ledgerly");
    let (id, number) = voucher_with(&api, "SI/0412");
    assert_eq!(number, "SI/0412");
    let books = api.books().unwrap().value().unwrap().clone();
    let mut app = AppCtx {
        books: Some(books),
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let ctx = egui::Context::default();
    let mut screen = VoucherViewScreen::new(id);
    let _left = frame(&ctx, &mut screen, &mut app, RawInput::default());
    assert_eq!(screen.title(), "SI/0412");
    assert_eq!(screen.page_title(), "Voucher SI/0412");
    let _left = frame(&ctx, &mut screen, &mut app, key(Key::P, Modifiers::COMMAND));
    until(&ctx, &mut screen, &mut app, |s| s.print_preview().is_some());
    assert_eq!(screen.title(), "Sales invoice");
    assert_eq!(screen.page_title(), "Sales invoice SI/0412");
    assert_eq!(
        screen.subtitle().as_deref(),
        Some("Saved \u{b7} Shree Ganesh Traders \u{b7} 14 Nov 2026")
    );
    let _left = frame(
        &ctx,
        &mut screen,
        &mut app,
        key(Key::Escape, Modifiers::NONE),
    );
    assert_eq!(screen.title(), "SI/0412");
    assert_eq!(screen.page_title(), "Voucher SI/0412");
}
