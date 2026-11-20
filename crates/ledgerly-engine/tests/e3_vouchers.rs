//! Card E3 tests: voucher post, alter, cancel and delete (spec section 5, invariants 1, 2, 7, 8).
//!
//! Every expected value carries a provenance comment. All are `derived` by hand from the spec
//! text (section 5 invariants) and from the numbering configuration of the fixture types; no
//! expected value is an output of the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "tests may unwrap and index (engineering rule B2)"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_engine::vouchers::{
    alter_voucher, alter_voucher_with_lock, cancel_voucher, cancel_voucher_with_lock,
    cancelled_form, delete_voucher, delete_voucher_with_lock, post_voucher, post_voucher_with_lock,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Books, Company,
    EntityKind, Features, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, RestartPeriod, Voucher, VoucherError, VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_store::Event;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);
const DEBTOR: LedgerId = LedgerId(10);
const SALES: LedgerId = LedgerId(20);

const T_PAYMENT: VoucherTypeId = VoucherTypeId(1);
const T_SALES: VoucherTypeId = VoucherTypeId(2);
const T_JOURNAL: VoucherTypeId = VoucherTypeId(3);
const T_RECEIPT: VoucherTypeId = VoucherTypeId(4);
const T_RETIRED: VoucherTypeId = VoucherTypeId(5);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn ledger(id: LedgerId, name: &str, bill_wise: bool) -> Ledger {
    Ledger {
        id,
        name: name.to_string(),
        aliases: Vec::new(),
        group: ledgerly_core::GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: bill_wise.then_some(BillWiseConfig {
            default_credit_days: None,
        }),
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn vtype(
    id: VoucherTypeId,
    base: BaseVoucherKind,
    method: NumberingMethod,
    prefix: &str,
    width: u8,
    active: bool,
) -> VoucherType {
    VoucherType {
        id,
        name: format!("{base:?}"),
        base,
        abbreviation: String::new(),
        numbering: NumberingConfig {
            method,
            prevent_renumbering: false,
            prefix: prefix.to_string(),
            suffix: String::new(),
            width,
            restart: RestartPeriod::Never,
            series: Vec::new(),
        },
        active,
        tally: None,
    }
}

fn empty_books(gst: bool) -> Books {
    let mut masters = Masters::default();
    for l in [
        ledger(CASH, "Cash", false),
        ledger(RENT, "Rent", false),
        ledger(DEBTOR, "Debtor A", true),
        ledger(SALES, "Sales", false),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        // Payment: automatic, prefix "PAY-", at least 3 digits, never restarts.
        vtype(
            T_PAYMENT,
            BaseVoucherKind::Payment,
            NumberingMethod::Automatic,
            "PAY-",
            3,
            true,
        ),
        // Sales: manual numbers (the user types the invoice number).
        vtype(
            T_SALES,
            BaseVoucherKind::Sales,
            NumberingMethod::Manual,
            "",
            0,
            true,
        ),
        vtype(
            T_JOURNAL,
            BaseVoucherKind::Journal,
            NumberingMethod::None,
            "",
            0,
            true,
        ),
        vtype(
            T_RECEIPT,
            BaseVoucherKind::Receipt,
            NumberingMethod::None,
            "",
            0,
            true,
        ),
        vtype(
            T_RETIRED,
            BaseVoucherKind::Journal,
            NumberingMethod::None,
            "",
            0,
            false,
        ),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Test Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features {
                bill_wise: true,
                gst,
                ..Features::default()
            },
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn with(mut books: Books, vs: Vec<Voucher>) -> Books {
    for v in vs {
        books.vouchers.insert(v.id, v);
    }
    books
}

fn line(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    LedgerLine {
        ledger,
        amount,
        bills,
        cost: Vec::new(),
        bank: None,
    }
}

fn alloc(kind: BillRefKind, name: &str, amount: SignedAmount) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.to_string(),
        due_date: None,
        credit_days: None,
        amount,
    }
}

fn voucher(
    id: u64,
    type_id: VoucherTypeId,
    date: Date,
    number: Option<&str>,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: number.map(str::to_string),
        series: None,
        date,
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

/// Payment: Dr rent, Cr cash, Rs `a`.
fn payment(id: u64, date: Date, number: Option<&str>, a: i64) -> Voucher {
    voucher(
        id,
        T_PAYMENT,
        date,
        number,
        vec![
            line(RENT, SignedAmount::dr(rs(a)), Vec::new()),
            line(CASH, SignedAmount::cr(rs(a)), Vec::new()),
        ],
    )
}

/// Sales invoice: Dr debtor (new bill named like the number), Cr sales.
fn invoice(id: u64, date: Date, number: &str, a: i64) -> Voucher {
    let mut v = voucher(
        id,
        T_SALES,
        date,
        Some(number),
        vec![
            line(
                DEBTOR,
                SignedAmount::dr(rs(a)),
                vec![alloc(BillRefKind::New, number, SignedAmount::dr(rs(a)))],
            ),
            line(SALES, SignedAmount::cr(rs(a)), Vec::new()),
        ],
    );
    v.party = Some(DEBTOR);
    v
}

/// Journal that opens a bill: Dr debtor (new bill `name`), Cr sales.
fn bill_journal(id: u64, date: Date, name: &str, a: i64) -> Voucher {
    voucher(
        id,
        T_JOURNAL,
        date,
        None,
        vec![
            line(
                DEBTOR,
                SignedAmount::dr(rs(a)),
                vec![alloc(BillRefKind::New, name, SignedAmount::dr(rs(a)))],
            ),
            line(SALES, SignedAmount::cr(rs(a)), Vec::new()),
        ],
    )
}

/// Receipt that settles bill `name`: Cr debtor (against), Dr cash.
fn receipt_against(id: u64, date: Date, name: &str, a: i64) -> Voucher {
    voucher(
        id,
        T_RECEIPT,
        date,
        None,
        vec![
            line(
                DEBTOR,
                SignedAmount::cr(rs(a)),
                vec![alloc(BillRefKind::Against, name, SignedAmount::cr(rs(a)))],
            ),
            line(CASH, SignedAmount::dr(rs(a)), Vec::new()),
        ],
    )
}

fn events(o: Outcome<Vec<Event>>) -> Vec<Event> {
    match o {
        Outcome::Value(e) => e,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

// ---------------------------------------------------------------- post

#[test]
fn valid_payment_posts_one_voucher_posted_with_allocated_number() {
    // provenance: derived — plan card E3 "valid payment -> one VoucherPosted". The Payment type is
    // automatic with prefix "PAY-" and width 3 and the books hold no payment yet, so the first
    // counter is 1, formatted as zero-padded to 3 digits: "PAY-001".
    let books = empty_books(false);
    let v = payment(1, date!(2026 - 05 - 10), None, 500);
    let mut expected = v.clone();
    expected.number = Some("PAY-001".to_string());
    let out = events(post_voucher(v, &books).unwrap());
    assert_eq!(out, vec![Event::VoucherPosted(expected)]);
}

#[test]
fn second_automatic_payment_gets_next_counter() {
    // provenance: derived — one stored "PAY-001" so the next counter is 2: "PAY-002".
    let books = with(
        empty_books(false),
        vec![payment(1, date!(2026 - 05 - 10), Some("PAY-001"), 500)],
    );
    let out = events(post_voucher(payment(2, date!(2026 - 05 - 11), None, 100), &books).unwrap());
    let Event::VoucherPosted(p) = &out[0] else {
        panic!("expected VoucherPosted");
    };
    assert_eq!(p.number.as_deref(), Some("PAY-002"));
}

#[test]
fn unbalanced_voucher_errors_with_totals() {
    // provenance: derived — invariant 1 (spec section 5): debits must equal credits.
    // Dr Rs 500 = 50,000 paise; Cr Rs 400 = 40,000 paise.
    let mut v = payment(1, date!(2026 - 05 - 10), None, 500);
    v.ledger_lines[1].amount = SignedAmount::cr(rs(400));
    let e = post_voucher(v, &empty_books(false)).unwrap_err();
    assert_eq!(
        e,
        EngineError::Voucher(VoucherError::Unbalanced {
            dr: Paise::new(50_000),
            cr: Paise::new(40_000),
        })
    );
}

#[test]
fn posting_an_existing_id_errors_duplicate() {
    // provenance: derived — a voucher id names one voucher; a second post of the same id would
    // overwrite history (spec section 5: events are append-only).
    let books = with(
        empty_books(false),
        vec![payment(1, date!(2026 - 05 - 10), Some("PAY-001"), 500)],
    );
    let e = post_voucher(payment(1, date!(2026 - 05 - 11), None, 100), &books).unwrap_err();
    assert!(matches!(e, EngineError::Duplicate { .. }), "{e:?}");
}

#[test]
fn manual_type_without_number_errors() {
    // provenance: derived — a manual type's number is "typed by the user" (NumberingMethod docs);
    // none given means nothing to store.
    let mut v = invoice(1, date!(2026 - 05 - 10), "INV-1", 1000);
    v.number = None;
    let e = post_voucher(v, &empty_books(false)).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn duplicate_manual_number_errors() {
    // provenance: derived — invariant 7: numbers are unique within type and series. Restart is
    // Never so the whole history is one period.
    let books = with(
        empty_books(false),
        vec![invoice(1, date!(2026 - 05 - 10), "INV-1", 1000)],
    );
    let e = post_voucher(invoice(2, date!(2026 - 06 - 10), "INV-1", 700), &books).unwrap_err();
    assert!(matches!(e, EngineError::Duplicate { .. }), "{e:?}");
}

#[test]
fn gst_sales_number_of_17_characters_errors_number_too_long() {
    // provenance: derived — CGST Rules 2017 rule 46(b): serial number of at most 16 characters.
    // "INV-2026-0000001" is 16 characters, "INV-2026-00000012" is 17.
    let books = empty_books(true);
    let ok = invoice(1, date!(2026 - 05 - 10), "INV-2026-0000001", 1000);
    assert!(post_voucher(ok, &books).is_ok());
    let long = invoice(2, date!(2026 - 05 - 10), "INV-2026-00000012", 1000);
    let e = post_voucher(long, &books).unwrap_err();
    assert_eq!(
        e,
        EngineError::NumberTooLong {
            number: "INV-2026-00000012".to_string(),
            max: 16,
        }
    );
}

#[test]
fn inactive_voucher_type_errors() {
    // provenance: derived — VoucherType::active docs: "Whether vouchers of this type can be
    // entered"; false means entry is refused.
    let v = voucher(
        1,
        T_RETIRED,
        date!(2026 - 05 - 10),
        None,
        vec![
            line(RENT, SignedAmount::dr(rs(10)), Vec::new()),
            line(CASH, SignedAmount::cr(rs(10)), Vec::new()),
        ],
    );
    let e = post_voucher(v, &empty_books(false)).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn posting_in_locked_period_errors_period_locked() {
    // provenance: derived — invariant 8: a date on or before the lock date is blocked, the day
    // after is not. Lock 30 June 2026.
    let books = empty_books(false);
    let lock = Some(date!(2026 - 06 - 30));
    let e = post_voucher_with_lock(payment(1, date!(2026 - 06 - 30), None, 5), &books, lock)
        .unwrap_err();
    assert_eq!(
        e,
        EngineError::PeriodLocked {
            until: date!(2026 - 06 - 30)
        }
    );
    assert!(
        post_voucher_with_lock(payment(1, date!(2026 - 07 - 01), None, 5), &books, lock).is_ok()
    );
}

#[test]
fn bill_settlement_over_open_amount_errors_over_settled() {
    // provenance: derived — invariant 2: Against references must not over-settle. Open Rs 1,000
    // (100,000 paise); a receipt of Rs 1,200 (120,000 paise) against it exceeds it.
    let books = with(
        empty_books(false),
        vec![bill_journal(1, date!(2026 - 05 - 01), "B1", 1000)],
    );
    let e = post_voucher(
        receipt_against(2, date!(2026 - 05 - 02), "B1", 1200),
        &books,
    )
    .unwrap_err();
    assert!(matches!(e, EngineError::OverSettled { .. }), "{e:?}");
}

// ---------------------------------------------------------------- alter

#[test]
fn altering_voucher_inside_locked_period_errors_period_locked() {
    // provenance: derived — invariant 8 and plan card E3. The stored voucher is dated 15 June,
    // the lock is 30 June, so the alteration is blocked and the error carries the lock date.
    let stored = payment(1, date!(2026 - 06 - 15), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut changed = stored;
    changed.narration = "corrected".to_string();
    let e = alter_voucher_with_lock(changed, &books, Some(date!(2026 - 06 - 30))).unwrap_err();
    assert_eq!(
        e,
        EngineError::PeriodLocked {
            until: date!(2026 - 06 - 30)
        }
    );
}

#[test]
fn moving_an_open_voucher_into_a_locked_date_errors() {
    // provenance: derived — invariant 8 applies to the new date as well as the stored one: a
    // voucher stored on 10 July cannot be redated to 10 June under a 30 June lock.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut moved = stored;
    moved.date = date!(2026 - 06 - 10);
    let e = alter_voucher_with_lock(moved, &books, Some(date!(2026 - 06 - 30))).unwrap_err();
    assert_eq!(
        e,
        EngineError::PeriodLocked {
            until: date!(2026 - 06 - 30)
        }
    );
}

#[test]
fn altering_open_voucher_returns_before_and_after() {
    // provenance: derived — the frozen alter_voucher checks no lock; the event carries the stored
    // voucher as `before` and the submitted one as `after` (store Event::VoucherAltered docs).
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut changed = stored.clone();
    changed.narration = "March rent".to_string();
    let out = events(alter_voucher(changed.clone(), &books).unwrap());
    assert_eq!(
        out,
        vec![Event::VoucherAltered {
            before: stored,
            after: changed
        }]
    );
}

#[test]
fn altering_with_no_change_returns_no_events() {
    // provenance: derived — an alteration identical to the stored voucher changes nothing, so
    // there is nothing to record.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    assert!(events(alter_voucher(stored, &books).unwrap()).is_empty());
}

#[test]
fn altering_unknown_voucher_errors_not_found() {
    // provenance: derived — nothing is stored under id 9.
    let e = alter_voucher(
        payment(9, date!(2026 - 07 - 10), None, 5),
        &empty_books(false),
    )
    .unwrap_err();
    assert!(matches!(e, EngineError::NotFound { .. }), "{e:?}");
}

#[test]
fn altering_a_cancelled_voucher_errors() {
    // provenance: derived — spec section 5: a cancelled voucher keeps its number and has no
    // amounts; amounts cannot be put back by an alteration.
    let stored = cancelled_form(&payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500));
    let books = with(empty_books(false), vec![stored.clone()]);
    let e = alter_voucher(
        payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500),
        &books,
    )
    .unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn altering_the_type_errors() {
    // provenance: derived — a voucher's number belongs to its type's counter, so the type of a
    // stored voucher cannot change.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut changed = stored;
    changed.type_id = T_JOURNAL;
    changed.number = None;
    let e = alter_voucher(changed, &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn renumbering_a_sales_invoice_errors() {
    // provenance: derived — invariant 7: sales-side numbers never change after save.
    let stored = invoice(1, date!(2026 - 07 - 10), "INV-1", 1000);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut changed = stored;
    changed.number = Some("INV-9".to_string());
    let e = alter_voucher(changed, &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn renumbering_a_payment_to_a_free_number_is_allowed_but_not_to_a_used_one() {
    // provenance: derived — invariant 7 forbids duplicates, and only sales-side or
    // prevent_renumbering types are fixed; Payment here is neither.
    let a = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let b = payment(2, date!(2026 - 07 - 11), Some("PAY-002"), 700);
    let books = with(empty_books(false), vec![a.clone(), b]);
    let mut free = a.clone();
    free.number = Some("PAY-009".to_string());
    assert!(alter_voucher(free, &books).is_ok());
    let mut used = a;
    used.number = Some("PAY-002".to_string());
    let e = alter_voucher(used, &books).unwrap_err();
    assert!(matches!(e, EngineError::Duplicate { .. }), "{e:?}");
}

#[test]
fn altering_status_to_cancelled_errors() {
    // provenance: derived — cancelling is its own command (cancel_voucher); an alteration keeps
    // the voucher live.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let mut changed = stored;
    changed.status = VoucherStatus::Cancelled;
    let e = alter_voucher(changed, &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn shrinking_a_bill_below_its_settlements_errors() {
    // provenance: derived — invariant 2: open amount = bill - settlements. Journal opens Rs 1,000,
    // a receipt settles Rs 400. Altering the journal to Rs 300 leaves 300 - 400 < 0: over-settled.
    let j = bill_journal(1, date!(2026 - 05 - 01), "B1", 1000);
    let r = receipt_against(2, date!(2026 - 05 - 02), "B1", 400);
    let books = with(empty_books(false), vec![j, r]);
    let e = alter_voucher(bill_journal(1, date!(2026 - 05 - 01), "B1", 300), &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

// ---------------------------------------------------------------- cancel

#[test]
fn cancel_keeps_the_number_and_drops_the_amounts() {
    // provenance: derived — spec section 5: cancelled vouchers keep their number and carry no
    // amounts. The event holds the stored voucher; its cancelled form has status Cancelled,
    // number "PAY-001" and no lines.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let out = events(cancel_voucher(VoucherId(1), &books).unwrap());
    assert_eq!(
        out,
        vec![Event::VoucherCancelled {
            before: stored.clone()
        }]
    );
    let after = cancelled_form(&stored);
    assert_eq!(after.status, VoucherStatus::Cancelled);
    assert_eq!(after.number.as_deref(), Some("PAY-001"));
    assert!(after.ledger_lines.is_empty());
    assert!(after.inventory_lines.is_empty());
    assert!(after.gst.is_none());
}

#[test]
fn cancelled_number_is_not_reused_by_the_next_automatic_voucher() {
    // provenance: derived — invariant 7: every stored voucher keeps its number, cancelled ones
    // included, so after a cancelled "PAY-001" the next counter is 2.
    let cancelled = cancelled_form(&payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500));
    let books = with(empty_books(false), vec![cancelled]);
    let out = events(post_voucher(payment(2, date!(2026 - 07 - 11), None, 5), &books).unwrap());
    let Event::VoucherPosted(p) = &out[0] else {
        panic!("expected VoucherPosted");
    };
    assert_eq!(p.number.as_deref(), Some("PAY-002"));
}

#[test]
fn cancelling_twice_errors() {
    // provenance: derived — a cancelled voucher is already in the cancelled state.
    let cancelled = cancelled_form(&payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500));
    let books = with(empty_books(false), vec![cancelled]);
    let e = cancel_voucher(VoucherId(1), &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn cancelling_unknown_voucher_errors_not_found() {
    // provenance: derived — nothing is stored under id 9.
    let e = cancel_voucher(VoucherId(9), &empty_books(false)).unwrap_err();
    assert!(matches!(e, EngineError::NotFound { .. }), "{e:?}");
}

#[test]
fn cancelling_inside_locked_period_errors() {
    // provenance: derived — invariant 8: cancel is a change, blocked on or before the lock date.
    let stored = payment(1, date!(2026 - 06 - 30), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored]);
    let e =
        cancel_voucher_with_lock(VoucherId(1), &books, Some(date!(2026 - 06 - 30))).unwrap_err();
    assert_eq!(
        e,
        EngineError::PeriodLocked {
            until: date!(2026 - 06 - 30)
        }
    );
}

#[test]
fn cancelling_a_sales_invoice_is_allowed() {
    // provenance: derived — invariant 7: sales vouchers are never deleted, "cancel instead".
    let stored = invoice(1, date!(2026 - 07 - 10), "INV-1", 1000);
    let books = with(empty_books(false), vec![stored.clone()]);
    let out = events(cancel_voucher(VoucherId(1), &books).unwrap());
    assert_eq!(out, vec![Event::VoucherCancelled { before: stored }]);
}

#[test]
fn cancelling_a_bill_that_is_settled_errors() {
    // provenance: derived — invariant 2: a receipt settling Rs 400 of bill B1 needs B1 to exist;
    // cancelling the journal that opens it would leave the receipt pointing at nothing.
    let j = bill_journal(1, date!(2026 - 05 - 01), "B1", 1000);
    let r = receipt_against(2, date!(2026 - 05 - 02), "B1", 400);
    let books = with(empty_books(false), vec![j, r]);
    let e = cancel_voucher(VoucherId(1), &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

// ---------------------------------------------------------------- delete

#[test]
fn deleting_a_sales_voucher_errors_sales_series_no_delete() {
    // provenance: derived — invariant 7 and plan card E3: sales vouchers are never deleted;
    // cancel instead.
    let books = with(
        empty_books(false),
        vec![invoice(1, date!(2026 - 07 - 10), "INV-1", 1000)],
    );
    let e = delete_voucher(VoucherId(1), &books).unwrap_err();
    assert_eq!(e, EngineError::SalesSeriesNoDelete);
}

#[test]
fn deleting_a_cancelled_sales_voucher_still_errors() {
    // provenance: derived — a cancelled voucher keeps its number, and deleting it would free the
    // number (invariant 7).
    let cancelled = cancelled_form(&invoice(1, date!(2026 - 07 - 10), "INV-1", 1000));
    let books = with(empty_books(false), vec![cancelled]);
    let e = delete_voucher(VoucherId(1), &books).unwrap_err();
    assert_eq!(e, EngineError::SalesSeriesNoDelete);
}

#[test]
fn deleting_a_payment_returns_voucher_deleted() {
    // provenance: derived — Payment is not sales-side and does not prevent renumbering, so it may
    // be deleted; the event holds the stored voucher.
    let stored = payment(1, date!(2026 - 07 - 10), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored.clone()]);
    let out = events(delete_voucher(VoucherId(1), &books).unwrap());
    assert_eq!(out, vec![Event::VoucherDeleted { before: stored }]);
}

#[test]
fn deleting_inside_locked_period_errors_period_locked() {
    // provenance: derived — invariant 8; checked before the sales-series rule so the user learns
    // the period is locked.
    let stored = payment(1, date!(2026 - 06 - 15), Some("PAY-001"), 500);
    let books = with(empty_books(false), vec![stored]);
    let e =
        delete_voucher_with_lock(VoucherId(1), &books, Some(date!(2026 - 06 - 30))).unwrap_err();
    assert_eq!(
        e,
        EngineError::PeriodLocked {
            until: date!(2026 - 06 - 30)
        }
    );
}

#[test]
fn deleting_unknown_voucher_errors_not_found() {
    // provenance: derived — nothing is stored under id 9.
    let e = delete_voucher(VoucherId(9), &empty_books(false)).unwrap_err();
    assert!(matches!(e, EngineError::NotFound { .. }), "{e:?}");
}

#[test]
fn deleting_a_bill_that_is_settled_errors() {
    // provenance: derived — invariant 2, as for cancel: the receipt needs bill B1 to exist.
    let j = bill_journal(1, date!(2026 - 05 - 01), "B1", 1000);
    let r = receipt_against(2, date!(2026 - 05 - 02), "B1", 400);
    let books = with(empty_books(false), vec![j, r]);
    let e = delete_voucher(VoucherId(1), &books).unwrap_err();
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
}

#[test]
fn deleting_the_settling_receipt_is_allowed() {
    // provenance: derived — removing a settlement only reopens the bill; nothing depends on a
    // receipt.
    let j = bill_journal(1, date!(2026 - 05 - 01), "B1", 1000);
    let r = receipt_against(2, date!(2026 - 05 - 02), "B1", 400);
    let books = with(empty_books(false), vec![j, r.clone()]);
    let out = events(delete_voucher(VoucherId(2), &books).unwrap());
    assert_eq!(out, vec![Event::VoucherDeleted { before: r }]);
}
