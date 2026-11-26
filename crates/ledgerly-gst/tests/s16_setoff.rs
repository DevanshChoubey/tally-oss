//! Set-off of input tax credit against output tax (round s16, engine item E-1).
//!
//! Provenance of every expected value is `derived`: worked by hand from CGST Act s.49(5), s.49A,
//! Rule 88A as explained by CBIC Circular 98/17/2019-GST (23 April 2019), and s.170, as written in
//! the comment of each test. Read 2026-11-26.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::Paise;
use ledgerly_gst::GstError;
use ledgerly_gst::gstr3b::Gstr3bTaxAmounts;
use ledgerly_gst::setoff::{round_to_rupee, section_49a_in_force, set_off};

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn heads(i: Paise, c: Paise, s: Paise, x: Paise) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: i,
        central: c,
        state_ut: s,
        cess: x,
    }
}

fn rupees(i: i64, c: i64, s: i64) -> Gstr3bTaxAmounts {
    heads(rs(i), rs(c), rs(s), Paise::ZERO)
}

#[test]
fn circular_98_illustration_leaves_no_cash_and_one_hundred_of_central_credit() {
    // provenance: derived. Circular 98/17/2019-GST para 5: liability IGST 1000, CGST 300,
    // SGST 300; credit IGST 1300, CGST 200, SGST 200. Both of the circular's options pay
    // everything from credit (total 1600 of 1700) and leave 100 of credit unused: option 1 leaves
    // it in central tax, option 2 in State tax. Option 1 is the split that covers each head's
    // shortfall first (central then State, 100 each) and puts the last spare 100 on central tax.
    let r = set_off(rupees(1000, 300, 300), rupees(1300, 200, 200)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 0));
    assert_eq!(r.credit_unused, rupees(0, 100, 0));
}

#[test]
fn november_of_the_sample_company_pays_39_498_in_cash() {
    // provenance: derived from the sample company's November sales and purchases (mock-v2
    // index.html VOUCHERS; the sample seed is the same data), rates 18% and 5%:
    //   SI/0401 Gujarat 10 x 8,900.00 = 89,000.00 at 18% IGST 16,020.00
    //   SI/0405 Tamil Nadu 60 x 1,450.00 = 87,000.00 at 5% IGST 4,350.00
    //   SI/0412 Maharashtra 130,085.00 at 18%: CGST and SGST 11,707.65 each
    //   SI/0415 Maharashtra 9,640.00 at 5%: 241.00 each; SI/0417 37,500.00 at 18%: 3,375.00 each
    //   IGST 20,370.00; CGST = SGST = 11,707.65 + 241.00 + 3,375.00 = 15,323.65.
    // Credit accepted in IMS: KP/2231 IGST 11,520.00. s.49(5)(a): IGST 20,370.00 - 11,520.00 =
    // 8,850.00 in cash. CGST and SGST have no credit: 15,323.65 each, s.170 rounds each to
    // 15,324.00. Cash 8,850 + 15,324 + 15,324 = 39,498.00.
    let liability = heads(
        Paise::new(2_037_000),
        Paise::new(1_532_365),
        Paise::new(1_532_365),
        Paise::ZERO,
    );
    let credit = heads(Paise::new(1_152_000), Paise::ZERO, Paise::ZERO, Paise::ZERO);
    let r = set_off(liability, credit).expect("set off");
    assert_eq!(r.left_to_pay.integrated, Paise::new(885_000));
    assert_eq!(r.left_to_pay.central, Paise::new(1_532_365));
    assert_eq!(r.cash_payable, rupees(8850, 15324, 15324));
    assert_eq!(r.cash_total().expect("total"), Paise::new(3_949_800));
}

#[test]
fn central_credit_pays_central_tax_then_integrated_tax() {
    // provenance: derived, s.49(5)(b): central credit 300 pays central tax 100 first, the
    // remaining 200 may pay integrated tax 500. Integrated tax left 300.
    let r = set_off(rupees(500, 100, 0), rupees(0, 300, 0)).expect("set off");
    assert_eq!(r.cash_payable, rupees(300, 0, 0));
    assert_eq!(r.credit_unused, rupees(0, 0, 0));
}

#[test]
fn state_credit_never_pays_central_tax() {
    // provenance: derived, s.49(5)(f): State/UT credit 100 cannot pay central tax 100.
    let r = set_off(rupees(0, 100, 0), rupees(0, 0, 100)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 100, 0));
    assert_eq!(r.credit_unused, rupees(0, 0, 100));
}

