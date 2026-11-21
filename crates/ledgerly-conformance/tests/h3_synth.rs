//! Card H3 tests: the synthetic books behind the benchmarks (spec section 8).
//!
//! The benchmark is only as meaningful as its data, so the generator is tested: it is
//! deterministic, its vouchers are valid, its postings balance, and the report figures it produces
//! match the closed form worked out by hand below.
//!
//! Hand derivation for eight vouchers (`i` = 0..=7) with one debtor and one creditor. The rules
//! are in the module doc of `benches/synth/mod.rs`: amount in paise is
//! `((i % 9973) + 100) * 100 + (i % 100)`, and kind is `i % 4` = Sales, Purchase, Receipt, Payment.
//!
//! | i | amount (paise) | kind |
//! |---|---|---|
//! | 0 | 100*100 + 0 = 10000 | Sales |
//! | 1 | 101*100 + 1 = 10101 | Purchase |
//! | 2 | 102*100 + 2 = 10202 | Receipt |
//! | 3 | 103*100 + 3 = 10303 | Payment |
//! | 4 | 104*100 + 4 = 10404 | Sales |
//! | 5 | 105*100 + 5 = 10505 | Purchase |
//! | 6 | 106*100 + 6 = 10606 | Receipt |
//! | 7 | 107*100 + 7 = 10707 | Payment |
//!
//! Totals by kind: Sales 10000 + 10404 = 20404; Purchase 10101 + 10505 = 20606; Receipt
//! 10202 + 10606 = 20808; Payment 10303 + 10707 = 21010.
//!
//! Lines: Sales Dr debtor / Cr Sales; Purchase Dr Purchases / Cr creditor; Receipt Dr Bank /
//! Cr debtor; Payment Dr creditor / Cr Bank. Closing balances (debit positive):
//!
//! - Sales: Cr 20404.
//! - Purchases: Dr 20606.
//! - Bank: Dr 20808 (receipts) - 21010 (payments) = -202, so Cr 202.
//! - Debtor: Dr 20404 (sales) - 20808 (receipts) = -404, so Cr 404.
//! - Creditor: Dr 21010 (payments) - 20606 (purchases) = +404, so Dr 404.
//!
//! Trial balance totals: debit 20606 + 404 = 21010; credit 20404 + 202 + 404 = 21010.
//! Profit and loss: trading account debit net = purchases 20606 - sales 20404 = 202, so gross
//! profit is -202 (a loss), and with no indirect items net profit is -202.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

#[path = "../benches/synth/mod.rs"]
mod synth;

use ledgerly_core::{LedgerId, Outcome, Paise, SignedAmount, VoucherId};
use ledgerly_engine::vouchers::post_voucher;
use ledgerly_model::Books;
use ledgerly_model::validate::validate_voucher;
use ledgerly_reports::profit_loss::profit_loss;
use ledgerly_reports::trial_balance::trial_balance;
use ledgerly_reports::{Cell, Detail, DrillKey, Report, ReportKind, RowKind};
use ledgerly_stock::StockValuation;
use ledgerly_store::Event;
use synth::{
    LEDGER_BANK, LEDGER_PURCHASES, LEDGER_SALES, Shape, SynthError, amount_paise, fresh_voucher,
    fy_end, fy_start, report_params, synth_books,
};

const DEBTOR: LedgerId = LedgerId(1000);
const CREDITOR: LedgerId = LedgerId(1001);

fn eight() -> Shape {
    Shape {
        vouchers: 8,
        debtors: 1,
        creditors: 1,
    }
}

fn dr(paise: i64) -> Cell {
    Cell::Amount(SignedAmount::dr(Paise::new(paise)))
}

fn cr(paise: i64) -> Cell {
    Cell::Amount(SignedAmount::cr(Paise::new(paise)))
}

fn ledger_cells(report: &Report, ledger: LedgerId) -> Vec<Cell> {
    report
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Ledger && r.key == Some(DrillKey::Ledger(ledger)))
        .unwrap_or_else(|| panic!("no row for ledger {ledger:?}"))
        .cells
        .clone()
}

fn tb(books: &Books) -> Report {
    let params = report_params(ReportKind::TrialBalance, Detail::Detailed).unwrap();
    match trial_balance(books, &params).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("trial balance unsupported: {u:?}"),
    }
}

#[test]
fn amounts_follow_the_formula() {
    // provenance: derived, from the formula in the module doc of benches/synth/mod.rs.
    assert_eq!(amount_paise(0).unwrap(), Paise::new(10000));
    assert_eq!(amount_paise(7).unwrap(), Paise::new(10707));
    // i = 9973 wraps the first term to 0: (0 + 100) * 100 + 9973 % 100 = 10000 + 73.
    assert_eq!(amount_paise(9973).unwrap(), Paise::new(10073));
}

#[test]
fn year_is_financial_year_2025_26() {
    // provenance: derived, Indian financial year runs 1 April to 31 March.
    assert_eq!(fy_start().unwrap().to_string(), "2025-04-01");
    assert_eq!(fy_end().unwrap().to_string(), "2026-03-31");
}

