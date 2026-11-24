//! Command and query facade used by the app, the CLI and the MCP server (card W1, Wave 2).
//!
//! The entry point is [`Api`]: [`Api::execute`] runs a [`Command`], [`Api::report`] runs a
//! [`ledgerly_reports::ReportParams`] and [`Api::search`] finds ledgers, vouchers, reports and
//! settings. Every figure comes from the engine or the report engines (spec section 4).

mod ai;
mod bank;
mod command;
mod error;
mod events;
pub mod facade;
mod gst;
mod home;
mod maintenance;
mod nudges;
mod print;
mod report;
mod review;
mod search;
mod tally_import;
mod voucher_gst;

pub use ai::{AiLogEntry, AiTier};
pub use command::{Command, CommandResult, CreatedId};
// Report types the app names when it builds and reads reports (card W7). The app depends on this
// crate only, so the report model is re-exported here instead of adding a manifest dependency.
pub use error::ApiError;
pub use facade::Api;
pub use gst::{
    Bucket, BucketedVoucher, Gstr1, Gstr1Return, Gstr3bTaxAmounts, Gstr3bWorking, RulePack,
};
pub use home::{
    CUSTOMER_LATE_DAYS, CashAndBank, HomeFigures, LateBills, MonthSales, SUPPLIER_LATE_DAYS,
};
pub use ledgerly_gst::compute::GstBreakup;
pub use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, NegativeStockPolicy, Voucher,
};
pub use ledgerly_reports::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
pub use ledgerly_store::backup::{RebuildOutcome, RebuildReport};
pub use ledgerly_store::verify::{ChainProblem, VerifyReport};
pub use maintenance::{VoucherCheck, verify_file};
pub use nudges::{DuplicatePayment, Gstr1Due, HomeNudges, PaymentRef, gstr1_next_due};
pub use report::ExportFormat;
pub use search::{MatchedOn, SearchHit};
pub use tally_import::ImportSummary;
pub use voucher_gst::InvoiceGst;

pub use ledgerly_bank::brs::Brs;
pub use ledgerly_bank::matcher::{Candidate, LineDirection, MatchRule};
pub use ledgerly_bank::statement::BankStatement;
pub use review::{ReviewItem, ReviewQueue, ReviewSource, UnlistedSource};
