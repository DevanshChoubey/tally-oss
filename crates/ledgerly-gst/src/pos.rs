//! Place of supply (IGST Act s.10, s.12). **Card G3** implements this module.
//!
//! Frozen interface (plan card G3):
//!
//! ```text
//! pub struct SupplyFacts { .. }   // defined by G3: supplier state, recipient registration/state/address, goods movement, kind
//! pub enum PlaceOfSupply { Intra { tax: IntraTax }, Inter, ZeroRatedExport { with_payment: bool }, ZeroRatedSez { with_payment: bool }, DeemedExport }
//! pub enum IntraTax { Sgst, Utgst }
//! pub fn place_of_supply(facts: &SupplyFacts) -> Outcome<PlaceOfSupply>;
//! ```
