//! S23 tests: the conditions on input tax credit that decide GSTR-3B tables 4(B) and the time
//! limit: CGST Act s.16(4)/(5), the 180-day rule (s.16(2) second proviso with Rule 37), and the
//! common-credit attribution of Rules 42 and 43.
//!
//! Provenance: `derived`, from the texts read on CBIC's tax information portal
//! (taxinformation.cbic.gov.in, Acts > CGST Act > section 16; Rules > CGST Rules > rules 37, 42,
//! 43) on 2026-11-27:
//! * s.16(4) as substituted w.e.f. 1.10.2022 (Notification 18/2022-CT; s.100 Finance Act 2022): no
//!   credit "after the thirtieth day of November following the end of financial year to which
//!   such invoice or debit note pertains or furnishing of the relevant annual return, whichever is
//!   earlier".
//! * s.16(5) (inserted by s.118 Finance (No. 2) Act 2024): for invoices and debit notes of
//!   2017-18 to 2020-21, "notwithstanding" (4), credit in any return filed up to 30 November 2021.
//! * s.16(2) second proviso (as substituted w.e.f. 1.10.2023) and Rule 37(1) (substituted w.e.f.
//!   01.10.2022 by Notification 19/2022-CT, with the words "whether wholly or partly", "or
//!   reverse" and "proportionate to the amount not paid" inserted w.e.f. the same day by
//!   Notification 26/2022-CT): a recipient who has not paid the value with tax "within a period of
//!   one hundred and eighty days from the date of issue of invoice", other than for supplies
//!   under reverse charge, reverses the credit "proportionate to the amount not paid" in "FORM
//!   GSTR-3B for the tax period immediately following the period of one hundred and eighty days";
//!   supplies without consideration (Schedule I) and amounts added under s.15(2)(b) are deemed
//!   paid.
//! * Rule 42(1): C1 = T - (T1+T2+T3); C2 = C1 - T4; D1 = (E/F) x C2; D2 = five per cent of C2
//!   where common credit is partly for non-business purposes; C3 = C2 - (D1+D2); D1 and D2 are
//!   reversed, each tax head separately (clause (l)).
//! * Rule 43(1)(e): Tm = Tc / 60. Rule 43(1)(g): Te = (E/F) x Tr, where clause (f), which
//!   defined Tr, was omitted w.e.f. 01.04.2020 (Notification 16/2020-CT).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Outcome, Paise, Period};
use ledgerly_gst::GstError;
use ledgerly_gst::gstr3b::Gstr3bTaxAmounts;
use ledgerly_gst::itc_conditions::{
    CommonCredit, CommonCreditInputs, ReturnCycle, UnpaidInvoice, UnpaidReversal, itc_in_time,
    itc_time_limit, rule37_reversal, rule42_common_credit, rule43_exempt_share,
    rule43_monthly_common_credit,
};
use time::macros::date;

fn igst(paise: i64) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: Paise::new(paise),
        ..Gstr3bTaxAmounts::default()
    }
}

fn cgst_sgst(paise: i64) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        central: Paise::new(paise),
        state_ut: Paise::new(paise),
        ..Gstr3bTaxAmounts::default()
    }
}

fn period(from: time::Date, to: time::Date) -> Period {
    Period::new(from, to).unwrap()
}

// ---------------------------------------------------------------- s.16(4) and (5)

#[test]
fn credit_is_due_by_30_november_after_the_financial_year() {
    // provenance: derived — s.16(4): an invoice of 15 January 2025 pertains to FY 2024-25 (ends
    // 31 March 2025); the thirtieth day of November following is 30 November 2025.
    assert_eq!(
        itc_time_limit(date!(2025 - 01 - 15), None),
        Outcome::Value(date!(2025 - 11 - 30))
    );
    // 31 March 2025 is still FY 2024-25; 1 April 2025 is FY 2025-26, so 30 November 2026.
    assert_eq!(
        itc_time_limit(date!(2025 - 03 - 31), None),
        Outcome::Value(date!(2025 - 11 - 30))
    );
    assert_eq!(
        itc_time_limit(date!(2025 - 04 - 01), None),
        Outcome::Value(date!(2026 - 11 - 30))
    );
}

