//! Card E1 tests: group and ledger commands (spec section 5, invariant 5; research R1).
//!
//! Provenance, for every expected value below, is `derived`:
//! - spec section 5, invariant 5: "Reserved groups/ledgers cannot be deleted; renaming is harmless";
//! - research R1 (docs/research/01-core-accounting.md): the 28 reserved groups "can be altered
//!   (renaming is discouraged); cannot be deleted"; "Group tree ... Nature (Asset/Liability/
//!   Income/Expense) only on primaries, inherited" (spec section 5, Group);
//! - the plan card E1: deleting a ledger with postings errors `InUse { vouchers: n }`, a cycle in
//!   the group parent errors;
//! - a tree has no cycle by definition: a group is an ancestor of the groups under it, so placing
//!   it under itself or under one of those makes it its own ancestor.
//!
//! The number n in `InUse` is the number of distinct vouchers (counted by hand in each test), not
//! the number of lines.
//!
//! Cases with no source (changing a reserved group's parent or nature; moving a reserved ledger
//! to another group) are `pending` and assert `Unsupported`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, GroupId, LedgerId, Outcome, Paise, Rate, SignedAmount, StateCode, VoucherId,
    VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_engine::masters_accounts::{
    alter_group, alter_ledger, create_group, create_ledger, delete_group, delete_ledger,
};
use ledgerly_model::{
    Address, BillAllocation, BillRefKind, BillWiseConfig, Books, Company, EntityKind, Features,
    Group, GstLine, Ledger, LedgerLine, MasterSnapshot, Masters, Nature, NegativeStockPolicy,
    ReservedGroup, ReservedLedger, StoredGst, TaxLedgerKind, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::Event;
use time::macros::date;

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: parent.map(GroupId),
        reserved: None,
        nature,
        affects_gross_profit: false,
        tally: None,
    }
}

fn reserved_group(id: u64, name: &str, kind: ReservedGroup, nature: Option<Nature>) -> Group {
    Group {
        ineligible_itc: None,
        reserved: Some(kind),
        ..group(id, name, None, nature)
    }
}

