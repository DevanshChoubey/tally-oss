//! Masters, vouchers and the [`Books`] read model.
//!
//! Plain data with serde. Domain logic elsewhere is written as pure functions over [`Books`]
//! (plan: "Architecture"). Store-free invariant checks live in [`validate`].

mod books;
mod company;
mod error;
mod group;
mod inventory;
mod ledger;
mod masters;
mod records;
mod voucher;
mod voucher_type;

pub mod features;
pub mod seed_groups;
pub mod seed_voucher_types;
pub mod validate;

pub use books::{Books, Posting, Scenario, StockMove};
pub use company::{
    Address, Company, EntityKind, Features, Flag, GstRegistration, NegativeStockPolicy,
};
pub use error::VoucherError;
pub use group::{Group, Nature, ReservedGroup};
pub use inventory::{
    AltUnit, Batch, CostCategory, CostCentre, Godown, GstHistory, OpeningStock, StandardCost,
    StockGroup, StockItem, Unit, UnitKind, ValuationMethod,
};
pub use ledger::{
    BankDetails, BillWiseConfig, GstClassification, Ledger, PartyDetails, RegistrationType,
    ReservedLedger, TaxDetails, TaxLedgerKind, Taxability,
};
pub use masters::{MasterSnapshot, Masters};
pub use records::{BankMatch, MatchKind, ProposalRecord};
pub use voucher::{
    BankAllocation, BillAllocation, BillRefKind, CostAllocation, GodownAllocation, GstLine,
    InstrumentKind, InventoryLine, LedgerLine, StoredGst, Voucher, VoucherMode, VoucherStatus,
};
pub use voucher_type::{
    BaseVoucherKind, NumberingConfig, NumberingMethod, RestartPeriod, VoucherType,
};
