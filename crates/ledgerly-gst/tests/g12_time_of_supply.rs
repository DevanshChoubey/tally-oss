//! Card G12 tests: time of supply (CGST Act 2017 s.12, s.13, s.14; CGST Rules 2017 rule 47).
//!
//! Every expected value is `derived`: the quoted clause of the Act and the hand reasoning sit next
//! to it. The quotations are of CGST Act 2017 s.12(2), s.13(2) and s.14 and CGST Rules 2017
//! rule 47, as the sections read after the 2017 enactment; they are typed from the Act, not
//! copied from a file in this repository (the repo holds no copy), so the owner should check the
//! wording against the gazetted text before a case is moved to the conformance pass-list.
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

// ---- s.14: change in rate of tax ----
//
// s.14: "Notwithstanding anything contained in section 12 or section 13, the time of supply, where
// there is a change in the rate of tax in respect of goods or services or both, shall be
// determined in the following manner, namely:-
// (a) in case the goods or services or both have been supplied before the change in rate of tax,-
//   (i) where the invoice for the same has been issued and the payment is also received after the
//   change in rate of tax, the time of supply shall be the date of receipt of payment or the date
//   of issue of invoice, whichever is earlier; or
//   (ii) where the invoice has been issued prior to the change in rate of tax but payment is
//   received after the change in the rate of tax, the time of supply shall be the date of issue of
//   invoice; or
//   (iii) where the payment has been received before the change in rate of tax, but the invoice
//   has been issued after the change in rate of tax, the time of supply shall be the date of
//   receipt of payment;
// (b) in case the goods or services or both have been supplied after the change in rate of tax,-
//   (i) where the payment is received after the change in rate of tax, but the invoice has been
//   issued prior to the change in rate of tax, the time of supply shall be the date of receipt of
//   payment; or
//   (ii) where the invoice has been issued and the payment is received before the change in rate
//   of tax, the time of supply shall be the date of receipt of payment or date of issue of
//   invoice, whichever is earlier; or
//   (iii) where the invoice has been issued after the change in rate of tax but the payment has
//   been received prior to the change in rate of tax, the time of supply shall be the date of
//   issue of invoice."
// Explanation: "the date of receipt of payment shall be the date on which the payment is entered
// in the books of account of the supplier or the date on which the payment is credited to his
// bank account, whichever is earlier."
//
// "Before" below means earlier than the change date; the change date itself counts as after.

// ---- s.14(1)(a): supplied before the change in rate ----

