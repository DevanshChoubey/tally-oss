//! s11: the read-only voucher view's Total row and Audit card come from the engine and the log.
//!
//! Provenance: derived. The vouchers are posted under stated actors at stated times, so what the
//! card must say follows from the rules in `voucher_audit.rs`: the totals are the sums of the
//! debit and of the credit lines (worked out by hand below), "who" is the actor's name in the
//! directory, "when" is the record's own time, and a change is listed once per later record.
//! The mock (design/mock-v2/index.html, `balance-sheet.voucher`) is the reference for what the
//! screen needs: "Created 14 Nov 2026 by Ananya K., typed by hand", "Not altered since", and the
//! lock state of the period.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_api::{Api, ChangeKind, Command, EntryOrigin};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, UserId,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::Actor;
use time::macros::{date, datetime};

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group,
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

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// A payment of Rs 1,000.00 to Rent, split into two debit lines (600.00 + 400.00) against one
/// credit of 1,000.00 from Cash, so the debit side has two lines to add.
fn payment(id: u64, on: Date, narration: &str) -> Voucher {
    let type_id: VoucherTypeId = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
        .unwrap()
        .id;
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: narration.to_owned(),
        ledger_lines: vec![
            line(RENT, SignedAmount::dr(Paise::new(60_000))),
            line(RENT, SignedAmount::dr(Paise::new(40_000))),
            line(CASH, SignedAmount::cr(Paise::new(100_000))),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
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
    }
}

fn open(dir: &std::path::Path) -> Api {
    let mut api = Api::create(
        &dir.join("c.ledgerly"),
        &company(),
        Clock::Fixed(datetime!(2026-11-14 10:30 +05:30)),
    )
    .unwrap();
    api.execute(Command::CreateLedger(ledger(
        CASH,
        "Cash",
        reserved(ReservedGroup::CashInHand),
    )))
    .unwrap();
    api.execute(Command::CreateLedger(ledger(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
    )))
    .unwrap();
    api.set_user_name(UserId(1), "Ananya K.").unwrap();
    api.set_user_name(UserId(2), "Rohan D.").unwrap();
    api
}

fn detail(api: &Api, id: u64) -> ledgerly_api::VoucherDetail {
    match api.voucher_detail(VoucherId(id)).unwrap() {
        Outcome::Value(d) => d,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

#[test]
fn totals_are_the_sums_of_each_side() {
    // provenance: derived. Debit lines 600.00 + 400.00 = 1,000.00; the one credit line is 1,000.00.
    let dir = tempfile::tempdir().unwrap();
    let mut api = open(dir.path());
    api.set_actor(Actor::User(UserId(1)));
    api.execute(Command::PostVoucher(payment(1, date!(2026 - 11 - 12), "")))
        .unwrap();
    let d = detail(&api, 1);
    assert_eq!(d.debit_total, Paise::new(100_000));
    assert_eq!(d.credit_total, Paise::new(100_000));
}

#[test]
fn the_audit_names_who_when_how_and_the_chain_place() {
    // provenance: derived. Voucher 1 is posted by user 1 ("Ananya K.") at the fixed clock,
    // 14 Nov 2026 10:30 +05:30, typed. Voucher 2 is drafted by the assistant and confirmed by
    // user 2 ("Rohan D."). The two posts are consecutive commits of one event each, so voucher 2's
    // record is the next one after voucher 1's (a company file starts with its seeded masters, so
    // no absolute number is asserted). Nothing altered either.
    let dir = tempfile::tempdir().unwrap();
    let mut api = open(dir.path());
    api.set_actor(Actor::User(UserId(1)));
    api.execute(Command::PostVoucher(payment(1, date!(2026 - 11 - 12), "")))
        .unwrap();
    api.set_actor(Actor::Assistant {
        model: "assistant".to_owned(),
        on_behalf_of: UserId(2),
    });
    api.execute(Command::PostVoucher(payment(2, date!(2026 - 11 - 13), "")))
        .unwrap();

    let a1 = detail(&api, 1).audit.unwrap();
    assert_eq!(a1.entered_by, "Ananya K.");
    assert_eq!(a1.origin, EntryOrigin::Typed);
    assert_eq!(a1.posted_at, datetime!(2026-11-14 10:30 +05:30));
    assert!(a1.event.0 > 0);
    assert_eq!(a1.hash.len(), 64);
    assert!(a1.hash.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(a1.changes, Vec::new());

    let a2 = detail(&api, 2).audit.unwrap();
    assert_eq!(a2.entered_by, "Rohan D.");
    assert_eq!(a2.origin, EntryOrigin::AiDraft);
    assert_eq!(a2.event.0, a1.event.0 + 1);
    assert_ne!(a1.hash, a2.hash);
}

#[test]
fn an_alteration_and_the_lock_are_reported() {
    // provenance: derived. Voucher 1 is altered by user 2 (narration changed) after posting, so
    // the card lists one Altered change by "Rohan D.". The books are locked to 15 Nov 2026
    // (PeriodLocked), so a voucher dated 12 Nov is in a locked period and one dated 16 Nov is not.
    let dir = tempfile::tempdir().unwrap();
    let mut api = open(dir.path());
    api.set_actor(Actor::User(UserId(1)));
    api.execute(Command::PostVoucher(payment(1, date!(2026 - 11 - 12), "")))
        .unwrap();
    api.execute(Command::PostVoucher(payment(2, date!(2026 - 11 - 16), "")))
        .unwrap();
    api.set_actor(Actor::User(UserId(2)));
    let mut altered = api
        .books()
        .unwrap()
        .value()
        .unwrap()
        .vouchers
        .get(&VoucherId(1))
        .unwrap()
        .clone();
    altered.narration = "Corrected".to_owned();
    api.execute(Command::AlterVoucher(altered)).unwrap();
    api.execute(Command::LockPeriod(date!(2026 - 11 - 15)))
        .unwrap();

    let d1 = detail(&api, 1);
    let changes = &d1.audit.as_ref().unwrap().changes;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Altered);
    assert_eq!(changes[0].by, "Rohan D.");
    assert!(d1.period_locked());
    assert!(!detail(&api, 2).period_locked());
}

#[test]
fn a_voucher_not_in_the_books_is_unsupported() {
    // provenance: derived (rule A1): nothing is made up for a voucher that does not exist.
    let dir = tempfile::tempdir().unwrap();
    let api = open(dir.path());
    assert!(matches!(
        api.voucher_detail(VoucherId(99)).unwrap(),
        Outcome::Unsupported(_)
    ));
}
