//! Card U7: report viewer and drill. Every expected value is derived by hand, and the derivation
//! is written next to it; nothing is copied from this crate's output. The sources of truth are
//! spec 6.2 ("Every figure drills down: group -> ledger -> voucher. Esc returns with selection
//! preserved."), spec 7 ("Enter drills, Esc returns with selection preserved", "Alt+F1 detail
//! toggle", "F2 date/period", Indian digit grouping, "no colour-only signals") and the report
//! model of `ledgerly-reports` that `ViewReport` mirrors. The screen shows figures and never
//! computes them, so the amounts below are fixture constants chosen so the arithmetic can be
//! checked by eye.
//!
//! Cases without provenance are not asserted as figures; they are asserted to say `Unsupported`
//! (see the last section).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::report_view::{
    DrillTarget, Notice, PeriodTextError, ReportSource, ReportToken, ReportViewScreen, ViewCell,
    ViewDetail, ViewError, ViewKey, ViewParams, ViewReport, ViewRow, ViewRowKind, amount_text,
    cell_text, date_text, parse_period_date, parse_period_text, period_text,
};
use ledgerly_app::shell::Shell;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Decimal, GroupId, LedgerId, Outcome, Paise, Period, PeriodError, Qty, SignedAmount,
    StockItemId, Unsupported, VoucherId,
};
use time::macros::date;

// ---- fixture ---------------------------------------------------------------------------------

const CURRENT_ASSETS: GroupId = GroupId(1);
const CAPITAL: GroupId = GroupId(2);
const CASH: LedgerId = LedgerId(10);
const HDFC: LedgerId = LedgerId(11);
const VOUCHER: VoucherId = VoucherId(100);

/// 1,50,000.00 rupees.
const CASH_PAISE: i64 = 15_000_000;
/// 10,84,567.00 rupees.
const HDFC_PAISE: i64 = 108_456_700;
/// 12,34,567.00 rupees. Derivation: 1,50,000.00 + 10,84,567.00 = 12,34,567.00, that is
/// `15_000_000` + `108_456_700` = `123_456_700` paise.
const ASSETS_PAISE: i64 = 123_456_700;

fn fy_2025() -> Period {
    Period::new(date!(2025 - 04 - 01), date!(2026 - 03 - 31)).unwrap()
}

fn params(detail: ViewDetail) -> ViewParams {
    ViewParams {
        period: fy_2025(),
        detail,
    }
}

fn dr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> ViewCell {
    ViewCell::Amount(SignedAmount::cr(Paise::new(paise)))
}

fn row(
    key: Option<ViewKey>,
    label: &str,
    depth: u8,
    kind: ViewRowKind,
    cells: Vec<ViewCell>,
) -> ViewRow {
    ViewRow {
        key,
        label: label.to_owned(),
        depth,
        kind,
        cells,
    }
}

/// A one-column balance report. Condensed: assets, capital, a note. Detailed: the assets group is
/// followed by its two ledgers.
fn balances(p: ViewParams) -> ViewReport {
    let mut rows = vec![row(
        Some(ViewKey::Group(CURRENT_ASSETS)),
        "Current Assets",
        0,
        ViewRowKind::Group,
        vec![dr(ASSETS_PAISE)],
    )];
    if p.detail == ViewDetail::Detailed {
        rows.push(row(
            Some(ViewKey::Ledger(CASH)),
            "Cash",
            1,
            ViewRowKind::Ledger,
            vec![dr(CASH_PAISE)],
        ));
        rows.push(row(
            Some(ViewKey::Ledger(HDFC)),
            "HDFC Current",
            1,
            ViewRowKind::Ledger,
            vec![dr(HDFC_PAISE)],
        ));
    }
    rows.push(row(
        Some(ViewKey::Group(CAPITAL)),
        "Capital",
        0,
        ViewRowKind::Group,
        vec![cr(ASSETS_PAISE)],
    ));
    rows.push(row(
        None,
        "Figures are in rupees",
        0,
        ViewRowKind::Text,
        vec![ViewCell::Empty],
    ));
    ViewReport {
        token: ReportToken("balances".to_owned()),
        title: "Balances".to_owned(),
        params: p,
        columns: vec!["Closing balance".to_owned()],
        rows,
        totals: vec![dr(ASSETS_PAISE)],
        notes: vec!["Closing stock is provisional.".to_owned()],
    }
}