fn ledger(id: u64, name: &str, group: u64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(GroupId(group)),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: u64, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: LedgerId(ledger),
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn voucher(id: u64, status: VoucherStatus, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: None,
        series: None,
        date: date!(2025 - 05 - 10),
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Groups: 1 Current Assets (reserved, primary, Asset), 2 Cash-in-Hand (reserved, under 1),
/// 3 Sundry Debtors (reserved, under 1), 10 Region (custom primary, Income), 11 North (under 10),
/// 12 North-East (under 11).
/// Ledgers: 1 Cash (reserved, group 2), 2 Debtor A (group 3), 3 Unused (group 3).
fn books() -> Books {
    let mut masters = Masters::default();
    let mut cash_group = reserved_group(2, "Cash-in-Hand", ReservedGroup::CashInHand, None);
    cash_group.parent = Some(GroupId(1));
    let mut debtors = reserved_group(3, "Sundry Debtors", ReservedGroup::SundryDebtors, None);
    debtors.parent = Some(GroupId(1));
    for g in [
        reserved_group(
            1,
            "Current Assets",
            ReservedGroup::CurrentAssets,
            Some(Nature::Asset),
        ),
        cash_group,
        debtors,
        group(10, "Region", None, Some(Nature::Income)),
        group(11, "North", Some(10), None),
        group(12, "North-East", Some(11), None),
    ] {
        masters.groups.insert(g.id, g);
    }
    let mut cash = ledger(1, "Cash", 2);
    cash.reserved = Some(ReservedLedger::Cash);
    for l in [cash, ledger(2, "Debtor A", 3), ledger(3, "Unused", 3)] {
        masters.ledgers.insert(l.id, l);
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
            fy_start: date!(2025 - 04 - 01),
            books_begin: date!(2025 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn with_vouchers(mut b: Books, vs: Vec<Voucher>) -> Books {
    for v in vs {
        b.vouchers.insert(v.id, v);
    }
    b
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "matches the Result<Outcome> the handlers return, so tests compare like with like"
)]
fn no_events() -> Result<Outcome<Vec<Event>>, EngineError> {
    Ok(Outcome::Value(Vec::new()))
}

// ---------------------------------------------------------------- groups

#[test]
fn deleting_a_reserved_group_errors_reserved_group() {
    // provenance: derived, spec 5 invariant 5 and R1: reserved groups cannot be deleted.
    let b = books();
    assert_eq!(
        delete_group(GroupId(1), &b),
        Err(EngineError::ReservedGroup(ReservedGroup::CurrentAssets))
    );
    // Even a reserved group with no children or ledgers is protected (Cash-in-Hand has a ledger,
    // so use a reserved group that is empty: the check is on the marker, not on emptiness).
    let mut b = books();
    b.masters.groups.insert(
        GroupId(20),
        reserved_group(20, "Provisions", ReservedGroup::Provisions, None),
    );
    assert_eq!(
        delete_group(GroupId(20), &b),
        Err(EngineError::ReservedGroup(ReservedGroup::Provisions))
    );
}

#[test]
fn a_renamed_reserved_group_is_still_protected() {
    // provenance: derived, spec 5 invariant 5: "renaming is harmless"; reserved status keys off the
    // marker, never the name (R1 data model: reserved_kind).
    let mut b = books();
    if let Some(g) = b.masters.groups.get_mut(&GroupId(3)) {
        g.name = "Customers".to_owned();
    }
    assert_eq!(
        delete_group(GroupId(3), &b),
        Err(EngineError::ReservedGroup(ReservedGroup::SundryDebtors))
    );
}

#[test]
fn a_group_cannot_be_its_own_parent() {
    // provenance: derived, the tree definition: parent = self makes the group its own ancestor.
    let b = books();
    let mut g = b.masters.groups[&GroupId(10)].clone();
    g.parent = Some(GroupId(10));
    g.nature = None;
    assert_eq!(alter_group(g, &b), Err(EngineError::GroupCycle));
}

#[test]
fn a_group_cannot_move_under_its_child_or_grandchild() {
    // provenance: derived, the tree definition. Region(10) > North(11) > North-East(12).
    // Moving Region under North, or under North-East, makes Region its own ancestor.
    let b = books();
    for new_parent in [11, 12] {
        let mut g = b.masters.groups[&GroupId(10)].clone();
        g.parent = Some(GroupId(new_parent));
        g.nature = None;
        assert_eq!(alter_group(g, &b), Err(EngineError::GroupCycle));
    }
    // Moving North under North-East is also a cycle (12 is below 11).
    let mut g = b.masters.groups[&GroupId(11)].clone();
    g.parent = Some(GroupId(12));
    assert_eq!(alter_group(g, &b), Err(EngineError::GroupCycle));
}

#[test]
fn moving_a_group_to_an_unrelated_parent_is_allowed() {
    // provenance: derived, the tree definition: Current Assets(1) is not below North-East(12), so
    // North-East may move under it. It stays a sub-group, so it carries no nature.
    let b = books();
    let mut g = b.masters.groups[&GroupId(12)].clone();
    g.parent = Some(GroupId(1));
    let before = b.masters.groups[&GroupId(12)].clone();
    assert_eq!(
        alter_group(g.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterAltered {
            before: MasterSnapshot::Group(before),
            after: MasterSnapshot::Group(g),
        }]))
    );
}

#[test]
fn an_existing_loop_in_stored_data_is_reported_as_a_cycle_not_followed_forever() {
    // provenance: derived, the tree definition. 11 and 12 point at each other (corrupt data);
    // the command must still terminate and refuse.
    let mut b = books();
    if let Some(g) = b.masters.groups.get_mut(&GroupId(11)) {
        g.parent = Some(GroupId(12));
    }
    let mut g = b.masters.groups[&GroupId(10)].clone();
    g.parent = Some(GroupId(12));
    g.nature = None;
    assert_eq!(alter_group(g, &b), Err(EngineError::GroupCycle));
}

#[test]
fn create_group_under_a_parent_emits_one_created_event() {
    // provenance: derived, spec 5: nature only on primaries, inherited by sub-groups.
    let b = books();
    let g = group(30, "South", Some(10), None);
    assert_eq!(
        create_group(g.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterCreated(
            MasterSnapshot::Group(g)
        )]))
    );
}

