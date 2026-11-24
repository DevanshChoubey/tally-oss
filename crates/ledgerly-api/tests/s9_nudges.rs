//! Home nudges from the engine: the GSTR-1 due date and rule D-02, possible duplicate payments.
//!
//! Provenance. The due date is `derived` from CBIC notification 83/2020-Central Tax of 10 Nov 2020
//! (under section 37 CGST Act), which gives monthly filers the 11th day of the month after the tax
//! period (read 2026-11-25 at <https://taxguru.in/goods-and-service-tax/cbic-notifies-time-limit-furnishing-form-gstr-1.html>
//! and <https://www.taxscan.in/cbic-notifies-rules-for-monthly-quarterly-filing-of-gstr-1-read-notification/83253/>).
//! No holiday shift is applied. The return "next due" is the earliest month whose due date is on or
//! after today: that choice is ours. Rule D-02 is our own rule (no accounting product's help page
//! defines it): two payments are a possible duplicate when both are live (regular or post-dated),
//! each has one debit ledger line, the ledger and the amount are the same, the narration is the
//! same once trimmed, spaced singly and lower-cased (and not empty), they fall within 30 days of
//! each other, and they do not settle different bills. Every expected value below is worked out by
//! hand from the vouchers written here.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Command, CommandResult, DuplicatePayment, gstr1_next_due};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, Company, EntityKind, Features, Ledger,
    LedgerLine, NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const POWER: LedgerId = LedgerId(2);