#[test]
fn an_annual_return_furnished_earlier_ends_the_time_limit_earlier() {
    // provenance: derived — s.16(4) "whichever is earlier": FY 2024-25 annual return furnished
    // on 20 October 2025, before 30 November 2025.
    assert_eq!(
        itc_time_limit(date!(2024 - 06 - 10), Some(date!(2025 - 10 - 20))),
        Outcome::Value(date!(2025 - 10 - 20))
    );
    // Furnished on 5 December 2025, after 30 November: 30 November is earlier.
    assert_eq!(
        itc_time_limit(date!(2024 - 06 - 10), Some(date!(2025 - 12 - 05))),
        Outcome::Value(date!(2025 - 11 - 30))
    );
}

#[test]
fn financial_year_2021_22_follows_the_substituted_text() {
    // provenance: derived — FY 2021-22 ends 31 March 2022; under the old text its limit would be
    // the September 2022 return's due date, which falls after 1.10.2022, when the substituted
    // text ("thirtieth day of November") was already in force: 30 November 2022.
    assert_eq!(
        itc_time_limit(date!(2021 - 05 - 01), None),
        Outcome::Value(date!(2022 - 11 - 30))
    );
}

#[test]
fn years_2017_18_to_2020_21_run_to_30_november_2021() {
    // provenance: derived — s.16(5): "notwithstanding" (4), invoices of FY 2017-18 to 2020-21
    // may be claimed in a return filed up to 30 November 2021, whatever the annual return date.
    assert_eq!(
        itc_time_limit(date!(2019 - 08 - 01), None),
        Outcome::Value(date!(2021 - 11 - 30))
    );
    assert_eq!(
        itc_time_limit(date!(2019 - 08 - 01), Some(date!(2020 - 12 - 15))),
        Outcome::Value(date!(2021 - 11 - 30))
    );
    assert_eq!(
        itc_time_limit(date!(2017 - 07 - 01), None),
        Outcome::Value(date!(2021 - 11 - 30))
    );
    assert_eq!(
        itc_time_limit(date!(2021 - 03 - 31), None),
        Outcome::Value(date!(2021 - 11 - 30))
    );
}

