//! Card S4 tests: `load_books` replays the event log into masters, vouchers and postings.
//!
//! Provenance. Every expected value is derived from the definitions in the doc comments of
//! `ledgerly_model::Books` / `Posting` / `Voucher` and of `ledgerly_store::Event`, and from the
//! card text, never from output of the code under test:
//! - Ordering: `Books` says `postings` are sorted by `(date, voucher)`. Fixtures are committed in
//!   an order that differs from the sorted order, and the expected order is written out by hand
//!   from the dates and ids below.
//! - Altered voucher: `Event::VoucherAltered { before, after }` replaces the voucher, so only the
//!   postings of `after` remain (card text: "altered voucher shows only the new postings").
//! - Cancelled voucher: `Event::VoucherCancelled` is "number kept, amounts removed" and the card
//!   says it has zero postings but is listed.
//! - Postings: one per ledger line, same ledger, side and paise as the line (a posting is "derived
//!   from a voucher line"); status copied from the voucher.
//! - Amounts: 1,000 paise etc. are literal fixture values; nothing is computed.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] fns unwrap fixture values"
)]

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{
    BankStatementId, Clock, CompanyId, Date, DrCr, GroupId, Gstin, LedgerId, Month, OffsetDateTime,
    Paise, Pan, Period, SignedAmount, StateCode, UserId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BankAllocation, BankMatch, Books, Company, EntityKind, Features, Group,
    GstRegistration, InstrumentKind, LedgerLine, MasterSnapshot, MatchKind, NegativeStockPolicy,
    Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn company() -> Company {
    // Fictional data; GSTIN check character derived by hand in ledgerly-core's gstin tests.
    Company {
        id: CompanyId(7),
        name: "Example Traders Pvt Ltd".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Sample Road".to_owned()],
            pincode: Some("560001".to_owned()),
            state: Some(StateCode::new(29).unwrap()),
            country: String::new(),
        },
        state: StateCode::new(29).unwrap(),
        pan: Some(Pan::parse("ABCDE1234F").unwrap()),
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date(2017, Month::July, 1),
            composition: false,
        }],
        fy_start: date(2026, Month::April, 1),
        books_begin: date(2026, Month::April, 1),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn new_store(dir: &Path) -> SqliteStore {
    let path = dir.join("books.ledgerly");
    SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: None,
        app_version: "0.0.0-test".to_owned(),
    }
}