/// What the stand-in host does when the screen asks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    RefreshUnsupported,
    RefreshRagged,
    RefreshFails,
}

/// Records every request the screen makes and answers them.
struct Host {
    refreshes: RefCell<Vec<ViewParams>>,
    drills: RefCell<Vec<(ViewKey, ViewParams)>>,
    mode: Cell<Mode>,
}

/// Does nothing but have a title.
struct Leaf(String);

impl Screen for Leaf {
    fn title(&self) -> String {
        self.0.clone()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ui.label(self.0.as_str());
        ScreenAction::Stay
    }
}

fn ledger_vouchers(p: ViewParams) -> ViewReport {
    ViewReport {
        token: ReportToken("ledger".to_owned()),
        title: "Cash vouchers".to_owned(),
        params: p,
        columns: vec!["Amount".to_owned()],
        rows: vec![row(
            Some(ViewKey::Voucher(VOUCHER)),
            "Receipt 1",
            0,
            ViewRowKind::Voucher,
            vec![dr(CASH_PAISE)],
        )],
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

fn host_source(host: &Rc<Host>) -> ReportSource {
    let for_refresh = Rc::clone(host);
    let for_drill = Rc::clone(host);
    ReportSource::new(
        move |_token, p| {
            for_refresh.refreshes.borrow_mut().push(p);
            match for_refresh.mode.get() {
                Mode::Normal => Ok(Outcome::Value(balances(p))),
                Mode::RefreshUnsupported => Ok(Outcome::unsupported(
                    "detail level",
                    "no verified rule for this view",
                )),
                Mode::RefreshRagged => {
                    let mut r = balances(p);
                    r.rows[0].cells.push(ViewCell::Empty);
                    Ok(Outcome::Value(r))
                }
                Mode::RefreshFails => Err(ViewError::Source {
                    message: "the books are locked".to_owned(),
                }),
            }
        },
        move |_token, key, p| {
            for_drill.drills.borrow_mut().push((key.clone(), p));
            match key {
                ViewKey::Group(_) | ViewKey::Ledger(_) => {
                    Ok(Outcome::Value(DrillTarget::Report(ledger_vouchers(p))))
                }
                ViewKey::Voucher(_) => Ok(Outcome::Value(DrillTarget::Screen(Box::new(Leaf(
                    "Voucher 100".to_owned(),
                ))))),
                ViewKey::Item(_) | ViewKey::Bill { .. } => Ok(Outcome::unsupported(
                    "drill into stock items and bills",
                    "no report exists for them yet",
                )),
            }
        },
    )
}

fn host() -> Rc<Host> {
    Rc::new(Host {
        refreshes: RefCell::new(Vec::new()),
        drills: RefCell::new(Vec::new()),
        mode: Cell::new(Mode::Normal),
    })
}

fn screen_for(host: &Rc<Host>, detail: ViewDetail) -> ReportViewScreen {
    ReportViewScreen::new(balances(params(detail)), host_source(host)).unwrap()
}

// ---- egui helpers ----------------------------------------------------------------------------

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

fn frame(ctx: &egui::Context, screen: &mut ReportViewScreen, input: RawInput) -> ScreenAction {
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
    action
}

fn shell_frame(ctx: &egui::Context, shell: &mut Shell, input: RawInput) {
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

fn trail(shell: &Shell) -> Vec<String> {
    shell.tabs().active_stack().unwrap().breadcrumbs()
}

fn error_text(s: &ReportViewScreen) -> String {
    let Some(Notice::Error(t)) = s.notice() else {
        panic!("expected an error notice, got {:?}", s.notice());
    };
    t.clone()
}

// ---- cell text -------------------------------------------------------------------------------

#[test]
fn amounts_use_indian_grouping_and_say_dr_or_cr() {
    // derived: 123_456_700 paise = 1_234_567.00 rupees. Indian grouping puts the last three
    // digits in one group and pairs to the left (spec 7: "12,34,567.00"): 12,34,567.00.
    assert_eq!(cell_text(&dr(ASSETS_PAISE)), "12,34,567.00 Dr");
    assert_eq!(cell_text(&cr(ASSETS_PAISE)), "12,34,567.00 Cr");
    // derived: 15_000_000 paise = 150_000.00 rupees = 1,50,000.00.
    assert_eq!(cell_text(&dr(CASH_PAISE)), "1,50,000.00 Dr");
    // derived: zero is 0.00 and still carries a side word (no colour-only signal, spec 7).
    assert_eq!(cell_text(&dr(0)), "0.00 Dr");
    assert_eq!(
        amount_text(SignedAmount::cr(Paise::new(5))),
        "0.05 Cr",
        "5 paise is 0.05 rupees"
    );
}

#[test]
fn signed_quantity_text_date_and_empty_cells() {
    // derived: -5_000 paise = -50.00 rupees; a loss is shown with its minus sign.
    assert_eq!(cell_text(&ViewCell::Signed(Paise::new(-5_000))), "-50.00");
    // derived: 12500 with scale 3 is the decimal 12.500, shown as typed in the books.
    assert_eq!(
        cell_text(&ViewCell::Qty(Qty(Decimal::new(12_500, 3)))),
        "12.500"
    );
    assert_eq!(cell_text(&ViewCell::Text("Opening".to_owned())), "Opening");
    assert_eq!(cell_text(&ViewCell::Empty), "");
    // derived: 2025-04-01 is 1 April 2025, written DD-Mon-YYYY like the report column labels.
    assert_eq!(
        cell_text(&ViewCell::Date(date!(2025 - 04 - 01))),
        "01-Apr-2025"
    );
}

#[test]
fn a_provisional_amount_says_so_next_to_the_figure() {
    // spec 6.2/6.4: closing stock may be provisional; the reason is shown, not hidden.
    // derived: 5_000 paise = 50.00 rupees.
    let cell = ViewCell::Provisional {
        amount: SignedAmount::dr(Paise::new(5_000)),
        reason: "stock rate missing".to_owned(),
    };
    assert_eq!(
        cell_text(&cell),
        "50.00 Dr (provisional: stock rate missing)"
    );
}

#[test]
fn an_unsupported_cell_shows_words_and_no_figure() {
    // rule A1: a value, Unsupported or an error; never a number that was not computed.
    let cell = ViewCell::Unsupported(Unsupported {
        what: "ageing of advances".to_owned(),
        reason: "no rule yet".to_owned(),
    });
    assert_eq!(
        cell_text(&cell),
        "Unsupported: ageing of advances (no rule yet)"
    );
}

#[test]
fn dates_and_periods_read_like_the_report_column_labels() {
    // derived: ledgerly-reports labels a period `01-Apr-2025 to 31-Mar-2026` (params.rs docs).
    assert_eq!(date_text(date!(2026 - 03 - 31)), "31-Mar-2026");
    assert_eq!(period_text(fy_2025()), "01-Apr-2025 to 31-Mar-2026");
    // derived: every month abbreviation, January to December.
    let months = [
        (date!(2025 - 01 - 05), "05-Jan-2025"),
        (date!(2025 - 02 - 05), "05-Feb-2025"),
        (date!(2025 - 03 - 05), "05-Mar-2025"),
        (date!(2025 - 04 - 05), "05-Apr-2025"),
        (date!(2025 - 05 - 05), "05-May-2025"),
        (date!(2025 - 06 - 05), "05-Jun-2025"),
        (date!(2025 - 07 - 05), "05-Jul-2025"),
        (date!(2025 - 08 - 05), "05-Aug-2025"),
        (date!(2025 - 09 - 05), "05-Sep-2025"),
        (date!(2025 - 10 - 05), "05-Oct-2025"),
        (date!(2025 - 11 - 05), "05-Nov-2025"),
        (date!(2025 - 12 - 05), "05-Dec-2025"),
    ];
    for (d, text) in months {
        assert_eq!(date_text(d), text);
    }
}

// ---- period text -----------------------------------------------------------------------------

#[test]
fn period_dates_parse_in_the_three_forms() {
    // derived: the same day, 1 April 2025, in each supported spelling.
    let want = date!(2025 - 04 - 01);
    assert_eq!(parse_period_date("01-04-2025").unwrap(), want);
    assert_eq!(parse_period_date("1/4/2025").unwrap(), want);
    assert_eq!(parse_period_date("2025-04-01").unwrap(), want);
    assert_eq!(parse_period_date("  01-04-2025 ").unwrap(), want);
}

#[test]
fn period_dates_that_do_not_exist_or_are_ambiguous_are_refused() {
    // derived: a two-digit year would need a guess about the century, so it is refused.
    assert!(parse_period_date("31-03-26").is_err());
    // derived: 2025 is not a leap year (not divisible by 4), so February has 28 days.
    assert!(parse_period_date("29-02-2025").is_err());
    // derived: 2024 is divisible by 4 and not by 100, so it is a leap year.
    assert_eq!(
        parse_period_date("29-02-2024").unwrap(),
        date!(2024 - 02 - 29)
    );
    // derived: there is no month 13 and no day 0.
    assert!(parse_period_date("01-13-2025").is_err());
    assert!(parse_period_date("00-04-2025").is_err());
    for text in [
        "",
        "1-2",
        "ab-01-2025",
        "01-04-2025-1",
        "01--2025",
        "-01-04-2025",
    ] {
        let err = parse_period_date(text).unwrap_err();
        assert!(matches!(err, PeriodTextError::NotADate { .. }), "{text}");
    }
}

#[test]
fn a_period_needs_its_start_on_or_before_its_end() {
    let p = parse_period_text("01-04-2025", "30-06-2025").unwrap();
    assert_eq!(p.from(), date!(2025 - 04 - 01));
    assert_eq!(p.to(), date!(2025 - 06 - 30));
    // derived: Period::new allows from == to (an inclusive one-day range).
    assert!(parse_period_text("01-04-2025", "01-04-2025").is_ok());
    // derived: a start after the end is not a period.
    let err = parse_period_text("02-04-2025", "01-04-2025").unwrap_err();
    assert!(matches!(
        err,
        PeriodTextError::Period(PeriodError::Reversed { .. })
    ));
}

// ---- shape -----------------------------------------------------------------------------------

#[test]
fn a_row_with_the_wrong_number_of_cells_is_rejected() {
    // A figure under the wrong column heading is a wrong number: rows need one cell per column.
    let mut report = balances(params(ViewDetail::Condensed));
    report.rows[1].cells.push(ViewCell::Empty);
    // derived: the second row (Capital) has 2 cells for 1 column.
    assert_eq!(
        report.check_shape().unwrap_err(),
        ViewError::RowShape {
            row: 2,
            label: "Capital".to_owned(),
            found: 2,
            expected: 1
        }
    );
    let host = host();
    assert!(ReportViewScreen::new(report, host_source(&host)).is_err());
}

#[test]
fn totals_must_match_the_columns_or_be_absent() {
    let mut report = balances(params(ViewDetail::Condensed));
    report.totals.push(ViewCell::Empty);
    assert_eq!(
        report.check_shape().unwrap_err(),
        ViewError::TotalsShape {
            found: 2,
            expected: 1
        }
    );
    report.totals.clear();
    assert!(report.check_shape().is_ok());
}

// ---- selection -------------------------------------------------------------------------------

#[test]
fn selection_moves_and_stops_at_both_ends() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    // derived: the detailed fixture has 5 rows, indexes 0 to 4.
    assert_eq!(s.selected(), 0);
    s.select_prev(1);
    assert_eq!(s.selected(), 0, "no wrap above the first row");
    s.select_next(1);
    assert_eq!(s.selected(), 1);
    assert_eq!(s.selected_row().unwrap().label, "Cash");
    s.select_next(100);
    assert_eq!(s.selected(), 4, "stops at the last row");
    s.select(2);
    assert_eq!(s.selected(), 2);
    s.select(99);
    assert_eq!(s.selected(), 4);
}

#[test]
fn an_empty_report_has_no_selection_and_nothing_to_open() {
    let host = host();
    let mut report = balances(params(ViewDetail::Condensed));
    report.rows.clear();
    let mut s = ReportViewScreen::new(report, host_source(&host)).unwrap();
    s.select_next(3);
    assert_eq!(s.selected(), 0);
    assert!(s.selected_row().is_none());
    assert!(matches!(s.drill(), ScreenAction::Stay));
    assert!(matches!(s.notice(), Some(Notice::Info(t)) if t.contains("no rows")));
    assert!(host.drills.borrow().is_empty());
}

#[test]
fn arrow_page_home_and_end_keys_move_the_selection() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    frame(&ctx, &mut s, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.selected(), 1);
    frame(&ctx, &mut s, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.selected(), 2);
    frame(&ctx, &mut s, key(Key::ArrowUp, Modifiers::NONE));
    assert_eq!(s.selected(), 1);
    // derived: a page is 10 rows and the fixture has 5, so Page Down lands on the last (4).
    frame(&ctx, &mut s, key(Key::PageDown, Modifiers::NONE));
    assert_eq!(s.selected(), 4);
    frame(&ctx, &mut s, key(Key::PageUp, Modifiers::NONE));
    assert_eq!(s.selected(), 0);
    frame(&ctx, &mut s, key(Key::End, Modifiers::NONE));
    assert_eq!(s.selected(), 4);
    frame(&ctx, &mut s, key(Key::Home, Modifiers::NONE));
    assert_eq!(s.selected(), 0);
}

// ---- drill -----------------------------------------------------------------------------------

#[test]
fn enter_on_a_group_row_opens_a_report_with_the_same_period_and_detail() {
    // spec 6.2: group -> ledger -> voucher. The drill carries the period and detail of the row.
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    let ScreenAction::Push(child) = s.drill() else {
        panic!("a group row must open a screen");
    };
    assert_eq!(child.title(), "Cash vouchers");
    let drills = host.drills.borrow();
    assert_eq!(drills.len(), 1);
    assert_eq!(drills[0].0, ViewKey::Group(CURRENT_ASSETS));
    assert_eq!(drills[0].1, params(ViewDetail::Detailed));
    assert!(s.notice().is_none());
}

#[test]
fn enter_on_a_voucher_row_opens_the_screen_the_host_gives() {
    // spec 6.2: the last drill level is the voucher. The host decides what a voucher opens.
    let host = host();
    let child = ledger_vouchers(params(ViewDetail::Detailed));
    let mut s = ReportViewScreen::new(child, host_source(&host)).unwrap();
    let ScreenAction::Push(v) = s.drill() else {
        panic!("a voucher row must open a screen");
    };
    assert_eq!(v.title(), "Voucher 100");
    assert_eq!(host.drills.borrow()[0].0, ViewKey::Voucher(VOUCHER));
}

#[test]
fn enter_on_a_row_with_no_key_says_so_and_asks_nothing_of_the_host() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    s.select(2);
    assert!(matches!(s.drill(), ScreenAction::Stay));
    assert!(matches!(
        s.notice(),
        Some(Notice::Info(t)) if t.contains("Figures are in rupees") && t.contains("nothing to open")
    ));
    assert!(host.drills.borrow().is_empty());
}

