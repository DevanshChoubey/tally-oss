//! Card U12: audit views. The screen shows strings that card RP10 produced and computes nothing,
//! so the cases here are about layout logic only: reading the period, queueing requests, folding
//! child rows, filtering rows, reading the certificate verdict and never hiding an `Unsupported`
//! answer. Every expected value is derived by hand next to the case (calendar facts and row
//! indices counted from the fixture), never taken from this crate's output.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeSet;

use egui::RawInput;
use ledgerly_app::screens::audit::{
    AuditInputError, AuditRequest, AuditRow, AuditScreen, AuditState, AuditTable, AuditVerdict,
    AuditViewKind, format_period_date, visible_rows,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Period, PeriodError};
use time::macros::date;

fn fy_2026_27() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn row(depth: u8, cells: &[&str]) -> AuditRow {
    AuditRow {
        depth,
        cells: cells.iter().map(|c| (*c).to_owned()).collect(),
    }
}

/// Six rows: A(0) with two children, B(0) with one child, C(0).
///
/// Indices: 0 A, 1 a1, 2 a2, 3 B, 4 b1, 5 C.
fn grouped() -> Vec<AuditRow> {
    vec![
        row(0, &["1", "Alpha voucher"]),
        row(1, &["", "amount 100 -> 200"]),
        row(1, &["", "narration x -> y"]),
        row(0, &["2", "Beta voucher"]),
        row(1, &["", "date moved"]),
        row(0, &["3", "Gamma voucher"]),
    ]
}

fn table(rows: Vec<AuditRow>) -> AuditTable {
    AuditTable {
        title: "Test".to_owned(),
        columns: vec!["Seq".to_owned(), "What".to_owned()],
        rows,
        notes: vec!["a note".to_owned()],
    }
}

