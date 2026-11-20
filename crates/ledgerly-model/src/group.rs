use ledgerly_core::{GroupId, TallyRef};
use serde::{Deserialize, Serialize};

/// An account group in the chart of accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// Identifier.
    pub id: GroupId,
    /// Name (editable, even for reserved groups).
    pub name: String,
    /// Alternative names for search.
    pub aliases: Vec<String>,
    /// Parent group; `None` for a primary group.
    pub parent: Option<GroupId>,
    /// Which reserved group this is, if any. Reserved groups are identified by this, never by name.
    pub reserved: Option<ReservedGroup>,
    /// Nature; set only on primary groups, inherited by children.
    pub nature: Option<Nature>,
    /// Whether balances under this group go to the trading account (gross profit).
    pub affects_gross_profit: bool,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// Nature of a primary group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Nature {
    /// Asset.
    Asset,
    /// Liability.
    Liability,
    /// Income.
    Income,
    /// Expense.
    Expense,
}

/// The 28 reserved groups every company starts with (15 primary, 13 sub-groups). Research: R1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReservedGroup {
    /// Primary: branches and divisions.
    BranchDivisions,
    /// Primary: capital account.
    CapitalAccount,
    /// Primary: current assets.
    CurrentAssets,
    /// Primary: current liabilities.
    CurrentLiabilities,
    /// Primary: direct expenses.
    DirectExpenses,
    /// Primary: direct incomes.
    DirectIncomes,
    /// Primary: fixed assets.
    FixedAssets,
    /// Primary: indirect expenses.
    IndirectExpenses,
    /// Primary: indirect incomes.
    IndirectIncomes,
    /// Primary: investments.
    Investments,
    /// Primary: loans (liability).
    Loans,
    /// Primary: miscellaneous expenses (asset).
    MiscExpensesAsset,
    /// Primary: purchase accounts.
    PurchaseAccounts,
    /// Primary: sales accounts.
    SalesAccounts,
    /// Primary: suspense account.
    Suspense,
    /// Sub-group: bank accounts.
    BankAccounts,
    /// Sub-group: bank overdraft accounts.
    BankOd,
    /// Sub-group: cash in hand.
    CashInHand,
    /// Sub-group: deposits (asset).
    Deposits,
    /// Sub-group: duties and taxes.
    DutiesAndTaxes,
    /// Sub-group: loans and advances (asset).
    LoansAndAdvancesAsset,
    /// Sub-group: provisions.
    Provisions,
    /// Sub-group: reserves and surplus.
    ReservesAndSurplus,
    /// Sub-group: secured loans.
    SecuredLoans,
    /// Sub-group: stock in hand.
    StockInHand,
    /// Sub-group: sundry creditors.
    SundryCreditors,
    /// Sub-group: sundry debtors.
    SundryDebtors,
    /// Sub-group: unsecured loans.
    UnsecuredLoans,
}
