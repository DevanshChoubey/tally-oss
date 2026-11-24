//! The Home figures and nudges on the sample company. Expected values are the mock's own
//! (`design/mock-v2/index.html`, `SCREENS.home`, evaluated on its sample data) or hand workings
//! from the seed's vouchers; none is copied from engine output.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_api::Api;
use ledgerly_core::{Clock, Outcome, Paise};
use time::OffsetDateTime;
use time::macros::date;

fn sample() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir_in("/media/devansh/Experiment/playground/claude-tmp").unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("sample.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    (dir, api)
}

fn value<T>(o: Outcome<T>) -> T {
    match o {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unsupported: {u:?}"),
    }
}

fn rupees(n: i64) -> Paise {
    Paise::new(n * 100)
}

#[test]
fn tiles_match_the_mock_on_21_nov() {
    let (_d, api) = sample();
    let f = api.home_figures(date!(2026 - 11 - 21)).unwrap();
    // provenance: derived, the mock's Home tiles on 21 Nov 2026: Cash and bank 10,19,862.00
    // ("Cash and 2 bank accounts"), Receivables 6,18,790.00 ("3 bills overdue more than 30
    // days"), Payables 7,07,970.00 ("3 supplier bills past due").
    let cb = value(f.cash_and_bank);
    assert_eq!(cb.total, rupees(1_019_862));
    assert_eq!(cb.bank_accounts, 2);
    assert_eq!(value(f.receivables), rupees(618_790));
    assert_eq!(value(f.payables), rupees(707_970));
    assert_eq!(value(f.receivables_overdue).count, 3);
    assert_eq!(value(f.payables_past_due).count, 3);
}

#[test]
fn november_sales_are_the_three_invoices_to_date() {
    let (_d, api) = sample();
    let f = api.home_figures(date!(2026 - 11 - 21)).unwrap();
    let months = value(f.sales_by_month);
    assert_eq!(months.len(), 8, "Apr to Nov");
    let nov = months.last().unwrap();
    assert_eq!(nov.month_start, date!(2026 - 11 - 01));
    assert!(nov.to_date);
    // provenance: derived from the seed's November sales on or before 21 Nov: SI/0412 =
    // 25*3850 + 67*505 = 130,085; SI/0401 = 10*8900 = 89,000; SI/0405 = 60*1450 = 87,000;
    // SI/0415 = 4*1450 + 6*640 = 9,640; SI/0417 = 120*312.5 = 37,500. Total 3,53,225.00 before GST.
    assert_eq!(
        nov.sales,
        rupees(130_085 + 89_000 + 87_000 + 9_640 + 37_500)
    );
}

#[test]
fn rule_d02_flags_the_city_power_pair() {
    let (_d, api) = sample();
    let n = api.home_nudges(date!(2026 - 11 - 21)).unwrap();
    let dups = value(n.duplicates);
    // provenance: derived from the seed: PY/0304 (28 Oct) and PY/0316 (12 Nov) are both Electricity,
    // 14,860.00, narration "City Power, October bill"; 28 Oct to 12 Nov is 3 + 12 = 15 days; the
    // mock's nudge: "PY/0316 on 12 Nov repeats PY/0304 on 28 Oct".
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].amount, rupees(14_860));
    assert_eq!(dups[0].days_apart, 15);
    assert_eq!(dups[0].first.number.as_deref(), Some("PY/0304"));
    assert_eq!(dups[0].second.number.as_deref(), Some("PY/0316"));
}

#[test]
fn gstr1_next_due_is_the_eleventh_of_december() {
    let (_d, api) = sample();
    let n = api.home_nudges(date!(2026 - 11 - 21)).unwrap();
    // provenance: derived, GSTR-1 for November is due on the 11th of the next month
    // (Notification 83/2020-Central Tax).
    assert_eq!(n.gstr1.unwrap().due, date!(2026 - 12 - 11));
}
