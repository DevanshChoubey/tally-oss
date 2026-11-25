//! Card S14: Ctrl+P on a report opens a print preview of the engine's pages; saving from it
//! writes the PDF the engine produced.
//!
//! Provenance. Behaviour: TallyPrime's flow is print, then preview, then print: "Press I
//! (Preview) to verify, if needed" (<https://help.tallysolutions.com/print-invoices-reports/>,
//! read 2026-11-25, page last updated 2026-11-20). The mock (`design/mock-v2/index.html`,
//! states `profit-and-loss.print-preview` and `.print-busy`) has Ctrl+P open the preview, Ctrl+P
//! inside it save as PDF and Esc close it. Page count: derived. A day book row is one table
//! row, at least 9 point text plus 2 x 3 padding (18 points); 200 rows are at least 3,600 points
//! against a page of 842 points less margins (2 x 36) and the footer (28), so they need four
//! pages or more, and the footer on page 1 says "Page 1 of N" with N the page count.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{
    Api, ApiError, ColumnSpec, Command, Detail, PageItem, ReportFilter, ReportKind, ReportParams,
};
use ledgerly_app::screens::report_view::{ReportSource, ReportViewScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const VOUCHERS: u64 = 200;

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, reserved: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(reserved),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn journal(id: u64, on: Date) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    let amount = Paise::new(10_000);
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![
            line(CASH, SignedAmount::dr(amount)),
            line(CAPITAL, SignedAmount::cr(amount)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// A company file with 200 journals on 10 April, so the day book prints on several pages. The
/// file name is unique per test; the same books are made again for the expected bytes.
fn setup(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut api = Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for l in [
        ledger(CASH, "Cash", ReservedGroup::CashInHand),
        ledger(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    for id in 1..=VOUCHERS {
        committed(api.execute(Command::PostVoucher(journal(id, date!(2026 - 04 - 10)))));
    }
    api
}

fn committed(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn day_book() -> ReportParams {
    let period = Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap();
    ReportParams {
        kind: ReportKind::DayBook,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn press(key: Key, modifiers: Modifiers) -> RawInput {
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

/// One frame of the screen with `input`; returns whether the screen asked the stack to leave.
fn frame(
    ctx: &egui::Context,
    screen: &mut ReportViewScreen,
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

fn frames_until(
    ctx: &egui::Context,
    screen: &mut ReportViewScreen,
    app: &mut AppCtx,
    done: impl Fn(&ReportViewScreen) -> bool,
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

struct Rig {
    ctx: egui::Context,
    screen: ReportViewScreen,
    app: AppCtx,
    dialogs: Arc<AtomicUsize>,
    target: PathBuf,
    answer: Arc<Mutex<Option<PathBuf>>>,
}

/// A loaded day book whose save dialog counts its openings and answers with `target`.
fn rig(name: &str) -> Rig {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-out"));
    let _removed = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("day-book.pdf");
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(setup(&format!("{name}.ledgerly"))).unwrap()),
        ..AppCtx::default()
    };
    let ctx = egui::Context::default();
    let mut screen = ReportViewScreen::worker(ReportSource::worker(day_book())).unwrap();
    frames_until(&ctx, &mut screen, &mut app, |s| !s.is_loading());
    let dialogs = Arc::new(AtomicUsize::new(0));
    let answer = Arc::new(Mutex::new(Some(target.clone())));
    let (count, reply) = (Arc::clone(&dialogs), Arc::clone(&answer));
    screen.set_print_picker(Box::new(move |_name: &str| {
        count.fetch_add(1, Ordering::SeqCst);
        reply.lock().unwrap().clone()
    }));
    Rig {
        ctx,
        screen,
        app,
        dialogs,
        target,
        answer,
    }
}

fn ctrl_p() -> RawInput {
    press(Key::P, Modifiers::COMMAND)
}

fn footer_texts(doc: &ledgerly_api::PrintedDocument, page: usize) -> Vec<String> {
    doc.pages[page]
        .items
        .iter()
        .filter_map(|i| match i {
            PageItem::Text { text, .. } if text.starts_with("Page ") => Some(text.clone()),
            PageItem::Text { .. } | PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn ctrl_p_opens_a_preview_of_the_engines_pages_and_opens_no_save_dialog() {
    let mut r = rig("s14_open");
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_some()
    });
    let doc = r.screen.print_preview().unwrap();
    assert!(doc.pages.len() >= 4, "{} pages", doc.pages.len());
    let total = doc.pages.len();
    assert_eq!(footer_texts(doc, 0), [format!("Page 1 of {total}")]);
    assert_eq!(r.dialogs.load(Ordering::SeqCst), 0, "no dialog before Save");
    assert!(!r.target.exists());
}

#[test]
fn esc_closes_the_preview_without_leaving_the_screen_and_writes_nothing() {
    let mut r = rig("s14_esc");
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_some()
    });
    let left = frame(
        &r.ctx,
        &mut r.screen,
        &mut r.app,
        press(Key::Escape, Modifiers::NONE),
    );
    assert!(r.screen.print_preview().is_none(), "Esc closes the preview");
    assert!(
        !left,
        "Esc returns to the report, not to the screen before it"
    );
    assert_eq!(r.dialogs.load(Ordering::SeqCst), 0);
    assert!(!r.target.exists());
}

#[test]
fn ctrl_p_in_the_preview_saves_the_same_bytes_the_engine_writes_and_closes_it() {
    // Identity check: the same books made again by `setup` print to the same bytes (rule A7,
    // deterministic output), so the file must equal `print_report` of those books, and so must
    // the bytes the preview held.
    let expected = {
        let Ok(Outcome::Value(pdf)) = setup("s14_expected.ledgerly").print_report(&day_book())
        else {
            panic!("the day book should print");
        };
        pdf
    };
    let mut r = rig("s14_save");
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_some()
    });
    assert_eq!(r.screen.print_preview().unwrap().pdf, expected);
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_none() && s.notice().is_some()
    });
    assert_eq!(r.dialogs.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(&r.target).unwrap(), expected);
    let said = r.screen.notice().unwrap().text().to_owned();
    assert!(said.starts_with("Saved PDF to "), "{said}");
}

#[test]
fn a_cancelled_save_dialog_leaves_the_preview_open() {
    let mut r = rig("s14_cancel");
    *r.answer.lock().unwrap() = None;
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_some()
    });
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    for _ in 0..5 {
        let _left = frame(&r.ctx, &mut r.screen, &mut r.app, RawInput::default());
    }
    assert_eq!(r.dialogs.load(Ordering::SeqCst), 1);
    assert!(
        r.screen.print_preview().is_some(),
        "cancel keeps the preview"
    );
    assert!(!r.target.exists());
}

#[test]
fn the_screen_is_busy_while_the_worker_prepares_the_pdf_and_shows_no_preview_yet() {
    let mut r = rig("s14_busy");
    let hold = r.app.api.as_ref().unwrap().hold();
    let _left = frame(&r.ctx, &mut r.screen, &mut r.app, ctrl_p());
    for _ in 0..5 {
        let _left = frame(&r.ctx, &mut r.screen, &mut r.app, RawInput::default());
    }
    assert!(r.screen.is_print_rendering(), "the worker is held");
    assert!(r.screen.print_preview().is_none());
    drop(hold);
    frames_until(&r.ctx, &mut r.screen, &mut r.app, |s| {
        s.print_preview().is_some()
    });
    assert!(!r.screen.is_print_rendering());
}