#[test]
fn an_unsupported_drill_is_reported_not_swallowed() {
    // rule A1: ledgerly-reports has no report for items or bills (drill.rs), so the host answers
    // Unsupported and the screen says so.
    let host = host();
    let mut report = balances(params(ViewDetail::Condensed));
    report.rows[0].key = Some(ViewKey::Item(StockItemId(7)));
    let mut s = ReportViewScreen::new(report, host_source(&host)).unwrap();
    assert!(matches!(s.drill(), ScreenAction::Stay));
    let text = error_text(&s);
    assert!(text.contains("Not supported yet"), "{text}");
    assert!(text.contains("no report exists for them yet"), "{text}");
}

#[test]
fn a_bill_row_goes_to_the_host_with_its_name() {
    let host = host();
    let mut report = balances(params(ViewDetail::Condensed));
    report.rows[0].key = Some(ViewKey::Bill {
        ledger: HDFC,
        name: "INV-1".to_owned(),
    });
    let mut s = ReportViewScreen::new(report, host_source(&host)).unwrap();
    s.drill();
    assert_eq!(
        host.drills.borrow()[0].0,
        ViewKey::Bill {
            ledger: HDFC,
            name: "INV-1".to_owned()
        }
    );
}

#[test]
fn enter_key_in_a_frame_drills() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    let action = frame(&ctx, &mut s, key(Key::Enter, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Push(_)));
}

