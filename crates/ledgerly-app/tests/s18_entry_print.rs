//! S18: Ctrl+P on an entry screen prints the voucher it just saved.
//!
//! Provenance. The accounting product's pages say to record the voucher and then print it:
//! "Record your invoice/voucher with all the required transaction details, press Ctrl+P (Current)"
//! (help.tallysolutions.com/print-documents/, read 2026-11-26), and the print flow is print,
//! preview, then print (help.tallysolutions.com/print-invoices-reports/, read 2026-11-25). The
//! same page lists Alt+P as the Print menu (printer set-up), not as print-current, so Alt+P is not
//! bound here. Neither page says a voucher that is not yet recorded can be printed, so Ctrl+P on
//! unsaved lines asks to save first, and on a screen that has saved nothing says so. The preview
//! is headed as the mock's `journal.print-preview` is (index.html `addPrintPreview("journal",
//! ...)`): crumb = the entry screen's name, head "Journal JV/00nn", subtitle "Saved · 20 Nov 2026".
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
use ledgerly_api::{Api, PageItem};
use ledgerly_app::screens::voucher_entry::{DraftRow, LedgerRef, VoucherEntryScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, DrCr, FinancialYear};
use ledgerly_model::BaseVoucherKind;
use time::OffsetDateTime;
use time::macros::date;

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

fn frame(ctx: &egui::Context, s: &mut VoucherEntryScreen, app: &mut AppCtx, input: RawInput) {
    let out = ctx.run_ui(input, |ui| {
        let _stay: ScreenAction = s.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

fn until(
    ctx: &egui::Context,
    s: &mut VoucherEntryScreen,
    app: &mut AppCtx,
    done: impl Fn(&VoucherEntryScreen) -> bool,
) {
    for _ in 0..2000 {
        frame(ctx, s, app, RawInput::default());
        if done(s) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the screen did not settle within the bound");
}

/// A journal screen on the sample company, with the worker, and the two ledgers it will use.
fn journal(
    name: &str,
) -> (
    VoucherEntryScreen,
    AppCtx,
    ledgerly_core::LedgerId,
    ledgerly_core::LedgerId,
) {
    let api = sample(name);
    let books = api.books().unwrap().value().unwrap().clone();
    let type_id = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    let by_name = |n: &str| {
        books
            .masters
            .ledgers
            .values()
            .find(|l| l.name == n)
            .unwrap()
            .id
    };
    let (dep, plant) = (by_name("Depreciation"), by_name("Plant and machinery"));
    let fy = FinancialYear::april(2026).unwrap();
    let mut screen = VoucherEntryScreen::open(&books, type_id, fy, date!(2026 - 11 - 20))
        .unwrap()
        .value()
        .unwrap();
    screen.draft_mut().date_text = "20-11-2026".to_owned();
    let app = AppCtx {
        books: Some(books),
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    (screen, app, dep, plant)
}

fn fill(
    screen: &mut VoucherEntryScreen,
    dep: ledgerly_core::LedgerId,
    plant: ledgerly_core::LedgerId,
) {
    let row = |ledger, side, text: &str| DraftRow {
        ledger: Some(LedgerRef::Chosen(ledger)),
        ledger_text: text.to_owned(),
        side,
        amount_text: "100".to_owned(),
        bills: Vec::new(),
        bills_done: false,
    };
    screen.draft_mut().rows = vec![
        row(dep, DrCr::Dr, "Depreciation"),
        row(plant, DrCr::Cr, "Plant and machinery"),
    ];
}

#[test]
fn ctrl_p_on_a_screen_that_saved_nothing_says_so() {
    // derived: nothing was recorded here, so there is no voucher to print (help page: record the
    // voucher, then Ctrl+P). No preview opens.
    let (mut s, mut app, _dep, _plant) = journal("s18_print_none.ledgerly");
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::P, Modifiers::COMMAND));
    assert!(s.print_preview().is_none());
    assert_eq!(
        s.info_text(),
        Some("Nothing saved yet. Save the voucher with Ctrl+A, then press Ctrl+P to print it.")
    );
}

#[test]
fn ctrl_p_with_unsaved_lines_asks_to_save_first() {
    // derived: the page says to record the voucher before Ctrl+P and says nothing of printing an
    // unrecorded one, so the lines on screen are not printed.
    let (mut s, mut app, dep, plant) = journal("s18_print_unsaved.ledgerly");
    fill(&mut s, dep, plant);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::P, Modifiers::COMMAND));
    assert!(s.print_preview().is_none());
    assert_eq!(
        s.info_text(),
        Some("Save the voucher first with Ctrl+A. Ctrl+P prints a saved voucher.")
    );
}

#[test]
fn ctrl_p_after_a_save_previews_that_voucher_and_esc_closes_it() {
    // derived: after Ctrl+A the screen starts a fresh voucher; Ctrl+P prints the one just saved.
    // Head and subtitle as the mock's journal preview: crumb "Journal" (the screen's own name),
    // head "Journal <number>", subtitle "Saved · 20 Nov 2026" (the date typed, 20-11-2026).
    let (mut s, mut app, dep, plant) = journal("s18_print_saved.ledgerly");
    fill(&mut s, dep, plant);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    until(&ctx, &mut s, &mut app, |s| {
        s.info_text() == Some("Saved. Next voucher.")
    });
    frame(&ctx, &mut s, &mut app, key(Key::P, Modifiers::COMMAND));
    until(&ctx, &mut s, &mut app, |s| s.print_preview().is_some());
    assert_eq!(s.title(), "Journal");
    let number = s.page_title().strip_prefix("Journal ").unwrap().to_owned();
    assert!(number.starts_with("JV/"), "head is {number}");
    // The pages are the saved voucher's: they carry the number the head names.
    let doc = s.print_preview().unwrap();
    let on_page = doc
        .pages
        .iter()
        .flat_map(|p| p.items.iter())
        .any(|i| matches!(i, PageItem::Text { text, .. } if text.contains(&number)));
    assert!(on_page, "no page text names {number}");
    assert_eq!(s.subtitle().as_deref(), Some("Saved \u{b7} 20 Nov 2026"));
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(s.print_preview().is_none());
    assert_eq!(s.title(), "Journal");
}
