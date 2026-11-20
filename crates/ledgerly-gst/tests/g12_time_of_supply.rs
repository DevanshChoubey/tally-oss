//! Card G12 tests: time of supply (CGST Act 2017 s.12, s.13, s.14; CGST Rules 2017 rule 47).
//!
//! Every expected value is `derived`: the clause and the hand reasoning sit next to it. The clause
//! wording below is a close paraphrase of the Act written from the section structure, not a
//! verbatim copy; the owner should check it against the gazetted text before the case is moved to
//! the conformance pass-list.
//!
//! Rate change used throughout: 22 September 2025 (GST 2.0 slabs, spec 6.3). "Before" dates are
//! 10 September 2025, "after" dates are 30 September 2025; the change date itself counts as after.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Date, Month, Outcome};
use ledgerly_gst::time_of_supply::{
    GoodsOrServices, RateSide, SupplyDates, rate_change_rule, time_of_supply,
};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn change() -> Date {
    d(2025, Month::September, 22)
}

fn before() -> Date {
    d(2025, Month::September, 10)
}

fn after() -> Date {
    d(2025, Month::September, 30)
}

fn dates(
    supply: Option<Date>,
    invoice: Option<Date>,
    payment: Option<Date>,
    kind: GoodsOrServices,
) -> SupplyDates {
    SupplyDates {
        invoice,
        supply,
        payment,
        kind,
    }
}

fn side(supply: Option<Date>, invoice: Option<Date>, payment: Option<Date>) -> Outcome<RateSide> {
    rate_change_rule(
        &dates(supply, invoice, payment, GoodsOrServices::Services),
        change(),
    )
}

// ---- s.14(1)(a): supplied before the change in rate ----

