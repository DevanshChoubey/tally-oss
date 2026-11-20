//! Pure command handlers: each takes a command and `&Books` and returns the events to commit
//! (`Result<Outcome<Vec<Event>>, EngineError>`). Nothing here touches storage.

pub mod bills;
mod error;
pub mod locks;
pub mod masters_accounts;
pub mod masters_stock;
pub mod numbering;
pub mod scenario;
pub mod vouchers;

pub use error::EngineError;
