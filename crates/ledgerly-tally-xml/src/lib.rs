//! Tally XML (MIT OR Apache-2.0): envelope, parsers, writer and request builders. No network.

pub mod envelope;
mod error;
pub mod masters;
pub mod reports;
pub mod request;
pub mod vouchers;
pub mod writer;

pub use error::{TallyXmlError, XmlWarning};
