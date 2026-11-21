//! Card U10: the Tally import wizard (spec 6.7). Every expected value is derived by hand and the
//! derivation is written next to it; nothing is copied from this crate's output. Figures are
//! fictional. No Tally release is involved, so nothing here is `observed`.
//!
//! Not covered because there is no provenance for it yet (the screen does not offer it):
//! AI column mapping (spec 6.8), pull periods that are not whole financial years, live sync.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::import::{
    DetectError, DetectState, ImportError, ImportRequest, ImportScreen, ImportSource, ImportWizard,
    MapKind, MapReport, MapRow, MapState, MapStatus, PullProgress, PullStage, PullState,
    PullSummary, ReconReport, ReconRow, ReconSection, ReconSectionReport, ReconState, RowStatus,
    Step, TALLY_PORT, format_signed_amount,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Date, FinancialYear, Outcome, Paise, Period, PeriodError, SignedAmount};
use time::macros::date;

// ---- helpers ---------------------------------------------------------------------------------

fn today() -> Date {
    date!(2026 - 11 - 21)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// A wizard that found two companies and chose the first.
fn at_choose() -> ImportWizard {
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: TALLY_PORT })
        .unwrap();
    w.detect_result(Ok(names(&["Demo Traders", "Sample Mills"])))
        .unwrap();
    w.select_company(0).unwrap();
    w
}

fn summary(batches: u32) -> PullSummary {
    PullSummary {
        vouchers: 12,
        batches,
        capture_dir: PathBuf::from("capture"),
    }
}

/// A wizard whose pull finished.
fn at_pull_done() -> ImportWizard {
    let mut w = at_choose();
    w.start_pull().unwrap();
    w.pull_finished(summary(1)).unwrap();
    w
}

fn map_row(kind: MapKind, name: &str, status: MapStatus) -> MapRow {
    MapRow {
        kind,
        tally_name: name.to_owned(),
        guid: None,
        status,
    }
}

fn clean_map() -> MapReport {
    MapReport {
        rows: vec![map_row(MapKind::Ledger, "Cash", MapStatus::Mapped)],
        warnings: Vec::new(),
    }
}

/// A wizard waiting for the books to take the data.
fn at_committing() -> ImportWizard {
    let mut w = at_pull_done();
    w.continue_to_map().unwrap();
    w.map_ready(clean_map()).unwrap();
    w.confirm_map().unwrap();
    w
}

fn dr(paise: i64) -> Option<SignedAmount> {
    Some(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> Option<SignedAmount> {
    Some(SignedAmount::cr(Paise::new(paise)))
}

fn row(label: &str, ours: Option<SignedAmount>, theirs: Option<SignedAmount>) -> ReconRow {
    ReconRow {
        label: label.to_owned(),
        ours,
        theirs,
    }
}

fn period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn section(section: ReconSection, rows: Vec<ReconRow>) -> ReconSectionReport {
    ReconSectionReport {
        section,
        outcome: Outcome::Value(rows),
    }
}

/// All six sections, one matching row each.
fn matching_sections() -> Vec<ReconSectionReport> {
    ReconSection::ALL
        .iter()
        .map(|s| section(*s, vec![row("item", dr(500_000), dr(500_000))]))
        .collect()
}

// ---- steps and stages ------------------------------------------------------------------------

#[test]
fn steps_follow_spec_6_7_order() {
    // provenance: derived — spec 6.7 and plan card U10: detect → choose company → pull → map →
    // reconciliation report.
    let labels: Vec<&str> = Step::ALL.iter().map(|s| s.label()).collect();
    assert_eq!(
        labels,
        ["Detect", "Choose company", "Pull", "Map", "Reconcile"]
    );
}

#[test]
fn pull_stages_are_in_dependency_order() {
    // provenance: derived — spec 6.7 step 2: groups → ledgers → units → stock groups/categories →
    // items → godowns → cost categories/centres → voucher types, then vouchers.
    assert_eq!(
        PullStage::ALL,
        [
            PullStage::Groups,
            PullStage::Ledgers,
            PullStage::Units,
            PullStage::StockGroupsAndCategories,
            PullStage::StockItems,
            PullStage::Godowns,
            PullStage::CostCategoriesAndCentres,
            PullStage::VoucherTypes,
            PullStage::Vouchers,
        ]
    );
}

#[test]
fn reconciliation_covers_the_six_reports_of_spec_6_7() {
    // provenance: derived — spec 6.7 step 4: TB, P&L, BS, stock summary, party outstandings and
    // GST summaries.
    let labels: Vec<&str> = ReconSection::ALL.iter().map(|s| s.label()).collect();
    assert_eq!(
        labels,
        [
            "Trial balance",
            "Profit and loss",
            "Balance sheet",
            "Stock summary",
            "Party outstandings",
            "GST summary"
        ]
    );
}

// ---- detect ----------------------------------------------------------------------------------

#[test]
fn detect_asks_for_localhost_port_9000() {
    // provenance: derived — spec 6.7 step 1: "Detect Tally on localhost:9000".
    let mut w = ImportWizard::new(today()).unwrap();
    assert_eq!(TALLY_PORT, 9000);
    w.start_detect(ImportSource::Live { port: TALLY_PORT })
        .unwrap();
    let reqs = w.take_requests();
    assert!(matches!(
        reqs.as_slice(),
        [ImportRequest::Detect {
            source: ImportSource::Live { port: 9000 }
        }]
    ));
    assert_eq!(*w.detect_state(), DetectState::Checking);
    assert!(w.take_requests().is_empty());
}

#[test]
fn detect_cannot_be_started_twice_while_a_request_is_out() {
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: TALLY_PORT })
        .unwrap();
    let err = w
        .start_detect(ImportSource::Live { port: TALLY_PORT })
        .unwrap_err();
    assert!(matches!(err, ImportError::WrongStep { .. }));
    // One request only.
    assert_eq!(w.take_requests().len(), 1);
}

