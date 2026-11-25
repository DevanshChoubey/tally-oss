//! Debit and credit totals of a voucher's ledger lines (round s12: the journal's foot box).
//!
//! Provenance: `derived`, from the mock's journal (design/mock-v2/index.html, the comment above
//! `SCREENS.journal`): a debit of 1,840.00 against a credit of 1,480.00 differs by 360.00
//! (184000 paise - 148000 paise = 36000 paise); the balanced case is 1,840.00 both sides.

use ledgerly_core::{DrCr, LedgerId, Paise, SignedAmount};
use ledgerly_model::LedgerLine;
use ledgerly_model::validate::LineTotals;

fn line(ledger: u64, paise: i64, side: DrCr) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: LedgerId(ledger),
        amount: SignedAmount {
            amount: Paise::new(paise),
            side,
        },
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

#[test]
fn totals_are_kept_per_side() {
    let lines = [line(1, 184_000, DrCr::Dr), line(2, 148_000, DrCr::Cr)];
    let t = LineTotals::of(&lines).unwrap();
    assert_eq!(t.debit, Paise::new(184_000));
    assert_eq!(t.credit, Paise::new(148_000));
}

#[test]
fn the_difference_names_the_short_side() {
    let lines = [line(1, 184_000, DrCr::Dr), line(2, 148_000, DrCr::Cr)];
    let t = LineTotals::of(&lines).unwrap();
    // 184000 - 148000 = 36000 paise more debit than credit: the credit side is short.
    assert_eq!(
        t.difference().unwrap(),
        Some(SignedAmount {
            amount: Paise::new(36_000),
            side: DrCr::Dr
        })
    );
    let even = LineTotals::of(&[line(1, 184_000, DrCr::Dr), line(2, 184_000, DrCr::Cr)]).unwrap();
    assert_eq!(even.difference().unwrap(), None);
}
