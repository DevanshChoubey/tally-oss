use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A quantity of stock in some unit. Exact decimal; never a float.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Qty(pub Decimal);

impl Qty {
    /// Zero quantity.
    pub const ZERO: Qty = Qty(Decimal::ZERO);
}

/// A rate: a per-unit price in rupees, or a percentage. Each field that holds a `Rate` documents
/// which one it is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Rate(pub Decimal);
