//! Review queue decisions: the duplicate row (rule D-02) and what accepting or rejecting a row
//! does (mock `SCREENS.review`: Y or Ctrl+A accepts, N or Alt+D rejects).
//!
//! Provenance. Rule D-02 pairs are written by hand below; the wording is ours. Every expected
//! string and figure is derived from those literals: PY/1 on 28 Oct 2026 and PY/2 on 12 Nov 2026
//! are 3 + 12 = 15 days apart, and Rs 14,860 is `1_486_000` paise.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{DuplicatePayment, PaymentRef};
use ledgerly_app::screens::review::{
    RowOrigin, Verdict, accept_verdict, duplicate_row, reject_notice,
};
use ledgerly_app::ui::conf::Confidence;
use ledgerly_core::{Paise, VoucherId};
use time::macros::date;

fn pay(id: u64, number: Option<&str>, on: time::Date) -> PaymentRef {
    PaymentRef {
        id: VoucherId(id),
        number: number.map(str::to_owned),
        date: on,
        locked: false,
        bank_date: None,
    }
}

fn pair(days: i64) -> DuplicatePayment {
    DuplicatePayment {
        first: pay(1, Some("PY/1"), date!(2026 - 10 - 28)),
        second: pay(2, Some("PY/2"), date!(2026 - 11 - 12)),
        ledger: "Electricity".to_owned(),
        payee: None,
        amount: Paise::new(1_486_000),
        narration: "City Power, October bill".to_owned(),
        days_apart: days,
    }
}

#[test]
fn a_duplicate_row_names_both_payments_and_the_rule() {
    // provenance: derived. The row is built from the pair above; the words are ours.
    let row = duplicate_row(&pair(15));
    assert_eq!(row.kind, "Possible duplicate");
    assert_eq!(
        row.what,
        "PY/2 on 12 Nov 2026 repeats PY/1 on 28 Oct 2026: same ledger (Electricity), amount and narration."
    );
    assert_eq!(row.source, "Rule D-02 \u{b7} 15 days apart");
    assert_eq!(row.amount, Some(Paise::new(1_486_000)));
    assert_eq!(row.confidence, Confidence::Rule);
    assert_eq!(row.ledger, None);
    assert_eq!(row.origin, RowOrigin::Duplicate);
    assert_eq!(row.key, "duplicate:1:2");
}

#[test]
fn one_day_apart_is_singular_and_a_voucher_with_no_number_is_named_by_its_id() {
    // provenance: derived. 1 day, not "1 days"; a voucher without a number reads "voucher 2".
    let mut p = pair(1);
    p.second = pay(2, None, date!(2026 - 10 - 29));
    let row = duplicate_row(&p);
    assert_eq!(row.source, "Rule D-02 \u{b7} 1 day apart");
    assert!(
        row.what
            .starts_with("Voucher 2 on 29 Oct 2026 repeats PY/1"),
        "{}",
        row.what
    );
}

#[test]
fn accepting_a_duplicate_sends_nothing_and_says_what_to_do() {
    // provenance: derived. Rule D-02 flags a pair; there is no proposal to post, so Y only
    // explains the two ways out (cancel the second voucher, or say both are right with N).
    let row = duplicate_row(&pair(15));
    assert_eq!(
        accept_verdict(&row),
        Verdict::Say(
            "Nothing to accept: this is a warning, not a draft. Open it (Enter) to compare the two payments in the day book and cancel the second one, or press N if both are right."
                .to_owned()
        )
    );
}

#[test]
fn rejecting_says_nothing_was_posted_and_that_the_engine_keeps_no_record() {
    // provenance: derived. The engine stores no rejection yet, so the notice must not claim
    // the proposal is gone for good (mock: "Rejected. Nothing was posted.").
    let row = duplicate_row(&pair(15));
    assert_eq!(
        reject_notice(&row),
        "Rejected for this session. Nothing was posted or changed. Ledgerly keeps no record of a rejection yet, so it returns the next time the queue is opened."
    );
}
