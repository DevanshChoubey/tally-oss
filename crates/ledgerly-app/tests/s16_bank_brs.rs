//! Round s16: the reconciliation statement lists each entry the bank has not cleared under its
//! total, and Ctrl+E writes the statement to a file. Every expected value is derived by hand from
//! the sample's November vouchers (the mock's `VOUCHERS`; the totals are derived in
//! `crates/ledgerly-sample/tests/s9_bank.rs`); none is copied from engine output. The row layout
//! is the mock's `brs` (`design/mock-v2/index.html`, `bank.brs`): a total line, its entries
//! indented under it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::Duration;

use ledgerly_app::screens::bank::{BankScreen, BrsRowKind};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, LedgerId, Outcome, Paise};
use time::OffsetDateTime;
use time::macros::date;

const DIR: &str = "/media/devansh/Experiment/playground/claude-tmp";

fn open() -> (tempfile::TempDir, ApiWorker, LedgerId) {
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
    (dir, ApiWorker::spawn(api).unwrap(), bank)
}

fn settle(screen: &mut BankScreen, worker: &ApiWorker) {
    for _ in 0..4000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the bank screen did not settle within the bound");
}

fn rs(rupees: i64, paise: i64) -> Paise {
    Paise::new(rupees * 100 + paise)
}

#[test]
fn the_statement_lists_each_uncleared_entry_under_its_total() {
    // provenance: derived (mock VOUCHERS and s9_bank.rs): deposits RC/0215 10 Nov Shree Ganesh
    // Traders 80,000.00 and RC/0219 20 Nov Kisan Seva Kendra 1,47,384.50 (total 2,27,384.50);
    // payments PY/0315 5 Nov Rent 25,000.00, PY/0316 12 Nov Electricity 14,860.00, CT/0021 15 Nov
    // Cash 20,000.00 and PY/0318 20 Nov Kaveri Packaging Pvt Ltd 50,000.00, cheque 004512
    // (total 1,09,860.00). The dates read as the app writes a date ("10 Nov 2026").
    let (_d, w, bank) = open();
    let mut screen = BankScreen::worker("Saptagiri", bank, date!(2026 - 11 - 20));
    settle(&mut screen, &w);
    let Outcome::Value(view) = screen.brs() else {
        panic!("expected a statement");
    };
    let got: Vec<(BrsRowKind, &str, Paise)> = view
        .rows
        .iter()
        .map(|r| (r.kind, r.label.as_str(), r.amount))
        .collect();
    assert_eq!(
        got,
        vec![
            (
                BrsRowKind::Books,
                "Balance as per company books",
                rs(727_524, 50)
            ),
            (
                BrsRowKind::Uncleared,
                "Amounts not reflected in bank: deposits",
                rs(227_384, 50)
            ),
            (
                BrsRowKind::Item,
                "10 Nov 2026 \u{b7} RC/0215 \u{b7} Shree Ganesh Traders",
                rs(80_000, 0)
            ),
            (
                BrsRowKind::Item,
                "20 Nov 2026 \u{b7} RC/0219 \u{b7} Kisan Seva Kendra",
                rs(147_384, 50)
            ),
            (
                BrsRowKind::Uncleared,
                "Amounts not reflected in bank: payments",
                rs(109_860, 0)
            ),
            (
                BrsRowKind::Item,
                "5 Nov 2026 \u{b7} PY/0315 \u{b7} Rent",
                rs(25_000, 0)
            ),
            (
                BrsRowKind::Item,
                "12 Nov 2026 \u{b7} PY/0316 \u{b7} Electricity",
                rs(14_860, 0)
            ),
            (
                BrsRowKind::Item,
                "15 Nov 2026 \u{b7} CT/0021 \u{b7} Cash",
                rs(20_000, 0)
            ),
            (
                BrsRowKind::Item,
                "20 Nov 2026 \u{b7} PY/0318 \u{b7} Kaveri Packaging Pvt Ltd \u{b7} cheque 004512",
                rs(50_000, 0)
            ),
            (BrsRowKind::Expected, "Balance as per bank", rs(610_000, 0)),
            (
                BrsRowKind::Statement,
                "Balance as per imported bank statement",
                rs(823_305, 50)
            ),
            (BrsRowKind::Difference, "Difference", rs(213_305, 50)),
        ]
    );
}

#[test]
fn ctrl_e_writes_the_statement_to_the_file_chosen() {
    // provenance: derived: the file holds the same statement (totals and entries) the screen
    // shows; the CSV amounts have two decimals and no grouping (crates/ledgerly-reports export
    // conventions), so the books balance reads 727524.50.
    let (d, w, bank) = open();
    let mut screen = BankScreen::worker("Saptagiri", bank, date!(2026 - 11 - 20));
    settle(&mut screen, &w);
    let path = d.path().join("brs.csv");
    screen.export_statement_to(path.clone());
    settle(&mut screen, &w);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("727524.50"), "{text}");
    assert!(text.contains("PY/0318"), "{text}");
    assert!(text.contains("Kaveri Packaging Pvt Ltd"), "{text}");
    assert_eq!(
        screen.export_notice(),
        Some(format!("Exported to {}.", path.display()).as_str())
    );
}

#[test]
fn a_name_that_is_not_csv_or_xlsx_writes_nothing_and_says_so() {
    // provenance: derived: the report export's rule (report_view): the format comes from the
    // file's extension; any other is refused and nothing is written.
    let (d, w, bank) = open();
    let mut screen = BankScreen::worker("Saptagiri", bank, date!(2026 - 11 - 20));
    settle(&mut screen, &w);
    let path = d.path().join("brs.txt");
    screen.export_statement_to(path.clone());
    settle(&mut screen, &w);
    assert!(!path.exists());
    assert!(
        screen
            .export_notice()
            .unwrap()
            .contains("choose a file name ending in .csv or .xlsx"),
        "{:?}",
        screen.export_notice()
    );
}
