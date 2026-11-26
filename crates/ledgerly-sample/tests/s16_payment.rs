//! GSTR-3B payment of tax and the IMS count on the sample company (round s16, engine item E-1).
//!
//! Every expected value is `derived` by hand, as written next to it, from the sample's November
//! vouchers and 2B file (the same data as `design/mock-v2/index.html` `VOUCHERS` and `G2B`) and
//! CGST Act s.49(5), s.49A, s.170, Rule 88A (Circular 98/17/2019-GST). Read 2026-11-26.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_api::{Api, ImsAction};
use ledgerly_core::{Clock, Gstin, Outcome, Paise, Period};
use time::OffsetDateTime;
use time::macros::date;

fn company(with_2b: bool) -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let api = if with_2b {
        ledgerly_sample::create(&path, clock).unwrap()
    } else {
        ledgerly_sample::create_without_2b(&path, clock).unwrap()
    };
    (dir, api)
}

fn gstin() -> Gstin {
    Gstin::parse("27ZZZZZ0000Z1ZQ").unwrap()
}

fn november() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

fn paise(rupees: i64, p: i64) -> Paise {
    Paise::new(rupees * 100 + p)
}

#[test]
fn november_cash_payable_is_39_498_after_the_credit_accepted_in_ims() {
    // provenance: derived. Output tax (sales SI/0401, 0405, 0412, 0415, 0417): IGST 16,020.00 +
    // 4,350.00 = 20,370.00; CGST = SGST = 11,707.65 + 241.00 + 3,375.00 = 15,323.65.
    // Credit accepted in IMS: only G1, Kaveri KP/2231, IGST 11,520.00 (64,000.00 at 18%).
    // IGST left 20,370.00 - 11,520.00 = 8,850.00; s.170 rounds CGST and SGST 15,323.65 each to
    // 15,324.00; cash 8,850.00 + 15,324.00 + 15,324.00 = 39,498.00.
    let (_d, api) = company(true);
    let Outcome::Value(p) = api.gstr3b_payment(&gstin(), november()).unwrap() else {
        panic!("expected a value");
    };
    assert_eq!(p.credit_accepted.integrated, paise(11_520, 0));
    assert_eq!(p.credit_accepted.central, Paise::ZERO);
    assert_eq!(p.set_off.left_to_pay.integrated, paise(8_850, 0));
    assert_eq!(p.set_off.left_to_pay.central, paise(15_323, 65));
    assert_eq!(p.set_off.cash_payable.integrated, paise(8_850, 0));
    assert_eq!(p.set_off.cash_payable.central, paise(15_324, 0));
    assert_eq!(p.set_off.cash_payable.state_ut, paise(15_324, 0));
    assert_eq!(p.set_off.cash_total().unwrap(), paise(39_498, 0));
}

#[test]
fn records_with_no_action_are_shown_apart_and_not_used() {
    // provenance: derived from the 2B file (mock G2B): G2 Nilgiri 102,500.00 at 18% = IGST
    // 18,450.00; G4 Rupa 68,000.00 at 18% = IGST 12,240.00; G3 Vardhaman 12,000.00 at 18%
    // intra-state = CGST 1,080.00 and SGST 1,080.00. IGST 18,450 + 12,240 = 30,690.00. Three
    // documents have no action (the mock's nudge reads "3 2B records have no IMS action").
    let (_d, api) = company(true);
    let Outcome::Value(p) = api.gstr3b_payment(&gstin(), november()).unwrap() else {
        panic!("expected a value");
    };
    assert_eq!(p.no_action_documents, 3);
    assert_eq!(p.credit_no_action.integrated, paise(30_690, 0));
    assert_eq!(p.credit_no_action.central, paise(1_080, 0));
    assert_eq!(p.credit_no_action.state_ut, paise(1_080, 0));
    assert_eq!(p.credit_in_books.integrated, paise(29_520, 0)); // 11,520 + 18,000 (PB/0141, PB/0144)
}