fn line(ledger: u64, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ledger: LedgerId(ledger),
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn dr(paise: i64) -> SignedAmount {
    SignedAmount::dr(Paise::new(paise))
}

fn cr(paise: i64) -> SignedAmount {
    SignedAmount::cr(Paise::new(paise))
}

fn voucher(id: u64, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(format!("V{id}")),
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
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn group(id: u64, name: &str) -> Group {
    Group {
        id: GroupId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        tally: None,
    }
}

fn two_line(id: u64, on: Date, paise: i64) -> Voucher {
    voucher(id, on, vec![line(10, dr(paise)), line(20, cr(paise))])
}

/// Asserts the load failed with `StoreError::Encoding` whose message names `needle`.
fn assert_encoding_error(result: &Result<Books, StoreError>, needle: &str) {
    assert!(
        matches!(result, Err(StoreError::Encoding(m)) if m.contains(needle)),
        "{result:?}"
    );
}

#[test]
fn empty_log_gives_empty_books_with_the_company() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(dir.path());
    let books = store.load_books(None).unwrap();
    assert_eq!(books.company, company());
    assert!(books.vouchers.is_empty());
    assert_eq!(books.postings, [] as [ledgerly_model::Posting; 0]);
    assert_eq!(books.stock_moves, [] as [ledgerly_model::StockMove; 0]);
    assert_eq!(books.masters, ledgerly_model::Masters::default());
}

#[test]
fn postings_come_back_sorted_by_date_then_voucher() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // Committed out of order on purpose: 12 Apr (id 1), 5 Apr (id 3), 5 Apr (id 2).
    let v1 = two_line(1, date(2026, Month::April, 12), 1_000);
    let v3 = two_line(3, date(2026, Month::April, 5), 3_000);
    let v2 = two_line(2, date(2026, Month::April, 5), 2_000);
    store
        .commit(
            &[
                Event::VoucherPosted(v1),
                Event::VoucherPosted(v3),
                Event::VoucherPosted(v2),
            ],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    // Derived order: date 5 Apr first, ids 2 then 3; then 12 Apr id 1; each voucher's lines in
    // line order (debit line on ledger 10, then credit line on ledger 20).
    let got: Vec<(u64, u64, DrCr, i64)> = books
        .postings
        .iter()
        .map(|p| {
            (
                p.voucher.0,
                p.ledger.0,
                p.amount.side,
                p.amount.amount.get(),
            )
        })
        .collect();
    let want = vec![
        (2, 10, DrCr::Dr, 2_000),
        (2, 20, DrCr::Cr, 2_000),
        (3, 10, DrCr::Dr, 3_000),
        (3, 20, DrCr::Cr, 3_000),
        (1, 10, DrCr::Dr, 1_000),
        (1, 20, DrCr::Cr, 1_000),
    ];
    assert_eq!(got, want);
    assert_eq!(books.vouchers.len(), 3);
    assert!(books.postings.iter().all(|p| p.type_id == VoucherTypeId(1)));
    assert!(
        books
            .postings
            .iter()
            .all(|p| p.status == VoucherStatus::Regular)
    );
    // Dates are copied from the vouchers.
    assert_eq!(books.postings[0].date, date(2026, Month::April, 5));
    assert_eq!(books.postings[4].date, date(2026, Month::April, 12));
}

#[test]
fn altered_voucher_shows_only_the_new_postings() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let before = two_line(1, date(2026, Month::April, 5), 1_000);
    let after = voucher(
        1,
        date(2026, Month::April, 6),
        vec![line(30, dr(1_500)), line(40, cr(1_500))],
    );
    store
        .commit(&[Event::VoucherPosted(before.clone())], &meta())
        .unwrap();
    store
        .commit(
            &[Event::VoucherAltered {
                before,
                after: after.clone(),
            }],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    assert_eq!(books.vouchers.get(&VoucherId(1)), Some(&after));
    let got: Vec<(u64, i64)> = books
        .postings
        .iter()
        .map(|p| (p.ledger.0, p.amount.amount.get()))
        .collect();
    assert_eq!(got, vec![(30, 1_500), (40, 1_500)]);
    assert!(
        books
            .postings
            .iter()
            .all(|p| p.date == date(2026, Month::April, 6))
    );
}

#[test]
fn cancelled_voucher_has_zero_postings_but_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let v1 = two_line(1, date(2026, Month::April, 5), 1_000);
    let v2 = two_line(2, date(2026, Month::April, 6), 2_000);
    store
        .commit(
            &[
                Event::VoucherPosted(v1.clone()),
                Event::VoucherPosted(v2),
                Event::VoucherCancelled { before: v1 },
            ],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    let cancelled = books.vouchers.get(&VoucherId(1)).unwrap();
    assert_eq!(cancelled.status, VoucherStatus::Cancelled);
    // Number is kept; amounts are removed.
    assert_eq!(cancelled.number.as_deref(), Some("V1"));
    assert_eq!(
        cancelled.ledger_lines,
        [] as [ledgerly_model::LedgerLine; 0]
    );
    assert!(books.postings.iter().all(|p| p.voucher != VoucherId(1)));
    assert_eq!(books.postings.len(), 2);
    assert_eq!(books.vouchers.len(), 2);
}

#[test]
fn deleted_voucher_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let v1 = two_line(1, date(2026, Month::April, 5), 1_000);
    store
        .commit(
            &[
                Event::VoucherPosted(v1.clone()),
                Event::VoucherDeleted { before: v1 },
            ],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    assert!(books.vouchers.is_empty());
    assert_eq!(books.postings, [] as [ledgerly_model::Posting; 0]);
}

#[test]
fn optional_voucher_keeps_its_status_on_postings() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let mut v = two_line(1, date(2026, Month::April, 5), 1_000);
    v.status = VoucherStatus::Optional;
    store.commit(&[Event::VoucherPosted(v)], &meta()).unwrap();
    let books = store.load_books(None).unwrap();
    assert_eq!(books.postings.len(), 2);
    assert!(
        books
            .postings
            .iter()
            .all(|p| p.status == VoucherStatus::Optional)
    );
}

