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
pub use error::ApiError;
pub use facade::Api;
pub use gst::{Bucket, BucketedVoucher, Gstr1, Gstr1Return, Gstr3bWorking, RulePack};
pub use search::{MatchedOn, SearchHit};
pub use tally_import::ImportSummary;

pub use ledgerly_bank::brs::Brs;
pub use ledgerly_bank::matcher::{Candidate, LineDirection, MatchRule};
pub use ledgerly_bank::statement::BankStatement;