#[test]
fn esc_returns_to_the_report_with_the_selection_where_it_was() {
    // spec 6.2 and 7: "Enter drills, Esc returns with selection preserved". The shell pops on
    // Esc (the screen leaves Esc alone), and the parent was never dropped. The proof the
    // selection survived: Enter after the return asks the host for the same row as before.
    let host = host();
    let s = screen_for(&host, ViewDetail::Detailed);
    let mut shell = Shell::new(Box::new(s));
    let ctx = egui::Context::default();
    shell_frame(&ctx, &mut shell, RawInput::default());
    // Down once: row 1 of the detailed fixture is Cash.
    shell_frame(&ctx, &mut shell, key(Key::ArrowDown, Modifiers::NONE));
    shell_frame(&ctx, &mut shell, key(Key::Enter, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers"]);
    shell_frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances"]);
    shell_frame(&ctx, &mut shell, key(Key::Enter, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers"]);
    let drills = host.drills.borrow();
    assert_eq!(drills.len(), 2);
    assert_eq!(drills[0].0, ViewKey::Ledger(CASH));
    assert_eq!(drills[1].0, ViewKey::Ledger(CASH));
}

#[test]
fn drilling_two_levels_and_backing_out_one_level_at_a_time() {
    // spec 6.2: group -> ledger -> voucher, and Esc returns one level each time.
    let host = host();
    let mut shell = Shell::new(Box::new(screen_for(&host, ViewDetail::Detailed)));
    let ctx = egui::Context::default();
    shell_frame(&ctx, &mut shell, RawInput::default());
    shell_frame(&ctx, &mut shell, key(Key::Enter, Modifiers::NONE));
    shell_frame(&ctx, &mut shell, key(Key::Enter, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers", "Voucher 100"]);
    shell_frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers"]);
    shell_frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances"]);
}

#[test]
fn a_drilled_report_that_is_not_rectangular_is_not_shown() {
    let source = ReportSource::new(
        |_t, p| Ok(Outcome::Value(balances(p))),
        |_t, _k, p| {
            let mut r = ledger_vouchers(p);
            r.rows[0].cells.clear();
            Ok(Outcome::Value(DrillTarget::Report(r)))
        },
    );
    let mut s = ReportViewScreen::new(balances(params(ViewDetail::Condensed)), source).unwrap();
    assert!(matches!(s.drill(), ScreenAction::Stay));
    assert!(error_text(&s).contains("has 0 cells but the report has 1 columns"));
}

#[test]
fn a_host_failure_while_drilling_is_shown() {
    let source = ReportSource::new(
        |_t, p| Ok(Outcome::Value(balances(p))),
        |_t, _k, _p| {
            Err(ViewError::Source {
                message: "the books are locked".to_owned(),
            })
        },
    );
    let mut s = ReportViewScreen::new(balances(params(ViewDetail::Condensed)), source).unwrap();
    assert!(matches!(s.drill(), ScreenAction::Stay));
    assert_eq!(error_text(&s), "the books are locked");
}

// ---- Alt+F1 ----------------------------------------------------------------------------------

#[test]
fn alt_f1_toggles_detail_and_keeps_the_selection_on_the_same_row() {
    // spec 7: "Alt+F1 detail toggle"; spec 6.2: "Reports reshape in place".
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    // Condensed rows: 0 Current Assets, 1 Capital, 2 note. Select Capital.
    frame(&ctx, &mut s, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.selected_row().unwrap().label, "Capital");
    frame(&ctx, &mut s, key(Key::F1, Modifiers::ALT));
    assert_eq!(s.report().params.detail, ViewDetail::Detailed);
    // derived: the detailed fixture inserts Cash and HDFC after row 0, so Capital is row 3.
    assert_eq!(s.selected(), 3);
    assert_eq!(s.selected_row().unwrap().key, Some(ViewKey::Group(CAPITAL)));
    frame(&ctx, &mut s, key(Key::F1, Modifiers::ALT));
    assert_eq!(s.report().params.detail, ViewDetail::Condensed);
    assert_eq!(s.selected(), 1);
    // The host was asked twice, with the same period and the detail flipped each time.
    let asked = host.refreshes.borrow();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0], params(ViewDetail::Detailed));
    assert_eq!(asked[1], params(ViewDetail::Condensed));
}

#[test]
fn a_selected_row_that_disappears_keeps_its_position() {
    // Cash (row 1 of the detailed report) is not in the condensed report. The selection stays at
    // position 1, which is Capital there: it is clamped, never left past the end.
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    s.select(1);
    s.toggle_detail();
    assert_eq!(s.selected(), 1);
    assert_eq!(s.selected_row().unwrap().label, "Capital");
    // The last detailed row (the note, index 4) is found again by kind and label: row 2.
    let mut t = screen_for(&host, ViewDetail::Detailed);
    t.select(4);
    t.toggle_detail();
    assert_eq!(t.selected(), 2);
    assert_eq!(t.selected_row().unwrap().label, "Figures are in rupees");
}

#[test]
fn an_unsupported_refresh_leaves_the_old_report_and_says_why() {
    // rule A1: when the host cannot do the reshape, the old report stays, still labelled with the
    // period and detail it was run with, and the reason is shown.
    let host = host();
    host.mode.set(Mode::RefreshUnsupported);
    let mut s = screen_for(&host, ViewDetail::Condensed);
    s.toggle_detail();
    assert_eq!(s.report().params.detail, ViewDetail::Condensed);
    assert_eq!(s.report().rows.len(), 3);
    let text = error_text(&s);
    assert!(text.contains("detail level"), "{text}");
    assert!(text.contains("no verified rule for this view"), "{text}");
}

#[test]
fn a_refresh_that_is_not_rectangular_or_fails_changes_nothing() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    host.mode.set(Mode::RefreshRagged);
    s.toggle_detail();
    assert_eq!(s.report().params.detail, ViewDetail::Condensed);
    assert!(error_text(&s).contains("has 2 cells but the report has 1 columns"));
    host.mode.set(Mode::RefreshFails);
    s.toggle_detail();
    assert_eq!(s.report().params.detail, ViewDetail::Condensed);
    assert_eq!(error_text(&s), "the books are locked");
}

// ---- F2 --------------------------------------------------------------------------------------

#[test]
fn f2_opens_the_period_editor_and_enter_applies_the_typed_period() {
    // spec 7: "F2 date/period". Typing 01-04-2025 to 30-06-2025 asks the host for exactly that
    // period, with the detail unchanged.
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    assert!(!s.period_editor_open());
    frame(&ctx, &mut s, key(Key::F2, Modifiers::NONE));
    assert!(s.period_editor_open());
    s.set_period_text("01-04-2025", "30-06-2025");
    frame(&ctx, &mut s, key(Key::Enter, Modifiers::NONE));
    assert!(!s.period_editor_open());
    let asked = host.refreshes.borrow();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].period.from(), date!(2025 - 04 - 01));
    assert_eq!(asked[0].period.to(), date!(2025 - 06 - 30));
    assert_eq!(asked[0].detail, ViewDetail::Condensed);
}

#[test]
fn a_bad_period_keeps_the_editor_open_and_asks_nothing() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    s.open_period_editor();
    s.set_period_text("31-03-26", "30-06-2025");
    s.apply_period_edit();
    assert!(s.period_editor_open());
    assert!(error_text(&s).contains("not a date"));
    s.set_period_text("02-04-2025", "01-04-2025");
    s.apply_period_edit();
    assert!(s.period_editor_open());
    assert!(error_text(&s).contains("after its end"));
    assert!(host.refreshes.borrow().is_empty());
}

