//! Card W7 tests: the report viewer in worker mode, against a real `Api` on a company file.
//!
//! Provenance. The trial balance figures are `derived` by hand from the two vouchers posted
//! below, with the arithmetic written next to each check. None is copied from the engine.
//!
//! Tally citations, read 2026-11-21:
//! * <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Financial_Statements/Display_Trial_Balance.htm>
//!   (the article body, read in its last part): "the total of all debit balances should be equal
//!   the total of all credit balances", and the Trial Balance "does not display the closing
//!   stock". The same page is silent on how a group with ledgers on both sides is laid out. For
//!   that the reference is the screenshot provenance recorded in
//!   `crates/ledgerly-reports/tests/rp2_trial_balance.rs` (Tally's Current Liabilities group row
//!   shows a Debit and a Credit amount on one row). That is `observed` in Tally's own screenshots,
//!   not in the page text.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{
    Api, ApiError, ColumnSpec, Command, Detail, ReportFilter, ReportKind, ReportParams,
};
use ledgerly_app::screens::report_view::{ReportSource, ReportViewScreen, ViewCell, ViewReport};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
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
const CUSTOMER: LedgerId = LedgerId(3);
const KARNATAKA: u8 = 29;

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn april() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap()
}

fn first_half_of_april() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 15)).unwrap()
}

fn second_choice() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 20)).unwrap()
}

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(KARNATAKA).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn plain(id: LedgerId, name: &str, reserved: ReservedGroup) -> Ledger {
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

fn journal(id: u64, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
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
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: std::collections::BTreeMap::new(),
    }
}

/// A company file with three ledgers and two vouchers, and the API that owns it. The file name
/// is unique per test, so the tests can run in parallel.
fn setup(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    for ledger in [
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        plain(CUSTOMER, "Customer One", ReservedGroup::SundryDebtors),
    ] {
        value(api.execute(Command::CreateLedger(ledger)));
    }
    // Capital of 1,00,000 paid into Cash.
    value(api.execute(Command::PostVoucher(journal(
        1,
        date!(2026 - 04 - 10),
        vec![
            line(CASH, SignedAmount::dr(rs(100_000))),
            line(CAPITAL, SignedAmount::cr(rs(100_000))),
        ],
    ))));
    // An advance of 5,000 from Customer One into Cash.
    value(api.execute(Command::PostVoucher(journal(
        2,
        date!(2026 - 04 - 10),
        vec![
            line(CASH, SignedAmount::dr(rs(5_000))),
            line(CUSTOMER, SignedAmount::cr(rs(5_000))),
        ],
    ))));
    api
}

