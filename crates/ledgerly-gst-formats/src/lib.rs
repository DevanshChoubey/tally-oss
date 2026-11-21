//! GST return data shapes and their official formats. MIT OR Apache-2.0. Built only from official
//! templates and public specifications.

pub mod einvoice;
mod error;
pub mod ewb;
pub mod gstr1;
pub mod gstr1_xlsx;
pub mod gstr2b;
pub mod gstr3b;
pub mod ims;
mod json_digits;

pub use error::FormatError;
