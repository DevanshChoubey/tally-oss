//! Time of supply (CGST s.12, s.13, s.14). **Card G12** implements this module.
//!
//! Frozen interface (plan card G12):
//!
//! ```text
//! pub enum GoodsOrServices { Goods, Services }
//! pub struct SupplyDates { pub invoice: Option<Date>, pub supply: Option<Date>, pub payment: Option<Date>, pub kind: GoodsOrServices }
//! pub enum RateSide { Before, After }
//! pub fn time_of_supply(d: &SupplyDates) -> Outcome<Date>;
//! pub fn rate_change_rule(d: &SupplyDates, change_date: Date) -> Outcome<RateSide>;
//! ```
