//! Inventory (spec 6.4): units, positions and valuation over `&[StockMove]`.

pub mod avg;
pub mod fifo;
pub mod negative;
pub mod other_methods;
pub mod positions;
pub mod units;
mod valuation;

pub use valuation::{ProvisionalReason, StockError, StockValuation, ValuationStatus};