#[test]
fn offline_path_asks_to_read_the_exported_file() {
    // provenance: derived — spec 6.7 step 5: "parse XML files the user exported from Tally".
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Export(PathBuf::from("books.xml")))
        .unwrap();
    let reqs = w.take_requests();
    let [ImportRequest::Detect { source }] = reqs.as_slice() else {
        panic!("expected one detect request");
    };
    assert_eq!(*source, ImportSource::Export(PathBuf::from("books.xml")));
    assert_eq!(
        *w.source(),
        ImportSource::Export(PathBuf::from("books.xml"))
    );
}

#[test]
fn tally_not_running_names_the_port_and_stays_on_detect() {
    // provenance: derived — plan card B1: connection refused → TallyNotRunning with the port.
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
    w.detect_result(Err(DetectError::NotRunning { port: 9000 }))
        .unwrap();
    assert_eq!(*w.detect_state(), DetectState::NotRunning { port: 9000 });
    assert_eq!(w.step(), Step::Detect);
    // The user can try again.
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
}

#[test]
fn other_detect_errors_show_their_reason() {
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
    w.detect_result(Err(DetectError::Other("HTTP 500".to_owned())))
        .unwrap();
    assert_eq!(
        *w.detect_state(),
        DetectState::Failed("HTTP 500".to_owned())
    );
    assert_eq!(w.step(), Step::Detect);
}

#[test]
fn an_empty_company_list_is_a_failure_not_an_empty_choice() {
    // provenance: derived — rule A1: no silent default; a step with nothing to choose from must
    // say so rather than advance.
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
    w.detect_result(Ok(Vec::new())).unwrap();
    assert!(matches!(w.detect_state(), DetectState::Failed(_)));
    assert_eq!(w.step(), Step::Detect);
}

#[test]
fn companies_move_the_wizard_to_the_choose_step_without_choosing_one() {
    // provenance: derived — rule A1 (no silent default): the user picks the company.
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
    w.detect_result(Ok(names(&["Only Company"]))).unwrap();
    assert_eq!(w.step(), Step::ChooseCompany);
    assert_eq!(w.companies(), ["Only Company".to_owned()]);
    assert_eq!(w.selected(), None);
    assert_eq!(w.start_pull().unwrap_err(), ImportError::NoCompany);
}

#[test]
fn a_late_detect_reply_is_refused() {
    let mut w = ImportWizard::new(today()).unwrap();
    // No request is out.
    let err = w.detect_result(Ok(names(&["X"]))).unwrap_err();
    assert!(matches!(err, ImportError::WrongStep { .. }));
    assert_eq!(w.step(), Step::Detect);
}

// ---- choose company and period ---------------------------------------------------------------

#[test]
fn year_choices_end_with_the_current_financial_year() {
    // provenance: derived — Indian FY runs 1 April to 31 March, so 21 Nov 2026 is in FY 2026-27;
    // the four years before it start on 1 April 2022, 2023, 2024 and 2025.
    let w = ImportWizard::new(today()).unwrap();
    let starts: Vec<Date> = w.fy_options().iter().map(|fy| fy.start).collect();
    assert_eq!(
        starts,
        [
            date!(2022 - 04 - 01),
            date!(2023 - 04 - 01),
            date!(2024 - 04 - 01),
            date!(2025 - 04 - 01),
            date!(2026 - 04 - 01),
        ]
    );
    let (first, last) = w.years();
    assert_eq!(first.start, date!(2026 - 04 - 01));
    assert_eq!(last.start, date!(2026 - 04 - 01));
}