fn value(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn trial_balance(period: Period) -> ReportParams {
    ReportParams {
        kind: ReportKind::TrialBalance,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

/// Polls the screen until its report arrives. Bounded, with a short sleep between polls.
fn settle(screen: &mut ReportViewScreen, worker: &ApiWorker) {
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the report did not arrive within the bound");
}

fn row_cells(report: &ViewReport, label: &str) -> Vec<ViewCell> {
    report
        .rows
        .iter()
        .find(|r| r.label == label)
        .map(|r| r.cells.clone())
        .unwrap()
}

fn dr_cell(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr_cell(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

#[test]
fn worker_trial_balance_shows_a_group_with_both_sides_and_gross_totals() {
    // provenance: derived. Vouchers 1 and 2 above. Cash Dr 1,00,000 + 5,000 = 1,05,000.
    // Customer One Cr 5,000 (Sundry Debtors, inside Current Assets). Capital Cr 1,00,000.
    // Current Assets row: Dr 1,05,000 (Cash) and Cr 5,000 (Customer One), both sides shown.
    // Totals: Dr 1,05,000; Cr 5,000 + 1,00,000 = 1,05,000. Netting the group would give
    // Dr 1,00,000 and totals of 1,00,000, which is not what the rule above produces.
    // Tally: the Trial Balance page states that total debits equal total credits (read
    // 2026-11-21, see the module header). The both-sides layout is from the rp2 screenshot
    // provenance (see the module header).
    let api = setup("w7_tb_mixed_group.ledgerly");
    let worker = ApiWorker::spawn(api).unwrap();
    let base = trial_balance(april());
    let mut screen = ReportViewScreen::worker(ReportSource::worker(base)).unwrap();

    settle(&mut screen, &worker);

    let report = screen.report();
    assert_eq!(
        row_cells(report, "Current Assets"),
        vec![dr_cell(10_500_000), cr_cell(500_000)]
    );
    assert_eq!(row_cells(report, "Cash")[0], dr_cell(10_500_000));
    assert_eq!(
        report.totals,
        vec![dr_cell(10_500_000), cr_cell(10_500_000)]
    );
    assert_eq!(report.params.period, april());
}

#[test]
fn a_report_answer_for_an_older_request_is_dropped() {
    // The worker runs one job at a time, in order. A holds the first job; the answer for A is
    // not wanted once B is requested, so the screen must keep showing the placeholder period
    // (the base period) after A has run, and must show B's period at the end.
    let worker = ApiWorker::spawn(setup("w7_stale_drop.ledgerly")).unwrap();
    let (release_first, first_gate) = mpsc::channel::<()>();
    let (release_second, second_gate) = mpsc::channel::<()>();
    let (started_tx, started_rx) = mpsc::channel::<()>();

    let _holding = worker.run(move |_api| {
        let _released = first_gate.recv();
    });
    let mut screen =
        ReportViewScreen::worker(ReportSource::worker(trial_balance(april()))).unwrap();

    // Request A (period 1 to 15 April) is queued behind the first gate.
    screen.set_period(first_half_of_april());
    screen.tick(Some(&worker));
    assert!(screen.is_loading());

    // A second gate, queued after A. Once it has started, A has run on the worker.
    let _second_hold = worker.run(move |_api| {
        let _sent = started_tx.send(());
        let _released = second_gate.recv();
    });

    // Request B (period 1 to 20 April) replaces A's ticket.
    screen.set_period(second_choice());
    screen.tick(Some(&worker));
    assert!(screen.is_loading());

    release_first.send(()).unwrap();
    started_rx.recv().unwrap();
    screen.tick(Some(&worker));
    assert_eq!(
        screen.report().params.period,
        april(),
        "A's answer must not be shown after B was asked for"
    );
    assert!(screen.is_loading());

    release_second.send(()).unwrap();
    settle(&mut screen, &worker);
    assert_eq!(screen.report().params.period, second_choice());
    assert_eq!(row_cells(screen.report(), "Cash")[0], dr_cell(10_500_000));
}

#[test]
fn a_report_request_without_a_company_says_so_and_runs_nothing() {
    // A worker-mode screen with no worker (no company open) shows an error and keeps the
    // placeholder; it never shows a figure.
    let mut screen =
        ReportViewScreen::worker(ReportSource::worker(trial_balance(april()))).unwrap();
    screen.tick(None);
    assert!(!screen.is_loading());
    assert_eq!(screen.report().rows.len(), 0);
    assert!(screen.notice().is_some());
}

// ---- Export (Ctrl+E) ----------------------------------------------------------------------------

/// Runs frames of `screen` with `app` until `done` holds. Bounded, with a short sleep between
/// frames so the worker can answer.
fn frames_until(
    screen: &mut ReportViewScreen,
    app: &mut AppCtx,
    first: Option<(Key, Modifiers)>,
    done: impl Fn(&ReportViewScreen) -> bool,
) {
    let ctx = egui::Context::default();
    let mut key = first;
    for _ in 0..2000 {
        let events = key
            .take()
            .map(|(key, modifiers)| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            })
            .into_iter()
            .collect();
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            let _stay = screen.ui(ui, app);
        });
        out.drop_without_applying_deltas();
        if done(screen) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the screen did not settle within the bound");
}

fn export_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn ctrl_e_writes_the_report_on_screen_to_the_file_chosen() {
    // provenance: index.html:1624 and 1634: Ctrl+E exports "exactly what is on screen" as CSV or
    // XLSX. Derived: the file is the CSV layout of rp8 (a `Particulars` heading line, then the
    // rows) and the screen says where it went. The save dialog is suggested the report's title.
    let dir = export_dir("w7_export_ctrl_e");
    let out = dir.join("tb.csv");
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(setup("w7_export_ctrl_e.ledgerly")).unwrap()),
        ..AppCtx::default()
    };
    let mut screen =
        ReportViewScreen::worker(ReportSource::worker(trial_balance(april()))).unwrap();
    frames_until(&mut screen, &mut app, None, |s| !s.is_loading());
    let suggested = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = std::sync::Arc::clone(&suggested);
    let chosen = out.clone();
    screen.set_export_picker(Box::new(move |name: &str| {
        *seen.lock().unwrap() = name.to_owned();
        Some(chosen.clone())
    }));
    frames_until(
        &mut screen,
        &mut app,
        Some((Key::E, Modifiers::COMMAND)),
        |s| s.notice().is_some() && !s.is_exporting(),
    );
    let notice = screen.notice().unwrap().text().to_owned();
    assert!(notice.contains("tb.csv"), "{notice}");
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("Particulars,"), "{text}");
    assert!(text.contains("Cash"), "{text}");
    let title = screen.report().title.clone();
    assert_eq!(*suggested.lock().unwrap(), format!("{title}.csv"));
}