#[test]
fn esc_closes_the_period_editor_without_leaving_the_screen() {
    // The screen keeps Esc while the editor is open, so the shell does not pop it.
    let host = host();
    let mut shell = Shell::new(Box::new(screen_for(&host, ViewDetail::Condensed)));
    let ctx = egui::Context::default();
    shell_frame(&ctx, &mut shell, RawInput::default());
    shell_frame(&ctx, &mut shell, key(Key::Enter, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers"]);
    // Open the editor on the child, then Esc: still two screens deep.
    shell_frame(&ctx, &mut shell, key(Key::F2, Modifiers::NONE));
    shell_frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances", "Cash vouchers"]);
    // A second Esc, with no editor open, goes back.
    shell_frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(trail(&shell), ["Balances"]);
    assert!(host.refreshes.borrow().is_empty());
}

#[test]
fn the_editor_starts_with_the_current_period() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Condensed);
    s.open_period_editor();
    // Applying without typing re-asks for the period already shown, in the same form it was
    // written, which proves the fields were filled with it.
    s.apply_period_edit();
    assert_eq!(host.refreshes.borrow()[0].period, fy_2025());
}

// ---- drawing ---------------------------------------------------------------------------------

#[test]
fn the_screen_draws_rows_totals_notes_and_the_editor() {
    let host = host();
    let mut s = screen_for(&host, ViewDetail::Detailed);
    assert_eq!(s.title(), "Balances");
    let ctx = egui::Context::default();
    for _ in 0..3 {
        assert!(matches!(
            frame(&ctx, &mut s, RawInput::default()),
            ScreenAction::Stay
        ));
    }
    s.open_period_editor();
    assert!(matches!(
        frame(&ctx, &mut s, RawInput::default()),
        ScreenAction::Stay
    ));
}

