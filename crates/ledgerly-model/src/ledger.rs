use ledgerly_core::{GroupId, Gstin, Hsn, LedgerId, Pan, Rate, SignedAmount, StateCode, TallyRef};
use serde::{Deserialize, Serialize};

use crate::company::Address;

/// A ledger account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// Identifier.
    pub id: LedgerId,
    /// Name.
    pub name: String,
    /// Alternative names for search.
    pub aliases: Vec<String>,
    /// Parent group.
    pub group: GroupId,
    /// Opening balance at books-begin.
    pub opening: SignedAmount,
    /// Bill-wise tracking, when enabled for this ledger.
    pub bill_wise: Option<BillWiseConfig>,
    /// Whether cost centres apply to amounts posted here.
    pub cost_centres_applicable: bool,
    /// Party details for debtors and creditors.
    pub party: Option<PartyDetails>,
    /// Bank details for bank ledgers.
    pub bank: Option<BankDetails>,
    /// Tax details for duties-and-taxes ledgers.
    pub tax: Option<TaxDetails>,
    /// GST classification for sales/purchase/income/expense ledgers.
    pub gst: Option<GstClassification>,
    /// Which reserved ledger this is, if any.
    pub reserved: Option<ReservedLedger>,
    /// Ineligible for input credit, set on this ledger. `None` means not set here: the parent
    /// group chain, then the company setting, decides (see `ledgerly_gst::itc`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ineligible_itc: Option<bool>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// Reserved ledgers every company has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReservedLedger {
    /// Cash.
    Cash,
    /// Profit and loss account.
    ProfitAndLoss,
}

/// Bill-wise settings of a ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BillWiseConfig {
    /// Default credit period in days for new bills.
    pub default_credit_days: Option<u32>,
}

/// Party (customer/supplier) details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyDetails {
    /// GST registration type.
    pub registration: RegistrationType,
    /// GSTIN, for registered parties.
    pub gstin: Option<Gstin>,
    /// PAN.
    pub pan: Option<Pan>,
    /// State (drives place of supply defaults).
    pub state: Option<StateCode>,
    /// Address.
    pub address: Address,
}

/// GST registration type of a party.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegistrationType {
    /// Regular taxpayer.
    Regular,
    /// Composition taxpayer.
    Composition,
    /// Unregistered business.
    Unregistered,
    /// End consumer.
    Consumer,
    /// SEZ unit or developer.
    Sez,
    /// Party outside India.
    Overseas,
    /// UN body or embassy holding a UIN.
    Uin,
}

/// Bank account details of a bank ledger. Account numbers are masked in UI and logs (spec 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankDetails {
    /// Account number.
    pub account_number: String,
    /// IFSC.
    pub ifsc: String,
    /// Bank name.
    pub bank_name: String,
    /// Branch.
    pub branch: String,
}

/// Tax details of a duties-and-taxes ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaxDetails {
    /// Which tax.
    pub kind: TaxLedgerKind,
    /// Rate in percent, when the ledger is rate-specific.
    pub rate_percent: Option<Rate>,
}

/// Kind of tax ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaxLedgerKind {
    /// Central GST.
    Cgst,
    /// State GST.
    Sgst,
    /// Union territory GST.
    Utgst,
    /// Integrated GST.
    Igst,
    /// Compensation cess.
    Cess,
    /// Tax deducted at source (not computed in v1).
    Tds,
    /// Tax collected at source (not computed in v1).
    Tcs,
    /// Any other duty.
    Other,
}

/// GST classification on a sales, purchase, income or expense ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GstClassification {
    /// HSN/SAC.
    pub hsn: Option<Hsn>,
    /// Taxability.
    pub taxability: Taxability,
    /// Rate in percent, when fixed on the ledger.
    pub rate_percent: Option<Rate>,
}

/// GST taxability of a supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Taxability {
    /// Taxable at a rate.
    Taxable,
    /// Exempt.
    Exempt,
    /// Nil-rated.
    NilRated,
    /// Outside GST.
    NonGst,
}