#[test]
fn rejecting_the_one_accepted_record_leaves_no_credit_to_set_off() {
    // provenance: derived. With KP/2231 rejected no document is accepted, so credit is nil and
    // cash is the whole tax: IGST 20,370.00, CGST and SGST 15,323.65 rounded to 15,324.00 each;
    // total 20,370 + 15,324 + 15,324 = 51,018.00. Rejected is an action, so the no-action count
    // stays 3.
    let (_d, mut api) = company(true);
    let supplier = "29ZZZZZ0001Z1ZL";
    api.set_ims_action(&gstin(), november(), supplier, "KP/2231", ImsAction::Reject)
        .unwrap();
    let Outcome::Value(p) = api.gstr3b_payment(&gstin(), november()).unwrap() else {
        panic!("expected a value");
    };
    assert_eq!(p.credit_accepted.integrated, Paise::ZERO);
    assert_eq!(p.set_off.cash_total().unwrap(), paise(51_018, 0));
    assert_eq!(p.no_action_documents, 3);
}

#[test]
fn accepting_a_record_that_differs_from_the_books_is_refused() {
    // provenance: derived, rule A1. G2 (Nilgiri NST/1145) is 2B 18,450.00 against 18,000.00 in
    // the books, so crediting it would claim tax the books do not show; the answer is Unsupported
    // and names the document.
    let (_d, mut api) = company(true);
    api.set_ims_action(
        &gstin(),
        november(),
        "29ZZZZZ0002Z1ZK",
        "NST/1145",
        ImsAction::Accept,
    )
    .unwrap();
    match api.gstr3b_payment(&gstin(), november()).unwrap() {
        Outcome::Unsupported(u) => assert!(u.reason.contains("NST/1145"), "{}", u.reason),
        Outcome::Value(_) => panic!("expected unsupported"),
    }
}

#[test]
fn without_a_2b_file_the_payment_needs_gstr_2b() {
    // provenance: derived, rule A1: with no 2B file the credit accepted is unknown.
    let (_d, api) = company(false);
    match api.gstr3b_payment(&gstin(), november()).unwrap() {
        Outcome::Unsupported(u) => {
            assert_eq!(u.what, "Needs GSTR-2B");
            assert!(u.reason.contains("GSTR-2B"));
        }
        Outcome::Value(_) => panic!("expected unsupported"),
    }
}

#[test]
fn ims_count_is_three_for_november_and_none_without_a_file() {
    // provenance: derived from the mock's G2B: G1 has an action (Accept); G2, G3 and G4 have none.
    let (_d, api) = company(true);
    assert_eq!(
        api.ims_no_action_count(&gstin(), november()).unwrap(),
        Outcome::Value(Some(3))
    );
    let (_d2, api2) = company(false);
    assert_eq!(
        api2.ims_no_action_count(&gstin(), november()).unwrap(),
        Outcome::Value(None)
    );
}

#[test]
fn an_action_takes_a_record_out_of_the_ims_count() {
    // provenance: derived: Pending is an action, so 3 - 1 = 2 records remain without one.
    let (_d, mut api) = company(true);
    api.set_ims_action(
        &gstin(),
        november(),
        "24ZZZZZ0004Z1ZS",
        "RP/5521",
        ImsAction::Pending,
    )
    .unwrap();
    assert_eq!(
        api.ims_no_action_count(&gstin(), november()).unwrap(),
        Outcome::Value(Some(2))
    );
}

#[test]
fn home_reads_the_month_of_the_as_at_date() {
    // provenance: derived: the same figures as above, found for the single regular registration
    // and the calendar month of the as-at date (20 Nov 2026): cash 39,498.00 and 3 records
    // with no action.
    let (_d, api) = company(true);
    let g = api.home_gst(date!(2026 - 11 - 20)).unwrap();
    assert_eq!(g.cash_payable, Outcome::Value(paise(39_498, 0)));
    assert_eq!(g.ims_no_action, Outcome::Value(Some(3)));
}

#[test]
fn home_without_a_2b_file_says_it_needs_gstr_2b() {
    // provenance: derived: with no 2B file there is no accepted credit, so no cash figure and no
    // IMS count (the engine refuses rather than treating the credit as nil).
    let (_d, api) = company(false);
    let g = api.home_gst(date!(2026 - 11 - 20)).unwrap();
    let Outcome::Unsupported(u) = g.cash_payable else {
        panic!("expected unsupported");
    };
    assert_eq!(u.what, "Needs GSTR-2B");
    assert_eq!(g.ims_no_action, Outcome::Value(None));
}
