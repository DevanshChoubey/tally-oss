//! Card S16: the print preview's card holds the print settings; Update preview (Enter) prints
//! again with them, and the preview shows the new pages.
//!
//! Provenance. Behaviour: the settings a print has (paper, copies, pages, first page number) are
//! those of TallyPrime's print configuration (help.tallysolutions.com, page
//! "configure-for-print-export-share", read 2026-11-26); the mock states are
//! `profit-and-loss.print-config` and `.print-range-error`. Page counts: derived. 200 journals
//! on one day need at least four A4 pages (see `s14_print_preview`), so page 2 of N exists, and
//! pages 99 to 100 do not.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
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
        group: Some(group(reserved)),
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
}

fn rig(name: &str) -> Rig {
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(setup(&format!("{name}.ledgerly"))).unwrap()),
        ..AppCtx::default()
    };
    let ctx = egui::Context::default();
    let mut screen = ReportViewScreen::worker(ReportSource::worker(day_book())).unwrap();
    frames_until(&ctx, &mut screen, &mut app, |s| !s.is_loading());
    let _left = frame(
        &ctx,
        &mut screen,
        &mut app,
        press(Key::P, Modifiers::COMMAND),
    );
    frames_until(&ctx, &mut screen, &mut app, |s| s.print_preview().is_some());
    Rig { ctx, screen, app }
}

fn update(r: &mut Rig) {
    let _left = frame(
        &r.ctx,
        &mut r.screen,
        &mut r.app,
        press(Key::Enter, Modifiers::NONE),
    );
    for _ in 0..4 {
        let _left = frame(&r.ctx, &mut r.screen, &mut r.app, RawInput::default());
    }
}

fn settle(r: &mut Rig) {
    for _ in 0..400 {
        let _left = frame(&r.ctx, &mut r.screen, &mut r.app, RawInput::default());
        if !r.screen.is_print_rendering() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the print did not finish");
}

fn footer(doc: &ledgerly_api::PrintedDocument, page: usize) -> String {
    doc.pages[page]
        .items
        .iter()
        .find_map(|i| match i {
            PageItem::Text { text, .. } if text.starts_with("Page ") => Some(text.clone()),
            PageItem::Text { .. } | PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .unwrap()
}

#[test]
fn update_preview_prints_again_with_the_page_range_typed() {
    let mut r = rig("s16_cfg_range");
    let total = r.screen.print_preview().unwrap().pages.len();
    assert!(total >= 4);
    r.screen.print_form_mut().pages = "2".to_owned();
    update(&mut r);
    settle(&mut r);
    let doc = r.screen.print_preview().unwrap();
    assert_eq!(doc.pages.len(), 1, "only page 2 prints");
    assert_eq!(footer(doc, 0), format!("Page 2 of {total}"));
    assert_eq!(r.screen.print_setup_error(), None);
}

#[test]
fn a_range_past_the_end_is_said_and_the_last_good_pages_stay() {
    let mut r = rig("s16_cfg_bad_range");
    let before = r.screen.print_preview().unwrap().pdf.clone();
    r.screen.print_form_mut().pages = "99-100".to_owned();
    update(&mut r);
    settle(&mut r);
    let said = r.screen.print_setup_error().unwrap().to_owned();
    assert!(said.contains("99-100"), "{said}");
    assert_eq!(r.screen.print_preview().unwrap().pdf, before);
}

#[test]
fn copies_first_page_number_and_paper_reach_the_pdf() {
    let mut r = rig("s16_cfg_rest");
    {
        let form = r.screen.print_form_mut();
        form.copies = "2".to_owned();
        form.first_page = "5".to_owned();
        form.paper = ledgerly_api::Paper::A5;
    }
    update(&mut r);
    settle(&mut r);
    let doc = r.screen.print_preview().unwrap();
    assert_eq!(doc.copies, 2);
    assert_eq!(doc.pages[0].width, 420, "A5 is 420 points wide");
    assert!(
        footer(doc, 0).starts_with("Page 5 of "),
        "{}",
        footer(doc, 0)
    );
}

#[test]
fn settings_that_are_not_numbers_are_refused_in_words_without_printing() {
    let mut r = rig("s16_cfg_text");
    let before = r.screen.print_preview().unwrap().pdf.clone();
    r.screen.print_form_mut().copies = "two".to_owned();
    update(&mut r);
    let said = r.screen.print_setup_error().unwrap().to_owned();
    assert!(said.contains("Copies") && said.contains("two"), "{said}");
    assert!(!r.screen.is_print_rendering());
    assert_eq!(r.screen.print_preview().unwrap().pdf, before);
}
