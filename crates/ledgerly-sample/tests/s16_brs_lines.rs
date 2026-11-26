//! Round s16: the bank reconciliation statement lists each entry the bank has not cleared, with the
//! voucher number and the other side's name, and exports the statement. Every expected value is
//! derived by hand from the seed's November vouchers (`design/mock-v2/index.html` `VOUCHERS`;
//! the totals are derived in `s9_bank.rs`); none is copied from engine output. The model is the
//! one the accounting package's help page gives for its reconciliation statement
//! (`help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm`,
//! as `ledgerly_bank::brs` cites it).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_api::{Api, ExportFormat};
use ledgerly_core::{Clock, LedgerId, Outcome, Paise};
use time::OffsetDateTime;
use time::macros::date;

const DIR: &str = "/media/devansh/Experiment/playground/claude-tmp";

fn sample() -> (tempfile::TempDir, Api, LedgerId) {
    let dir = tempfile::tempdir_in(DIR).unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("sample.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("unsupported");
    };
    let bank = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Saptagiri Commercial Bank A/c")
        .unwrap()
        .id;
    (dir, api, bank)
}

fn paise(rupees: i64, p: i64) -> Paise {
    Paise::new(rupees * 100 + p)
}

#[test]
fn each_uncleared_entry_is_listed_with_its_voucher_and_the_other_side() {
    // provenance: derived (mock VOUCHERS). Deposits, by date: RC/0215 10 Nov, Shree Ganesh
    // Traders 80,000.00; RC/0219 20 Nov, Kisan Seva Kendra 1,47,384.50. Payments: PY/0315 5 Nov,
    // Rent 25,000.00; PY/0316 12 Nov, Electricity 14,860.00; CT/0021 15 Nov, Cash 20,000.00;
    // PY/0318 20 Nov, Kaveri Packaging Pvt Ltd 50,000.00, cheque 004512.
    let (_d, api, bank) = sample();
    let Outcome::Value(s) = api.brs_statement(bank, date!(2026 - 11 - 20)).unwrap() else {
        panic!("unsupported");
    };
    let seen = |lines: &[ledgerly_api::BrsLine]| {
        lines
            .iter()
            .map(|l| {
                (
                    l.number.clone().unwrap(),
                    l.date,
                    l.particulars.clone(),
                    l.instrument.clone(),
                    l.amount,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        seen(&s.deposits),
        vec![
            (
                "RC/0215".to_owned(),
                date!(2026 - 11 - 10),
                "Shree Ganesh Traders".to_owned(),
                None,
                paise(80_000, 0)
            ),
            (
                "RC/0219".to_owned(),
                date!(2026 - 11 - 20),
                "Kisan Seva Kendra".to_owned(),
                None,
                paise(147_384, 50)
            ),
        ]
    );
    assert_eq!(
        seen(&s.payments),
        vec![
            (
                "PY/0315".to_owned(),
                date!(2026 - 11 - 05),
                "Rent".to_owned(),
                None,
                paise(25_000, 0)
            ),
            (
                "PY/0316".to_owned(),
                date!(2026 - 11 - 12),
                "Electricity".to_owned(),
                None,
                paise(14_860, 0)
            ),
            (
                "CT/0021".to_owned(),
                date!(2026 - 11 - 15),
                "Cash".to_owned(),
                None,
                paise(20_000, 0)
            ),
            (
                "PY/0318".to_owned(),
                date!(2026 - 11 - 20),
                "Kaveri Packaging Pvt Ltd".to_owned(),
                Some("004512".to_owned()),
                paise(50_000, 0)
            ),
        ]
    );
}

#[test]
fn the_exported_statement_holds_the_totals_and_every_line() {
    // provenance: derived: the CSV is the statement as the screen draws it: the books balance
    // 7,27,524.50, deposits 2,27,384.50 with their two lines, payments 1,09,860.00 with their four,
    // the balance as per bank 6,10,000.00, the statement's 8,23,305.50 and the difference
    // 2,13,305.50 (s9_bank.rs). The amounts are written with two decimals and no grouping.
    let (dir, api, bank) = sample();
    let path = dir.path().join("brs.csv");
    let done = api
        .export_brs(bank, date!(2026 - 11 - 20), ExportFormat::Csv, &path)
        .unwrap();
    assert_eq!(done, Outcome::Value(()));
    let text = std::fs::read_to_string(&path).unwrap();
    for needle in [
        "Balance as per company books",
        "727524.50",
        "227384.50",
        "109860.00",
        "RC/0215",
        "RC/0219",
        "PY/0315",
        "PY/0316",
        "CT/0021",
        "PY/0318",
        "Kaveri Packaging Pvt Ltd",
        "610000.00",
        "823305.50",
        "213305.50",
    ] {
        assert!(text.contains(needle), "{needle} is missing from:\n{text}");
    }
}

#[test]
fn an_unknown_ledger_is_an_error_not_an_empty_statement() {
    // provenance: derived: a ledger the books do not hold has no statement.
    let (_d, api, _bank) = sample();
    assert!(
        api.brs_statement(LedgerId(99_999), date!(2026 - 11 - 20))
            .is_err()
    );
}