fn frame(screen: &mut AuditScreen) -> ScreenAction {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Pop;
    let out = ctx.run_ui(RawInput::default(), |ui| {
        action = screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
    action
}

// ---- kinds -----------------------------------------------------------------------------------

#[test]
fn six_kinds_each_with_its_own_label() {
    // provenance: derived - spec 6.6 and the card RP10 module doc list six auditor reports:
    // object history, edit log, deleted/altered, back-dated, edits after lock, certificate.
    let labels: BTreeSet<&str> = AuditViewKind::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(AuditViewKind::ALL.len(), 6);
    assert_eq!(labels.len(), 6);
    assert!(
        AuditViewKind::ALL
            .iter()
            .all(|k| !k.description().is_empty())
    );
}

#[test]
fn only_the_certificate_ignores_the_period() {
    // provenance: derived - RP10 module doc: the certificate renders a chain verification of the
    // whole log; the other five select records by `params.period`.
    for kind in AuditViewKind::ALL {
        assert_eq!(
            kind.uses_period(),
            kind != AuditViewKind::IntegrityCertificate,
            "{kind:?}"
        );
    }
}

// ---- period boxes ----------------------------------------------------------------------------

#[test]
fn new_screen_shows_the_given_period() {
    // provenance: derived - Indian FY 2026-27 runs 1 April 2026 to 31 March 2027.
    let screen = AuditScreen::new(fy_2026_27());
    assert_eq!(screen.period().unwrap(), fy_2026_27());
    assert_eq!(format_period_date(date!(2026 - 04 - 01)), "01-04-2026");
    assert_eq!(format_period_date(date!(2027 - 03 - 31)), "31-03-2027");
}

#[test]
fn period_accepts_day_first_and_year_first_with_dash_or_slash() {
    // provenance: derived - the same two calendar days written four ways.
    let mut screen = AuditScreen::new(fy_2026_27());
    for (from, to) in [
        ("01-04-2026", "30-04-2026"),
        ("1/4/2026", "30/4/2026"),
        ("2026-04-01", "2026-04-30"),
        ("  2026/4/1 ", "2026/04/30"),
    ] {
        screen.set_period_text(from, to);
        let want = Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap();
        assert_eq!(screen.period().unwrap(), want, "{from} .. {to}");
    }
}

#[test]
fn period_rejects_what_is_not_a_full_date() {
    // provenance: derived - 2026 is not a leap year, so February has 28 days; there is no month
    // 13; a two-digit year or a day-month pair is refused (an audit period never guesses a year).
    let mut screen = AuditScreen::new(fy_2026_27());
    for bad in [
        "31-02-2026",
        "01-13-2026",
        "1-4-26",
        "15-04",
        "abc",
        "1--2026",
        "01-04-2026-1",
    ] {
        screen.set_period_text(bad, "30-04-2026");
        assert!(
            matches!(
                screen.period(),
                Err(AuditInputError::Format { which: "from", .. })
            ),
            "{bad}"
        );
    }
    screen.set_period_text("01-04-2026", "");
    assert_eq!(
        screen.period().unwrap_err(),
        AuditInputError::Empty { which: "to" }
    );
}

#[test]
fn reversed_period_is_an_error_naming_both_dates() {
    // provenance: derived - 30 April 2026 is after 01 April 2026.
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_period_text("30-04-2026", "01-04-2026");
    assert_eq!(
        screen.period().unwrap_err(),
        AuditInputError::Period(PeriodError::Reversed {
            from: date!(2026 - 04 - 30),
            to: date!(2026 - 04 - 01),
        })
    );
}

// ---- requests --------------------------------------------------------------------------------

#[test]
fn nothing_is_requested_until_run() {
    let mut screen = AuditScreen::new(fy_2026_27());
    assert_eq!(screen.state(), &AuditState::NotRun);
    assert_eq!(screen.take_request(), None);
}

#[test]
fn run_queues_one_request_for_the_selected_kind_and_period() {
    // provenance: derived - the screen starts on the edit log (the report that lists everything
    // in a period); the request carries the typed period unchanged.
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_period_text("01-04-2026", "30-04-2026");
    screen.run();
    assert_eq!(screen.state(), &AuditState::Running);
    let want = AuditRequest {
        kind: AuditViewKind::EditLog,
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap(),
    };
    assert_eq!(screen.take_request(), Some(want));
    assert_eq!(screen.take_request(), None, "a request is handed over once");
}

#[test]
fn picking_a_kind_drops_the_old_table_and_asks_for_the_new_report() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_table(table(grouped()));
    screen.select_kind(AuditViewKind::BackDated);
    assert_eq!(screen.kind(), AuditViewKind::BackDated);
    assert_eq!(screen.state(), &AuditState::Running);
    assert_eq!(
        screen.take_request().map(|r| r.kind),
        Some(AuditViewKind::BackDated)
    );
    assert_eq!(screen.visible(), [] as [usize; 0]);
}

#[test]
fn a_bad_period_on_a_new_report_drops_the_old_table_and_its_queued_request() {
    // provenance: derived - the table on show must name the report the selector shows. Loaded
    // under the integrity certificate, then a bad period and a pick of "Edit log": no table may
    // remain, and the certificate's queued request must not reach the host. The old code kept the
    // certificate's table under the "Edit log" heading and copy-rows button.
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.select_kind(AuditViewKind::IntegrityCertificate);
    assert!(screen.take_request().is_some());
    screen.set_table(table(grouped()));
    assert!(matches!(screen.state(), AuditState::Loaded(_)));
    screen.set_period_text("nope", "30-04-2026");
    screen.select_kind(AuditViewKind::EditLog);
    assert_eq!(screen.kind(), AuditViewKind::EditLog);
    assert_eq!(screen.state(), &AuditState::NotRun);
    assert!(screen.input_error().is_some());
    assert_eq!(screen.visible(), [] as [usize; 0]);
    assert_eq!(screen.take_request(), None);

    // A request queued for the certificate, then a bad period on another report: the certificate
    // request is dropped as well.
    let mut queued = AuditScreen::new(fy_2026_27());
    queued.select_kind(AuditViewKind::IntegrityCertificate);
    queued.set_period_text("nope", "30-04-2026");
    queued.select_kind(AuditViewKind::BackDated);
    assert_eq!(queued.take_request(), None);
    assert_eq!(queued.state(), &AuditState::NotRun);
}

#[test]
fn a_bad_period_queues_nothing_and_keeps_what_is_shown() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_table(table(grouped()));
    screen.set_period_text("nope", "30-04-2026");
    screen.run();
    assert!(screen.input_error().is_some());
    assert_eq!(screen.take_request(), None);
    assert!(matches!(screen.state(), AuditState::Loaded(_)));
    // A good period clears the error.
    screen.set_period_text("01-04-2026", "30-04-2026");
    screen.run();
    assert!(screen.input_error().is_none());
    assert!(screen.take_request().is_some());
}

