//! Card RP1 tests: column expansion and drill-down.
//!
//! Provenance for every expected value is `derived` and written beside it. The month lengths come
//! from the Gregorian calendar (30 days hath September, April, June and November; February has
//! 28 days, 29 in a leap year). The financial year runs 1 April to 31 March (spec 6.2 and
//! `FinancialYear` in ledgerly-core). Drill targets come from spec 6.2: "Every figure drills down:
//! group -> ledger -> voucher".
#![allow(clippy::unwrap_used, reason = "tests may unwrap")]

use ledgerly_core::{FinancialYear, GroupId, LedgerId, Period, StockItemId, VoucherId};
use ledgerly_model::{Scenario, VoucherStatus};
use ledgerly_reports::drill::drill_down;
use ledgerly_reports::params::expand_columns;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};
use time::macros::date;

fn params(kind: ReportKind, period: Period, columns: ColumnSpec) -> ReportParams {
    ReportParams {
        kind,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn fy_2025() -> FinancialYear {
    FinancialYear::april(2025).unwrap()
}

fn row(key: Option<DrillKey>, kind: RowKind) -> ReportRow {
    ReportRow {
        key,
        label: "row".to_owned(),
        depth: 0,
        kind,
        cells: vec![Cell::Empty],
    }
}

fn report(p: ReportParams, rows: Vec<ReportRow>) -> Report {
    Report {
        title: "t".to_owned(),
        params: p,
        columns: Vec::new(),
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

#[test]
fn monthly_over_fy_is_twelve_columns_april_to_march() {
    // derived: an Indian FY has 12 calendar months, April first, March last (spec 6.2).
    let fy = fy_2025();
    let p = params(
        ReportKind::TrialBalance,
        fy.period().unwrap(),
        ColumnSpec::Monthly,
    );
    let cols = expand_columns(&p, fy).unwrap();
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Apr 2025", "May 2025", "Jun 2025", "Jul 2025", "Aug 2025", "Sep 2025", "Oct 2025",
            "Nov 2025", "Dec 2025", "Jan 2026", "Feb 2026", "Mar 2026"
        ]
    );
}

#[test]
fn monthly_columns_have_exact_month_boundaries() {
    // derived: Apr 30d, May 31, Jun 30, Jul 31, Aug 31, Sep 30, Oct 31, Nov 30, Dec 31, Jan 31,
    // Feb 2026 28 (2026 is not a leap year: not divisible by 4), Mar 31.
    let fy = fy_2025();
    let p = params(
        ReportKind::TrialBalance,
        fy.period().unwrap(),
        ColumnSpec::Monthly,
    );
    let cols = expand_columns(&p, fy).unwrap();
    let ends = [
        date!(2025 - 04 - 30),
        date!(2025 - 05 - 31),
        date!(2025 - 06 - 30),
        date!(2025 - 07 - 31),
        date!(2025 - 08 - 31),
        date!(2025 - 09 - 30),
        date!(2025 - 10 - 31),
        date!(2025 - 11 - 30),
        date!(2025 - 12 - 31),
        date!(2026 - 01 - 31),
        date!(2026 - 02 - 28),
        date!(2026 - 03 - 31),
    ];
    assert_eq!(cols.len(), ends.len());
    for (col, end) in cols.iter().zip(ends) {
        let per = col.period.unwrap();
        assert_eq!(per.to(), end);
        assert_eq!(per.from().day(), 1);
        assert_eq!(per.from().month(), end.month());
    }
}

#[test]
fn monthly_columns_tile_the_year_without_gap_or_overlap() {
    // derived: FY 2025-26 has 365 days (1 Apr 2025 to 31 Mar 2026, no 29 Feb); consecutive
    // columns must start the day after the previous one ends.
    let fy = fy_2025();
    let p = params(
        ReportKind::TrialBalance,
        fy.period().unwrap(),
        ColumnSpec::Monthly,
    );
    let cols = expand_columns(&p, fy).unwrap();
    let mut prev_end = None;
    let mut days: i64 = 0;
    for col in &cols {
        let per = col.period.unwrap();
        if let Some(e) = prev_end {
            assert_eq!(per.from().previous_day().unwrap(), e);
        }
        days += (per.to() - per.from()).whole_days() + 1;
        prev_end = Some(per.to());
    }
    assert_eq!(days, 365);
}

#[test]
fn monthly_leap_year_february_has_29_days() {
    // derived: FY 2023-24 contains 29 Feb 2024 (2024 divisible by 4 and not a century year).
    let fy = FinancialYear::april(2023).unwrap();
    let p = params(
        ReportKind::TrialBalance,
        fy.period().unwrap(),
        ColumnSpec::Monthly,
    );
    let cols = expand_columns(&p, fy).unwrap();
    assert_eq!(cols.len(), 12);
    let feb = cols.get(10).unwrap();
    assert_eq!(feb.label, "Feb 2024");
    assert_eq!(feb.period.unwrap().to(), date!(2024 - 02 - 29));
}

#[test]
fn monthly_over_part_year_clips_first_and_last_month() {
    // derived: 15 Apr to 10 Jun 2025 touches April, May, June: three columns; the first starts on
    // 15 Apr, the last ends on 10 Jun, so no column reaches outside the requested period.
    let fy = fy_2025();
    let per = Period::new(date!(2025 - 04 - 15), date!(2025 - 06 - 10)).unwrap();
    let p = params(ReportKind::TrialBalance, per, ColumnSpec::Monthly);
    let cols = expand_columns(&p, fy).unwrap();
    assert_eq!(cols.len(), 3);
    let first = cols.first().unwrap().period.unwrap();
    let mid = cols.get(1).unwrap().period.unwrap();
    let last = cols.get(2).unwrap().period.unwrap();
    assert_eq!(
        (first.from(), first.to()),
        (date!(2025 - 04 - 15), date!(2025 - 04 - 30))
    );
    assert_eq!(
        (mid.from(), mid.to()),
        (date!(2025 - 05 - 01), date!(2025 - 05 - 31))
    );
    assert_eq!(
        (last.from(), last.to()),
        (date!(2025 - 06 - 01), date!(2025 - 06 - 10))
    );
}

#[test]
fn monthly_outside_the_financial_year_is_an_error_not_a_guess() {
    // derived: Monthly columns are defined over the financial year (spec 6.2); a period reaching
    // outside it is rejected rather than silently clipped.
    let fy = fy_2025();
    let per = Period::new(date!(2025 - 03 - 15), date!(2025 - 06 - 10)).unwrap();
    let p = params(ReportKind::TrialBalance, per, ColumnSpec::Monthly);
    assert!(matches!(
        expand_columns(&p, fy),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn single_is_one_column_for_the_report_period() {
    // derived: ColumnSpec::Single = "One column for the report period" (model doc).
    let fy = fy_2025();
    let per = fy.period().unwrap();
    let p = params(ReportKind::ProfitAndLoss, per, ColumnSpec::Single);
    let cols = expand_columns(&p, fy).unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols.first().unwrap().period, Some(per));
    assert_eq!(cols.first().unwrap().label, "01-Apr-2025 to 31-Mar-2026");
}

#[test]
fn comparative_is_one_column_per_period_in_order() {
    // derived: ColumnSpec::Comparative = "One column per period" (model doc); order preserved.
    let fy = fy_2025();
    let prev = FinancialYear::april(2024).unwrap().period().unwrap();
    let cur = fy.period().unwrap();
    let p = params(
        ReportKind::ProfitAndLoss,
        cur,
        ColumnSpec::Comparative(vec![cur, prev]),
    );
    let cols = expand_columns(&p, fy).unwrap();
    assert_eq!(cols.len(), 2);
    assert_eq!(cols.first().unwrap().period, Some(cur));
    assert_eq!(cols.get(1).unwrap().period, Some(prev));
    assert_eq!(cols.get(1).unwrap().label, "01-Apr-2024 to 31-Mar-2025");
}

#[test]
fn comparative_with_no_periods_is_an_error() {
    // derived: zero columns would be an empty report presented as a result; reject (rule A1).
    let fy = fy_2025();
    let p = params(
        ReportKind::ProfitAndLoss,
        fy.period().unwrap(),
        ColumnSpec::Comparative(Vec::new()),
    );
    assert!(matches!(
        expand_columns(&p, fy),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn drill_from_group_row_gives_group_summary_with_same_period_and_scenario() {
    // derived: spec 6.2 "group -> ledger": a group row opens that group's summary, one level
    // down (ledgers), Detailed; period, as_of, scenario unchanged.
    let fy = fy_2025();
    let per = Period::new(date!(2025 - 04 - 01), date!(2025 - 09 - 30)).unwrap();
    let mut p = params(ReportKind::TrialBalance, per, ColumnSpec::Single);
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    p.as_of = date!(2025 - 09 - 30);
    let _ = fy;
    let g = GroupId(7);
    let r = report(
        p.clone(),
        vec![row(Some(DrillKey::Group(g)), RowKind::Group)],
    );
    let d = drill_down(&r, 0).unwrap();
    assert_eq!(d.kind, ReportKind::GroupSummary(g));
    assert_eq!(d.detail, Detail::Detailed);
    assert_eq!(d.period, per);
    assert_eq!(d.as_of, p.as_of);
    assert_eq!(d.scenario, p.scenario);
}

#[test]
fn drill_from_ledger_row_gives_ledger_vouchers_with_same_period_and_scenario() {
    // derived: spec 6.2 "ledger -> voucher": a ledger row opens that ledger's voucher list.
    let per = fy_2025().period().unwrap();
    let mut p = params(
        ReportKind::GroupSummary(GroupId(1)),
        per,
        ColumnSpec::Monthly,
    );
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    p.exceptions_only = true;
    let l = LedgerId(3);
    let r = report(
        p.clone(),
        vec![row(Some(DrillKey::Ledger(l)), RowKind::Ledger)],
    );
    let d = drill_down(&r, 0).unwrap();
    assert_eq!(d.kind, ReportKind::LedgerVouchers(l));
    assert_eq!(d.period, per);
    assert_eq!(d.as_of, p.as_of);
    assert_eq!(d.scenario, p.scenario);
    // A voucher list has no period columns, and drilling must not hide vouchers.
    assert_eq!(d.columns, ColumnSpec::Single);
    assert!(!d.exceptions_only);
}

#[test]
fn drill_keeps_the_filter() {
    // derived: the user's filter (here one voucher type) is part of the view and carries over.
    let per = fy_2025().period().unwrap();
    let mut p = params(ReportKind::TrialBalance, per, ColumnSpec::Single);
    p.filter.voucher_types = vec![ledgerly_core::VoucherTypeId(2)];
    let r = report(
        p.clone(),
        vec![row(Some(DrillKey::Group(GroupId(1))), RowKind::Group)],
    );
    assert_eq!(drill_down(&r, 0).unwrap().filter, p.filter);
}

#[test]
fn drill_has_no_target_for_unkeyed_rows_and_out_of_range_rows() {
    // derived: a row without a key (text, difference) has nowhere to go; a row index past the end
    // is not a row.
    let per = fy_2025().period().unwrap();
    let p = params(ReportKind::TrialBalance, per, ColumnSpec::Single);
    let r = report(p, vec![row(None, RowKind::Difference)]);
    assert_eq!(drill_down(&r, 0), None);
    assert_eq!(drill_down(&r, 1), None);
    assert_eq!(drill_down(&r, usize::MAX), None);
}

#[test]
fn drill_from_leaf_keys_has_no_report_target() {
    // derived: there is no ReportKind for a single voucher, stock item or bill, so these keys are
    // leaves (the app opens the voucher itself); returning a different report would show numbers
    // the user did not ask for.
    let per = fy_2025().period().unwrap();
    let p = params(ReportKind::DayBook, per, ColumnSpec::Single);
    let keys = [
        DrillKey::Voucher(VoucherId(1)),
        DrillKey::Item(StockItemId(1)),
        DrillKey::Bill {
            ledger: LedgerId(1),
            name: "INV-1".to_owned(),
        },
    ];
    for k in keys {
        let r = report(p.clone(), vec![row(Some(k), RowKind::Voucher)]);
        assert_eq!(drill_down(&r, 0), None);
    }
}
