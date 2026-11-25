//! Review queue: rule D-02 possible duplicate payments listed as proposals (mock `review`).
//!
//! Provenance. Rule D-02 is our own rule (see `s9_nudges.rs` for its definition and the hand
//! worked values); the queue lists exactly the pairs `Api::home_nudges` raises, so each expected
//! value below is derived by hand from the vouchers written here. A source that is listed leaves
//! `unlisted`; the count stays withheld while any other source is still unlisted.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Command, CommandResult, ReviewSource};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode, VoucherStatus,
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

const NARRATION: &str = "City Power, October bill";

fn two_payments(name: &str) -> Api {
    books(
        name,
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
    )
}

#[test]
fn the_queue_lists_the_duplicate_pair_rule_d02_finds() {
    // provenance: derived. PY/1 on 28 Oct and PY/2 on 12 Nov: same ledger (Electricity), same
    // amount 14,860 and narration; days apart 3 (29, 30, 31 Oct) + 12 = 15.
    let api = two_payments("s11_review_dup.ledgerly");
    let queue = api.review_queue_on(date!(2026 - 11 - 21)).unwrap();
    assert_eq!(queue.duplicates.len(), 1, "{:?}", queue.duplicates);
    let d = &queue.duplicates[0];
    assert_eq!((d.first.id, d.second.id), (VoucherId(1), VoucherId(2)));
    assert_eq!((d.amount, d.days_apart), (rs(14_860), 15));
    assert_eq!(d.ledger, "Electricity");
}

#[test]
fn a_listed_duplicate_check_leaves_the_unlisted_sources() {
    // provenance: derived. Of the five sources with no list, D-02 is now listed, so four remain,
    // in the queue's fixed order.
    let api = two_payments("s11_review_dup_unlisted.ledgerly");
    let queue = api.review_queue_on(date!(2026 - 11 - 21)).unwrap();
    let sources: Vec<ReviewSource> = queue.unlisted.iter().map(|u| u.source).collect();
    assert_eq!(
        sources,
        vec![
            ReviewSource::BillDraft,
            ReviewSource::PlainWordsDraft,
            ReviewSource::NewLedgerSuggestion,
            ReviewSource::Gstr2bExplanation,
        ]
    );
}

#[test]
fn a_post_dated_payment_joins_the_queue_only_from_its_own_date() {
    // provenance: derived from the D-02 definition: a post-dated payment is live on or after its
    // date. PY/2 is post-dated 12 Nov: on 11 Nov there is no pair, on 12 Nov there is one.
    let api = books(
        "s11_review_dup_postdated.ledgerly",
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
                VoucherStatus::PostDated,
            ),
        ],
    );
    assert_eq!(
        api.review_queue_on(date!(2026 - 11 - 11))
            .unwrap()
            .duplicates
            .len(),
        0
    );
    assert_eq!(
        api.review_queue_on(date!(2026 - 11 - 12))
            .unwrap()
            .duplicates
            .len(),
        1
    );
}

#[test]
fn the_count_adds_the_duplicates_once_every_source_is_listed() {
    // provenance: derived. While sources are unlisted the count is withheld (None); the duplicate
    // rows do not change that.
    let api = two_payments("s11_review_dup_count.ledgerly");
    let queue = api.review_queue_on(date!(2026 - 11 - 21)).unwrap();
    assert_eq!(queue.count(), None);
    let mut all = queue.clone();
    all.unlisted.clear();
    assert_eq!(all.count(), Some(all.items.len() + 1));
}