#[test]
fn an_export_to_a_name_that_is_not_csv_or_xlsx_writes_nothing_and_says_why() {
    // provenance: derived: the format follows the extension (`ExportFormat::from_path`); a name
    // with neither has no format, so nothing is written and the screen names the two that work.
    let dir = export_dir("w7_export_bad_name");
    let out = dir.join("tb.pdf");
    let worker = ApiWorker::spawn(setup("w7_export_bad_name.ledgerly")).unwrap();
    let mut screen =
        ReportViewScreen::worker(ReportSource::worker(trial_balance(april()))).unwrap();
    settle(&mut screen, &worker);
    screen.export_to(out.clone(), Some(&worker));
    assert!(!screen.is_exporting());
    let notice = screen.notice().unwrap().text().to_owned();
    assert!(
        notice.contains(".csv") && notice.contains(".xlsx"),
        "{notice}"
    );
    assert!(!out.exists());
}

#[test]
fn an_export_waits_for_the_report_to_arrive() {
    // provenance: derived from index.html:1580: while the report is computing, "the previous
    // result is not shown as current", so there is nothing on screen to export yet.
    let dir = export_dir("w7_export_loading");
    let out = dir.join("tb.csv");
    let worker = ApiWorker::spawn(setup("w7_export_loading.ledgerly")).unwrap();
    let mut screen =
        ReportViewScreen::worker(ReportSource::worker(trial_balance(april()))).unwrap();
    assert!(screen.is_loading());
    screen.export_to(out.clone(), Some(&worker));
    assert!(!screen.is_exporting());
    assert!(screen.notice().is_some());
    assert!(!out.exists());
}

