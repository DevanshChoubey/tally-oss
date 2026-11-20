//! Drill-down from a report row (card RP1, spec 6.2: "group -> ledger -> voucher").
//!
//! [`drill_down`] maps the [`DrillKey`] of a row to the parameters of the report that explains it:
//!
//! | Row key | Opens | Notes |
//! |---|---|---|
//! | `Group(g)` | `GroupSummary(g)`, `Detailed` | one level down: sub-groups and ledgers |
//! | `Ledger(l)` | `LedgerVouchers(l)`, `Detailed`, `Single` column, `exceptions_only` off | a voucher list has no period columns and must show every voucher |
//! | `Voucher`, `Item`, `Bill` | nothing (`None`) | no report kind exists for them; the app opens a voucher itself |
//!
//! Period, as-of date, scenario and filter carry over unchanged, so the drilled figures belong to
//! the same view as the row. A row without a key, or an index past the end, gives `None`.

use crate::{ColumnSpec, Detail, DrillKey, Report, ReportKind, ReportParams};

/// Parameters of the report a row drills into, or `None` when the row is a leaf or has no key.
pub fn drill_down(report: &Report, row: usize) -> Option<ReportParams> {
    let key = report.rows.get(row)?.key.as_ref()?;
    let base = &report.params;
    match key {
        DrillKey::Group(group) => Some(ReportParams {
            kind: ReportKind::GroupSummary(*group),
            detail: Detail::Detailed,
            ..base.clone()
        }),
        DrillKey::Ledger(ledger) => Some(ReportParams {
            kind: ReportKind::LedgerVouchers(*ledger),
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            exceptions_only: false,
            ..base.clone()
        }),
        DrillKey::Voucher(_) | DrillKey::Item(_) | DrillKey::Bill { .. } => None,
    }
}
