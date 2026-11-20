//! GST engine (spec 6.3). Rule packs are effective-dated data under `packs/`; every entry has a
//! legal reference or is `pending` (and then evaluates to `Unsupported`).

pub mod classify;
pub mod compute;
mod error;
pub mod gstr1;
pub mod gstr1a;
pub mod gstr3b;
pub mod itc;
pub mod pack;
pub mod pos;
pub mod rcm;
pub mod recon2b;
pub mod time_of_supply;

pub use error::GstError;