#[test]
fn march_belongs_to_the_previous_financial_year() {
    // provenance: derived — 31 March 2026 is the last day of FY 2025-26.
    let w = ImportWizard::new(date!(2026 - 03 - 31)).unwrap();
    assert_eq!(w.years().0.start, date!(2025 - 04 - 01));
}

#[test]
fn default_period_is_the_current_financial_year() {
    // provenance: derived — FY 2026-27 is 1 April 2026 to 31 March 2027 inclusive.
    let w = ImportWizard::new(today()).unwrap();
    let p = w.period().unwrap();
    assert_eq!(p.from(), date!(2026 - 04 - 01));
    assert_eq!(p.to(), date!(2027 - 03 - 31));
}

#[test]
fn several_years_make_one_period() {
    // provenance: derived — FY 2024-25 starts 1 April 2024; FY 2025-26 ends 31 March 2026.
    let mut w = at_choose();
    let first = FinancialYear::april(2024).unwrap();
    let last = FinancialYear::april(2025).unwrap();
    w.set_years(first, last).unwrap();
    let p = w.period().unwrap();
    assert_eq!(p.from(), date!(2024 - 04 - 01));
    assert_eq!(p.to(), date!(2026 - 03 - 31));
}

#[test]
fn reversed_years_are_refused_and_change_nothing() {
    // provenance: derived — a period cannot end before it starts.
    let mut w = at_choose();
    let before = w.years();
    let err = w
        .set_years(
            FinancialYear::april(2026).unwrap(),
            FinancialYear::april(2025).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        ImportError::Period(PeriodError::Reversed { .. })
    ));
    assert_eq!(w.years(), before);
}

#[test]
fn a_company_number_outside_the_list_is_refused() {
    let mut w = at_choose();
    assert_eq!(
        w.select_company(2).unwrap_err(),
        ImportError::CompanyIndex { index: 2, count: 2 }
    );
    assert_eq!(w.selected(), Some(0));
}

// ---- pull ------------------------------------------------------------------------------------

#[test]
fn the_pull_request_carries_company_period_and_a_live_cancel_token() {
    // provenance: derived — spec 6.7 step 2 (pull the chosen company for the chosen dates) and
    // rule D2 (every import can be cancelled).
    let mut w = at_choose();
    w.select_company(1).unwrap();
    w.take_requests();
    w.start_pull().unwrap();
    assert_eq!(w.step(), Step::Pull);
    assert!(matches!(w.pull_state(), PullState::Running(_)));
    let reqs = w.take_requests();
    let [
        ImportRequest::Pull {
            source,
            company,
            period,
            cancel,
        },
    ] = reqs.as_slice()
    else {
        panic!("expected one pull request");
    };
    assert_eq!(*source, ImportSource::Live { port: 9000 });
    assert_eq!(company, "Sample Mills");
    assert_eq!(period.from(), date!(2026 - 04 - 01));
    assert_eq!(period.to(), date!(2027 - 03 - 31));
    assert!(!cancel.is_cancelled());
    w.cancel().unwrap();
    assert!(cancel.is_cancelled());
}

#[test]
fn batch_percent_rounds_down() {
    // provenance: derived — 2 of 8 batches = 2 × 100 / 8 = 25; 1 of 3 = 100 / 3 = 33.33, shown
    // as 33 so that 100 means every batch is done.
    let progress = |done, total| PullProgress {
        stage: PullStage::Vouchers,
        batches_done: done,
        batches_total: total,
        vouchers_pulled: 0,
    };
    assert_eq!(progress(2, Some(8)).batch_percent(), Some(25));
    assert_eq!(progress(1, Some(3)).batch_percent(), Some(33));
    assert_eq!(progress(0, Some(3)).batch_percent(), Some(0));
    assert_eq!(progress(3, Some(3)).batch_percent(), Some(100));
}

#[test]
fn batch_percent_is_unknown_without_a_usable_total() {
    // provenance: derived — a percentage of an unknown or zero total is not a number (rule A1).
    let progress = |done, total| PullProgress {
        stage: PullStage::Vouchers,
        batches_done: done,
        batches_total: total,
        vouchers_pulled: 0,
    };
    assert_eq!(progress(2, None).batch_percent(), None);
    assert_eq!(progress(0, Some(0)).batch_percent(), None);
}

#[test]
fn batch_percent_never_exceeds_100() {
    // provenance: derived — 5 done of 3 is a host error; the display clamps to all done.
    let p = PullProgress {
        stage: PullStage::Vouchers,
        batches_done: 5,
        batches_total: Some(3),
        vouchers_pulled: 0,
    };
    assert_eq!(p.batch_percent(), Some(100));
}

