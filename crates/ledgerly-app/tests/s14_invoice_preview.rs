//! Card S14: the sales invoice (and with it any credit or debit note) prints through the same preview as the
//! reports: Ctrl+P on the voucher shows the engine's pages, Ctrl+P there saves them, Esc closes.
//!
//! Provenance. Flow: print, then preview, then print (<https://help.tallysolutions.com/print-invoices-reports/>,
//! read 2026-11-25). Documents: derived from the print crate's contract
//! (`ledgerly_print::pdf` module docs): the first page of a tax document carries its title, "Tax
//! Invoice", "Credit Note" or "Debit Note", and its number. The sample company is the mock's
//! (SI/0412 is its sales invoice).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, PageItem};
use ledgerly_app::screens::report_view::VoucherViewScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, Outcome, VoucherId};
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

fn texts(doc: &ledgerly_api::PrintedDocument) -> Vec<String> {
    doc.pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter_map(|i| match i {
            PageItem::Text { text, .. } => Some(text.clone()),
            PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn each_tax_document_previews_saves_the_engines_bytes_and_closes_with_esc() {
    // The sample books hold no credit or debit note with a GST block, so the sales invoice is the
    // tax document run here; a credit or debit note takes the same path (`print_voucher_document`
    // asks the print crate, which `pr2_pdf` tests for each kind).
    {
        let (prefix, title) = ("SI/", "Tax Invoice");
        let api = sample(&format!(
            "s14_inv_{}.ledgerly",
            prefix.trim_end_matches('/')
        ));
        let (id, number) = voucher_with(&api, prefix);
        let books = api.books().unwrap().value().unwrap().clone();
        // Identity: the same sample, made again, prints to the same bytes (rule A7).
        let reference = sample(&format!(
            "s14_inv_ref_{}.ledgerly",
            prefix.trim_end_matches('/')
        ));
        let Ok(Outcome::Value(expected)) = reference.print_voucher(id) else {
            panic!(
                "{prefix}: the sample voucher should print: {:?}",
                reference.print_voucher(id)
            );
        };
        let mut app = AppCtx {
            books: Some(books),
            api: Some(ApiWorker::spawn(api).unwrap()),
            ..AppCtx::default()
        };
        let ctx = egui::Context::default();
        let mut screen = VoucherViewScreen::new(id);
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("s14_inv_out_{}.pdf", prefix.trim_end_matches('/')));
        let _removed = std::fs::remove_file(&out);
        let asked = Arc::new(Mutex::new(0_u32));
        let (count, to) = (Arc::clone(&asked), out.clone());
        screen.set_print_picker(Box::new(move |_name: &str| {
            *count.lock().unwrap() += 1;
            Some(to.clone())
        }));

        let _left = frame(&ctx, &mut screen, &mut app, key(Key::P, Modifiers::COMMAND));
        until(&ctx, &mut screen, &mut app, |s| s.print_preview().is_some());
        let doc = screen.print_preview().unwrap();
        let all = texts(doc);
        assert!(
            all.iter().any(|t| t == title),
            "{prefix}: no {title:?} in {all:?}"
        );
        assert!(
            all.iter().any(|t| t.contains(&number)),
            "{prefix}: no {number} in the pages"
        );
        assert_eq!(
            doc.pdf, expected,
            "{prefix}: the preview holds the engine's PDF"
        );
        assert_eq!(*asked.lock().unwrap(), 0, "{prefix}: no dialog before Save");

        // Esc closes and stays on the voucher.
        let left = frame(
            &ctx,
            &mut screen,
            &mut app,
            key(Key::Escape, Modifiers::NONE),
        );
        assert!(
            !left && screen.print_preview().is_none(),
            "{prefix}: Esc closes"
        );
        assert!(!out.exists());

        // Open again, then Ctrl+P saves the same bytes.
        let _left = frame(&ctx, &mut screen, &mut app, key(Key::P, Modifiers::COMMAND));
        until(&ctx, &mut screen, &mut app, |s| s.print_preview().is_some());
        let _left = frame(&ctx, &mut screen, &mut app, key(Key::P, Modifiers::COMMAND));
        until(&ctx, &mut screen, &mut app, |s| {
            s.print_preview().is_none() && s.notice().is_some()
        });
        assert_eq!(*asked.lock().unwrap(), 1);
        assert_eq!(
            std::fs::read(&out).unwrap(),
            expected,
            "{prefix}: saved bytes"
        );
    }
}
