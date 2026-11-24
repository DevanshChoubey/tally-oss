//! The seed's shape: what `assets/godavari.toml` holds. Text values (amounts, dates, quantities)
//! stay text here; `load` parses them and names the field when one is bad.

use serde::Deserialize;

/// The whole seed file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Seed {
    pub(crate) version: u32,
    pub(crate) lock_until: String,
    pub(crate) company: CompanySeed,
    #[serde(default)]
    pub(crate) unit: Vec<UnitSeed>,
    #[serde(default)]
    pub(crate) ledger: Vec<LedgerSeed>,
    #[serde(default)]
    pub(crate) item: Vec<ItemSeed>,
    #[serde(default)]
    pub(crate) voucher: Vec<VoucherSeed>,
    #[serde(default)]
    pub(crate) statement: Vec<StatementSeed>,
}

/// The company master.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompanySeed {
    pub(crate) name: String,
    pub(crate) gstin: String,
    pub(crate) state: u8,
    pub(crate) address: Vec<String>,
    pub(crate) entity: EntitySeed,
    pub(crate) fy_start: String,
    pub(crate) books_begin: String,
    pub(crate) today: String,
    pub(crate) round_off_ledger: String,
}

/// The kind of entity.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum EntitySeed {
    Company,
    Llp,
    Partnership,
    Proprietorship,
    Other,
}

/// A unit of measure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnitSeed {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) uqc: String,
}

/// The reserved groups the sample's ledgers sit under.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum GroupSeed {
    CapitalAccount,
    SecuredLoans,
    SundryCreditors,
    DutiesAndTaxes,
    Provisions,
    FixedAssets,
    SundryDebtors,
    CashInHand,
    BankAccounts,
    LoansAndAdvancesAsset,
    SalesAccounts,
    PurchaseAccounts,
    DirectExpenses,
    IndirectExpenses,
    IndirectIncomes,
}

/// A tax ledger's kind.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum TaxSeed {
    Cgst,
    Sgst,
    Igst,
    Tds,
}

/// How a party is registered.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum RegistrationSeed {
    Regular,
    Unregistered,
}

/// A bank account's details (fictional).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankSeed {
    pub(crate) name: String,
    pub(crate) account: String,
    pub(crate) ifsc: String,
    pub(crate) branch: String,
}

/// A ledger's GST classification.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GstClassSeed {
    pub(crate) taxability: TaxabilitySeed,
    pub(crate) rate: String,
}

/// The taxability the seed uses.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum TaxabilitySeed {
    Taxable,
}

/// A ledger master.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LedgerSeed {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) group: GroupSeed,
    pub(crate) opening_dr: Option<String>,
    pub(crate) opening_cr: Option<String>,
    #[serde(default)]
    pub(crate) aliases: Vec<String>,
    pub(crate) tax: Option<TaxSeed>,
    pub(crate) state: Option<u8>,
    pub(crate) city: Option<String>,
    pub(crate) registration: Option<RegistrationSeed>,
    pub(crate) gstin: Option<String>,
    #[serde(default)]
    pub(crate) bill_wise: bool,
    pub(crate) credit_days: Option<u32>,
    pub(crate) bank: Option<BankSeed>,
    pub(crate) gst: Option<GstClassSeed>,
    /// The user's "eligible for input credit" setting on a purchase ledger: `true` marks the
    /// ledger eligible (the credit is claimed whatever the HSN), as a person would after reading
    /// CGST Act s.17(5). Unset leaves the choice to the engine's HSN rules.
    pub(crate) eligible_itc: Option<bool>,
}

/// A stock item master.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemSeed {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) hsn: Option<String>,
    pub(crate) gst_rate: String,
    pub(crate) unit: String,
    /// The usual selling rate. The model keeps no selling rate on the item; vouchers carry theirs.
    pub(crate) rate: String,
}

/// The base kind of a voucher.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KindSeed {
    Sales,
    Purchase,
    Payment,
    Receipt,
    Journal,
    Contra,
}

/// How a voucher is entered.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModeSeed {
    /// Plain debit and credit lines.
    #[default]
    Plain,
    /// An item invoice: the engine builds the tax and the party line.
    Items,
    /// An accounting invoice whose particulars are typed; the engine adds the tax and the party.
    Taxed,
    /// An accounting invoice with no tax lines (an opening bill carried from before).
    Invoice,
}

/// Which section of the story a voucher belongs to.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SectionSeed {
    History,
    November,
}

/// One typed debit or credit line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LineSeed {
    pub(crate) ledger: String,
    pub(crate) amount: String,
    /// Opens a bill of this name on the line, due after the ledger's credit days.
    pub(crate) new_bill: Option<String>,
    /// Settles this open bill.
    pub(crate) against: Option<String>,
    /// A cheque number on a bank line.
    pub(crate) chq: Option<String>,
}

/// One item line of an item invoice.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemLineSeed {
    pub(crate) item: String,
    pub(crate) qty: String,
    pub(crate) rate: String,
}

/// A voucher.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VoucherSeed {
    pub(crate) section: SectionSeed,
    pub(crate) kind: KindSeed,
    #[serde(default)]
    pub(crate) mode: ModeSeed,
    pub(crate) date: String,
    pub(crate) number: String,
    /// Who entered it: `ananya` or `rohan` (the company's two users); the first when absent.
    pub(crate) by: Option<String>,
    /// The voucher was an assistant's draft that the user accepted.
    #[serde(default)]
    pub(crate) ai: bool,
    pub(crate) reference: Option<String>,
    pub(crate) party: Option<String>,
    /// The sales or purchase ledger of an item invoice.
    pub(crate) account: Option<String>,
    /// Where an item invoice's total is posted instead of the party (a cash sale).
    pub(crate) paid_to: Option<String>,
    /// The place of supply as a state code; the company's state when absent.
    pub(crate) place: Option<u8>,
    #[serde(default)]
    pub(crate) narration: String,
    #[serde(default)]
    pub(crate) items: Vec<ItemLineSeed>,
    #[serde(default)]
    pub(crate) dr: Vec<LineSeed>,
    #[serde(default)]
    pub(crate) cr: Vec<LineSeed>,
}

/// A bank statement to import.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatementSeed {
    pub(crate) ledger: String,
    pub(crate) file: String,
    pub(crate) format: String,
}