#[test]
fn the_certificate_does_not_need_a_valid_period() {
    // provenance: derived - the certificate covers the whole log (RP10 `integrity_certificate`
    // takes only the verification result), so unusable period boxes must not block it.
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_period_text("", "garbage");
    screen.select_kind(AuditViewKind::IntegrityCertificate);
    assert!(screen.input_error().is_none());
    assert_eq!(
        screen.take_request().map(|r| r.kind),
        Some(AuditViewKind::IntegrityCertificate)
    );
}

// ---- answers ---------------------------------------------------------------------------------

#[test]
fn unsupported_is_kept_with_its_reason_and_shows_no_rows() {
    // Rule A1: an Unsupported outcome is shown as such, never as an empty table.
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_unsupported("edit log filtered by ledger", "narrow by voucher type only");
    assert_eq!(
        screen.state(),
        &AuditState::Unsupported {
            what: "edit log filtered by ledger".to_owned(),
            reason: "narrow by voucher type only".to_owned(),
        }
    );
    assert_eq!(screen.visible(), [] as [usize; 0]);
}

#[test]
fn failure_is_kept() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_failed("event sequence number 7 appears more than once");
    assert_eq!(
        screen.state(),
        &AuditState::Failed("event sequence number 7 appears more than once".to_owned())
    );
}

// ---- folding and filtering -------------------------------------------------------------------

#[test]
fn all_rows_are_visible_when_nothing_is_folded_or_filtered() {
    // provenance: derived - six rows, indices 0..=5.
    let rows = grouped();
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), ""),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), "   "),
        vec![0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn folding_a_row_hides_only_its_children() {
    // provenance: derived - row 0's children are rows 1 and 2; row 3's child is row 4.
    let rows = grouped();
    let fold = |idx: &[usize]| idx.iter().copied().collect::<BTreeSet<usize>>();
    assert_eq!(visible_rows(&rows, &fold(&[0]), ""), vec![0, 3, 4, 5]);
    assert_eq!(visible_rows(&rows, &fold(&[3]), ""), vec![0, 1, 2, 3, 5]);
    assert_eq!(visible_rows(&rows, &fold(&[0, 3]), ""), vec![0, 3, 5]);
    // Folding a row with no children, or a child row, changes nothing.
    assert_eq!(visible_rows(&rows, &fold(&[5]), ""), vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(visible_rows(&rows, &fold(&[1]), ""), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn folding_respects_nesting_depth() {
    // provenance: derived - depths 0,1,2,1: row 2 nests under row 1, row 3 is a sibling of row 1.
    let rows = vec![
        row(0, &["top"]),
        row(1, &["mid"]),
        row(2, &["leaf"]),
        row(1, &["mid2"]),
    ];
    let fold = |idx: &[usize]| idx.iter().copied().collect::<BTreeSet<usize>>();
    assert_eq!(visible_rows(&rows, &fold(&[1]), ""), vec![0, 1, 3]);
    assert_eq!(visible_rows(&rows, &fold(&[0]), ""), vec![0]);
}

#[test]
fn filter_keeps_whole_groups_that_contain_the_text_ignoring_case() {
    // provenance: derived - "ALPHA" occurs only in row 0 (group rows 0..=2); "date moved" only in
    // row 4 (group rows 3..=4); "voucher" occurs in the first row of every group.
    let rows = grouped();
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), "ALPHA"),
        vec![0, 1, 2]
    );
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), "date moved"),
        vec![3, 4]
    );
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), "voucher"),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        visible_rows(&rows, &BTreeSet::new(), "delta"),
        Vec::<usize>::new()
    );
}

#[test]
fn filter_and_fold_combine() {
    // provenance: derived - the group of row 3 matches "beta"; folding row 3 hides row 4.
    let rows = grouped();
    let folded: BTreeSet<usize> = [3].into_iter().collect();
    assert_eq!(visible_rows(&rows, &folded, "beta"), vec![3]);
}

#[test]
fn rows_that_start_nested_are_still_shown() {
    // A malformed table (first row depth 1) must not lose rows or loop.
    let rows = vec![row(1, &["orphan"]), row(0, &["top"])];
    assert_eq!(visible_rows(&rows, &BTreeSet::new(), ""), vec![0, 1]);
}

