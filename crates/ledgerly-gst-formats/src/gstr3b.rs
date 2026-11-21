//! GSTR-3B portal values (shape only; G10 fills and compares). **Card G10** implements this module.
//!
//! Frozen interface (plan card G10):
//!
//! ```text
//! pub struct Gstr3bPortalValues { .. }
//! ```
//!
//! The values are those the user reads off the GST portal for the two tables Ledgerly works out
//! from the books: table 3.1(a) (outward taxable supplies other than zero rated, nil rated and
//! exempted) and table 4(A)(5) (all other ITC). Tables 3.1 and 3.2 are locked on the portal
//! (spec 6.3), so the user types them in; nothing here reads the portal.

use ledgerly_core::Paise;

/// The four tax heads of a GSTR-3B row, as `Paise`.
///
/// State tax and union territory tax share one column, as on the GSTR-3B form
/// ("State/UT tax").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Gstr3bTaxAmounts {
    /// Integrated tax.
    pub integrated: Paise,
    /// Central tax.
    pub central: Paise,
    /// State/UT tax.
    pub state_ut: Paise,
    /// Cess.
    pub cess: Paise,
}

/// What the GST portal shows for the tables Ledgerly works out from the books.
///
/// Every amount is typed in by the user; Ledgerly never fetches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Gstr3bPortalValues {
    /// Table 3.1(a): total taxable value.
    pub outward_taxable_value: Paise,
    /// Table 3.1(a): tax.
    pub outward_tax: Gstr3bTaxAmounts,
    /// Table 4(A)(5): all other ITC.
    pub itc_all_other: Gstr3bTaxAmounts,
}