#[test]
fn progress_updates_while_running() {
    let mut w = at_choose();
    w.start_pull().unwrap();
    let p = PullProgress {
        stage: PullStage::Ledgers,
        batches_done: 0,
        batches_total: None,
        vouchers_pulled: 0,
    };
    w.pull_progress(p).unwrap();
    assert_eq!(*w.pull_state(), PullState::Running(p));
}

#[test]
fn cancel_asks_the_pull_to_stop_and_nothing_is_imported() {
    // provenance: derived — rule D2 (cancellation stops promptly, leaves no partial state) and
    // plan card B2 (raw capture stays consistent); the wizard must not advance after a cancel.
    let mut w = at_choose();
    w.take_requests();
    w.start_pull().unwrap();
    w.cancel().unwrap();
    assert!(matches!(w.pull_state(), PullState::Cancelling(_)));
    assert!(!w.can_leave());
    // The host reports it stopped after 3 batches.
    w.pull_cancelled(3).unwrap();
    assert_eq!(*w.pull_state(), PullState::Cancelled { batches_kept: 3 });
    assert!(w.can_leave());
    assert!(matches!(
        w.continue_to_map().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
    assert_eq!(w.step(), Step::Pull);
    // Only the pull request was ever made: no map or commit request.
    let reqs = w.take_requests();
    assert_eq!(reqs.len(), 1);
}

#[test]
fn a_pull_that_finishes_after_cancel_is_discarded() {
    // provenance: derived — the user asked to cancel, so the result must not be imported even if
    // the last batch landed first.
    let mut w = at_choose();
    w.start_pull().unwrap();
    w.cancel().unwrap();
    w.pull_finished(summary(3)).unwrap();
    assert_eq!(*w.pull_state(), PullState::Cancelled { batches_kept: 3 });
}

#[test]
fn cancel_when_nothing_runs_is_refused() {
    let mut w = at_choose();
    assert!(matches!(
        w.cancel().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
    w.start_pull().unwrap();
    w.pull_finished(summary(1)).unwrap();
    assert!(matches!(
        w.cancel().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
}

#[test]
fn a_cancelled_pull_can_be_started_again_with_a_fresh_token() {
    // provenance: derived — the first token stays cancelled; reusing it would stop the retry.
    let mut w = at_choose();
    w.take_requests();
    w.start_pull().unwrap();
    w.cancel().unwrap();
    w.pull_cancelled(0).unwrap();
    w.back().unwrap();
    assert_eq!(w.step(), Step::ChooseCompany);
    assert_eq!(*w.pull_state(), PullState::Idle);
    w.start_pull().unwrap();
    let reqs = w.take_requests();
    assert_eq!(reqs.len(), 2);
    let tokens: Vec<bool> = reqs
        .iter()
        .filter_map(|r| match r {
            ImportRequest::Pull { cancel, .. } => Some(cancel.is_cancelled()),
            ImportRequest::Detect { .. }
            | ImportRequest::Map { .. }
            | ImportRequest::Commit { .. }
            | ImportRequest::Reconcile { .. } => None,
        })
        .collect();
    // The first request (before the retry) was cancelled, the second is live.
    assert_eq!(tokens, [true, false]);
}

#[test]
fn a_failed_pull_shows_its_reason_and_can_go_back() {
    let mut w = at_choose();
    w.start_pull().unwrap();
    w.pull_failed("batch 2: connection reset".to_owned())
        .unwrap();
    assert_eq!(
        *w.pull_state(),
        PullState::Failed("batch 2: connection reset".to_owned())
    );
    w.back().unwrap();
    assert_eq!(w.step(), Step::ChooseCompany);
}

#[test]
fn cannot_leave_or_go_back_while_the_pull_runs() {
    let mut w = at_choose();
    w.start_pull().unwrap();
    assert!(!w.can_leave());
    assert!(matches!(
        w.back().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
}

#[test]
fn closing_cancels_outstanding_work() {
    // provenance: derived — rule D2: leaving the screen stops the host's request.
    let mut w = at_choose();
    w.take_requests();
    w.start_pull().unwrap();
    let reqs = w.take_requests();
    let [ImportRequest::Pull { cancel, .. }] = reqs.as_slice() else {
        panic!("expected one pull request");
    };
    assert!(!cancel.is_cancelled());
    w.close();
    assert!(cancel.is_cancelled());
}

// ---- map -------------------------------------------------------------------------------------

#[test]
fn mapping_is_requested_only_after_a_finished_pull() {
    let mut w = at_choose();
    assert!(matches!(
        w.continue_to_map().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
    let mut w = at_pull_done();
    w.take_requests();
    w.continue_to_map().unwrap();
    assert_eq!(w.step(), Step::Map);
    assert_eq!(*w.map_state(), MapState::Working);
    let reqs = w.take_requests();
    let [ImportRequest::Map { company }] = reqs.as_slice() else {
        panic!("expected one map request");
    };
    assert_eq!(company, "Demo Traders");
}

#[test]
fn map_report_counts() {
    // provenance: derived — counting the rows below: 2 ledgers and 1 group; 1 unmapped row;
    // 1 warning row plus 2 report-level warnings = 3 warnings.
    let report = MapReport {
        rows: vec![
            map_row(MapKind::Ledger, "Cash", MapStatus::Mapped),
            map_row(
                MapKind::Ledger,
                "Bank",
                MapStatus::Warning("unknown field kept".to_owned()),
            ),
            map_row(
                MapKind::Group,
                "Odd Group",
                MapStatus::Unmapped("parent not found".to_owned()),
            ),
        ],
        warnings: vec!["field A dropped".to_owned(), "field B dropped".to_owned()],
    };
    assert_eq!(report.unmapped_count(), 1);
    assert_eq!(report.warning_count(), 3);
    let expected: BTreeMap<MapKind, usize> = [(MapKind::Group, 1), (MapKind::Ledger, 2)]
        .into_iter()
        .collect();
    assert_eq!(report.kind_counts(), expected);
}

#[test]
fn unmapped_items_block_the_import() {
    // provenance: derived — plan card B3: an unknown parent is an error, so the import must not
    // continue past an item with no mapping (rule A1).
    let mut w = at_pull_done();
    w.continue_to_map().unwrap();
    w.take_requests();
    w.map_ready(MapReport {
        rows: vec![map_row(
            MapKind::Group,
            "Odd Group",
            MapStatus::Unmapped("parent not found".to_owned()),
        )],
        warnings: Vec::new(),
    })
    .unwrap();
    assert_eq!(
        w.confirm_map().unwrap_err(),
        ImportError::UnmappedItems { count: 1 }
    );
    assert_eq!(w.step(), Step::Map);
    assert!(w.take_requests().is_empty());
}

#[test]
fn warnings_do_not_block_the_import() {
    // provenance: derived — spec 6.7 step 3 and rule A9: unknown fields are kept and reported,
    // which is a warning, not a stop.
    let mut w = at_pull_done();
    w.continue_to_map().unwrap();
    w.take_requests();
    w.map_ready(MapReport {
        rows: vec![map_row(
            MapKind::Ledger,
            "Cash",
            MapStatus::Warning("unknown field kept".to_owned()),
        )],
        warnings: vec!["field dropped".to_owned()],
    })
    .unwrap();
    w.confirm_map().unwrap();
    assert_eq!(w.step(), Step::Reconcile);
    assert_eq!(*w.recon_state(), ReconState::Committing);
    let reqs = w.take_requests();
    assert!(matches!(reqs.as_slice(), [ImportRequest::Commit { .. }]));
}

#[test]
fn a_map_failure_can_go_back_to_the_pull() {
    let mut w = at_pull_done();
    w.continue_to_map().unwrap();
    w.map_failed("cannot read capture".to_owned()).unwrap();
    assert_eq!(
        *w.map_state(),
        MapState::Failed("cannot read capture".to_owned())
    );
    w.back().unwrap();
    assert_eq!(w.step(), Step::Pull);
    assert!(matches!(w.pull_state(), PullState::Done(_)));
}

#[test]
fn a_late_map_reply_is_refused() {
    let mut w = at_pull_done();
    // Still at the pull step.
    assert!(matches!(
        w.map_ready(clean_map()).unwrap_err(),
        ImportError::WrongStep { .. }
    ));
}

#[test]
fn leaving_is_blocked_while_mapping_works() {
    let mut w = at_pull_done();
    w.continue_to_map().unwrap();
    assert!(!w.can_leave());
    w.map_ready(clean_map()).unwrap();
    assert!(w.can_leave());
}

// ---- reconciliation rows and summary ---------------------------------------------------------

#[test]
fn equal_figures_match() {
    // provenance: derived — ₹5,000.00 Dr against ₹5,000.00 Dr (500,000 paise at 100 paise a rupee).
    assert_eq!(
        row("Cash", dr(500_000), dr(500_000)).status(),
        RowStatus::Match
    );
}

#[test]
fn a_one_rupee_difference_is_a_difference() {
    // provenance: derived — plan card B4: a ledger that differs by ₹1 is listed with both values.
    // ₹10,000.00 = 1,000,000 paise; ₹10,001.00 = 1,000,100 paise.
    let r = row("Bank", dr(1_000_000), dr(1_000_100));
    assert_eq!(r.status(), RowStatus::Differs);
    assert_eq!(format_signed_amount(r.ours.unwrap()), "10,000.00 Dr");
    assert_eq!(format_signed_amount(r.theirs.unwrap()), "10,001.00 Dr");
}

#[test]
fn the_same_amount_on_opposite_sides_differs() {
    // provenance: derived — ₹100 Dr is not ₹100 Cr (opposite signs in the books).
    assert_eq!(
        row("Cash", dr(10_000), cr(10_000)).status(),
        RowStatus::Differs
    );
}

#[test]
fn zero_is_zero_on_either_side() {
    // provenance: derived — nil balance has no side: 0 Dr and 0 Cr are the same figure.
    assert_eq!(row("Nil", dr(0), cr(0)).status(), RowStatus::Match);
}

#[test]
fn a_missing_figure_is_never_a_match() {
    // provenance: derived — spec 6.7 step 4: differences are listed, never hidden. A figure on
    // one side only is a difference; none on either side compared nothing.
    assert_eq!(row("A", None, dr(1)).status(), RowStatus::OnlyInTally);
    assert_eq!(row("B", dr(1), None).status(), RowStatus::OnlyInOurs);
    assert_eq!(row("C", None, None).status(), RowStatus::NotReported);
}

#[test]
fn differences_are_labelled_in_words() {
    // provenance: derived — spec 7: no colour-only signals.
    assert_eq!(RowStatus::Match.label(), "Matches");
    assert!(RowStatus::Differs.label().contains("DIFFERS"));
    assert!(RowStatus::OnlyInTally.label().contains("DIFFERS"));
    assert!(RowStatus::OnlyInOurs.label().contains("DIFFERS"));
    assert!(RowStatus::NotReported.label().contains("NOT COMPARED"));
}

#[test]
fn equal_trial_balance_is_all_green() {
    // provenance: derived — plan card B4: equal TB → all green. Six sections, one matching row
    // each: 6 rows, 0 differing, nothing left uncompared.
    let report = ReconReport {
        period: period(),
        sections: matching_sections(),
    };
    let s = report.summary();
    assert_eq!(s.rows, 6);
    assert_eq!(s.differing_rows, 0);
    assert!(s.not_compared.is_empty());
    assert!(s.all_match());
}

#[test]
fn one_differing_row_is_counted_and_not_all_match() {
    // provenance: derived — five matching sections plus a TB with 2 rows, one of them ₹1 out:
    // 5 + 2 = 7 rows, 1 differing.
    let mut sections = matching_sections();
    sections.remove(0);
    sections.insert(
        0,
        section(
            ReconSection::TrialBalance,
            vec![
                row("Cash", dr(500_000), dr(500_000)),
                row("Bank", dr(1_000_000), dr(1_000_100)),
            ],
        ),
    );
    let s = ReconReport {
        period: period(),
        sections,
    }
    .summary();
    assert_eq!(s.rows, 7);
    assert_eq!(s.differing_rows, 1);
    assert!(!s.all_match());
}

#[test]
fn an_unsupported_section_is_reported_not_compared() {
    // provenance: derived — rule A1: a section the engine cannot compare says so; the report
    // cannot be called a match.
    let mut sections = matching_sections();
    sections.retain(|s| s.section != ReconSection::GstSummary);
    sections.push(ReconSectionReport {
        section: ReconSection::GstSummary,
        outcome: Outcome::unsupported("GST summary", "no verified rule pack"),
    });
    let s = ReconReport {
        period: period(),
        sections,
    }
    .summary();
    assert_eq!(s.rows, 5);
    assert_eq!(s.differing_rows, 0);
    assert_eq!(s.not_compared, [ReconSection::GstSummary]);
    assert!(!s.all_match());
}

#[test]
fn a_missing_or_empty_section_is_not_compared() {
    // provenance: derived — a report that silently leaves out a section must not read as a
    // match: StockSummary is absent, PartyOutstandings has no rows.
    let mut sections = matching_sections();
    sections.retain(|s| {
        s.section != ReconSection::StockSummary && s.section != ReconSection::PartyOutstandings
    });
    sections.push(section(ReconSection::PartyOutstandings, Vec::new()));
    let s = ReconReport {
        period: period(),
        sections,
    }
    .summary();
    assert_eq!(
        s.not_compared,
        [ReconSection::StockSummary, ReconSection::PartyOutstandings]
    );
    assert!(!s.all_match());
}

#[test]
fn an_empty_report_is_not_a_match() {
    let s = ReconReport {
        period: period(),
        sections: Vec::new(),
    }
    .summary();
    assert_eq!(s.rows, 0);
    assert_eq!(s.not_compared, ReconSection::ALL);
    assert!(!s.all_match());
}

#[test]
fn amounts_use_indian_grouping_and_show_the_side() {
    // provenance: derived — spec 7: Indian grouping 12,34,567.00; 123,456,700 paise =
    // 1,234,567 rupees. 5 paise is 0.05.
    assert_eq!(
        format_signed_amount(SignedAmount::dr(Paise::new(123_456_700))),
        "12,34,567.00 Dr"
    );
    assert_eq!(
        format_signed_amount(SignedAmount::cr(Paise::new(5))),
        "0.05 Cr"
    );
}

// ---- reconcile flow --------------------------------------------------------------------------

#[test]
fn the_whole_wizard_runs_detect_to_report() {
    // provenance: derived — spec 6.7 order of work: detect, pull, stage and map, write to the
    // books, compare reports. The requests below are that order.
    let mut w = ImportWizard::new(today()).unwrap();
    w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
    w.detect_result(Ok(names(&["Demo Traders"]))).unwrap();
    w.select_company(0).unwrap();
    w.start_pull().unwrap();
    w.pull_finished(summary(2)).unwrap();
    w.continue_to_map().unwrap();
    w.map_ready(clean_map()).unwrap();
    w.confirm_map().unwrap();
    w.commit_done().unwrap();
    assert_eq!(*w.recon_state(), ReconState::Reconciling);
    let report = ReconReport {
        period: period(),
        sections: matching_sections(),
    };
    w.recon_ready(report.clone()).unwrap();
    assert_eq!(*w.recon_state(), ReconState::Ready(report));

    let kinds: Vec<&str> = w
        .take_requests()
        .iter()
        .map(|r| match r {
            ImportRequest::Detect { .. } => "detect",
            ImportRequest::Pull { .. } => "pull",
            ImportRequest::Map { .. } => "map",
            ImportRequest::Commit { .. } => "commit",
            ImportRequest::Reconcile { .. } => "reconcile",
        })
        .collect();
    assert_eq!(kinds, ["detect", "pull", "map", "commit", "reconcile"]);
}

#[test]
fn the_reconcile_request_names_company_and_period() {
    let mut w = at_committing();
    w.take_requests();
    w.commit_done().unwrap();
    let reqs = w.take_requests();
    let [
        ImportRequest::Reconcile {
            company, period, ..
        },
    ] = reqs.as_slice()
    else {
        panic!("expected one reconcile request");
    };
    assert_eq!(company, "Demo Traders");
    assert_eq!(period.from(), date!(2026 - 04 - 01));
    assert_eq!(period.to(), date!(2027 - 03 - 31));
}

#[test]
fn a_failed_commit_writes_nothing_and_goes_back_to_the_mapping() {
    // provenance: derived — rule D3 (one transaction, all or nothing): a failed commit leaves
    // the books as they were, so the user can go back and confirm again.
    let mut w = at_committing();
    assert!(!w.can_leave());
    w.commit_failed("disk full".to_owned()).unwrap();
    assert_eq!(
        *w.recon_state(),
        ReconState::CommitFailed("disk full".to_owned())
    );
    assert!(w.can_leave());
    w.back().unwrap();
    assert_eq!(w.step(), Step::Map);
    assert!(matches!(w.map_state(), MapState::Ready(_)));
    w.confirm_map().unwrap();
    assert_eq!(*w.recon_state(), ReconState::Committing);
}

#[test]
fn after_a_successful_commit_there_is_no_way_back() {
    // provenance: derived — the books already hold the data; going back would invite importing
    // it twice.
    let mut w = at_committing();
    w.commit_done().unwrap();
    assert!(matches!(
        w.back().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
}

#[test]
fn a_failed_comparison_can_be_run_again() {
    let mut w = at_committing();
    w.take_requests();
    w.commit_done().unwrap();
    w.recon_failed("Tally closed the connection".to_owned())
        .unwrap();
    assert_eq!(
        *w.recon_state(),
        ReconState::Failed("Tally closed the connection".to_owned())
    );
    w.retry_reconcile().unwrap();
    assert_eq!(*w.recon_state(), ReconState::Reconciling);
    let reqs = w.take_requests();
    assert_eq!(reqs.len(), 2);
    assert!(
        reqs.iter()
            .all(|r| matches!(r, ImportRequest::Reconcile { .. }))
    );
}

#[test]
fn replies_out_of_turn_are_refused() {
    let mut w = at_committing();
    let report = ReconReport {
        period: period(),
        sections: matching_sections(),
    };
    // The commit has not been answered yet.
    assert!(matches!(
        w.recon_ready(report).unwrap_err(),
        ImportError::WrongStep { .. }
    ));
    assert!(matches!(
        w.retry_reconcile().unwrap_err(),
        ImportError::WrongStep { .. }
    ));
    assert_eq!(*w.recon_state(), ReconState::Committing);
}

// ---- the screen ------------------------------------------------------------------------------

fn frame(
    ctx: &egui::Context,
    screen: &mut ImportScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
    action
}

fn escape() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

fn screen() -> ImportScreen {
    ImportScreen::new(today()).unwrap()
}

#[test]
fn the_screen_is_titled_as_an_interop_label() {
    // provenance: derived — CLAUDE.md: "Tally" appears in UI copy only as a factual interop label
    // ("Import from Tally").
    assert_eq!(screen().title(), "Import from Tally");
}

#[test]
fn every_step_renders() {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    // Detect, with a failure showing.
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut()
        .start_detect(ImportSource::Live { port: 9000 })
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut()
        .detect_result(Err(DetectError::NotRunning { port: 9000 }))
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    // Choose.
    s.wizard_mut()
        .start_detect(ImportSource::Live { port: 9000 })
        .unwrap();
    s.wizard_mut()
        .detect_result(Ok(names(&["Demo Traders"])))
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut().select_company(0).unwrap();
    s.wizard_mut().start_pull().unwrap();
    // Pull, in the voucher stage.
    s.wizard_mut()
        .pull_progress(PullProgress {
            stage: PullStage::Vouchers,
            batches_done: 1,
            batches_total: Some(4),
            vouchers_pulled: 5000,
        })
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut().pull_finished(summary(4)).unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    // Map.
    s.wizard_mut().continue_to_map().unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut()
        .map_ready(MapReport {
            rows: vec![
                map_row(MapKind::Ledger, "Cash", MapStatus::Mapped),
                map_row(
                    MapKind::Group,
                    "Odd",
                    MapStatus::Unmapped("parent not found".to_owned()),
                ),
            ],
            warnings: vec!["field dropped".to_owned()],
        })
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
}

#[test]
fn the_report_renders_with_differences_and_unsupported_sections() {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    {
        let w = s.wizard_mut();
        w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
        w.detect_result(Ok(names(&["Demo Traders"]))).unwrap();
        w.select_company(0).unwrap();
        w.start_pull().unwrap();
        w.pull_finished(summary(1)).unwrap();
        w.continue_to_map().unwrap();
        w.map_ready(clean_map()).unwrap();
        w.confirm_map().unwrap();
    }
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.wizard_mut().commit_done().unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let mut sections = matching_sections();
    sections.remove(0);
    sections.insert(
        0,
        section(
            ReconSection::TrialBalance,
            vec![row("Bank", dr(1_000_000), dr(1_000_100))],
        ),
    );
    sections.pop();
    sections.push(ReconSectionReport {
        section: ReconSection::GstSummary,
        outcome: Outcome::unsupported("GST summary", "no verified rule pack"),
    });
    s.wizard_mut()
        .recon_ready(ReconReport {
            period: period(),
            sections,
        })
        .unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
}

#[test]
fn escape_on_the_first_step_closes_the_screen() {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    let action = frame(&ctx, &mut s, &mut app, escape());
    assert!(matches!(action, ScreenAction::Pop));
}

#[test]
fn escape_during_the_pull_cancels_instead_of_leaving() {
    // provenance: derived — rule D2: Esc stops the import; the screen stays until the host
    // reports the pull stopped.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    {
        let w = s.wizard_mut();
        w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
        w.detect_result(Ok(names(&["Demo Traders"]))).unwrap();
        w.select_company(0).unwrap();
        w.start_pull().unwrap();
    }
    let action = frame(&ctx, &mut s, &mut app, escape());
    assert!(matches!(action, ScreenAction::Stay));
    assert!(matches!(s.wizard().pull_state(), PullState::Cancelling(_)));
    // A second Esc cannot leave while the request is still stopping.
    let action = frame(&ctx, &mut s, &mut app, escape());
    assert!(matches!(action, ScreenAction::Stay));
    assert!(s.notice().is_some());
    // Once the host reports it stopped, Esc leaves.
    s.wizard_mut().pull_cancelled(0).unwrap();
    let action = frame(&ctx, &mut s, &mut app, escape());
    assert!(matches!(action, ScreenAction::Pop));
}

#[test]
fn escape_cannot_interrupt_the_write_to_the_books() {
    // provenance: derived — rule D3: the commit is one transaction and is not interruptible.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    {
        let w = s.wizard_mut();
        w.start_detect(ImportSource::Live { port: 9000 }).unwrap();
        w.detect_result(Ok(names(&["Demo Traders"]))).unwrap();
        w.select_company(0).unwrap();
        w.start_pull().unwrap();
        w.pull_finished(summary(1)).unwrap();
        w.continue_to_map().unwrap();
        w.map_ready(clean_map()).unwrap();
        w.confirm_map().unwrap();
    }
    let action = frame(&ctx, &mut s, &mut app, escape());
    assert!(matches!(action, ScreenAction::Stay));
    assert_eq!(s.wizard().step(), Step::Reconcile);
}