const RENT: LedgerId = LedgerId(3);
const SUPPLIER: LedgerId = LedgerId(4);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn ledger(id: LedgerId, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(rs(1_000_000)),
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

/// A payment of `rupees` from cash to `to` with `narration`.
fn payment(
    id: u64,
    number: &str,
    on: Date,
    to: LedgerId,
    rupees: i64,
    narration: &str,
    status: VoucherStatus,
) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
        .unwrap()
        .id;
    Voucher {
        id: VoucherId(id),
        type_id,
        number: Some(number.to_owned()),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: narration.to_owned(),
        ledger_lines: vec![
            line(to, SignedAmount::dr(rs(rupees))),
            line(CASH, SignedAmount::cr(rs(rupees))),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn books(name: &str, vouchers: Vec<Voucher>) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for l in [
        ledger(CASH, "Cash", ReservedGroup::CashInHand),
        ledger(POWER, "Electricity", ReservedGroup::IndirectExpenses),
        ledger(RENT, "Rent", ReservedGroup::IndirectExpenses),
        Ledger {
            bill_wise: Some(ledgerly_model::BillWiseConfig {
                default_credit_days: None,
            }),
            opening: SignedAmount::dr(Paise::ZERO),
            ..ledger(SUPPLIER, "Supplier", ReservedGroup::SundryCreditors)
        },
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    for v in vouchers {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

fn duplicates(api: &Api, today: Date) -> Vec<DuplicatePayment> {
    match api.home_nudges(today).unwrap().duplicates {
        Outcome::Value(d) => d,
        other @ Outcome::Unsupported(_) => panic!("{other:?}"),
    }
}

const NARRATION: &str = "City Power, October bill";

#[test]
fn gstr1_next_due_is_the_eleventh_of_the_month_after_the_period() {
    // provenance: derived (notification 83/2020-CT, see the module note). Each case is the
    // earliest month whose 11th-of-next-month due date is on or after today.
    let cases = [
        // 21 Nov: October's return was due 11 Nov (passed), so November's, due 11 Dec.
        (
            date!(2026 - 11 - 21),
            date!(2026 - 11 - 01),
            date!(2026 - 11 - 30),
            date!(2026 - 12 - 11),
        ),
        // 5 Nov: October's return is due 11 Nov, still ahead.
        (
            date!(2026 - 11 - 05),
            date!(2026 - 10 - 01),
            date!(2026 - 10 - 31),
            date!(2026 - 11 - 11),
        ),
        // On the due day itself the return is still due that day.
        (
            date!(2026 - 11 - 11),
            date!(2026 - 10 - 01),
            date!(2026 - 10 - 31),
            date!(2026 - 11 - 11),
        ),
        // The day after, it has passed: November's.
        (
            date!(2026 - 11 - 12),
            date!(2026 - 11 - 01),
            date!(2026 - 11 - 30),
            date!(2026 - 12 - 11),
        ),
        // January: December's return, due 11 Jan, across the year end.
        (
            date!(2027 - 01 - 05),
            date!(2026 - 12 - 01),
            date!(2026 - 12 - 31),
            date!(2027 - 01 - 11),
        ),
        // 20 Dec: December's return, due 11 Jan 2027.
        (
            date!(2026 - 12 - 20),
            date!(2026 - 12 - 01),
            date!(2026 - 12 - 31),
            date!(2027 - 01 - 11),
        ),
        // February has 28 days in 2027: January's return is due 11 Feb; on 20 Feb it is Feb's.
        (
            date!(2027 - 02 - 20),
            date!(2027 - 02 - 01),
            date!(2027 - 02 - 28),
            date!(2027 - 03 - 11),
        ),
    ];
    for (today, from, to, due) in cases {
        let (period, got_due) = gstr1_next_due(today).unwrap();
        assert_eq!(
            (period.from(), period.to(), got_due),
            (from, to, due),
            "today {today}"
        );
    }
}

#[test]
fn the_same_payment_twice_in_a_month_is_flagged() {
    // provenance: derived. PY/1 on 28 Oct and PY/2 on 12 Nov: same ledger (Electricity), same
    // amount 14,860, same narration. Days apart: 3 (29, 30, 31 Oct) + 12 = 15.
    let api = books(
        "s9_nudge_dup.ledgerly",
        vec![
            payment(
                1,
                "PY/1",
                date!(2026 - 10 - 28),
                POWER,
                14_860,
                NARRATION,
                VoucherStatus::Regular,
            ),
            payment(
                2,
                "PY/2",
                date!(2026 - 11 - 12),
                POWER,
                14_860,
                NARRATION,
                VoucherStatus::Regular,
            ),
        ],
    );
    let found = duplicates(&api, date!(2026 - 11 - 21));
    assert_eq!(found.len(), 1, "{found:?}");
    let d = found.first().unwrap();
    assert_eq!(
        (
            d.first.id,
            d.first.number.as_deref(),
            d.first.date,
            d.second.id,
            d.second.number.as_deref(),
            d.second.date
        ),
        (
            VoucherId(1),
            Some("PY/1"),
            date!(2026 - 10 - 28),
            VoucherId(2),
            Some("PY/2"),
            date!(2026 - 11 - 12)
        )
    );
    assert_eq!(
        (
            d.ledger.as_str(),
            d.amount,
            d.narration.as_str(),
            d.days_apart
        ),
        ("Electricity", rs(14_860), NARRATION, 15)
    );
}

#[test]
fn payments_that_differ_or_are_far_apart_are_not_flagged() {
    // provenance: derived. Each pair below breaks one condition of rule D-02:
    //  - 1 and 3: amount 14,860 against 14,861;
    //  - 4 and 5: narration "Rent, May" against "Rent, June";
    //  - 6 and 7: ledger Rent against Electricity;
    //  - 8 and 9: both narrations empty (an empty narration says nothing);
    //  - 10 and 11: 1 Apr and 2 May are 31 days apart (30 in April + 1), over the 30-day limit;
    //  - 12 and 13: the second is Optional, so it is not a live voucher.
    let r = VoucherStatus::Regular;
    let api = books(
        "s9_nudge_not_dup.ledgerly",
        vec![
            payment(1, "PY/1", date!(2026 - 06 - 01), POWER, 14_860, "A", r),
            payment(3, "PY/3", date!(2026 - 06 - 02), POWER, 14_861, "A", r),
            payment(
                4,
                "PY/4",
                date!(2026 - 06 - 03),
                RENT,
                5_000,
                "Rent, May",
                r,
            ),
            payment(
                5,
                "PY/5",
                date!(2026 - 06 - 04),
                RENT,
                5_000,
                "Rent, June",
                r,
            ),
            payment(
                6,
                "PY/6",
                date!(2026 - 06 - 05),
                RENT,
                7_000,
                "Same words",
                r,
            ),
            payment(
                7,
                "PY/7",
                date!(2026 - 06 - 06),
                POWER,
                7_000,
                "Same words",
                r,
            ),
            payment(8, "PY/8", date!(2026 - 06 - 07), RENT, 8_000, "", r),
            payment(9, "PY/9", date!(2026 - 06 - 08), RENT, 8_000, "", r),
            payment(10, "PY/10", date!(2026 - 07 - 01), RENT, 9_000, "Lease", r),
            payment(11, "PY/11", date!(2026 - 08 - 01), RENT, 9_000, "Lease", r),
            payment(
                12,
                "PY/12",
                date!(2026 - 09 - 01),
                POWER,
                3_000,
                "Filing",
                r,
            ),
            payment(
                13,
                "PY/13",
                date!(2026 - 09 - 02),
                POWER,
                3_000,
                "Filing",
                VoucherStatus::Optional,
            ),
        ],
    );
    assert_eq!(duplicates(&api, date!(2026 - 11 - 21)), Vec::new());
}

#[test]
fn only_neighbours_within_thirty_days_pair_up() {
    // provenance: derived. Three identical payments on 1 Oct, 20 Oct and 25 Nov. 1 Oct to 20 Oct
    // is 19 days (a pair); 20 Oct to 25 Nov is 11 + 25 = 36 days (no pair).
    let api = books(
        "s9_nudge_chain.ledgerly",
        vec![
            payment(
                1,
                "PY/1",
                date!(2026 - 10 - 01),
                POWER,
                100,
                "Same",
                VoucherStatus::Regular,
            ),
            payment(
                2,
                "PY/2",
                date!(2026 - 10 - 20),
                POWER,
                100,
                "Same",
                VoucherStatus::Regular,
            ),
            payment(
                3,
                "PY/3",
                date!(2026 - 11 - 25),
                POWER,
                100,
                "Same",
                VoucherStatus::Regular,
            ),
        ],
    );
    let found = duplicates(&api, date!(2026 - 11 - 26));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found.first().map(|d| (d.first.id, d.second.id)),
        Some((VoucherId(1), VoucherId(2)))
    );
}

#[test]
fn payments_settling_different_bills_are_not_flagged() {
    // provenance: derived. Supplier owes two bills of 2,000 (B-1, B-2), opened by journals on
    // 1 Sep. Payments PY/1 (1 Oct) and PY/2 (5 Oct) both pay 2,000 to the supplier with the
    // narration "Supplier": PY/1 settles B-1 and PY/2 settles B-2, so each pays its own bill and
    // nothing repeats, although the ledger, amount and narration match and the dates are 4 days
    // apart (without the bills clause the pair would be flagged).
    let r = VoucherStatus::Regular;
    let bills = |name: &str| {
        vec![BillAllocation {
            kind: BillRefKind::Against,
            name: name.to_owned(),
            due_date: None,
            credit_days: None,
            amount: SignedAmount::dr(rs(2_000)),
        }]
    };
    let with_bill = |mut v: Voucher, name: &str| {
        if let Some(l) = v.ledger_lines.first_mut() {
            l.bills = bills(name);
        }
        v
    };
    let opening = |id: u64, name: &str| {
        let mut v = payment(
            id,
            &format!("JV/{id}"),
            date!(2026 - 09 - 01),
            RENT,
            2_000,
            "Bill",
            r,
        );
        v.type_id = default_voucher_types()
            .into_iter()
            .find(|t| t.base == BaseVoucherKind::Journal)
            .unwrap()
            .id;
        v.ledger_lines = vec![
            line(RENT, SignedAmount::dr(rs(2_000))),
            LedgerLine {
                bills: vec![BillAllocation {
                    kind: BillRefKind::New,
                    name: name.to_owned(),
                    due_date: None,
                    credit_days: None,
                    amount: SignedAmount::cr(rs(2_000)),
                }],
                ..line(SUPPLIER, SignedAmount::cr(rs(2_000)))
            },
        ];
        v
    };
    let mut first = payment(
        3,
        "PY/1",
        date!(2026 - 10 - 01),
        SUPPLIER,
        2_000,
        "Supplier",
        r,
    );
    first = with_bill(first, "B-1");
    let mut second = payment(
        4,
        "PY/2",
        date!(2026 - 10 - 05),
        SUPPLIER,
        2_000,
        "Supplier",
        r,
    );
    second = with_bill(second, "B-2");
    let api = books(
        "s9_nudge_bills.ledgerly",
        vec![opening(1, "B-1"), opening(2, "B-2"), first, second],
    );
    assert_eq!(duplicates(&api, date!(2026 - 11 - 21)), Vec::new());
}

#[test]
fn a_payment_on_or_before_the_lock_date_is_marked_locked() {
    // provenance: derived. The books are locked to 31 Oct 2026; PY/1 is dated 28 Oct (locked) and
    // PY/2 12 Nov (open).
    let mut api = books(
        "s9_nudge_lock.ledgerly",
        vec![
            payment(
                1,
                "PY/1",
                date!(2026 - 10 - 28),
                POWER,
                14_860,
                NARRATION,
                VoucherStatus::Regular,
            ),
            payment(
                2,
                "PY/2",
                date!(2026 - 11 - 12),
                POWER,
                14_860,
                NARRATION,
                VoucherStatus::Regular,
            ),
        ],
    );
    committed(api.execute(Command::LockPeriod(date!(2026 - 10 - 31))));
    let found = duplicates(&api, date!(2026 - 11 - 21));
    let d = found.first().unwrap();
    assert_eq!((d.first.locked, d.second.locked), (true, false));
}