#[test]
fn period_limits_vouchers_and_postings_by_date_inclusive() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let events: Vec<Event> = [
        (1, date(2026, Month::March, 31)),
        (2, date(2026, Month::April, 1)),
        (3, date(2026, Month::April, 30)),
        (4, date(2026, Month::May, 1)),
    ]
    .into_iter()
    .map(|(id, d)| Event::VoucherPosted(two_line(id, d, 100)))
    .collect();
    store.commit(&events, &meta()).unwrap();
    let april = Period::new(date(2026, Month::April, 1), date(2026, Month::April, 30)).unwrap();
    let books = store.load_books(Some(april)).unwrap();
    // Derived: the period is inclusive at both ends, so ids 2 and 3 are in, 1 and 4 are out.
    let ids: Vec<u64> = books.vouchers.keys().map(|k| k.0).collect();
    assert_eq!(ids, vec![2, 3]);
    assert_eq!(books.postings.len(), 4);
}

#[test]
fn masters_are_created_altered_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store
        .commit(
            &[
                Event::MasterCreated(MasterSnapshot::Group(group(1, "Sundry Debtors"))),
                Event::MasterCreated(MasterSnapshot::Group(group(2, "Temp"))),
            ],
            &meta(),
        )
        .unwrap();
    store
        .commit(
            &[
                Event::MasterAltered {
                    before: MasterSnapshot::Group(group(1, "Sundry Debtors")),
                    after: MasterSnapshot::Group(group(1, "Customers")),
                },
                Event::MasterDeleted(MasterSnapshot::Group(group(2, "Temp"))),
            ],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    assert_eq!(books.masters.groups.len(), 1);
    assert_eq!(
        books.masters.groups.get(&GroupId(1)),
        Some(&group(1, "Customers"))
    );
}

#[test]
fn confirmed_match_sets_bank_date_on_bank_lines_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let mut v = two_line(1, date(2026, Month::April, 5), 1_000);
    v.ledger_lines[0].bank = Some(BankAllocation {
        instrument: InstrumentKind::Neft,
        instrument_number: None,
        instrument_date: None,
        bank_date: None,
        favouring: None,
    });
    let matched = BankMatch {
        statement: BankStatementId(1),
        statement_lines: vec![1],
        vouchers: vec![VoucherId(1)],
        kind: MatchKind::Manual,
        bank_date: date(2026, Month::April, 7),
    };
    store
        .commit(
            &[Event::VoucherPosted(v), Event::MatchConfirmed(matched)],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    let loaded = books.vouchers.get(&VoucherId(1)).unwrap();
    let bank = loaded.ledger_lines[0].bank.as_ref().unwrap();
    assert_eq!(bank.bank_date, Some(date(2026, Month::April, 7)));
    assert!(loaded.ledger_lines[1].bank.is_none());
}

#[test]
fn events_that_do_not_change_books_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store
        .commit(
            &[
                Event::SettingChanged {
                    key: "k".to_owned(),
                    before: "a".to_owned(),
                    after: "b".to_owned(),
                },
                Event::PeriodLocked {
                    until: date(2026, Month::March, 31),
                },
                Event::StatementImported(BankStatementId(1)),
            ],
            &meta(),
        )
        .unwrap();
    let books = store.load_books(None).unwrap();
    assert!(books.vouchers.is_empty());
    assert_eq!(books.postings, [] as [ledgerly_model::Posting; 0]);
}

#[test]
fn duplicate_post_fails_naming_the_event() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let v = two_line(1, date(2026, Month::April, 5), 1_000);
    store
        .commit(
            &[Event::VoucherPosted(v.clone()), Event::VoucherPosted(v)],
            &meta(),
        )
        .unwrap();
    // Derived: the second event (seq 2) creates an id that exists, which a valid log never does.
    assert_encoding_error(&store.load_books(None), "event 2");
}

