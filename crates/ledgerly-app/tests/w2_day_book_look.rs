//! The day book drawn to the mock (`design/mock-v2/index.html`, `SCREENS['day-book']` at lines
//! 1636-1660 and the `table.t` rules at lines 148-158): the page head's tabs and subtitle, the
//! table's cells and geometry, the total row, the keys and the date editor.
//!
//! TallyPrime's Day Book opens on the current date, the last date a voucher was entered on; F2
//! changes the date and Alt+F2 asks for a period (the Tally help page "Viewing Day Book",
//! <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Display_Day_Book.htm>
//! and <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Registers_Ledgers/Display_Day_Book.htm>,
//! read 2026-11-23). The mock agrees: its default state is one day with "F2 another date · Alt+F2
//! a period".
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use ledgerly_app::screens::report_view::day_book::{self, SubPart};
use ledgerly_app::screens::report_view::{EditorKind, ReportSource, ReportViewScreen, ViewCell};
use ledgerly_app::shell::KeyHint;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Paise, Period, SignedAmount};
use ledgerly_model::Scenario;
use time::macros::date;

fn day(d: time::Date) -> Period {
    Period::new(d, d).unwrap()
}

// ---- the page head ------------------------------------------------------------------------------

#[test]
fn the_tabs_are_day_and_the_month_by_name() {
    // provenance: index.html:1645: the tabs read "Day" and "November" (the month of the day
    // shown). Derived: in another month the second tab names that month. Catches "Month to date".
    assert_eq!(
        day_book::tab_labels(day(date!(2026 - 11 - 20))),
        ["Day".to_owned(), "November".to_owned()]
    );
    assert_eq!(
        day_book::tab_labels(day(date!(2027 - 03 - 31))),
        ["Day".to_owned(), "March".to_owned()]
    );
}

#[test]
fn a_period_that_is_neither_tab_selects_no_tab() {
    // provenance: derived from index.html:1645, where each tab is one view: Day is one day,
    // November is the month up to the day shown. A period from 1 April to 20 November is neither,
    // so neither tab is underlined. Catches the 2026-11-23 capture, which underlined "Month to
    // date" over 1 Apr 2026 to 20 Nov 2026.
    let year_to_date = Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap();
    assert_eq!(day_book::tab_of(year_to_date), None);
    let month = Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 20)).unwrap();
    assert_eq!(day_book::tab_of(month), Some(day_book::MONTH_TAB));
    assert_eq!(
        day_book::tab_of(day(date!(2026 - 11 - 20))),
        Some(day_book::DAY_TAB)
    );
}

#[test]
fn the_day_subtitle_shows_its_keys_as_keycaps() {
    // provenance: index.html:1644: `${fmtDate(day)} · ${kb('F2')} another date · ${kb('Alt+F2')}
    // a period`, where `kb` draws a keycap.
    assert_eq!(
        day_book::subtitle_parts(day(date!(2026 - 11 - 20))),
        [
            SubPart::Text("20 Nov 2026 \u{00B7} ".to_owned()),
            SubPart::Key("F2"),
            SubPart::Text(" another date \u{00B7} ".to_owned()),
            SubPart::Key("Alt+F2"),
            SubPart::Text(" a period".to_owned()),
        ]
    );
}

#[test]
fn a_range_subtitle_spans_the_range() {
    // provenance: index.html:1644: "1 to 21 Nov 2026 · every voucher, in order" for the month.
    // Derived: a range across months names both dates in full.
    let month = Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 21)).unwrap();
    assert_eq!(
        day_book::subtitle(month),
        "1 to 21 Nov 2026 \u{00B7} every voucher, in order"
    );
    let range = Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap();
    assert_eq!(
        day_book::subtitle(range),
        "1 Apr 2026 to 20 Nov 2026 \u{00B7} every voucher, in order"
    );
}

// ---- the table ----------------------------------------------------------------------------------

#[test]
fn a_date_cell_reads_like_the_mocks_dates() {
    // provenance: index.html:1652: the Date cell is `fmtDate(v.date)`, "20 Nov 2026". Catches
    // "20-Nov-2026".
    assert_eq!(
        day_book::date_text(&ViewCell::Date(date!(2026 - 11 - 02))),
        "2 Nov 2026"
    );
    assert_eq!(day_book::date_text(&ViewCell::Empty), "");
}

