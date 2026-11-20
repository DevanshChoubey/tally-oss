//! Card M1 tests: the reserved groups seed.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests may unwrap")]

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::GroupId;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{Group, Nature, ReservedGroup};

use ReservedGroup as R;

fn by_reserved(groups: &[Group], r: ReservedGroup) -> &Group {
    groups
        .iter()
        .find(|g| g.reserved == Some(r))
        .expect("reserved group present")
}

const ALL: [ReservedGroup; 28] = [
    R::BranchDivisions,
    R::CapitalAccount,
    R::CurrentAssets,
    R::CurrentLiabilities,
    R::DirectExpenses,
    R::DirectIncomes,
    R::FixedAssets,
    R::IndirectExpenses,
    R::IndirectIncomes,
    R::Investments,
    R::Loans,
    R::MiscExpensesAsset,
    R::PurchaseAccounts,
    R::SalesAccounts,
    R::Suspense,
    R::BankAccounts,
    R::BankOd,
    R::CashInHand,
    R::Deposits,
    R::DutiesAndTaxes,
    R::LoansAndAdvancesAsset,
    R::Provisions,
    R::ReservesAndSurplus,
    R::SecuredLoans,
    R::StockInHand,
    R::SundryCreditors,
    R::SundryDebtors,
    R::UnsecuredLoans,
];

#[test]
fn has_28_groups_with_ids_1_to_28() {
    // provenance: derived — R1 §Reserved groups: "28 reserved groups (15 primary, 13 sub-groups)".
    let groups = reserved_groups();
    assert_eq!(groups.len(), 28);
    let ids: BTreeSet<u64> = groups.iter().map(|g| g.id.0).collect();
    let expected: BTreeSet<u64> = (1..=28).collect();
    assert_eq!(ids, expected, "ids are unique and exactly 1..=28");
}

#[test]
fn fifteen_primary_thirteen_sub() {
    // provenance: derived — R1 §Reserved groups: Primary (15); Sub-groups (13).
    let groups = reserved_groups();
    assert_eq!(groups.iter().filter(|g| g.parent.is_none()).count(), 15);
    assert_eq!(groups.iter().filter(|g| g.parent.is_some()).count(), 13);
}

#[test]
fn every_reserved_variant_appears_exactly_once() {
    // provenance: derived — R1 §Reserved groups: one group per reserved name; the enum has 28 variants.
    let groups = reserved_groups();
    let mut counts: BTreeMap<ReservedGroup, usize> = BTreeMap::new();
    for g in &groups {
        let r = g.reserved.expect("every seeded group is reserved");
        *counts.entry(r).or_default() += 1;
    }
    for v in ALL {
        assert_eq!(counts.get(&v), Some(&1), "{v:?}");
    }
    assert_eq!(counts.len(), 28);
}

#[test]
fn sales_accounts_is_income_affecting_gross_profit() {
    // provenance: derived — R1 §Reserved groups table: "Sales Accounts | P&L | Income, affects GP".
    let groups = reserved_groups();
    let g = by_reserved(&groups, R::SalesAccounts);
    assert_eq!(g.nature, Some(Nature::Income));
    assert!(g.affects_gross_profit);
    assert_eq!(g.parent, None);
}

#[test]
fn duties_and_taxes_is_under_current_liabilities() {
    // provenance: derived — R1 §Reserved groups: "Current Liabilities: Duties & Taxes, Provisions, Sundry Creditors".
    let groups = reserved_groups();
    let parent = by_reserved(&groups, R::CurrentLiabilities).id;
    assert_eq!(by_reserved(&groups, R::DutiesAndTaxes).parent, Some(parent));
}

