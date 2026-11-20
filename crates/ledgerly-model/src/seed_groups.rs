//! The reserved groups every company starts with. **Card M1** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn reserved_groups() -> Vec<Group>;   // 28 groups, ids 1..=28, one per ReservedGroup variant
//! ```
//!
//! Ids follow the order of [`ALL`] (1-based), so the seed is deterministic. Names, parents,
//! natures and the gross-profit flag come from R1 section "Reserved groups (28)".
//!
//! Two primary groups, `Suspense` and `BranchDivisions`, have a nature that R1 marks
//! "unconfirmed". With no provenance their `nature` is `None` (pending) instead of a guess.

use ledgerly_core::GroupId;

use crate::group::{Group, Nature, ReservedGroup};

/// Every reserved group, in id order (id = position + 1).
const ALL: [ReservedGroup; 28] = [
    ReservedGroup::BranchDivisions,
    ReservedGroup::CapitalAccount,
    ReservedGroup::CurrentAssets,
    ReservedGroup::CurrentLiabilities,
    ReservedGroup::DirectExpenses,
    ReservedGroup::DirectIncomes,
    ReservedGroup::FixedAssets,
    ReservedGroup::IndirectExpenses,
    ReservedGroup::IndirectIncomes,
    ReservedGroup::Investments,
    ReservedGroup::Loans,
    ReservedGroup::MiscExpensesAsset,
    ReservedGroup::PurchaseAccounts,
    ReservedGroup::SalesAccounts,
    ReservedGroup::Suspense,
    ReservedGroup::BankAccounts,
    ReservedGroup::BankOd,
    ReservedGroup::CashInHand,
    ReservedGroup::Deposits,
    ReservedGroup::DutiesAndTaxes,
    ReservedGroup::LoansAndAdvancesAsset,
    ReservedGroup::Provisions,
    ReservedGroup::ReservesAndSurplus,
    ReservedGroup::SecuredLoans,
    ReservedGroup::StockInHand,
    ReservedGroup::SundryCreditors,
    ReservedGroup::SundryDebtors,
    ReservedGroup::UnsecuredLoans,
];

/// Seed facts for one reserved group.
struct Spec {
    name: &'static str,
    parent: Option<ReservedGroup>,
    nature: Option<Nature>,
    affects_gross_profit: bool,
}

const fn primary(name: &'static str, nature: Option<Nature>, gp: bool) -> Spec {
    Spec {
        name,
        parent: None,
        nature,
        affects_gross_profit: gp,
    }
}

const fn sub(name: &'static str, parent: ReservedGroup) -> Spec {
    Spec {
        name,
        parent: Some(parent),
        nature: None,
        affects_gross_profit: false,
    }
}

/// The seed facts for `group`. Exhaustive over `ReservedGroup`, no wildcard arm.
const fn spec(group: ReservedGroup) -> Spec {
    use ReservedGroup as R;
    match group {
        // Primary groups. Natures: R1 table; Suspense and Branch/Divisions are unconfirmed there.
        R::BranchDivisions => primary("Branch / Divisions", None, false),
        R::CapitalAccount => primary("Capital Account", Some(Nature::Liability), false),
        R::CurrentAssets => primary("Current Assets", Some(Nature::Asset), false),
        R::CurrentLiabilities => primary("Current Liabilities", Some(Nature::Liability), false),
        R::DirectExpenses => primary("Direct Expenses", Some(Nature::Expense), true),
        R::DirectIncomes => primary("Direct Incomes", Some(Nature::Income), true),
        R::FixedAssets => primary("Fixed Assets", Some(Nature::Asset), false),
        R::IndirectExpenses => primary("Indirect Expenses", Some(Nature::Expense), false),
        R::IndirectIncomes => primary("Indirect Incomes", Some(Nature::Income), false),
        R::Investments => primary("Investments", Some(Nature::Asset), false),
        R::Loans => primary("Loans (Liability)", Some(Nature::Liability), false),
        R::MiscExpensesAsset => primary("Misc. Expenses (ASSET)", Some(Nature::Asset), false),
        R::PurchaseAccounts => primary("Purchase Accounts", Some(Nature::Expense), true),
        R::SalesAccounts => primary("Sales Accounts", Some(Nature::Income), true),
        R::Suspense => primary("Suspense A/c", None, false),
        // Sub-groups with their parents (R1 "Sub-groups (13)").
        R::BankAccounts => sub("Bank Accounts", R::CurrentAssets),
        R::BankOd => sub("Bank OD A/c", R::Loans),
        R::CashInHand => sub("Cash-in-Hand", R::CurrentAssets),
        R::Deposits => sub("Deposits (Asset)", R::CurrentAssets),
        R::DutiesAndTaxes => sub("Duties & Taxes", R::CurrentLiabilities),
        R::LoansAndAdvancesAsset => sub("Loans & Advances (Asset)", R::CurrentAssets),
        R::Provisions => sub("Provisions", R::CurrentLiabilities),
        R::ReservesAndSurplus => sub("Reserves & Surplus", R::CapitalAccount),
        R::SecuredLoans => sub("Secured Loans", R::Loans),
        R::StockInHand => sub("Stock-in-Hand", R::CurrentAssets),
        R::SundryCreditors => sub("Sundry Creditors", R::CurrentLiabilities),
        R::SundryDebtors => sub("Sundry Debtors", R::CurrentAssets),
        R::UnsecuredLoans => sub("Unsecured Loans", R::Loans),
    }
}

/// The id of `group`: its 1-based position in [`ALL`].
fn id_of(group: ReservedGroup) -> Option<GroupId> {
    let pos = ALL.iter().position(|g| *g == group)?;
    u64::try_from(pos).ok()?.checked_add(1).map(GroupId)
}

/// The 28 reserved groups, ids `1..=28`, one per [`ReservedGroup`] variant.
///
/// Sub-groups carry no nature (they inherit it from their primary group). `Suspense` and
/// `BranchDivisions` have `nature: None` because R1 marks their nature unconfirmed.
pub fn reserved_groups() -> Vec<Group> {
    ALL.iter()
        .filter_map(|&reserved| {
            let s = spec(reserved);
            let id = id_of(reserved)?;
            let parent = match s.parent {
                Some(p) => Some(id_of(p)?),
                None => None,
            };
            Some(Group {
                id,
                name: s.name.to_owned(),
                aliases: Vec::new(),
                parent,
                reserved: Some(reserved),
                nature: s.nature,
                affects_gross_profit: s.affects_gross_profit,
                tally: None,
            })
        })
        .collect()
}