#[test]
fn a_voucher_amount_has_no_dr() {
    // provenance: index.html:1652: the Amount cell is `inr(vTotal(v))`, a figure with no side.
    // The engine's voucher amount is the voucher's debit total (ledgerly-reports `amount_cell`),
    // so "Dr" adds nothing. Derived: a credit figure, which the engine does not write today,
    // keeps its "Cr" so it is never read as a debit.
    let dr = ViewCell::Amount(SignedAmount::dr(Paise::new(11_800_000)));
    let cr = ViewCell::Amount(SignedAmount::cr(Paise::new(250_000)));
    assert_eq!(day_book::amount_text(&dr), "1,18,000.00");
    assert_eq!(day_book::amount_text(&cr), "2,500.00 Cr");
}

#[test]
fn the_total_row_counts_the_vouchers() {
    // provenance: index.html:1655: `${list.length} vouchers`, "5 vouchers". Derived: one voucher
    // reads "1 voucher".
    assert_eq!(day_book::total_label(5), "5 vouchers");
    assert_eq!(day_book::total_label(1), "1 voucher");
}

#[test]
fn each_line_of_the_table_has_the_mocks_height() {
    // provenance: derived from index.html:149-150 and 156, as for the balance sheet
    // (`each_line_of_the_statement_has_the_mocks_height`): the heading 6 + 17 + 6 + 1 = 30, a row
    // 7 + 19 + 7 + 1 = 34, the total 7 + 19 + 7 + 3 px double rule = 36. Three vouchers and the
    // total.
    assert_eq!(
        day_book::line_heights(3, true),
        [30.0, 34.0, 34.0, 34.0, 36.0]
    );
    assert_eq!(day_book::line_heights(1, false), [30.0, 34.0]);
}

// ---- keys and the date editor -------------------------------------------------------------------

fn day_book_screen() -> ReportViewScreen {
    let params = ReportParams {
        kind: ReportKind::DayBook,
        period: day(date!(2026 - 11 - 20)),
        as_of: date!(2026 - 11 - 20),
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    ReportViewScreen::worker(ReportSource::worker(params)).unwrap()
}

fn pairs(hints: &[KeyHint]) -> Vec<(String, String)> {
    hints
        .iter()
        .map(|h| (h.keys.clone(), h.label.clone()))
        .collect()
}

#[test]
fn the_key_bar_reads_like_the_mocks() {
    // provenance: index.html:1659: "J / K" Move, Enter Open voucher, Ctrl+I Origin, F2 Date,
    // Esc Back. Ctrl+I needs who entered a voucher and how (engine item E-4), so it is not
    // offered (design/PARITY-GAPS.md). Catches "F2 Period".
    let expected: Vec<(String, String)> = [
        ("J / K", "Move"),
        ("Enter", "Open voucher"),
        ("F2", "Date"),
        ("Esc", "Back"),
    ]
    .iter()
    .map(|(k, l)| ((*k).to_owned(), (*l).to_owned()))
    .collect();
    assert_eq!(
        pairs(&day_book_screen().keys(&ledgerly_app::i18n::Tr::default())),
        expected
    );
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

fn frame(screen: &mut ReportViewScreen, input: RawInput) {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let out = ctx.run_ui(input, |ui| {
        screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn f2_asks_for_a_date_and_alt_f2_for_a_period() {
    // provenance: TallyHelp "Viewing Day Book" (module header): F2 changes the date, Alt+F2 asks
    // for a From and a To date. The mock says the same (index.html:1644). Catches F2 opening the
    // from-to editor in the day book.
    let mut screen = day_book_screen();
    frame(&mut screen, press(Key::F2, Modifiers::NONE));
    assert_eq!(screen.editor_kind(), Some(EditorKind::Date));
    screen.cancel_period_edit();
    frame(&mut screen, press(Key::F2, Modifiers::ALT));
    assert_eq!(screen.editor_kind(), Some(EditorKind::Period));
}
