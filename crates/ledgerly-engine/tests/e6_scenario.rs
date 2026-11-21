//! Card E6 tests: which postings a scenario includes.
//!
//! Provenance for every expected value is `derived` from spec section 5, invariant 3
//! (docs/specs/2026-11-20-ledgerly-design.md): "Optional, memorandum, reversing and future
//! post-dated vouchers never affect base balances; only a named scenario includes them", and from
//! the doc comments on `Scenario` in ledgerly-model: `Actual` = "Regular vouchers, plus post-dated
//! ones on or before the as-of date"; `Including(s)` = `Actual` plus vouchers with statuses `s`.
//! Postings after `as_of` are out, so a balance is "as of" a date.
//!
//! The memorandum and reversing rules follow the review fix of round 2: a named scenario that
//! lists statuses cannot include them, so the answer is `Unsupported`, never a silent drop. A
//! voucher type missing from the masters is an error, never a silent exclusion.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_engine::scenario::effective_postings;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, Posting, RestartPeriod, Scenario, VoucherStatus, VoucherType,
};
use time::macros::date;

const T_SALES: VoucherTypeId = VoucherTypeId(1);
const T_MEMO: VoucherTypeId = VoucherTypeId(2);
const T_REVERSING: VoucherTypeId = VoucherTypeId(3);
const T_UNKNOWN: VoucherTypeId = VoucherTypeId(99);

fn vtype(id: VoucherTypeId, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: format!("{base:?}"),
        base,
        abbreviation: String::new(),
        numbering: NumberingConfig {
            method: NumberingMethod::Manual,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Never,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn posting(
    voucher: u64,
    date: time::Date,
    status: VoucherStatus,
    type_id: VoucherTypeId,
) -> Posting {
    Posting {
        voucher: VoucherId(voucher),
        date,
        ledger: LedgerId(10),
        amount: SignedAmount::dr(Paise::new(10_000)),
        status,
        type_id,
    }
}

fn books(postings: Vec<Posting>) -> Books {
    let mut masters = Masters::default();
    for (id, base) in [
        (T_SALES, BaseVoucherKind::Sales),
        (T_MEMO, BaseVoucherKind::Memorandum),
        (T_REVERSING, BaseVoucherKind::ReversingJournal),
    ] {
        masters.voucher_types.insert(id, vtype(id, base));
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings,
        stock_moves: Vec::new(),
    }
}

/// Voucher numbers of the included postings. Fails the test on an error or an `Unsupported`.
fn included(result: Result<Outcome<Vec<&Posting>>, EngineError>) -> Vec<u64> {
    let outcome = result.expect("effective_postings should not return an error here");
    let found = outcome
        .value()
        .expect("effective_postings should return a value here, not Unsupported");
    found.iter().map(|p| p.voucher.0).collect()
}

const AS_OF: time::Date = date!(2026 - 06 - 30);

#[test]
fn optional_excluded_from_actual() {
    // Invariant 3: an optional voucher never affects base balances; Actual is the base.
    // Expected: only voucher 1 (Regular) is returned.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Optional, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![1]
    );
}

#[test]
fn optional_included_in_named_scenario() {
    // Invariant 3: "only a named scenario includes them". Including([Optional]) = Actual plus
    // Optional vouchers, so both vouchers, in book order.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Optional, T_SALES),
    ]);
    let named = Scenario::Including(vec![VoucherStatus::Optional]);
    assert_eq!(included(effective_postings(&b, &named, AS_OF)), vec![1, 2]);
}

#[test]
fn including_empty_list_equals_actual() {
    // Scenario doc: Including(s) = Actual plus statuses in s; with s empty nothing is added.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Optional, T_SALES),
    ]);
    let empty = Scenario::Including(Vec::new());
    assert_eq!(included(effective_postings(&b, &empty, AS_OF)), vec![1]);
}

