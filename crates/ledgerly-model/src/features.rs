//! Feature dependency rules. **Card M3** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub enum FeatureError { .. }                       // defined by M3 in this file
//! impl Features {
//!     pub fn validate(&self, company: &Company) -> Result<(), FeatureError>;
//!     pub fn requires(flag: Flag) -> Vec<Flag>;
//! }
//! ```
//!
//! Rules (spec section 5: "Feature flags are a typed struct with dependency rules"):
//!
//! - Batches and godowns are properties of stock items, so each requires inventory.
//! - GST needs at least one GST registration (a GSTIN) on the company.
//!
//! The remaining flags (cost centres, bill-wise, bank reconciliation, optional vouchers,
//! post-dated vouchers) operate on ledgers and vouchers alone and have no prerequisite.

use thiserror::Error;

use crate::company::{Company, Features, Flag};

/// Every flag, in declaration order. Validation reports the first violation in this order.
const ALL_FLAGS: [Flag; 9] = [
    Flag::Inventory,
    Flag::Godowns,
    Flag::Batches,
    Flag::CostCentres,
    Flag::BillWise,
    Flag::Gst,
    Flag::BankReconciliation,
    Flag::OptionalVouchers,
    Flag::PostDated,
];

/// A feature combination that cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FeatureError {
    /// A feature is on while a feature it depends on is off.
    #[error(
        "{flag:?} is enabled but it requires {requires:?}, which is disabled; enable {requires:?} first or disable {flag:?}"
    )]
    MissingRequirement {
        /// The enabled feature.
        flag: Flag,
        /// The disabled feature it needs.
        requires: Flag,
    },
    /// GST is on but the company has no GST registration.
    #[error("GST is enabled but the company has no GSTIN; add a GST registration or disable GST")]
    GstWithoutGstin,
}

impl Features {
    /// Whether `flag` is switched on.
    fn is_enabled(&self, flag: Flag) -> bool {
        match flag {
            Flag::Inventory => self.inventory,
            Flag::Godowns => self.godowns,
            Flag::Batches => self.batches,
            Flag::CostCentres => self.cost_centres,
            Flag::BillWise => self.bill_wise,
            Flag::Gst => self.gst,
            Flag::BankReconciliation => self.bank_reconciliation,
            Flag::OptionalVouchers => self.optional_vouchers,
            Flag::PostDated => self.post_dated,
        }
    }

    /// Checks the dependency rules for these features on `company`.
    ///
    /// Returns the first violation, flags taken in declaration order of [`Flag`]; the
    /// per-flag prerequisites are checked before the company-level GSTIN rule.
    ///
    /// # Errors
    /// [`FeatureError::MissingRequirement`] when a feature is on and one of
    /// [`Features::requires`] is off; [`FeatureError::GstWithoutGstin`] when GST is on and
    /// `company.registrations` is empty.
    pub fn validate(&self, company: &Company) -> Result<(), FeatureError> {
        for flag in ALL_FLAGS {
            if !self.is_enabled(flag) {
                continue;
            }
            for requires in Self::requires(flag) {
                if !self.is_enabled(requires) {
                    return Err(FeatureError::MissingRequirement { flag, requires });
                }
            }
        }
        if self.gst && company.registrations.is_empty() {
            return Err(FeatureError::GstWithoutGstin);
        }
        Ok(())
    }

    /// The flags that must be on whenever `flag` is on (direct prerequisites only).
    #[must_use]
    pub fn requires(flag: Flag) -> Vec<Flag> {
        match flag {
            Flag::Godowns | Flag::Batches => vec![Flag::Inventory],
            Flag::Inventory
            | Flag::CostCentres
            | Flag::BillWise
            | Flag::Gst
            | Flag::BankReconciliation
            | Flag::OptionalVouchers
            | Flag::PostDated => Vec::new(),
        }
    }
}