#[test]
fn sub_group_parents_match_r1() {
    // provenance: derived — R1 §Reserved groups, "Sub-groups (13), with their parents".
    let expected: [(ReservedGroup, ReservedGroup); 13] = [
        (R::BankAccounts, R::CurrentAssets),
        (R::CashInHand, R::CurrentAssets),
        (R::Deposits, R::CurrentAssets),
        (R::LoansAndAdvancesAsset, R::CurrentAssets),
        (R::StockInHand, R::CurrentAssets),
        (R::SundryDebtors, R::CurrentAssets),
        (R::DutiesAndTaxes, R::CurrentLiabilities),
        (R::Provisions, R::CurrentLiabilities),
        (R::SundryCreditors, R::CurrentLiabilities),
        (R::BankOd, R::Loans),
        (R::SecuredLoans, R::Loans),
        (R::UnsecuredLoans, R::Loans),
        (R::ReservesAndSurplus, R::CapitalAccount),
    ];
    let groups = reserved_groups();
    for (child, parent) in expected {
        let pid = by_reserved(&groups, parent).id;
        assert_eq!(by_reserved(&groups, child).parent, Some(pid), "{child:?}");
    }
}

#[test]
fn primary_natures_and_gross_profit_flags_match_r1() {
    // provenance: derived — R1 §Reserved groups table (statement + nature columns).
    // Suspense and Branch/Divisions are "unconfirmed" in R1, so they are covered by
    // `unconfirmed_natures_are_not_guessed`.
    let expected: [(ReservedGroup, Nature, bool); 13] = [
        (R::CapitalAccount, Nature::Liability, false),
        (R::Loans, Nature::Liability, false),
        (R::CurrentLiabilities, Nature::Liability, false),
        (R::FixedAssets, Nature::Asset, false),
        (R::Investments, Nature::Asset, false),
        (R::CurrentAssets, Nature::Asset, false),
        (R::MiscExpensesAsset, Nature::Asset, false),
        (R::SalesAccounts, Nature::Income, true),
        (R::PurchaseAccounts, Nature::Expense, true),
        (R::DirectIncomes, Nature::Income, true),
        (R::DirectExpenses, Nature::Expense, true),
        (R::IndirectIncomes, Nature::Income, false),
        (R::IndirectExpenses, Nature::Expense, false),
    ];
    let groups = reserved_groups();
    for (r, n, gp) in expected {
        let g = by_reserved(&groups, r);
        assert_eq!(g.nature, Some(n), "{r:?}");
        assert_eq!(g.affects_gross_profit, gp, "{r:?}");
    }
}

#[test]
fn unconfirmed_natures_are_not_guessed() {
    // provenance: derived — R1 marks Suspense A/c and Branch/Divisions nature "(unconfirmed)";
    // with no provenance the nature stays None (pending) rather than a guess.
    let groups = reserved_groups();
    for r in [R::Suspense, R::BranchDivisions] {
        let g = by_reserved(&groups, r);
        assert_eq!(g.nature, None, "{r:?}");
        assert!(!g.affects_gross_profit, "{r:?}");
        assert_eq!(g.parent, None, "{r:?}");
    }
}

#[test]
fn sub_groups_carry_no_nature_and_parents_exist() {
    // provenance: derived — R1 §Business rules 7: "Only top-level groups carry a nature; children inherit it."
    let groups = reserved_groups();
    let ids: BTreeSet<GroupId> = groups.iter().map(|g| g.id).collect();
    for g in &groups {
        if let Some(p) = g.parent {
            assert!(ids.contains(&p));
            assert_eq!(g.nature, None, "{:?}", g.reserved);
            assert!(!g.affects_gross_profit, "{:?}", g.reserved);
        }
    }
}

#[test]
fn names_nonempty_unique_and_deterministic() {
    // provenance: derived — R1 §Reserved groups lists 28 distinct names; the seed must be deterministic (rule A7).
    let a = reserved_groups();
    let b = reserved_groups();
    assert_eq!(a, b);
    let names: BTreeSet<&str> = a.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names.len(), 28);
    assert!(a.iter().all(|g| !g.name.is_empty() && g.tally.is_none()));
}
