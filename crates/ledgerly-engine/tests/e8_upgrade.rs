//! S23: the predefined ledgers given to a company made before round s22.
//!
//! Provenance (`derived`): TallyPrime help, "Ledgers in TallyPrime"
//! (<https://help.tallysolutions.com/ledgers-in-tallyprime/>, read 2026-11-27): "TallyPrime
//! provides predefined ledgers such as Cash and Profit & Loss A/c"; Cash sits under Cash-in-Hand
//! and Profit & Loss A/c under Primary (round s22, `ledgerly_model::seed_ledgers`). The expected
//! ids are counted by hand: the next id is one more than the highest ledger id in the fixture.
//! A hand-made Cash in Cash-in-Hand keeps its id and opening; only its role changes, so no figure
//! moves. A ledger named Cash in another group is left alone, because its balance would move.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_engine::upgrade::{PredefinedStep, PredefinedUpgrade, predefined_ledgers_upgrade};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, Ledger, MasterSnapshot, Masters,
    NegativeStockPolicy, ReservedGroup, ReservedLedger,
};
use ledgerly_store::Event;
use time::macros::date;

fn group_of(role: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(role))
        .unwrap()
        .id
}

fn ledger(id: u64, name: &str, group: GroupId, opening: i64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
        opening: SignedAmount::dr(Paise::new(opening)),
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

/// An old company: the reserved groups and the given ledgers, no predefined ledgers.
fn old_books(ledgers: Vec<Ledger>) -> Books {
    let mut masters = Masters::default();
    for g in reserved_groups() {
        masters.groups.insert(g.id, g);
    }
    for l in ledgers {
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
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
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

fn run(books: &Books) -> PredefinedUpgrade {
    match predefined_ledgers_upgrade(books).unwrap() {
        Outcome::Value(u) => u,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

fn created(events: &[Event]) -> Vec<Ledger> {
    events
        .iter()
        .filter_map(|e| {
            if let Event::MasterCreated(MasterSnapshot::Ledger(l)) = e {
                Some(l.clone())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn a_company_with_no_cash_gets_both_predefined_ledgers_with_the_next_ids() {
    let rent = ledger(4, "Rent", group_of(ReservedGroup::IndirectExpenses), 0);
    let books = old_books(vec![rent]);
    let up = run(&books);
    let new = created(&up.events);
    assert_eq!(new.len(), 2, "{:?}", up.events);
    // Derived: highest ledger id is 4, so Cash is 5 and Profit & Loss A/c is 6.
    assert_eq!(new[0].id, LedgerId(5));
    assert_eq!(new[0].name, "Cash");
    assert_eq!(new[0].group, Some(group_of(ReservedGroup::CashInHand)));
    assert_eq!(new[0].reserved, Some(ReservedLedger::Cash));
    assert_eq!(new[1].id, LedgerId(6));
    assert_eq!(new[1].name, "Profit & Loss A/c");
    assert_eq!(new[1].group, None);
    assert_eq!(new[1].reserved, Some(ReservedLedger::ProfitAndLoss));
    assert_eq!(
        up.steps,
        vec![
            PredefinedStep::Created {
                role: ReservedLedger::Cash,
                id: LedgerId(5),
                name: "Cash".to_owned()
            },
            PredefinedStep::Created {
                role: ReservedLedger::ProfitAndLoss,
                id: LedgerId(6),
                name: "Profit & Loss A/c".to_owned()
            },
        ]
    );
}

#[test]
fn a_hand_made_cash_in_cash_in_hand_takes_the_role_and_keeps_its_id_and_opening() {
    let cash = ledger(1, "Cash", group_of(ReservedGroup::CashInHand), 250_000);
    let rent = ledger(2, "Rent", group_of(ReservedGroup::IndirectExpenses), 0);
    let books = old_books(vec![cash.clone(), rent]);
    let up = run(&books);
    let after = Ledger {
        reserved: Some(ReservedLedger::Cash),
        ..cash.clone()
    };
    assert_eq!(
        up.events[0],
        Event::MasterAltered {
            before: MasterSnapshot::Ledger(cash),
            after: MasterSnapshot::Ledger(after),
        }
    );
    let new = created(&up.events);
    assert_eq!(new.len(), 1);
    // Derived: highest ledger id is 2, so Profit & Loss A/c is 3.
    assert_eq!(new[0].id, LedgerId(3));
    assert_eq!(
        up.steps[0],
        PredefinedStep::Adopted {
            role: ReservedLedger::Cash,
            id: LedgerId(1),
            name: "Cash".to_owned()
        }
    );
}

#[test]
fn a_ledger_named_cash_in_another_group_is_left_alone_and_reported() {
    let cash = ledger(1, "Cash", group_of(ReservedGroup::BankAccounts), 0);
    let books = old_books(vec![cash]);
    let up = run(&books);
    assert_eq!(
        up.events.len(),
        1,
        "only Profit & Loss A/c: {:?}",
        up.events
    );
    let PredefinedStep::NotDone { role, reason } = &up.steps[0] else {
        panic!("{:?}", up.steps[0]);
    };
    assert_eq!(*role, ReservedLedger::Cash);
    assert!(reason.contains("'Bank Accounts'"), "{reason}");
    assert!(reason.contains("'Cash-in-Hand'"), "{reason}");
}

#[test]
fn upgraded_books_need_nothing_more() {
    let books = old_books(vec![ledger(
        4,
        "Rent",
        group_of(ReservedGroup::IndirectExpenses),
        0,
    )]);
    let up = run(&books);
    let mut upgraded = books.clone();
    for l in created(&up.events) {
        upgraded.masters.ledgers.insert(l.id, l);
    }
    let again = run(&upgraded);
    assert_eq!(again.events, Vec::new());
    assert!(matches!(
        again.steps[..],
        [
            PredefinedStep::AlreadyThere { .. },
            PredefinedStep::AlreadyThere { .. }
        ]
    ));
}