#[test]
fn post_dated_after_as_of_excluded() {
    // Invariant 3: "future post-dated vouchers never affect base balances". As of 30 Jun 2026,
    // a post-dated voucher dated 15 Jul 2026 is future (out); one dated 30 Jun (on the as-of
    // date, "on or before") is in; one dated 10 Jun is in.
    let b = books(vec![
        posting(1, date!(2026 - 06 - 10), VoucherStatus::PostDated, T_SALES),
        posting(2, date!(2026 - 06 - 30), VoucherStatus::PostDated, T_SALES),
        posting(3, date!(2026 - 07 - 15), VoucherStatus::PostDated, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![1, 2]
    );
}

#[test]
fn post_dated_after_as_of_excluded_in_named_scenario_too() {
    // as_of is the cut for every scenario: a posting dated 15 Jul is after 30 Jun.
    let b = books(vec![
        posting(1, date!(2026 - 07 - 15), VoucherStatus::PostDated, T_SALES),
        posting(2, date!(2026 - 07 - 15), VoucherStatus::Optional, T_SALES),
    ]);
    let named = Scenario::Including(vec![VoucherStatus::Optional]);
    assert_eq!(
        included(effective_postings(&b, &named, AS_OF)),
        Vec::<u64>::new()
    );
}

#[test]
fn regular_after_as_of_excluded() {
    // Balances are as of a date: a regular posting dated 1 Jul is after 30 Jun.
    let b = books(vec![
        posting(1, date!(2026 - 06 - 30), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 07 - 01), VoucherStatus::Regular, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![1]
    );
}

#[test]
fn memorandum_never_in_actual() {
    // Invariant 3: memorandum vouchers never affect base balances. Even with Regular status the
    // Memorandum type keeps voucher 2 out, and Actual has no Unsupported case.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Regular, T_MEMO),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![1]
    );
}

#[test]
fn memorandum_in_empty_named_scenario_is_excluded() {
    // Scenario doc: Including(s) = Actual plus statuses in s. With s empty nothing is added, so
    // a memorandum voucher stays out and the answer is a value, not Unsupported.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Optional, T_MEMO),
    ]);
    let empty = Scenario::Including(Vec::new());
    assert_eq!(included(effective_postings(&b, &empty, AS_OF)), vec![1]);
}

#[test]
fn memorandum_in_named_scenario_is_unsupported_not_dropped() {
    // Review fix round 2: no rule says how a named scenario includes a memorandum voucher. The
    // result is Unsupported for the whole computation. Silently dropping voucher 2 would make a
    // total that is short by its amount. Same rule as the reports (balance sheet, ledger).
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Optional, T_MEMO),
    ]);
    let named = Scenario::Including(vec![VoucherStatus::Optional]);
    let result = effective_postings(&b, &named, AS_OF);
    assert!(matches!(result, Ok(Outcome::Unsupported(_))));
}

#[test]
fn reversing_journal_in_named_scenario_is_unsupported_not_dropped() {
    // Same rule as the memorandum case, for a reversing journal (invariant 3).
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(
            2,
            date!(2026 - 05 - 03),
            VoucherStatus::Regular,
            T_REVERSING,
        ),
    ]);
    let named = Scenario::Including(vec![VoucherStatus::Optional]);
    let result = effective_postings(&b, &named, AS_OF);
    assert!(matches!(result, Ok(Outcome::Unsupported(_))));
}

#[test]
fn memorandum_dated_after_as_of_does_not_block_named_scenario() {
    // Rule 1 cuts by date before the type is examined: a memorandum voucher dated 1 Jul is after
    // 30 Jun, so it cannot change the answer and the named scenario still returns a value.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_SALES),
        posting(2, date!(2026 - 07 - 01), VoucherStatus::Regular, T_MEMO),
    ]);
    let named = Scenario::Including(vec![VoucherStatus::Optional]);
    assert_eq!(included(effective_postings(&b, &named, AS_OF)), vec![1]);
}

#[test]
fn cancelled_never_included() {
    // Voucher status doc: "Cancelled: number kept, no amounts", so never in any scenario, even
    // when the scenario lists Cancelled.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Cancelled, T_SALES),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Regular, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![2]
    );
    let named = Scenario::Including(vec![VoucherStatus::Cancelled]);
    assert_eq!(included(effective_postings(&b, &named, AS_OF)), vec![2]);
}

#[test]
fn unknown_voucher_type_is_an_error_not_a_silent_exclusion() {
    // Review fix round 2 (rule A1): a posting dated on or before the as-of date whose voucher type
    // is not in the masters cannot be classified. It is an error naming the type, never dropped.
    let b = books(vec![
        posting(1, date!(2026 - 05 - 01), VoucherStatus::Regular, T_UNKNOWN),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Regular, T_SALES),
    ]);
    let result = effective_postings(&b, &Scenario::Actual, AS_OF);
    assert!(matches!(
        result,
        Err(EngineError::NotFound { ref what }) if what == "voucher type 99"
    ));
}

#[test]
fn unknown_voucher_type_after_as_of_is_not_checked() {
    // Rule 1 applies first: a posting dated 1 Jul is out by date, so its unknown type is not an
    // error and the result is the in-range sales voucher.
    let b = books(vec![
        posting(1, date!(2026 - 07 - 01), VoucherStatus::Regular, T_UNKNOWN),
        posting(2, date!(2026 - 05 - 02), VoucherStatus::Regular, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![2]
    );
}

#[test]
fn order_of_books_is_kept() {
    // The result preserves the order of Books::postings (sorted by date, voucher).
    let b = books(vec![
        posting(5, date!(2026 - 04 - 01), VoucherStatus::Regular, T_SALES),
        posting(3, date!(2026 - 04 - 02), VoucherStatus::Regular, T_SALES),
        posting(4, date!(2026 - 04 - 02), VoucherStatus::Regular, T_SALES),
    ]);
    assert_eq!(
        included(effective_postings(&b, &Scenario::Actual, AS_OF)),
        vec![5, 3, 4]
    );
}