#[test]
fn central_credit_never_pays_state_tax() {
    // provenance: derived, s.49(5)(e).
    let r = set_off(rupees(0, 0, 100), rupees(0, 100, 0)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 100));
    assert_eq!(r.credit_unused, rupees(0, 100, 0));
}

#[test]
fn state_credit_pays_integrated_tax_only_after_central_credit_is_gone() {
    // provenance: derived, provisos to s.49(5)(c): integrated tax 100; central credit 60 pays
    // 60 first, State credit 100 then pays the other 40 and 60 of it is unused.
    let r = set_off(rupees(100, 0, 0), rupees(0, 60, 100)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 0));
    assert_eq!(r.credit_unused, rupees(0, 0, 60));
}

#[test]
fn integrated_credit_is_used_up_before_central_credit() {
    // provenance: derived, s.49A: liability CGST 100 only; integrated credit 100 and central
    // credit 100. The integrated credit pays the central tax, the central credit stays whole.
    let r = set_off(rupees(0, 100, 0), rupees(100, 100, 0)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 0));
    assert_eq!(r.credit_unused, rupees(0, 100, 0));
}

#[test]
fn spare_integrated_credit_covers_the_head_that_lacks_credit() {
    // provenance: derived, Rule 88A (any order and proportion; policy: least cash). Liability
    // CGST 100, SGST 100; credit IGST 100, SGST 100. Using the 100 of IGST credit on SGST would
    // leave SGST credit idle and CGST 100 in cash; using it on CGST pays CGST 100 and the SGST
    // credit pays SGST 100: no cash and no credit unused.
    let r = set_off(rupees(0, 100, 100), rupees(100, 0, 100)).expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 0));
    assert_eq!(r.credit_unused, rupees(0, 0, 0));
}

#[test]
fn cess_credit_pays_cess_only() {
    // provenance: derived, GST (Compensation to States) Act s.11(2) proviso: cess credit 50
    // pays cess 30, leaving 20 unused; it does not touch IGST 40; IGST credit 100 pays IGST 40
    // and cannot pay cess.
    let r = set_off(
        heads(rs(40), Paise::ZERO, Paise::ZERO, rs(30)),
        heads(rs(100), Paise::ZERO, Paise::ZERO, rs(50)),
    )
    .expect("set off");
    assert_eq!(r.cash_payable, rupees(0, 0, 0));
    assert_eq!(r.credit_unused.cess, rs(20));
    assert_eq!(r.credit_unused.integrated, rs(60));
    let r = set_off(
        heads(Paise::ZERO, Paise::ZERO, Paise::ZERO, rs(30)),
        heads(rs(100), Paise::ZERO, Paise::ZERO, Paise::ZERO),
    )
    .expect("set off");
    assert_eq!(r.cash_payable.cess, rs(30));
}

#[test]
fn each_head_rounds_to_the_nearest_rupee_half_up() {
    // provenance: derived, s.170: 49 paise is ignored, 50 paise goes up to the next rupee.
    assert_eq!(
        round_to_rupee(Paise::new(1_049)).expect("r"),
        Paise::new(1_000)
    );
    assert_eq!(
        round_to_rupee(Paise::new(1_050)).expect("r"),
        Paise::new(1_100)
    );
    assert_eq!(round_to_rupee(Paise::new(0)).expect("r"), Paise::new(0));
}

#[test]
fn a_negative_amount_is_refused() {
    // provenance: derived, rule A1: a negative liability or credit is not a figure to set off.
    let bad = heads(Paise::new(-1), Paise::ZERO, Paise::ZERO, Paise::ZERO);
    assert!(matches!(
        set_off(bad, rupees(0, 0, 0)),
        Err(GstError::Invalid { .. })
    ));
}

#[test]
fn section_49a_starts_on_the_first_of_february_2019() {
    // provenance: derived, Circular 98/17/2019-GST para 1: in force from 1 February 2019.
    let d = |y, m: u8, day| {
        ledgerly_core::Date::from_calendar_date(
            y,
            ledgerly_core::Month::try_from(m).expect("m"),
            day,
        )
        .expect("date")
    };
    assert!(!section_49a_in_force(d(2019, 1, 31)));
    assert!(section_49a_in_force(d(2019, 2, 1)));
    assert!(section_49a_in_force(d(2026, 11, 1)));
}
