//! s11: the sample company's vouchers were entered on their own dates, by the users the seed
//! names, so the voucher view's Audit card reads as the mock's does.
//!
//! Provenance: the mock's `balance-sheet.voucher` (design/mock-v2/index.html, the Audit card of
//! SI/0412): "Created 14 Nov 2026 by Ananya K., typed by hand." and the Total row 1,53,500.30 on
//! both sides. Derived: Dr 1,53,500.00 (party) + 0.30 (round off) = 1,53,500.30; Cr 1,30,085.00
//! + 11,707.65 + 11,707.65 = 1,53,500.30.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests stop on any failure"
)]

use ledgerly_api::{EntryOrigin, VoucherDetail};
use ledgerly_core::{Clock, Outcome, Paise};
use time::OffsetDateTime;
use time::macros::date;

fn detail_of(number: &str) -> VoucherDetail {
    let dir = tempfile::tempdir().unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("sample.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let books = api.books().unwrap().value().unwrap();
    let id = books
        .vouchers
        .values()
        .find(|v| v.number.as_deref() == Some(number))
        .unwrap()
        .id;
    match api.voucher_detail(id).unwrap() {
        Outcome::Value(d) => d,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

#[test]
fn si_0412_was_created_on_14_nov_by_ananya_typed() {
    let d = detail_of("SI/0412");
    let audit = d.audit.unwrap();
    assert_eq!(audit.posted_at.date(), date!(2026 - 11 - 14));
    assert_eq!(audit.entered_by, "Ananya K.");
    assert_eq!(audit.origin, EntryOrigin::Typed);
    assert_eq!(audit.changes, Vec::new());
}

#[test]
fn si_0412_totals_match_the_mocks_total_row() {
    let d = detail_of("SI/0412");
    assert_eq!(d.debit_total, Paise::new(15_350_030));
    assert_eq!(d.credit_total, Paise::new(15_350_030));
}
