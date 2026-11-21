//! Bank statement formats. MIT OR Apache-2.0. Formats are data (`formats/*.toml`).

pub mod csv;
pub mod mt940;
pub mod registry;
mod statement;
pub mod xlsx;

pub use statement::{BankFmtError, ParsedStatement, RawLine, RawStatement};
