use ledgerly_core::{CompanyId, Date, Gstin, Pan, StateCode};
use serde::{Deserialize, Serialize};

/// A company: one set of books.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    /// Identifier.
    pub id: CompanyId,
    /// Display name.
    pub name: String,
    /// Name printed on documents, when different.
    pub mailing_name: Option<String>,
    /// Registered address.
    pub address: Address,
    /// State of the principal place of business.
    pub state: StateCode,
    /// PAN of the business.
    pub pan: Option<Pan>,
    /// GST registrations; one company may hold several GSTINs.
    pub registrations: Vec<GstRegistration>,
    /// First day of the financial year (normally 1 April).
    pub fy_start: Date,
    /// First date on which vouchers may be entered. `fy_start <= books_begin < fy_start + 1 year`.
    pub books_begin: Date,
    /// Legal form. The audit trail is always on for every kind.
    pub entity_kind: EntityKind,
    /// Enabled features.
    pub features: Features,
    /// What happens when a voucher would take stock below zero.
    pub negative_stock: NegativeStockPolicy,
}

/// A postal address.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Address {
    /// Address lines, in order.
    pub lines: Vec<String>,
    /// PIN code.
    pub pincode: Option<String>,
    /// State, for Indian addresses.
    pub state: Option<StateCode>,
    /// Country name; empty means India.
    pub country: String,
}

/// One GST registration of the company.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GstRegistration {
    /// The GSTIN.
    pub gstin: Gstin,
    /// State of the registration.
    pub state: StateCode,
    /// Date from which the registration applies.
    pub effective_from: Date,
    /// Regular or composition.
    pub composition: bool,
}

/// Legal form of the business.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    /// Company under the Companies Act.
    Company,
    /// Limited liability partnership.
    Llp,
    /// Partnership firm.
    Partnership,
    /// Sole proprietorship.
    Proprietorship,
    /// Trust, society, HUF or other.
    Other,
}

/// Feature switches. Dependencies between them are checked by [`Features::validate`]
/// (module [`crate::features`], card M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a flat set of independent feature switches"
)]
pub struct Features {
    /// Inventory (stock items, units, stock vouchers).
    pub inventory: bool,
    /// Multiple godowns.
    pub godowns: bool,
    /// Batches with manufacturing and expiry dates.
    pub batches: bool,
    /// Cost categories and cost centres.
    pub cost_centres: bool,
    /// Bill-wise details on party ledgers.
    pub bill_wise: bool,
    /// GST.
    pub gst: bool,
    /// Bank reconciliation.
    pub bank_reconciliation: bool,
    /// Optional vouchers (kept out of the books).
    pub optional_vouchers: bool,
    /// Post-dated vouchers.
    pub post_dated: bool,
}

/// One feature switch, for dependency rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Flag {
    /// [`Features::inventory`].
    Inventory,
    /// [`Features::godowns`].
    Godowns,
    /// [`Features::batches`].
    Batches,
    /// [`Features::cost_centres`].
    CostCentres,
    /// [`Features::bill_wise`].
    BillWise,
    /// [`Features::gst`].
    Gst,
    /// [`Features::bank_reconciliation`].
    BankReconciliation,
    /// [`Features::optional_vouchers`].
    OptionalVouchers,
    /// [`Features::post_dated`].
    PostDated,
}

/// Negative stock handling (spec 6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NegativeStockPolicy {
    /// Reject the voucher.
    Block,
    /// Save, with a warning.
    Warn,
    /// Save silently; valuations over the period are still marked provisional.
    Allow,
}