#[test]
fn s14_a_i_invoice_and_payment_both_after_change() {
    // s.14(a)(i): "the invoice ... has been issued and the payment is also received after the
    // change in rate of tax, the time of supply shall be the date of receipt of payment or the
    // date of issue of invoice, whichever is earlier". Both are after the change -> new rate.
    let got = side(Some(before()), Some(after()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_a_ii_invoice_before_payment_after() {
    // s.14(a)(ii): "the invoice has been issued prior to the change in rate of tax but payment is
    // received after ... the time of supply shall be the date of issue of invoice". The invoice
    // date is before the change -> old rate.
    let got = side(Some(before()), Some(before()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_a_iii_payment_before_invoice_after() {
    // s.14(a)(iii): "the payment has been received before the change in rate of tax, but the
    // invoice has been issued after ... the time of supply shall be the date of receipt of
    // payment". The payment date is before the change -> old rate.
    let got = side(Some(before()), Some(after()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_a_everything_before_change() {
    // Supply, invoice and payment all before the change: none of s.14(a)(i)-(iii) applies, so
    // s.12(2)/s.13(2) (earliest of the dates, quoted below) give a date before the change -> old
    // rate.
    let got = side(Some(before()), Some(before()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

// ---- s.14(1)(b): supplied after the change in rate ----

#[test]
fn s14_b_i_invoice_before_payment_after() {
    // s.14(b)(i): "the payment is received after the change in rate of tax, but the invoice has
    // been issued prior to the change in rate of tax, the time of supply shall be the date of
    // receipt of payment". The payment date is after the change -> new rate.
    let got = side(Some(after()), Some(before()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_b_ii_invoice_and_payment_both_before_change() {
    // s.14(b)(ii): "the invoice has been issued and the payment is received before the change in
    // rate of tax, the time of supply shall be the date of receipt of payment or date of issue of
    // invoice, whichever is earlier". Both are before the change -> old rate.
    let got = side(Some(after()), Some(before()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::Before));
}

#[test]
fn s14_b_iii_invoice_after_payment_before() {
    // s.14(b)(iii): "the invoice has been issued after the change in rate of tax but the payment
    // has been received prior to the change in rate of tax, the time of supply shall be the date
    // of issue of invoice". The invoice date is after the change -> new rate.
    let got = side(Some(after()), Some(after()), Some(before()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

#[test]
fn s14_b_everything_after_change() {
    // Supply, invoice and payment all after the change: none of s.14(b)(i)-(iii) applies, so
    // s.12(2)/s.13(2) give a date after the change -> new rate.
    let got = side(Some(after()), Some(after()), Some(after()));
    assert_eq!(got, Outcome::Value(RateSide::After));
}

// ---- boundary: the change date itself ----

#[test]
fn change_date_counts_as_after() {
    // A rate that changes "with effect from 22 Sep" applies to a time of supply on 22 Sep, so a
    // supply on 22 Sep is "after the change" (s.14(b)): invoice and payment both before it ->
    // s.14(b)(ii) -> old; invoice on 22 Sep, payment before -> s.14(b)(iii), "the date of issue
    // of invoice" = 22 Sep -> new.
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
    // Supply on 21 Sep (before the change, s.14(a)), invoice on 21 Sep, payment on 22 Sep (after):
    // s.14(a)(ii), "the date of issue of invoice" = 21 Sep -> old rate.
    let day_before = d(2025, Month::September, 21);
    assert_eq!(
        side(Some(day_before), Some(day_before), Some(change())),
        Outcome::Value(RateSide::Before)
    );
}

// ---- missing dates ----

#[test]
fn missing_supply_date_is_unsupported() {
    // s.14 first sorts the supply into (a) "supplied before the change in rate of tax" or (b)
    // "supplied after" it; without the supply date no clause can be chosen.
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
//
// s.12(2): "The time of supply of goods shall be the earliest of the following dates, namely:-
// (a) the date of issue of invoice by the supplier or the last date on which he is required,
// under section 31, to issue the invoice with respect to the supply; or
// (b) the date on which the supplier receives the payment with respect to the supply:
// Provided that where the supplier of taxable goods receives an amount up to one thousand rupees
// in excess of the amount indicated in the tax invoice, the time of supply to the extent of such
// excess amount shall, at the option of the said supplier, be the date of issue of invoice in
// respect of such excess amount."
// s.31(1)(a): for a supply involving movement of goods, the invoice is issued "before or at the
// time of removal of goods for supply to the recipient".

fn goods(supply: Option<Date>, invoice: Option<Date>, payment: Option<Date>) -> Outcome<Date> {
    time_of_supply(&dates(supply, invoice, payment, GoodsOrServices::Goods))
}

#[test]
fn goods_invoice_issued_before_removal_gives_invoice_date() {
    // s.12(2)(a): "the date of issue of invoice by the supplier or the last date on which he is
    // required, under section 31, to issue the invoice", whichever is earliest. s.31(1)(a) says
    // "before or at the time of removal", so the last date is the removal date, 1 Oct 2025.
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
    // s.12(2)(a), earliest of the invoice date and "the last date on which he is required, under
    // section 31, to issue the invoice": invoice issued 5 Oct, removal on 1 Oct, so the last date
    // (s.31(1)(a)) is 1 Oct, which is earlier -> 1 Oct.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 5)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn goods_payment_after_invoice_does_not_move_the_date() {
    // s.12(2): "the earliest of the following dates", (a) invoice date 1 Oct, (b) payment 9 Oct
    // -> 1 Oct.
    let got = goods(
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 1)),
        Some(d(2025, Month::October, 9)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn goods_advance_before_invoice_is_unsupported() {
    // s.12(2)(b), "the date on which the supplier receives the payment", gives the payment date
    // (15 Sep) as the earliest of the s.12(2) dates. Notification 66/2017-CT (central tax) makes
    // the invoice date the time of supply for registered persons other than composition dealers
    // (a rule recalled from the notification, not quoted); the registration type is not in the
    // input, so the two readings differ and the case is Unsupported.
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
//
// s.13(2): "The time of supply of services shall be the earliest of the following dates, namely:-
// (a) the date of issue of invoice by the supplier, if the invoice is issued within the period
// prescribed under section 31 or the date of receipt of payment, whichever is earlier; or
// (b) the date of provision of service, if the invoice is not issued within the period prescribed
// under section 31 or the date of receipt of payment, whichever is earlier; or
// (c) the date on which the recipient shows the receipt of the services in his books of account,
// in a case where the provision of services is not covered under clause (a) or clause (b)."
// CGST Rules 2017 rule 47: the invoice is issued "within a period of thirty days from the date of
// the supply of service"; for an insurer, a banking company, a financial institution or an NBFC
// the period is "forty-five days".

fn services(supply: Option<Date>, invoice: Option<Date>, payment: Option<Date>) -> Outcome<Date> {
    time_of_supply(&dates(supply, invoice, payment, GoodsOrServices::Services))
}

#[test]
fn services_invoice_within_30_days_gives_invoice_date() {
    // s.13(2)(a): "the date of issue of invoice ... if the invoice is issued within the period
    // prescribed under section 31 or the date of receipt of payment, whichever is earlier". Rule 47
    // period is 30 days; supply 1 Sep, invoice 11 Sep (10 days), no payment -> 11 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::September, 11)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 11)));
}

#[test]
fn services_invoice_within_30_days_with_earlier_payment_gives_payment_date() {
    // s.13(2)(a), "whichever is earlier": payment 5 Sep is earlier than invoice 11 Sep -> 5 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::September, 11)),
        Some(d(2025, Month::September, 5)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 5)));
}

#[test]
fn services_invoice_on_day_30_is_within_period() {
    // Rule 47, "within a period of thirty days": 1 Sep to 1 Oct is 30 days (September has 30
    // days), so the invoice is within the period; s.13(2)(a) -> invoice date, 1 Oct.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 1)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::October, 1)));
}

#[test]
fn services_invoice_after_45_days_gives_supply_date() {
    // s.13(2)(b): "the date of provision of service, if the invoice is not issued within the period
    // prescribed under section 31 or the date of receipt of payment, whichever is earlier". 1 Sep to
    // 31 Oct is 60 days (30 + 30), over even the 45-day period -> supply date, 1 Sep.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 31)),
        None,
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::September, 1)));
}

#[test]
fn services_late_invoice_with_payment_before_supply_gives_payment_date() {
    // s.13(2)(b), "whichever is earlier": payment 20 Aug is earlier than supply 1 Sep -> 20 Aug.
    let got = services(
        Some(d(2025, Month::September, 1)),
        Some(d(2025, Month::October, 31)),
        Some(d(2025, Month::August, 20)),
    );
    assert_eq!(got, Outcome::Value(d(2025, Month::August, 20)));
}

#[test]
fn services_invoice_31_to_45_days_after_is_unsupported_when_readings_differ() {
    // 1 Sep to 11 Oct is 40 days (30 + 10). Rule 47 gives "forty-five days" to banks and similar,
    // so s.13(2)(a) -> invoice date 11 Oct; for everyone else 40 > 30 days, so s.13(2)(b) -> supply
    // date 1 Sep. The dates differ and the entity type is not in the input -> Unsupported.
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