#[test]
fn alter_of_missing_voucher_and_cancel_twice_fail() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let v = two_line(1, date(2026, Month::April, 5), 1_000);
    store
        .commit(
            &[Event::VoucherAltered {
                before: v.clone(),
                after: v.clone(),
            }],
            &meta(),
        )
        .unwrap();
    assert!(matches!(
        store.load_books(None),
        Err(StoreError::Encoding(_))
    ));

    let dir2 = tempfile::tempdir().unwrap();
    let mut store2 = new_store(dir2.path());
    store2
        .commit(
            &[
                Event::VoucherPosted(v.clone()),
                Event::VoucherCancelled { before: v.clone() },
                Event::VoucherCancelled { before: v },
            ],
            &meta(),
        )
        .unwrap();
    assert!(matches!(
        store2.load_books(None),
        Err(StoreError::Encoding(_))
    ));
}

#[test]
fn master_alter_that_changes_the_id_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store
        .commit(
            &[
                Event::MasterCreated(MasterSnapshot::Group(group(1, "A"))),
                Event::MasterAltered {
                    before: MasterSnapshot::Group(group(1, "A")),
                    after: MasterSnapshot::Group(group(2, "A")),
                },
            ],
            &meta(),
        )
        .unwrap();
    assert!(matches!(
        store.load_books(None),
        Err(StoreError::Encoding(_))
    ));
}

#[test]
fn more_events_than_one_read_page_all_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // 2,005 single-line vouchers: more than two read pages of 1,000 events.
    let events: Vec<Event> = (1..=2_005_u64)
        .map(|id| {
            Event::VoucherPosted(voucher(
                id,
                date(2026, Month::April, 5),
                vec![line(10, dr(100))],
            ))
        })
        .collect();
    store.commit(&events, &meta()).unwrap();
    let books = store.load_books(None).unwrap();
    assert_eq!(books.vouchers.len(), 2_005);
    assert_eq!(books.postings.len(), 2_005);
    // Same date, so voucher id order.
    assert_eq!(books.postings[0].voucher, VoucherId(1));
    assert_eq!(books.postings[2_004].voucher, VoucherId(2_005));
}

// Error paths. Derived from the `Event`, `SignedAmount` and `BankMatch` doc comments: an event
// whose ids do not agree with the state built by the events before it cannot occur in a valid
// log, so the load must fail naming the event (seq numbers are 1-based, in commit order) rather
// than skip it.

#[test]
fn negative_ledger_line_amount_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // `SignedAmount::amount` is "never negative for a well-formed value".
    let v = voucher(
        1,
        date(2026, Month::April, 5),
        vec![line(10, dr(-100)), line(20, cr(100))],
    );
    store.commit(&[Event::VoucherPosted(v)], &meta()).unwrap();
    assert_encoding_error(&store.load_books(None), "negative amount");
}

#[test]
fn match_confirmed_for_missing_voucher_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let matched = BankMatch {
        statement: BankStatementId(1),
        statement_lines: vec![1],
        vouchers: vec![VoucherId(9)],
        kind: MatchKind::Manual,
        bank_date: date(2026, Month::April, 7),
    };
    store
        .commit(&[Event::MatchConfirmed(matched)], &meta())
        .unwrap();
    assert_encoding_error(&store.load_books(None), "event 1");
}

#[test]
fn match_confirmed_for_voucher_without_bank_line_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // `two_line` has no bank allocation on either line, so there is no bank date to set.
    let v = two_line(1, date(2026, Month::April, 5), 1_000);
    let matched = BankMatch {
        statement: BankStatementId(1),
        statement_lines: vec![1],
        vouchers: vec![VoucherId(1)],
        kind: MatchKind::Manual,
        bank_date: date(2026, Month::April, 7),
    };
    store
        .commit(
            &[Event::VoucherPosted(v), Event::MatchConfirmed(matched)],
            &meta(),
        )
        .unwrap();
    assert_encoding_error(&store.load_books(None), "event 2");
}

#[test]
fn delete_of_missing_voucher_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let v = two_line(1, date(2026, Month::April, 5), 1_000);
    store
        .commit(&[Event::VoucherDeleted { before: v }], &meta())
        .unwrap();
    assert_encoding_error(&store.load_books(None), "event 1");
}

#[test]
fn delete_of_missing_master_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    store
        .commit(
            &[Event::MasterDeleted(MasterSnapshot::Group(group(1, "A")))],
            &meta(),
        )
        .unwrap();
    assert_encoding_error(&store.load_books(None), "event 1");
}