#[test]
fn eight_vouchers_have_the_hand_derived_trial_balance() {
    let books = synth_books(eight()).unwrap();
    let report = tb(&books);
    assert_eq!(report.totals, vec![dr(21010), cr(21010)]);
    assert_eq!(
        ledger_cells(&report, LEDGER_SALES),
        vec![Cell::Empty, cr(20404)]
    );
    assert_eq!(
        ledger_cells(&report, LEDGER_PURCHASES),
        vec![dr(20606), Cell::Empty]
    );
    assert_eq!(
        ledger_cells(&report, LEDGER_BANK),
        vec![Cell::Empty, cr(202)]
    );
    assert_eq!(ledger_cells(&report, DEBTOR), vec![Cell::Empty, cr(404)]);
    assert_eq!(ledger_cells(&report, CREDITOR), vec![dr(404), Cell::Empty]);
}

#[test]
fn eight_vouchers_have_the_hand_derived_profit_and_loss() {
    let books = synth_books(eight()).unwrap();
    let params = report_params(ReportKind::ProfitAndLoss, Detail::Detailed).unwrap();
    let stock: Outcome<Vec<StockValuation>> = Outcome::Value(Vec::new());
    let Outcome::Value(report) = profit_loss(&books, &params, &stock, &stock).unwrap() else {
        panic!("profit and loss unsupported");
    };
    assert_eq!(report.totals, vec![Cell::Signed(Paise::new(-202))]);
}

#[test]
fn counts_follow_the_shape() {
    let books = synth_books(Shape {
        vouchers: 1000,
        debtors: 7,
        creditors: 5,
    })
    .unwrap();
    assert_eq!(books.vouchers.len(), 1000);
    // provenance: derived, every voucher has two ledger lines and each line is one posting.
    assert_eq!(books.postings.len(), 2000);
    // provenance: derived, 5 fixed ledgers (Sales, Purchases, Bank, Cash, P&L) + 7 + 5 parties.
    assert_eq!(books.masters.ledgers.len(), 17);
    // provenance: derived, one type per generated kind plus the unnumbered Journal.
    assert_eq!(books.masters.voucher_types.len(), 5);
}

#[test]
fn generation_is_deterministic_and_postings_are_sorted() {
    let shape = Shape::small(500);
    let a = synth_books(shape).unwrap();
    let b = synth_books(shape).unwrap();
    assert_eq!(a, b);
    assert!(
        a.postings
            .windows(2)
            .all(|w| (w[0].date, w[0].voucher) <= (w[1].date, w[1].voucher))
    );
}

#[test]
fn every_voucher_is_valid_and_balanced() {
    let books = synth_books(Shape {
        vouchers: 1000,
        debtors: 7,
        creditors: 5,
    })
    .unwrap();
    for v in books.vouchers.values() {
        validate_voucher(v, &books.masters, &books.company).unwrap();
    }
    // The trial balance refuses postings that do not balance, so a value proves they do.
    let report = tb(&books);
    assert_eq!(report.totals.len(), 2);
    let Some(Cell::Amount(debit)) = report.totals.first() else {
        panic!("no debit total");
    };
    let Some(Cell::Amount(credit)) = report.totals.get(1) else {
        panic!("no credit total");
    };
    assert_eq!(debit.amount, credit.amount);
}

#[test]
fn numbers_count_up_per_type() {
    // provenance: derived, voucher i gets counter i / 4 + 1 under its type's prefix, 7 digits.
    // Receipts are i = 2 and i = 6, which are voucher ids 3 and 7.
    let books = synth_books(eight()).unwrap();
    let number = |id: u64| {
        books
            .vouchers
            .get(&VoucherId(id))
            .and_then(|v| v.number.clone())
    };
    assert_eq!(number(3).as_deref(), Some("RC/0000001"));
    assert_eq!(number(7).as_deref(), Some("RC/0000002"));
    assert_eq!(number(1).as_deref(), Some("S/0000001"));
}

#[test]
fn fresh_vouchers_post_through_the_engine() {
    let shape = eight();
    let books = synth_books(shape).unwrap();

    // provenance: derived, the engine's numbering rule is highest counter + 1 (module doc of
    // ledgerly-engine numbering.rs); the two receipts hold counters 1 and 2, so the next is 3.
    let receipt = fresh_voucher(shape, true).unwrap();
    let Outcome::Value(events) = post_voucher(receipt, &books).unwrap() else {
        panic!("posting unsupported");
    };
    assert_eq!(events.len(), 1);
    let Event::VoucherPosted(posted) = &events[0] else {
        panic!("expected a VoucherPosted event");
    };
    assert_eq!(posted.number.as_deref(), Some("RC/0000003"));

    // provenance: derived, the Journal type is unnumbered, so the posted voucher has no number.
    let journal = fresh_voucher(shape, false).unwrap();
    let Outcome::Value(events) = post_voucher(journal, &books).unwrap() else {
        panic!("posting unsupported");
    };
    let Event::VoucherPosted(posted) = &events[0] else {
        panic!("expected a VoucherPosted event");
    };
    assert_eq!(posted.number, None);
}

#[test]
fn an_empty_shape_is_an_error_not_a_panic() {
    let zero_vouchers = Shape {
        vouchers: 0,
        ..eight()
    };
    assert_eq!(
        synth_books(zero_vouchers).unwrap_err(),
        SynthError::Shape("vouchers")
    );
    let zero_debtors = Shape {
        debtors: 0,
        ..eight()
    };
    assert_eq!(
        synth_books(zero_debtors).unwrap_err(),
        SynthError::Shape("debtors")
    );
    let zero_creditors = Shape {
        creditors: 0,
        ..eight()
    };
    assert_eq!(
        synth_books(zero_creditors).unwrap_err(),
        SynthError::Shape("creditors")
    );
}