#[test]
fn a_document_before_gst_has_no_time_limit() {
    // provenance: derived — the CGST Act came into force on 1 July 2017 (s.16 footnote
    // "Enforced w.e.f. 1st July, 2017"); a document of 30 June 2017 is not under it.
    assert!(matches!(
        itc_time_limit(date!(2017 - 06 - 30), None),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn credit_taken_on_the_last_day_is_in_time_and_the_next_day_is_not() {
    // provenance: derived — s.16(4) bars credit "after" the day: taken on 30 November 2025 for a
    // FY 2024-25 invoice is in time, on 1 December 2025 it is not.
    assert_eq!(
        itc_in_time(date!(2025 - 01 - 15), date!(2025 - 11 - 30), None),
        Outcome::Value(true)
    );
    assert_eq!(
        itc_in_time(date!(2025 - 01 - 15), date!(2025 - 12 - 01), None),
        Outcome::Value(false)
    );
}

// ---------------------------------------------------------------- the 180-day rule

fn unpaid(invoice: time::Date, payable: i64, itc: Gstr3bTaxAmounts) -> UnpaidInvoice {
    UnpaidInvoice {
        invoice_date: invoice,
        payable: Paise::new(payable),
        deemed_paid: Paise::ZERO,
        payments: Vec::new(),
        itc,
        reverse_charge: false,
    }
}

#[test]
fn an_invoice_not_paid_in_180_days_is_reversed_in_the_next_return() {
    // provenance: derived — invoice 10 January 2026 for 1,18,000 with IGST credit of 18,000,
    // nothing paid. 180 days from 10 January end on 9 July 2026 counting from the day after
    // (11-31 Jan 21, Feb 28, Mar 31, Apr 30, May 31, Jun 30 = 171, then 9 July = 180), or on
    // 8 July counting the invoice day: July either way, so the tax period immediately following
    // is August 2026, and the whole 18,000 is reversed.
    let out = rule37_reversal(
        &unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000)),
        ReturnCycle::Monthly,
    )
    .unwrap();
    assert_eq!(
        out,
        Outcome::Value(Some(UnpaidReversal {
            tax_period: period(date!(2026 - 08 - 01), date!(2026 - 08 - 31)),
            unpaid: Paise::new(11_800_000),
            reversal: igst(1_800_000),
        }))
    );
}

#[test]
fn a_part_payment_reverses_the_unpaid_share() {
    // provenance: derived — Rule 37(1) "proportionate to the amount not paid": 59,000 of 1,18,000
    // paid on 1 March 2026; unpaid 59,000 is half, so half of 18,000 = 9,000 is reversed.
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.payments = vec![(date!(2026 - 03 - 01), Paise::new(5_900_000))];
    let out = rule37_reversal(&inv, ReturnCycle::Monthly).unwrap();
    assert_eq!(
        out,
        Outcome::Value(Some(UnpaidReversal {
            tax_period: period(date!(2026 - 08 - 01), date!(2026 - 08 - 31)),
            unpaid: Paise::new(5_900_000),
            reversal: igst(900_000),
        }))
    );
}

#[test]
fn each_tax_head_is_reversed_in_proportion() {
    // provenance: derived — CGST 9,000 and SGST 9,000 on 1,18,000; 59,000 unpaid: 4,500 each.
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, cgst_sgst(900_000));
    inv.payments = vec![(date!(2026 - 02 - 01), Paise::new(5_900_000))];
    let out = rule37_reversal(&inv, ReturnCycle::Monthly).unwrap();
    match out {
        Outcome::Value(Some(r)) => assert_eq!(r.reversal, cgst_sgst(450_000)),
        other @ (Outcome::Value(None) | Outcome::Unsupported(_)) => {
            panic!("expected a reversal, got {other:?}")
        }
    }
}

#[test]
fn a_payment_after_180_days_does_not_count() {
    // provenance: derived — paid 20 July 2026, after the 180 days ended (8 or 9 July): the whole
    // credit is reversed (re-availed later under Rule 37(2)).
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.payments = vec![(date!(2026 - 07 - 20), Paise::new(11_800_000))];
    let out = rule37_reversal(&inv, ReturnCycle::Monthly).unwrap();
    match out {
        Outcome::Value(Some(r)) => assert_eq!(r.reversal, igst(1_800_000)),
        other @ (Outcome::Value(None) | Outcome::Unsupported(_)) => {
            panic!("expected a full reversal, got {other:?}")
        }
    }
}

#[test]
fn an_invoice_paid_in_full_in_time_is_not_reversed() {
    // provenance: derived — paid in full on 8 July 2026, inside the 180 days on either count.
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.payments = vec![(date!(2026 - 07 - 08), Paise::new(11_800_000))];
    assert_eq!(
        rule37_reversal(&inv, ReturnCycle::Monthly).unwrap(),
        Outcome::Value(None)
    );
}