#[test]
fn an_empty_report_and_one_without_totals_draw() {
    let host = host();
    let mut report = balances(params(ViewDetail::Condensed));
    report.totals.clear();
    report.notes.clear();
    let mut s = ReportViewScreen::new(report.clone(), host_source(&host)).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    report.rows.clear();
    let mut empty = ReportViewScreen::new(report, host_source(&host)).unwrap();
    frame(&ctx, &mut empty, RawInput::default());
    frame(&ctx, &mut empty, key(Key::Enter, Modifiers::NONE));
    assert!(empty.notice().is_some());
}

#[test]
fn a_long_register_draws() {
    // spec 8: long registers are virtualised, so 5,000 rows cost the rows in view. A smoke test
    // that the table accepts that many rows and a selection near the end.
    let host = host();
    let mut report = ledger_vouchers(params(ViewDetail::Detailed));
    let template = report.rows[0].clone();
    report.rows = (0..5_000).map(|_| template.clone()).collect();
    let mut s = ReportViewScreen::new(report, host_source(&host)).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut s, RawInput::default());
    frame(&ctx, &mut s, key(Key::End, Modifiers::NONE));
    assert_eq!(s.selected(), 4_999);
}

// ---- pending ---------------------------------------------------------------------------------

#[test]
fn comparative_and_monthly_columns_are_the_hosts_to_provide() {
    // pending: no provenance yet for how comparative or monthly columns should total or print.
    // The screen draws whatever columns the report has and never builds them; a host that cannot
    // do a layout answers Unsupported, which the screen shows (see
    // `an_unsupported_refresh_leaves_the_old_report_and_says_why`).
    let two = ViewReport {
        columns: vec!["Apr 2025".to_owned(), "May 2025".to_owned()],
        rows: vec![row(
            None,
            "Sales",
            0,
            ViewRowKind::Text,
            vec![dr(1_000), ViewCell::Empty],
        )],
        totals: Vec::new(),
        ..balances(params(ViewDetail::Condensed))
    };
    assert!(two.check_shape().is_ok());
}

#[test]
fn the_generic_report_names_its_move_keys_as_the_other_reports_do() {
    // provenance: the mock's report screens name the row keys "J / K" (the balance sheet and day
    // book key bars, design/PARITY-GAPS.md "Up / Down" entry); this screen moves on J and K as
    // well as the arrows (KeyCmd::Down on Key::J). Catches "Up / Down" coming back.
    let host = host();
    let s = screen_for(&host, ViewDetail::Condensed);
    let first = s.keys().into_iter().next().unwrap();
    assert_eq!(
        (first.keys.as_str(), first.label.as_str()),
        ("J / K", "Move")
    );
}
