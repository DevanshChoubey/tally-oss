use ledgerly_core::{Date, MoneyError, Paise, Qty, Rate, StockItemId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Closing stock of one item at a date, as computed by a valuation method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockValuation {
    /// Item.
    pub item: StockItemId,
    /// Date valued.
    pub as_of: Date,
    /// Closing quantity in base units.
    pub qty: Qty,
    /// Closing value.
    pub value: Paise,
    /// Unit rate used, in rupees, at full precision.
    pub rate: Option<Rate>,
    /// Final, or provisional with a reason (never silently final).
    pub status: ValuationStatus,
}

/// Whether a valuation is final.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValuationStatus {
    /// Final.
    Final,
    /// Provisional, with the reason.
    Provisional(ProvisionalReason),
}

/// Why a valuation is provisional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvisionalReason {
    /// The running quantity went below zero.
    NegativeStock {
        /// Item.
        item: StockItemId,
        /// First date it went negative.
        date: Date,
    },
}

/// Stock errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StockError {
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// Unit conversion problem.
    #[error("unit conversion: {reason}")]
    Unit {
        /// What is wrong.
        reason: String,
    },
    /// Standard cost valuation with no standard cost on or before the date.
    #[error("no standard cost for item {item:?} on {date}; enter one")]
    NoStandardCost {
        /// Item.
        item: StockItemId,
        /// Date.
        date: Date,
    },
    /// Negative stock under a Block policy.
    #[error("item {item:?} would go below zero on {date}")]
    NegativeStockBlocked {
        /// Item.
        item: StockItemId,
        /// Date.
        date: Date,
    },
    /// Any other problem.
    #[error("{field}: {reason}")]
    Invalid {
        /// Field.
        field: String,
        /// Reason.
        reason: String,
    },
}