#[test]
fn the_suggested_export_name_has_no_path_or_reserved_characters() {
    // provenance: derived. A drill's title carries ':' ("Ledger Vouchers: ...") and ledger names
    // carry '/' ("Profit and Loss A/c"). '/' and '\' are path separators, and Windows refuses
    // `< > : " | ? *` in a file name; each becomes '-'. Edge spaces and dots are dropped, and an
    // empty name falls back to "Report".
    use ledgerly_app::screens::report_view::export_file_name;
    assert_eq!(
        export_file_name("Ledger Vouchers: Profit and Loss A/c"),
        "Ledger Vouchers- Profit and Loss A-c.csv"
    );
    assert_eq!(
        export_file_name(r#"a\b<c>d"e|f?g*h"#),
        "a-b-c-d-e-f-g-h.csv"
    );
    assert_eq!(export_file_name(" . "), "Report.csv");
    assert_eq!(export_file_name("Trial Balance"), "Trial Balance.csv");
}

// ---- the palette's Recent rows, live from the engine -------------------------------------------

/// Chooses each of `entries` in the palette so they become its recent rows, then reopens it on
/// the empty query and lets `search` fetch the live details from `worker`.
fn recent_rows(
    entries: Vec<ledgerly_app::palette::PaletteEntry>,
    worker: &ApiWorker,
) -> ledgerly_app::palette::Palette {
    use ledgerly_app::palette::{Palette, PaletteMode, PaletteSearch};
    let mut palette = Palette::new();
    let titles: Vec<String> = entries.iter().map(|e| e.title.clone()).collect();
    palette.set_entries(entries);
    for title in titles {
        palette.open(PaletteMode::OnTop);
        palette.set_query(title);
        assert!(palette.accept().is_some());
    }
    let mut search = PaletteSearch::new();
    palette.open(PaletteMode::OnTop);
    search.tick(&mut palette, Some(worker));
    for _ in 0..2000 {
        if !search.is_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
        search.tick(&mut palette, Some(worker));
    }
    assert!(
        !search.is_pending(),
        "the details did not arrive within the bound"
    );
    palette
}

fn detail_of(palette: &ledgerly_app::palette::Palette, title: &str) -> String {
    palette
        .results()
        .into_iter()
        .find(|e| e.title == title)
        .map(|e| e.detail.clone())
        .unwrap()
}

#[test]
fn a_recent_ledger_shows_its_group_and_the_engines_balance() {
    // provenance: index.html:1115: a recent ledger reads "Ledger · Sundry debtors · 1,93,500.00
    // Dr". Derived from the fixture: voucher 2 credits Customer One 5,000 and nothing else
    // touches it, so its balance is 5,000.00 Cr; it sits in Sundry Debtors.
    use ledgerly_app::palette::{PaletteEntry, PaletteTarget};
    use ledgerly_app::shell::navigate::ledger_key;
    let worker = ApiWorker::spawn(setup("w7_recent_ledger.ledgerly")).unwrap();
    let palette = recent_rows(
        vec![
            PaletteEntry::new(PaletteTarget::Master(ledger_key(CUSTOMER)), "Customer One")
                .with_detail("Ledger, found by name"),
        ],
        &worker,
    );
    assert_eq!(
        detail_of(&palette, "Customer One"),
        "Ledger \u{00B7} Sundry Debtors \u{00B7} 5,000.00 Cr"
    );
}

#[test]
fn a_recent_balance_sheet_says_the_date_it_opens_at() {
    // provenance: index.html:1115: the recent Balance sheet reads "As at 21 Nov 2026". Derived:
    // a report opens at the last voucher date of the year (`default_period`), and both fixture
    // vouchers are dated 10 Apr 2026.
    use ledgerly_app::palette::{PaletteEntry, PaletteTarget};
    let worker = ApiWorker::spawn(setup("w7_recent_bs.ledgerly")).unwrap();
    let palette = recent_rows(
        vec![
            PaletteEntry::new(
                PaletteTarget::Report("balance-sheet".to_owned()),
                "Balance Sheet",
            )
            .with_detail("Report"),
        ],
        &worker,
    );
    assert_eq!(detail_of(&palette, "Balance Sheet"), "As at 10 Apr 2026");
}

#[test]
fn a_recent_ledger_the_engine_cannot_compute_shows_why_and_no_figure() {
    // provenance: derived. The ledger report is Unsupported for the reserved Profit and Loss
    // ledger (ledgerly-reports `ledger.rs`, "folds in prior-year profit at query time"), so the
    // row says the balance was not computed and why; it never shows 0.00.
    use ledgerly_app::palette::{PaletteEntry, PaletteTarget};
    use ledgerly_app::shell::navigate::ledger_key;
    const PL: LedgerId = LedgerId(9);
    let mut api = setup("w7_recent_pl.ledgerly");
    let mut pl = plain(PL, "Profit & Loss A/c", ReservedGroup::CapitalAccount);
    pl.reserved = Some(ledgerly_model::ReservedLedger::ProfitAndLoss);
    value(api.execute(Command::CreateLedger(pl)));
    let worker = ApiWorker::spawn(api).unwrap();
    let palette = recent_rows(
        vec![PaletteEntry::new(
            PaletteTarget::Master(ledger_key(PL)),
            "Profit & Loss A/c",
        )],
        &worker,
    );
    let detail = detail_of(&palette, "Profit & Loss A/c");
    assert!(
        detail.starts_with("Ledger \u{00B7} Capital Account \u{00B7} not computed: "),
        "{detail}"
    );
    assert!(detail.contains("prior-year profit"), "{detail}");
    assert!(!detail.contains("0.00"), "{detail}");
}