#[test]
fn has_children_looks_at_the_next_row_depth() {
    // provenance: derived - from the fixture: rows 0 and 3 are followed by deeper rows.
    let t = table(grouped());
    let got: Vec<bool> = (0..7).map(|i| t.has_children(i)).collect();
    assert_eq!(got, vec![true, false, false, true, false, false, false]);
}

#[test]
fn toggling_flips_and_a_new_table_unfolds_everything() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_table(table(grouped()));
    assert!(!screen.is_collapsed(0));
    screen.toggle_row(0);
    assert!(screen.is_collapsed(0));
    assert_eq!(screen.visible(), vec![0, 3, 4, 5]);
    screen.toggle_row(0);
    assert!(!screen.is_collapsed(0));
    screen.toggle_row(0);
    screen.set_table(table(grouped()));
    assert!(!screen.is_collapsed(0));
    assert_eq!(screen.visible(), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn screen_filter_applies_to_the_loaded_table() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.set_table(table(grouped()));
    screen.set_filter("gamma");
    assert_eq!(screen.filter(), "gamma");
    assert_eq!(screen.visible(), vec![5]);
}

// ---- certificate verdict ---------------------------------------------------------------------

#[test]
fn verdict_is_read_only_from_an_exact_result_row() {
    // provenance: derived - RP10 `integrity_certificate` writes a first row labelled "Result"
    // whose value is exactly "PASS" or "FAIL". Anything else must not read as a pass.
    let with = |value: &str| {
        table(vec![
            row(0, &["Events checked", "9"]),
            row(0, &["Result", value]),
        ])
    };
    assert_eq!(with("PASS").verdict(), Some(AuditVerdict::Pass));
    assert_eq!(with("FAIL").verdict(), Some(AuditVerdict::Fail));
    assert_eq!(with("pass").verdict(), None);
    assert_eq!(with("PASS (partial)").verdict(), None);
    assert_eq!(with("").verdict(), None);
    assert_eq!(table(vec![row(0, &["Head", "PASS"])]).verdict(), None);
    assert_eq!(table(Vec::new()).verdict(), None);
    assert_eq!(table(vec![row(0, &["Result"])]).verdict(), None);
}

// ---- copy ------------------------------------------------------------------------------------

#[test]
fn tsv_has_the_header_and_only_the_visible_rows() {
    // provenance: derived - header "Seq<TAB>What", then rows 0 and 3 joined by tabs; a tab or
    // line break inside a cell becomes a space so a row stays on one line.
    let mut rows = grouped();
    rows[0] = row(0, &["1", "Alpha\tvoucher\nline"]);
    let t = table(rows);
    assert_eq!(
        t.to_tsv(&[0, 3]),
        "Seq\tWhat\n1\tAlpha voucher line\n2\tBeta voucher\n"
    );
    assert_eq!(t.to_tsv(&[]), "Seq\tWhat\n");
    assert_eq!(
        t.to_tsv(&[99]),
        "Seq\tWhat\n",
        "an index past the end is skipped"
    );
}

// ---- drawing ---------------------------------------------------------------------------------

#[test]
fn title_is_audit_trail() {
    assert_eq!(
        AuditScreen::new(fy_2026_27()).title(&ledgerly_app::i18n::Tr::default()),
        "Audit trail"
    );
}

#[test]
fn every_state_draws_and_stays() {
    let mut screen = AuditScreen::new(fy_2026_27());
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.run();
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.set_table(table(grouped()));
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.toggle_row(0);
    screen.set_filter("beta");
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.set_unsupported("x", "y");
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.set_failed("boom");
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.set_period_text("bad", "bad");
    screen.run();
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
}

#[test]
fn certificate_and_empty_and_ragged_tables_draw() {
    let mut screen = AuditScreen::new(fy_2026_27());
    screen.select_kind(AuditViewKind::IntegrityCertificate);
    screen.set_table(table(vec![
        row(0, &["Result", "FAIL"]),
        row(1, &["Problem", "event 4 hash does not match"]),
    ]));
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    // No columns, no rows, and rows with fewer cells than columns.
    screen.set_table(AuditTable::default());
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
    screen.set_table(table(vec![row(0, &["only one cell"]), row(0, &[])]));
    assert!(matches!(frame(&mut screen), ScreenAction::Stay));
}