#[test]
fn create_primary_group_needs_a_nature_and_a_sub_group_must_not_have_one() {
    // provenance: derived, spec 5: "Nature ... only on primaries, inherited".
    let b = books();
    let no_nature = group(30, "Other Income Heads", None, None);
    assert!(matches!(
        create_group(no_nature, &b),
        Err(EngineError::Invalid { .. })
    ));
    let sub_with_nature = group(31, "South", Some(10), Some(Nature::Expense));
    assert!(matches!(
        create_group(sub_with_nature, &b),
        Err(EngineError::Invalid { .. })
    ));
    let primary = group(32, "Grants", None, Some(Nature::Income));
    assert!(matches!(create_group(primary, &b), Ok(Outcome::Value(_))));
}

#[test]
fn create_group_rejects_duplicates_missing_parent_empty_name_and_reserved_marker() {
    // provenance: derived, EngineError docs: Duplicate = "a name or number is already taken",
    // NotFound = "something referenced does not exist"; reserved groups are seeded (R1: 28 fixed).
    let b = books();
    assert_eq!(
        create_group(group(10, "Another", Some(1), None), &b),
        Err(EngineError::Duplicate {
            what: "group id 10".to_owned()
        })
    );
    assert_eq!(
        create_group(group(30, " North ", Some(10), None), &b),
        Err(EngineError::Duplicate {
            what: "group 'North'".to_owned()
        })
    );
    assert_eq!(
        create_group(group(30, "Ghost child", Some(99), None), &b),
        Err(EngineError::NotFound {
            what: "parent group 99".to_owned()
        })
    );
    assert!(matches!(
        create_group(group(30, "   ", Some(10), None), &b),
        Err(EngineError::Invalid { .. })
    ));
    let reserved = reserved_group(30, "Provisions", ReservedGroup::Provisions, None);
    assert!(matches!(
        create_group(reserved, &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn renaming_a_reserved_group_is_allowed_and_audited() {
    // provenance: derived, spec 5 invariant 5 "renaming is harmless"; R1 "can be altered".
    let b = books();
    let before = b.masters.groups[&GroupId(3)].clone();
    let mut after = before.clone();
    after.name = "Customers".to_owned();
    after.aliases = vec!["Debtors".to_owned()];
    assert_eq!(
        alter_group(after.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterAltered {
            before: MasterSnapshot::Group(before),
            after: MasterSnapshot::Group(after),
        }]))
    );
}

#[test]
fn altering_a_group_to_itself_emits_no_events() {
    // provenance: derived, rule A1 (no silent change): an unchanged master is not an audited change.
    let b = books();
    let g = b.masters.groups[&GroupId(10)].clone();
    assert_eq!(alter_group(g, &b), no_events());
}

#[test]
fn a_reserved_group_cannot_stop_being_reserved_or_change_kind() {
    // provenance: derived, R1: behaviour keys off the reserved marker, so the marker is identity.
    let b = books();
    let mut g = b.masters.groups[&GroupId(3)].clone();
    g.reserved = None;
    assert!(matches!(
        alter_group(g, &b),
        Err(EngineError::Invalid { .. })
    ));
    let mut g = b.masters.groups[&GroupId(10)].clone();
    g.reserved = Some(ReservedGroup::Provisions);
    assert!(matches!(
        alter_group(g, &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn pending_reserved_group_parent_or_nature_change_is_unsupported() {
    // pending: no source says whether a reserved group may be re-parented or change nature
    // (R1 only says "can be altered"). Must be Unsupported, never a guess.
    let b = books();
    let mut g = b.masters.groups[&GroupId(3)].clone();
    g.parent = None;
    g.nature = Some(Nature::Asset);
    assert!(matches!(alter_group(g, &b), Ok(Outcome::Unsupported(_))));
    let mut g = b.masters.groups[&GroupId(1)].clone();
    g.nature = Some(Nature::Liability);
    assert!(matches!(alter_group(g, &b), Ok(Outcome::Unsupported(_))));
}

#[test]
fn altering_an_unknown_group_errors_not_found() {
    // provenance: derived, EngineError::NotFound docs.
    let b = books();
    assert_eq!(
        alter_group(group(99, "Ghost", None, Some(Nature::Asset)), &b),
        Err(EngineError::NotFound {
            what: "group id 99".to_owned()
        })
    );
    assert_eq!(
        delete_group(GroupId(99), &b),
        Err(EngineError::NotFound {
            what: "group id 99".to_owned()
        })
    );
}

#[test]
fn deleting_a_group_that_still_has_children_or_ledgers_errors() {
    // provenance: derived, spec 5 invariant 2 spirit: no master may point at a deleted master.
    let b = books();
    // Region(10) has North(11) under it.
    assert_eq!(
        delete_group(GroupId(10), &b),
        Err(EngineError::InUseByMaster {
            what: "group 'Region'".to_owned(),
            by: "sub-group 'North'".to_owned(),
        })
    );
    // A custom group holding a ledger.
    let mut b = books();
    b.masters
        .ledgers
        .insert(LedgerId(9), ledger(9, "Zone ledger", 12));
    assert_eq!(
        delete_group(GroupId(12), &b),
        Err(EngineError::InUseByMaster {
            what: "group 'North-East'".to_owned(),
            by: "ledger 'Zone ledger'".to_owned(),
        })
    );
}

#[test]
fn deleting_an_empty_custom_group_emits_a_deleted_event_with_its_last_state() {
    // provenance: derived, spec 6.6: the audit event holds a full copy of the master.
    let b = books();
    let g = b.masters.groups[&GroupId(12)].clone();
    assert_eq!(
        delete_group(GroupId(12), &b),
        Ok(Outcome::Value(vec![Event::MasterDeleted(
            MasterSnapshot::Group(g)
        )]))
    );
}

// ---------------------------------------------------------------- ledgers

#[test]
fn deleting_a_reserved_ledger_errors_reserved_ledger() {
    // provenance: derived, spec 5 invariant 5: reserved ledgers cannot be deleted.
    let b = books();
    assert_eq!(
        delete_ledger(LedgerId(1), &b),
        Err(EngineError::ReservedLedger(ReservedLedger::Cash))
    );
}

#[test]
fn deleting_a_ledger_with_postings_errors_in_use_with_the_voucher_count() {
    // provenance: derived, plan card E1. Debtor A (2) is on three lines of two vouchers:
    // voucher 1 has two lines on it (still one voucher), voucher 2 has one. Vouchers = 2.
    let v1 = voucher(
        1,
        VoucherStatus::Regular,
        vec![
            line(2, SignedAmount::dr(Paise::new(10_000))),
            line(2, SignedAmount::dr(Paise::new(5000))),
            line(1, SignedAmount::cr(Paise::new(15_000))),
        ],
    );
    let v2 = voucher(
        2,
        VoucherStatus::Regular,
        vec![
            line(1, SignedAmount::dr(Paise::new(2000))),
            line(2, SignedAmount::cr(Paise::new(2000))),
        ],
    );
    let b = with_vouchers(books(), vec![v1, v2]);
    assert_eq!(
        delete_ledger(LedgerId(2), &b),
        Err(EngineError::InUse { vouchers: 2 })
    );
}

#[test]
fn cancelled_and_optional_vouchers_still_hold_the_ledger() {
    // provenance: derived, spec 5: a cancelled voucher keeps its number and lines (the voucher
    // still exists), and an optional voucher exists too; deleting the ledger would orphan them.
    // One cancelled and one optional voucher on Debtor A: n = 2.
    let cancelled = voucher(
        1,
        VoucherStatus::Cancelled,
        vec![line(2, SignedAmount::dr(Paise::new(1000)))],
    );
    let optional = voucher(
        2,
        VoucherStatus::Optional,
        vec![line(2, SignedAmount::dr(Paise::new(1000)))],
    );
    let b = with_vouchers(books(), vec![cancelled, optional]);
    assert_eq!(
        delete_ledger(LedgerId(2), &b),
        Err(EngineError::InUse { vouchers: 2 })
    );
}

#[test]
fn a_ledger_used_only_as_party_or_tax_ledger_is_in_use() {
    // provenance: derived, spec 5 Voucher: party and the GST block's tax ledgers are ledger
    // references too. Voucher 1 has Debtor A (2) as party only; voucher 2 names Unused (3) as a
    // GST tax ledger only. n = 1 each.
    let mut by_party = voucher(
        1,
        VoucherStatus::Regular,
        vec![line(1, SignedAmount::dr(Paise::new(500)))],
    );
    by_party.party = Some(LedgerId(2));
    let mut by_gst = voucher(
        2,
        VoucherStatus::Regular,
        vec![line(1, SignedAmount::dr(Paise::new(500)))],
    );
    by_gst.gst = Some(StoredGst {
        pack_version: "fixture".to_owned(),
        lines: vec![GstLine {
            ledger: LedgerId(3),
            kind: TaxLedgerKind::Cgst,
            taxable: Paise::new(10_000),
            rate_percent: Rate::default(),
            tax: Paise::ZERO,
        }],
        round_off: Paise::ZERO,
    });
    let b = with_vouchers(books(), vec![by_party, by_gst]);
    assert_eq!(
        delete_ledger(LedgerId(2), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    assert_eq!(
        delete_ledger(LedgerId(3), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
}

#[test]
fn deleting_an_unused_ledger_emits_a_deleted_event() {
    // provenance: derived, spec 6.6: the event holds a full copy of the master.
    let b = books();
    let l = b.masters.ledgers[&LedgerId(3)].clone();
    assert_eq!(
        delete_ledger(LedgerId(3), &b),
        Ok(Outcome::Value(vec![Event::MasterDeleted(
            MasterSnapshot::Ledger(l)
        )]))
    );
}

#[test]
fn deleting_a_ledger_with_an_opening_balance_is_refused() {
    // provenance: derived, spec 5 invariant 4: the opening trial balance is shown with an explicit
    // difference line; removing a ledger that carries an opening amount would change it silently.
    let mut b = books();
    if let Some(l) = b.masters.ledgers.get_mut(&LedgerId(3)) {
        l.opening = SignedAmount::cr(Paise::new(100_000));
    }
    assert!(matches!(
        delete_ledger(LedgerId(3), &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn deleting_an_unknown_ledger_errors_not_found() {
    // provenance: derived, EngineError::NotFound docs.
    assert_eq!(
        delete_ledger(LedgerId(99), &books()),
        Err(EngineError::NotFound {
            what: "ledger id 99".to_owned()
        })
    );
}

#[test]
fn create_ledger_emits_one_created_event() {
    // provenance: derived, plan card E1 and spec 6.6.
    let b = books();
    let mut l = ledger(10, "Debtor B", 3);
    l.opening = SignedAmount::dr(Paise::new(25_000_000));
    assert_eq!(
        create_ledger(l.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterCreated(
            MasterSnapshot::Ledger(l)
        )]))
    );
}

#[test]
fn create_ledger_rejects_bad_input() {
    // provenance: derived, EngineError docs (Duplicate, NotFound, Invalid) and spec 5 Ledger:
    // "opening (Paise, Dr/Cr)": the amount is non-negative, the side carries the sign.
    let b = books();
    assert_eq!(
        create_ledger(ledger(2, "Another", 3), &b),
        Err(EngineError::Duplicate {
            what: "ledger id 2".to_owned()
        })
    );
    assert_eq!(
        create_ledger(ledger(10, "Debtor A", 3), &b),
        Err(EngineError::Duplicate {
            what: "ledger 'Debtor A'".to_owned()
        })
    );
    assert_eq!(
        create_ledger(ledger(10, "Orphan", 99), &b),
        Err(EngineError::NotFound {
            what: "group 99 of ledger 'Orphan'".to_owned()
        })
    );
    assert!(matches!(
        create_ledger(ledger(10, "", 3), &b),
        Err(EngineError::Invalid { .. })
    ));
    let mut negative = ledger(10, "Negative", 3);
    negative.opening = SignedAmount::dr(Paise::new(-1));
    assert!(matches!(
        create_ledger(negative, &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn a_reserved_ledger_role_can_be_created_only_once() {
    // provenance: derived, spec 5: "Reserved ledgers: Cash, Profit & Loss" (one each).
    let b = books();
    let mut second_cash = ledger(10, "Petty Cash", 2);
    second_cash.reserved = Some(ReservedLedger::Cash);
    assert_eq!(
        create_ledger(second_cash, &b),
        Err(EngineError::Duplicate {
            what: "reserved ledger Cash".to_owned()
        })
    );
    let mut pnl = ledger(11, "Profit & Loss A/c", 1);
    pnl.reserved = Some(ReservedLedger::ProfitAndLoss);
    assert!(matches!(create_ledger(pnl, &b), Ok(Outcome::Value(_))));
}

#[test]
fn renaming_a_reserved_ledger_is_allowed() {
    // provenance: derived, spec 5 invariant 5 "renaming is harmless".
    let b = books();
    let before = b.masters.ledgers[&LedgerId(1)].clone();
    let mut after = before.clone();
    after.name = "Cash on Hand".to_owned();
    assert_eq!(
        alter_ledger(after.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterAltered {
            before: MasterSnapshot::Ledger(before),
            after: MasterSnapshot::Ledger(after),
        }]))
    );
}

#[test]
fn a_reserved_ledger_cannot_stop_being_reserved() {
    // provenance: derived, R1: behaviour keys off the reserved marker.
    let b = books();
    let mut l = b.masters.ledgers[&LedgerId(1)].clone();
    l.reserved = None;
    assert!(matches!(
        alter_ledger(l, &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn pending_moving_a_reserved_ledger_to_another_group_is_unsupported() {
    // pending: R1 says Cash sits under Cash-in-hand but does not say it may not move; must be
    // Unsupported, never a guess.
    let b = books();
    let mut l = b.masters.ledgers[&LedgerId(1)].clone();
    l.group = Some(GroupId(3));
    assert!(matches!(alter_ledger(l, &b), Ok(Outcome::Unsupported(_))));
}

#[test]
fn altering_a_ledger_checks_group_name_and_opening() {
    // provenance: derived, same rules as create_ledger.
    let b = books();
    let mut l = b.masters.ledgers[&LedgerId(2)].clone();
    l.group = Some(GroupId(99));
    assert_eq!(
        alter_ledger(l, &b),
        Err(EngineError::NotFound {
            what: "group 99 of ledger 'Debtor A'".to_owned()
        })
    );
    let mut l = b.masters.ledgers[&LedgerId(2)].clone();
    l.name = "Unused".to_owned();
    assert_eq!(
        alter_ledger(l, &b),
        Err(EngineError::Duplicate {
            what: "ledger 'Unused'".to_owned()
        })
    );
    assert_eq!(
        alter_ledger(ledger(99, "Ghost", 3), &b),
        Err(EngineError::NotFound {
            what: "ledger id 99".to_owned()
        })
    );
}

#[test]
fn altering_a_ledger_to_itself_emits_no_events() {
    // provenance: derived, rule A1 (no silent change): an unchanged master is not an audited change.
    let b = books();
    let l = b.masters.ledgers[&LedgerId(2)].clone();
    assert_eq!(alter_ledger(l, &b), no_events());
}

#[test]
fn bill_wise_cannot_be_turned_off_while_bill_allocations_exist() {
    // provenance: derived, spec 5 invariant 2: bill allocations on a bill-wise line sum to the line
    // amount. Debtor A is bill-wise with one voucher line carrying a New bill; turning it off would
    // leave that allocation on a non-bill-wise ledger.
    let mut b = books();
    if let Some(l) = b.masters.ledgers.get_mut(&LedgerId(2)) {
        l.bill_wise = Some(BillWiseConfig {
            default_credit_days: Some(30),
        });
    }
    let mut with_bill = line(2, SignedAmount::dr(Paise::new(10_000)));
    with_bill.bills = vec![BillAllocation {
        kind: BillRefKind::New,
        name: "INV-1".to_owned(),
        due_date: None,
        credit_days: Some(30),
        amount: SignedAmount::dr(Paise::new(10_000)),
    }];
    let v = voucher(
        1,
        VoucherStatus::Regular,
        vec![with_bill, line(1, SignedAmount::cr(Paise::new(10_000)))],
    );
    let b = with_vouchers(b, vec![v]);
    let mut off = b.masters.ledgers[&LedgerId(2)].clone();
    off.bill_wise = None;
    assert!(matches!(
        alter_ledger(off, &b),
        Err(EngineError::Invalid { .. })
    ));
    // Changing only the credit days is fine and is one audited alteration.
    let before = b.masters.ledgers[&LedgerId(2)].clone();
    let mut after = before.clone();
    after.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(45),
    });
    assert_eq!(
        alter_ledger(after.clone(), &b),
        Ok(Outcome::Value(vec![Event::MasterAltered {
            before: MasterSnapshot::Ledger(before),
            after: MasterSnapshot::Ledger(after),
        }]))
    );
}

#[test]
fn bill_wise_can_be_turned_off_when_no_bills_exist() {
    // provenance: derived, spec 5 invariant 2: nothing to orphan.
    let mut b = books();
    if let Some(l) = b.masters.ledgers.get_mut(&LedgerId(2)) {
        l.bill_wise = Some(BillWiseConfig {
            default_credit_days: None,
        });
    }
    let mut off = b.masters.ledgers[&LedgerId(2)].clone();
    off.bill_wise = None;
    assert!(matches!(alter_ledger(off, &b), Ok(Outcome::Value(_))));
}

#[test]
fn group_in_use_by_a_sub_group_or_ledger_is_typed_in_use_by_master() {
    // provenance: derived from the groups page (https://help.tallysolutions.com/tally-prime/accounting/groups-in-tallyprime/,
    // accessed 2026-11-21): a group with sub-groups or ledgers under it cannot be deleted. Hand
    // derivation: group 11 'North' sits under 'Region' (10), so the refusal names sub-group 'North';
    // group 12 'North-East' holds ledger 9 'Zone ledger', so the refusal names that ledger.
    let mut b = books();
    b.masters
        .ledgers
        .insert(LedgerId(9), ledger(9, "Zone ledger", 12));
    assert_eq!(
        delete_group(GroupId(12), &b),
        Err(EngineError::InUseByMaster {
            what: "group 'North-East'".to_owned(),
            by: "ledger 'Zone ledger'".to_owned(),
        })
    );
    assert_eq!(
        delete_group(GroupId(10), &b),
        Err(EngineError::InUseByMaster {
            what: "group 'Region'".to_owned(),
            by: "sub-group 'North'".to_owned(),
        })
    );
}
