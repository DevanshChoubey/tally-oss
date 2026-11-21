//! Command and query facade used by the app, the CLI and the MCP server (card W1, Wave 2).
//!
//! The entry point is [`Api`]: [`Api::execute`] runs a [`Command`], [`Api::report`] runs a
//! [`ledgerly_reports::ReportParams`] and [`Api::search`] finds ledgers, vouchers, reports and
//! settings. Every figure comes from the engine or the report engines (spec section 4).

mod bank;
mod command;
mod error;
mod events;
mod gst;
mod maintenance;
pub mod facade;
mod report;
mod search;
mod tally_import;

pub use command::{Command, CommandResult};
// Report types the app names when it builds and reads reports (card W7). The app depends on this
// crate only, so the report model is re-exported here instead of adding a manifest dependency.
pub use ledgerly_reports::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
pub use error::ApiError;
pub use facade::Api;
pub use search::{MatchedOn, SearchHit};