#[test]
fn s14_a_i_invoice_and_payment_both_after_change() {
    // s.14(1)(a)(i): supply before the change, invoice issued and payment received after the
    // change: time of supply is the earlier of payment and invoice, both after -> new rate.
    let got = side(Some(before()), Some(after()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_a_ii_invoice_before_payment_after() {
    // s.14(1)(a)(ii): supply before the change, invoice issued before it, payment after it:
    // time of supply is the invoice date, before the change -> old rate.
    let got = side(Some(before()), Some(before()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_a_iii_payment_before_invoice_after() {
    // s.14(1)(a)(iii): supply before the change, payment received before it, invoice issued
    // after it: time of supply is the date of payment, before the change -> old rate.
    let got = side(Some(before()), Some(after()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_a_everything_before_change() {
    // Supply, invoice and payment all before the change: s.14 has nothing to move; s.12/s.13
    // give a date before the change -> old rate.
    let got = side(Some(before()), Some(before()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

// ---- s.14(1)(b): supplied after the change in rate ----

#[test]
fn s14_b_i_invoice_before_payment_after() {
    // s.14(1)(b)(i): supply after the change, invoice issued before it, payment received after
    // it: time of supply is the date of payment, after the change -> new rate.
    let got = side(Some(after()), Some(before()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_b_ii_invoice_and_payment_both_before_change() {
    // s.14(1)(b)(ii): supply after the change but invoice issued and payment received before it:
    // time of supply is a date before the change -> old rate.
    let got = side(Some(after()), Some(before()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_b_iii_invoice_after_payment_before() {
    // s.14(1)(b)(iii): supply after the change, payment received before it, invoice issued after
    // it: time of supply is the invoice date, after the change -> new rate.
    let got = side(Some(after()), Some(after()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_b_everything_after_change() {
    // Supply, invoice and payment all after the change: date after the change -> new rate.
    let got = side(Some(after()), Some(after()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

// ---- boundary: the change date itself ----

#[test]
fn change_date_counts_as_after() {
    // A rate that changes "with effect from 22 Sep" applies to a time of supply on 22 Sep, so a
    // supply on 22 Sep is "after the change": s.14(1)(b), both before -> old, any one on 22 Sep
    // -> new.
    let day_before = d(2025, Month::September, 21);
    assert_eq!(
        side(Some(change()), Some(day_before), Some(day_before)),
        Outcome::Value(RateSide::Before)
    );
    assert_eq!(
        side(Some(change()), Some(change()), Some(day_before)),
        Outcome::Value(RateSide::After)
    );
}

#[test]
fn day_before_change_is_before() {
    // Supply on 21 Sep, invoice on 21 Sep, payment on 22 Sep: s.14(1)(a)(ii) -> invoice before the
    // change -> old rate.
    let day_before = d(2025, Month::September, 21);
    assert_eq!(
        side(Some(day_before), Some(day_before), Some(change())),
        Outcome::Value(RateSide::Before)
    );
}

// ---- missing dates ----

#[test]
fn missing_supply_date_is_unsupported() {
    // s.14 first sorts supply into (a) before or (b) after the change; without the supply date no
    // clause can be chosen.
    assert!(side(None, Some(before()), Some(before())).is_unsupported());
}

#[test]
fn supply_before_with_unknown_other_date_is_unsupported() {
    // Supply before the change, invoice after the change, payment not received: whether the
    // payment comes before the change is unknown, and (a)(i) vs (a)(iii) differ -> undecidable.
    assert!(side(Some(before()), Some(after()), None).is_unsupported());
    // Supply before, invoice not issued, payment after: (a)(i) vs (a)(ii) differ -> undecidable.
    assert!(side(Some(before()), None, Some(after())).is_unsupported());
    // Nothing known beyond the supply date.
    assert!(side(Some(before()), None, None).is_unsupported());
}

#[test]
fn supply_before_with_decisive_known_date_is_decided() {
    // Supply before the change and the invoice already issued before it: (a)(ii) or the all-before
    // case give a date before the change whatever the payment turns out to be -> old rate.
    assert_eq!(
        side(Some(before()), Some(before()), None),
        Outcome::Value(RateSide::Before)
    );
    // Supply before and payment received before: (a)(iii) or the all-before case -> old rate.
    assert_eq!(
        side(Some(before()), None, Some(before())),
        Outcome::Value(RateSide::Before)
    );
}

#[test]
fn supply_after_with_unknown_other_date_is_unsupported() {
    // Supply after the change, invoice not issued, payment before it: (b)(ii) vs (b)(iii) differ.
    assert!(side(Some(after()), None, Some(before())).is_unsupported());
    // Supply after, invoice before, payment not received: (b)(i) vs (b)(ii) differ.
    assert!(side(Some(after()), Some(before()), None).is_unsupported());
    assert!(side(Some(after()), None, None).is_unsupported());
}

#[test]
fn supply_after_with_decisive_known_date_is_decided() {
    // Supply after the change and the payment already received after it: (b)(i) or the all-after
    // case give a date after the change whatever the invoice -> new rate.
    assert_eq!(
        side(Some(after()), None, Some(after())),
        Outcome::Value(RateSide::After)
    );
    // Supply after and the invoice issued after: (b)(iii) or all-after -> new rate.
    assert_eq!(
        side(Some(after()), Some(after()), None),
        Outcome::Value(RateSide::After)
    );
}

// ---- s.12(2): goods ----

fn goods(supply: Option<Date>, invoice: Option<Date>, payment: Option<Date>) -> Outcome<Date> {
    time_of_supply(&dates(supply, invoice, payment, GoodsOrServices::Goods))
}

#[test]
fn goods_invoice_issued_before_removal_gives_invoice_date() {
    // s.12(2)(a): the earlier of the invoice date and the last date it had to be issued. s.31(1)(a)
    // requires the invoice on or before removal, so the last date is the removal date, 1 Oct 2025.
    // Invoice on 28 Sep is earlier -> 28 Sep.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::September, 28)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 28)));
}

#[test]
fn goods_late_invoice_gives_removal_date() {
    // Invoice issued 5 Oct, after removal on 1 Oct: the last date it had to be issued (s.31(1)(a))
    // is 1 Oct, which is earlier -> 1 Oct.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 5)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn goods_payment_after_invoice_does_not_move_the_date() {
    // s.12(2): earlier of invoice (1 Oct) and payment (9 Oct) -> 1 Oct.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 9)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn goods_advance_before_invoice_is_unsupported() {
    // s.12(2)(b) would give the payment date, N.66/2017-CT gives the invoice date for registered
    // persons other than composition dealers; the registration type is not in the input.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::September, 15)),
    );
    assert!(got.is_unsupported());
}

#[test]
fn goods_missing_dates_are_unsupported() {
    let oct1 = d(2025, Month::October, 1);
    assert!(goods(Some(oct1), None, None).is_unsupported());
    assert!(goods(None, Some(oct1), None).is_unsupported());
}

// ---- s.13(2): services ----

fn services(supply: Option<Date>, invoice: Option<Date>, payment: Option<Date>) -> Outcome<Date> {
    time_of_supply(&dates(supply, invoice, payment, GoodsOrServices::Services))
}

#[test]
fn services_invoice_within_30_days_gives_invoice_date() {
    // s.13(2)(a): invoice issued within the period of rule 47 (30 days from the supply): the
    // earlier of invoice and payment. Supply 1 Sep, invoice 11 Sep (10 days), no payment -> 11 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::September, 11)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 11)));
}

#[test]
fn services_invoice_within_30_days_with_earlier_payment_gives_payment_date() {
    // s.13(2)(a): payment 5 Sep is earlier than invoice 11 Sep -> 5 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::September, 11)),
        Some(d(2025, Month::September, 5)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 5)));
}

#[test]
fn services_invoice_on_day_30_is_within_period() {
    // 1 Sep to 1 Oct is 30 days (September has 30 days); rule 47 allows 30 days -> invoice date.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 1)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn services_invoice_after_45_days_gives_supply_date() {
    // s.13(2)(b): invoice not issued within the period: the earlier of the date the service was
    // provided and payment. 1 Sep to 31 Oct is 60 days (30 + 30), over even the 45-day period
    // -> supply date, 1 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 31)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 1)));
}

#[test]
fn services_late_invoice_with_payment_before_supply_gives_payment_date() {
    // s.13(2)(b): payment 20 Aug is earlier than supply 1 Sep -> 20 Aug.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 31)),
        Some(d(2025, Month::August, 20)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::August, 20)));
}

#[test]
fn services_invoice_31_to_45_days_after_is_unsupported_when_readings_differ() {
    // 1 Sep to 11 Oct is 40 days (30 + 10). Within 45 days for banks and similar (invoice date
    // 11 Oct), outside 30 days for everyone else (supply date 1 Sep): the dates differ.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 11)),
        None,
    );
    assert!(got.is_unsupported());
}

#[test]
fn services_invoice_31_to_45_days_after_is_decided_when_payment_precedes_both() {
    // 40-day gap again, payment 20 Aug is before both the invoice (11 Oct) and the supply (1 Sep):
    // both readings give the earlier of the candidate date and payment = 20 Aug.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 11)),
        Some(d(2025, Month::August, 20)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::August, 20)));
}

#[test]
fn services_missing_dates_are_unsupported() {
    let sep1 = d(2025, Month::September, 1);
    assert!(services(Some(sep1), None, None).is_unsupported());
    assert!(services(None, Some(sep1), None).is_unsupported());
}
