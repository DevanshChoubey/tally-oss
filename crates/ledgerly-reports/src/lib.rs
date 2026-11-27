//! Reports (spec 6.2): pure functions `(&Books, &ReportParams) -> Result<Outcome<Report>, ReportError>`.
//! The shared report model (params, rows, cells, errors) is re-exported at the crate root.

pub mod audit;
pub mod balance_sheet;
pub mod drill;
pub mod export;
pub mod ledger;
mod model;
pub mod outstanding;
pub mod params;
pub mod print_text;
pub mod profit_loss;
pub mod registers;
mod stock_reason;
pub mod stock_summary;
pub mod trial_balance;

pub use model::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError,
    ReportFilter, ReportKind, ReportParams, ReportRow, RowKind,
};

/// "group 7" for a ledger's group, "Primary" for a ledger with none (the root of the group tree).
pub(crate) fn group_text(group: Option<ledgerly_core::GroupId>) -> String {
    match group {
        Some(id) => format!("group {}", id.0),
        None => "Primary".to_owned(),
    }
}
