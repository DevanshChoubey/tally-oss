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
pub mod profit_loss;
pub mod registers;
pub mod stock_summary;
pub mod trial_balance;

pub use model::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError,
    ReportFilter, ReportKind, ReportParams, ReportRow, RowKind,
};
