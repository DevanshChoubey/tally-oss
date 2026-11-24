//! The bank reconciliation statement of Saptagiri on the sample company, 20 Nov 2026. Expected
//! values are hand workings from the seed and the mock's statement (`design/mock-v2/index.html`
//! lines 744-757); none is copied from engine output.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
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

fn paise(rupees: i64, p: i64) -> Paise {
    Paise::new(rupees * 100 + p)
}

#[test]
fn the_brs_lists_only_november_items_and_the_statement_balance() {
    let (_dir, api) = sample();
    let books = match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    let bank = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Saptagiri Commercial Bank A/c")
        .unwrap()
        .id;
    let Outcome::Value(brs) = api.brs(bank, date!(2026 - 11 - 20)).unwrap() else {
        panic!("the BRS is unsupported");
    };
    // provenance: derived. Opening 8,41,274.50 Dr (seed) + receipts April to October
    // 80,62,109.50 + November receipts 80,000 + 1,47,384.50 - payments in October
    // 19,73,016 + 14,860 + 48,05,508 + 15,00,000 = 82,93,384 - November payments
    // 25,000 + 14,860 + 20,000 + 50,000 = 1,09,860  =  7,27,524.50.
    assert_eq!(brs.books_balance, paise(727_524, 50));
    // provenance: derived. Every April to October line carries a bank date (2 or 3 days after
    // the voucher date, all before 5 Nov), so only the six November vouchers are uncleared:
    // deposits RC/0215 80,000 + RC/0219 1,47,384.50; payments PY/0315 25,000 + PY/0316 14,860 +
    // CT/0021 20,000 + PY/0318 50,000.
    assert_eq!(brs.uncleared_deposits_total, paise(227_384, 50));
    assert_eq!(brs.uncleared_payments_total, paise(109_860, 0));
    assert_eq!(brs.uncleared_deposits.len(), 2);
    assert_eq!(brs.uncleared_payments.len(), 4);
    // provenance: derived. 7,27,524.50 - 2,27,384.50 + 1,09,860 = 6,10,000.00, which is the
    // statement's opening balance (mock STMT.open = 610000).
    assert_eq!(brs.balance_as_per_bank, paise(610_000, 0));
    // provenance: derived from the mock's STMT: 6,10,000 + deposits (80,000 + 1,00,000 +
    // 1,47,384.50) - withdrawals (25,000 + 14,860 + 20,000 + 2,340 + 1,180 + 59 + 640 +
    // 50,000) = 8,23,305.50; the file states its end date, 20 Nov (:62F:).
    assert_eq!(brs.statement_balance, Outcome::Value(paise(823_305, 50)));
    // provenance: derived: statement - bank = 8,23,305.50 - 6,10,000.00 (our stated direction).
    assert_eq!(brs.difference, Outcome::Value(paise(213_305, 50)));
}