#[test]
fn a_payment_on_the_disputed_180th_day_is_unsupported() {
    // provenance: derived — 9 July 2026 is the 180th day counting from the day after the invoice
    // but the 181st counting the invoice day; the texts read do not say which (the General
    // Clauses Act was not read), so a payment that day cannot be placed.
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.payments = vec![(date!(2026 - 07 - 09), Paise::new(11_800_000))];
    assert!(matches!(
        rule37_reversal(&inv, ReturnCycle::Monthly).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn when_the_two_counts_end_in_different_months_the_period_is_unsupported() {
    // provenance: derived — invoice 2 January 2026: +180 days is 1 July, +179 days is 30 June;
    // the tax period immediately following is August on one count and July on the other.
    assert!(matches!(
        rule37_reversal(
            &unpaid(date!(2026 - 01 - 02), 11_800_000, igst(1_800_000)),
            ReturnCycle::Monthly
        )
        .unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn a_quarterly_filer_reverses_in_the_next_quarter() {
    // provenance: derived — the 180 days of a 10 January 2026 invoice end on 8 or 9 July 2026,
    // in the July-September quarter; the tax period immediately following is October-December.
    let out = rule37_reversal(
        &unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000)),
        ReturnCycle::Quarterly,
    )
    .unwrap();
    match out {
        Outcome::Value(Some(r)) => assert_eq!(
            r.tax_period,
            period(date!(2026 - 10 - 01), date!(2026 - 12 - 31))
        ),
        other @ (Outcome::Value(None) | Outcome::Unsupported(_)) => {
            panic!("expected a reversal, got {other:?}")
        }
    }
}

#[test]
fn reverse_charge_supplies_are_outside_the_180_day_rule() {
    // provenance: derived — s.16(2) second proviso and Rule 37(1): "other than the supplies on
    // which tax is payable on reverse charge basis".
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.reverse_charge = true;
    assert_eq!(
        rule37_reversal(&inv, ReturnCycle::Monthly).unwrap(),
        Outcome::Value(None)
    );
}

#[test]
fn amounts_deemed_paid_reduce_the_unpaid_value() {
    // provenance: derived — Rule 37(1) provisos: Schedule I value and s.15(2)(b) amounts are
    // deemed paid. All 1,18,000 deemed paid: nothing reversed. 59,000 deemed paid and nothing
    // else paid: 59,000 unpaid, 9,000 reversed.
    let mut inv = unpaid(date!(2026 - 01 - 10), 11_800_000, igst(1_800_000));
    inv.deemed_paid = Paise::new(11_800_000);
    assert_eq!(
        rule37_reversal(&inv, ReturnCycle::Monthly).unwrap(),
        Outcome::Value(None)
    );
    inv.deemed_paid = Paise::new(5_900_000);
    match rule37_reversal(&inv, ReturnCycle::Monthly).unwrap() {
        Outcome::Value(Some(r)) => assert_eq!(r.reversal, igst(900_000)),
        other @ (Outcome::Value(None) | Outcome::Unsupported(_)) => {
            panic!("expected a reversal, got {other:?}")
        }
    }
}

#[test]
fn an_invoice_before_the_substituted_rule_37_is_unsupported() {
    // provenance: derived — Rule 37(1) was substituted w.e.f. 01.10.2022; the earlier text was
    // not read.
    assert!(matches!(
        rule37_reversal(
            &unpaid(date!(2022 - 09 - 30), 11_800_000, igst(1_800_000)),
            ReturnCycle::Monthly
        )
        .unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn a_share_that_is_not_a_whole_number_of_paise_is_unsupported() {
    // provenance: derived — 200 of 300 paise unpaid on a credit of 100 paise is 66.67 paise;
    // Rule 37 does not say how a proportionate amount is rounded.
    let mut inv = unpaid(date!(2026 - 01 - 10), 300, igst(100));
    inv.payments = vec![(date!(2026 - 02 - 01), Paise::new(100))];
    assert!(matches!(
        rule37_reversal(&inv, ReturnCycle::Monthly).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn a_negative_payable_is_an_error() {
    // provenance: derived — the value of a supply with tax cannot be below zero.
    let inv = unpaid(date!(2026 - 01 - 10), -1, igst(100));
    assert!(matches!(
        rule37_reversal(&inv, ReturnCycle::Monthly),
        Err(GstError::Invalid { .. })
    ));
}

// ---------------------------------------------------------------- Rules 42 and 43

fn inputs_42(non_business_common: bool) -> CommonCreditInputs {
    // T 1,00,000; T1 5,000; T2 10,000; T3 5,000; T4 30,000; E 20,00,000; F 1,00,00,000.
    CommonCreditInputs {
        total: igst(10_000_000),
        non_business: igst(500_000),
        exempt: igst(1_000_000),
        blocked: igst(500_000),
        taxable_only: igst(3_000_000),
        exempt_turnover: Paise::new(200_000_000),
        total_turnover: Paise::new(1_000_000_000),
        common_partly_non_business: non_business_common,
    }
}

#[test]
fn rule_42_attributes_common_credit() {
    // provenance: derived — Rule 42(1): C1 = 1,00,000 - (5,000+10,000+5,000) = 80,000;
    // C2 = 80,000 - 30,000 = 50,000; D1 = (20,00,000 / 1,00,00,000) x 50,000 = 10,000;
    // D2 = 5% of 50,000 = 2,500; C3 = 50,000 - 12,500 = 37,500; reversed D1 + D2 = 12,500.
    assert_eq!(
        rule42_common_credit(&inputs_42(true)).unwrap(),
        Outcome::Value(CommonCredit {
            c1: igst(8_000_000),
            c2: igst(5_000_000),
            d1: igst(1_000_000),
            d2: igst(250_000),
            c3: igst(3_750_000),
            reversal: igst(1_250_000),
        })
    );
}

#[test]
fn rule_42_without_non_business_use_has_no_d2() {
    // provenance: derived — Rule 42(1)(j) applies only where common inputs are used partly for
    // non-business purposes: D2 = 0, C3 = 50,000 - 10,000 = 40,000.
    match rule42_common_credit(&inputs_42(false)).unwrap() {
        Outcome::Value(c) => {
            assert_eq!(c.d2, igst(0));
            assert_eq!(c.c3, igst(4_000_000));
            assert_eq!(c.reversal, igst(1_000_000));
        }
        Outcome::Unsupported(u) => panic!("unexpected {u:?}"),
    }
}

#[test]
fn rule_42_with_no_turnover_is_unsupported() {
    // provenance: derived — Rule 42(1)(i) proviso: with no turnover, E/F is taken from the last
    // tax period with turnover; that period is not an input here.
    let mut i = inputs_42(true);
    i.exempt_turnover = Paise::ZERO;
    i.total_turnover = Paise::ZERO;
    assert!(matches!(
        rule42_common_credit(&i).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn rule_42_share_not_in_whole_paise_is_unsupported() {
    // provenance: derived — E/F = 1/3 of C2 = 50,00,000 paise is 16,66,666.67 paise; Rule 42
    // does not say how D1 is rounded.
    let mut i = inputs_42(false);
    i.exempt_turnover = Paise::new(1);
    i.total_turnover = Paise::new(3);
    assert!(matches!(
        rule42_common_credit(&i).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn rule_42_parts_larger_than_the_total_are_an_error() {
    // provenance: derived — T1, T2 and T3 are parts "out of T"; together they cannot exceed T.
    let mut i = inputs_42(true);
    i.exempt = igst(20_000_000);
    assert!(matches!(
        rule42_common_credit(&i),
        Err(GstError::Invalid { .. })
    ));
}

#[test]
fn rule_43_spreads_common_capital_credit_over_sixty_months() {
    // provenance: derived — Rule 43(1)(e): Tm = Tc / 60; Tc of 6,00,000 gives 10,000 a month.
    assert_eq!(
        rule43_monthly_common_credit(igst(60_000_000)).unwrap(),
        Outcome::Value(igst(1_000_000))
    );
    // 100 paise / 60 is not a whole number of paise.
    assert!(matches!(
        rule43_monthly_common_credit(igst(100)).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn rule_43_exempt_share_is_unsupported_because_tr_is_undefined() {
    // provenance: derived — Rule 43(1)(g) computes Te from 'Tr', which clause (f) defined; (f)
    // was omitted w.e.f. 01.04.2020 and the text read defines Tr nowhere else.
    assert!(matches!(
        rule43_exempt_share(
            igst(1_000_000),
            Paise::new(200_000_000),
            Paise::new(1_000_000_000)
        ),
        Outcome::Unsupported(_)
    ));
}
