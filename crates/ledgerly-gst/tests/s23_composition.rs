//! S23 tests: the composition levy statement (FORM GST CMP-08) and annual return (FORM GSTR-4):
//! the central tax of Rule 7 and the due dates of Rule 62.
//!
//! Provenance: `derived`, from the CGST Rules read on CBIC's tax information portal
//! (taxinformation.cbic.gov.in, Rules > CGST Rules > rules 7 and 62) on 2026-11-28:
//! * Rule 7 (Table substituted w.e.f. 01.04.2020 by Notification 50/2020-CT): central tax at
//!   "half per cent. of the turnover in the State or Union territory" for manufacturers (sl.1);
//!   "two and a half per cent." of it for suppliers of Schedule II para 6(b) supplies (sl.2);
//!   "half per cent. of the turnover of taxable supplies of goods and services in the State or
//!   Union territory" for any other s.10(1)/(2) supplier (sl.3); and "three per cent. of the
//!   turnover of supplies of goods and services in the State or Union territory" under s.10(2A)
//!   (sl.4).
//! * Rule 62(1) (substituted by Notification 20/2019-CT dated 23.04.2019): CMP-08 "till the 18th
//!   day of the month succeeding such quarter"; GSTR-4 "till the thirtieth day of April following
//!   the end of such financial year", and by the proviso (Notification 12/2024-CT) "for a
//!   financial year from FY 2024-25 onwards ... till the thirtieth day of June".

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Outcome, Paise};
use ledgerly_gst::GstError;
use ledgerly_gst::composition::{
    CompositionCategory, CompositionTurnover, cmp08_central_tax, cmp08_due, cmp08_state_tax,
    gstr4_due,
};
use time::macros::date;

fn turnover(in_state: i64, taxable: i64) -> CompositionTurnover {
    CompositionTurnover {
        in_state: Paise::new(in_state),
        taxable_in_state: Paise::new(taxable),
    }
}

fn central(category: CompositionCategory, t: CompositionTurnover) -> Outcome<Paise> {
    cmp08_central_tax(category, t, date!(2026 - 04 - 01)).unwrap()
}

#[test]
fn a_manufacturer_pays_half_a_per_cent_of_turnover() {
    // provenance: derived — Rule 7 sl.1: 0.5% of 10,00,000.00 = 5,000.00 = 500,000 paise.
    assert_eq!(
        central(
            CompositionCategory::Manufacturer,
            turnover(100_000_000, 100_000_000)
        ),
        Outcome::Value(Paise::new(500_000))
    );
}

#[test]
fn a_restaurant_pays_two_and_a_half_per_cent() {
    // provenance: derived — Rule 7 sl.2: 2.5% of 4,00,000.00 = 10,000.00 = 1,000,000 paise.
    assert_eq!(
        central(
            CompositionCategory::ScheduleIIRestaurant,
            turnover(40_000_000, 40_000_000)
        ),
        Outcome::Value(Paise::new(1_000_000))
    );
}

#[test]
fn a_trader_pays_on_taxable_supplies_only() {
    // provenance: derived — Rule 7 sl.3: 0.5% of the turnover of taxable supplies. Turnover in the
    // State 5,00,000.00 of which 1,00,000.00 exempt: 0.5% of 4,00,000.00 = 2,000.00.
    assert_eq!(
        central(
            CompositionCategory::OtherSupplier,
            turnover(50_000_000, 40_000_000)
        ),
        Outcome::Value(Paise::new(200_000))
    );
}

#[test]
fn a_section_10_2a_supplier_pays_three_per_cent_of_all_supplies() {
    // provenance: derived — Rule 7 sl.4: 3% of the turnover of supplies (3,00,000.00, of which
    // 50,000.00 exempt, all counted) = 9,000.00.
    assert_eq!(
        central(
            CompositionCategory::Section10_2A,
            turnover(30_000_000, 25_000_000)
        ),
        Outcome::Value(Paise::new(900_000))
    );
}

#[test]
fn before_the_substituted_table_the_rate_is_unsupported() {
    // provenance: derived — the Table was substituted w.e.f. 01.04.2020; the earlier one was not
    // checked for the s.10(2A) row, so a quarter starting 1 January 2020 is not worked out.
    assert!(matches!(
        cmp08_central_tax(
            CompositionCategory::Manufacturer,
            turnover(100_000_000, 100_000_000),
            date!(2020 - 01 - 01)
        )
        .unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn tax_that_is_not_a_whole_number_of_paise_is_unsupported() {
    // provenance: derived — 0.5% of 1.01 (101 paise) is 0.505 paise; Rule 7 gives no rounding.
    assert!(matches!(
        central(CompositionCategory::Manufacturer, turnover(101, 101)),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn taxable_turnover_above_the_whole_is_an_error() {
    // provenance: derived — taxable supplies in the State are part of the turnover in the State.
    assert!(matches!(
        cmp08_central_tax(
            CompositionCategory::OtherSupplier,
            turnover(100, 200),
            date!(2026 - 04 - 01)
        ),
        Err(GstError::Invalid { .. })
    ));
}

#[test]
fn state_tax_is_unsupported_because_the_state_rules_were_not_read() {
    // provenance: derived — Rule 7 of the CGST Rules sets central tax only; the State and UT
    // rules were not read.
    assert!(matches!(
        cmp08_state_tax(CompositionCategory::Manufacturer),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn cmp08_is_due_on_the_18th_after_the_quarter() {
    // provenance: derived — Rule 62(1)(i): quarter April-June 2026 ends 30 June: 18 July 2026;
    // quarter January-March 2027 ends 31 March: 18 April 2027.
    assert_eq!(
        cmp08_due(date!(2026 - 06 - 30)),
        Outcome::Value(date!(2026 - 07 - 18))
    );
    assert_eq!(
        cmp08_due(date!(2027 - 03 - 31)),
        Outcome::Value(date!(2027 - 04 - 18))
    );
    // A day that does not end a quarter is not a quarter end.
    assert!(matches!(
        cmp08_due(date!(2026 - 06 - 29)),
        Outcome::Unsupported(_)
    ));
    // Before the 2019 substitution the statement did not exist in this form.
    assert!(matches!(
        cmp08_due(date!(2019 - 03 - 31)),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn gstr4_is_due_on_30_june_from_2024_25_and_30_april_before() {
    // provenance: derived — Rule 62(1)(ii) and its proviso: FY 2024-25 (ends 31 March 2025):
    // 30 June 2025; FY 2023-24: 30 April 2024; FY 2018-19 ended before the 2019 text: unsupported.
    assert_eq!(
        gstr4_due(date!(2025 - 03 - 31)),
        Outcome::Value(date!(2025 - 06 - 30))
    );
    assert_eq!(
        gstr4_due(date!(2024 - 03 - 31)),
        Outcome::Value(date!(2024 - 04 - 30))
    );
    assert!(matches!(
        gstr4_due(date!(2019 - 03 - 31)),
        Outcome::Unsupported(_)
    ));
    assert!(matches!(
        gstr4_due(date!(2025 - 03 - 30)),
        Outcome::Unsupported(_)
    ));
}
