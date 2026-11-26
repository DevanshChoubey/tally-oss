//! S22: the Account list of a single-entry Payment or Receipt, and every line of a Contra, offers
//! only cash and bank ledgers; the particulars of a Payment or Receipt offer the other ledgers.
//!
//! Provenance (derived from the accounting product's help pages, read 2026-11-27):
//! - Payment, single entry: "select the Cash ledger under Account and the expense or party
//!   ledger under Particulars"; for a cheque or e-fund transfer "Under Account, select the
//!   required Bank ledger" (<https://help.tallysolutions.com/payments-and-receipts-tally/>).
//! - Receipt, single entry: "Select the Cash ledger under Account"
//!   (<https://help.tallysolutions.com/record-receipt-transactions/>).
//! - Contra, single entry: "Select the Cash/Bank ledger account that you want to debit in the
//!   Account field" (<https://help.tallysolutions.com/record-contra-entry/>); a contra moves money
//!   between cash and bank accounts.
//!
//! No page lists the Account field's choices outright, so the rule is derived: the Account field
//! takes a ledger under Cash-in-Hand, Bank Accounts or Bank OD A/c (the three reserved groups that
//! hold cash and bank accounts, `ledgerly_model::seed_groups`). The sample company's own data
//! decides the expected names: "Cash" is under Cash-in-Hand; "Ananya Kulkarni, capital", "CGST"
//! and "Audit fee payable" are under Capital Account, Duties & Taxes and Current Liabilities.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_app::screens::voucher_entry::{PickerItem, VoucherEntryScreen};
use ledgerly_core::{Clock, FinancialYear, GroupId};
use ledgerly_model::{BaseVoucherKind, Books, ReservedGroup};
use time::OffsetDateTime;
use time::macros::date;

fn sample_books(name: &str) -> Books {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    api.books().unwrap().value().unwrap().clone()
}

fn open(books: &Books, base: BaseVoucherKind) -> VoucherEntryScreen {
    let type_id = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == base)
        .unwrap()
        .id;
    VoucherEntryScreen::open(
        books,
        type_id,
        FinancialYear::april(2026).unwrap(),
        date!(2026 - 11 - 21),
    )
    .unwrap()
    .value()
    .unwrap()
}

/// Whether `group` or one of its parents is Cash-in-Hand, Bank Accounts or Bank OD A/c.
fn under_cash_bank(books: &Books, mut group: Option<GroupId>) -> bool {
    for _ in 0..=books.masters.groups.len() {
        let Some(g) = group.and_then(|id| books.masters.groups.get(&id)) else {
            return false;
        };
        if matches!(
            g.reserved,
            Some(ReservedGroup::CashInHand | ReservedGroup::BankAccounts | ReservedGroup::BankOd)
        ) {
            return true;
        }
        group = g.parent;
    }
    false
}

fn names(books: &Books, items: &[PickerItem]) -> Vec<(String, bool)> {
    items
        .iter()
        .filter_map(|item| match item {
            PickerItem::Ledger(hit) => {
                let ledger = books.masters.ledgers.get(&hit.id).unwrap();
                Some((hit.name.clone(), under_cash_bank(books, Some(ledger.group))))
            }
            PickerItem::Create { .. } => None,
        })
        .collect()
}

fn assert_cash_bank_only(books: &Books, items: &[PickerItem], what: &str) {
    let listed = names(books, items);
    assert!(
        listed.iter().any(|(n, _)| n == "Cash"),
        "{what}: Cash is listed: {listed:?}"
    );
    for (name, cash_bank) in &listed {
        assert!(
            cash_bank,
            "{what}: only cash and bank ledgers are listed, found {name}"
        );
    }
    for absent in ["Ananya Kulkarni, capital", "CGST", "Audit fee payable"] {
        assert!(
            !listed.iter().any(|(n, _)| n == absent),
            "{what}: {absent} is not listed"
        );
    }
}

#[test]
fn the_payment_account_lists_only_cash_and_bank_ledgers() {
    let books = sample_books("s22-payment-account.ledgerly");
    let screen = open(&books, BaseVoucherKind::Payment);
    assert_cash_bank_only(
        &books,
        &screen.picker_items(&books.masters),
        "payment account",
    );
}

#[test]
fn the_receipt_account_lists_only_cash_and_bank_ledgers() {
    let books = sample_books("s22-receipt-account.ledgerly");
    let screen = open(&books, BaseVoucherKind::Receipt);
    assert_cash_bank_only(
        &books,
        &screen.picker_items(&books.masters),
        "receipt account",
    );
}

#[test]
fn a_typed_name_outside_cash_and_bank_is_not_offered_for_the_account() {
    let books = sample_books("s22-payment-typed.ledgerly");
    let mut screen = open(&books, BaseVoucherKind::Payment);
    screen.set_ledger_text(0, "Audit");
    let listed = names(&books, &screen.picker_items(&books.masters));
    assert!(
        listed.is_empty(),
        "no ledger outside cash and bank: {listed:?}"
    );
}

#[test]
fn the_payment_particulars_still_list_the_other_ledgers() {
    let books = sample_books("s22-payment-particulars.ledgerly");
    let mut screen = open(&books, BaseVoucherKind::Payment);
    screen.set_ledger_text(0, "Cash");
    screen.submit(&books.masters);
    // Past the account (and the mode, when the keyboard stops there) to the first particular.
    for _ in 0..3 {
        if matches!(
            screen.focus(),
            ledgerly_app::screens::voucher_entry::Field::Ledger(1)
        ) {
            break;
        }
        screen.submit(&books.masters);
    }
    assert!(matches!(
        screen.focus(),
        ledgerly_app::screens::voucher_entry::Field::Ledger(1)
    ));
    let listed = names(&books, &screen.picker_items(&books.masters));
    assert!(
        listed.iter().any(|(n, _)| n == "Audit fee payable"),
        "particulars list the other ledgers: {listed:?}"
    );
}

#[test]
fn every_contra_line_lists_only_cash_and_bank_ledgers() {
    let books = sample_books("s22-contra.ledgerly");
    let mut screen = open(&books, BaseVoucherKind::Contra);
    assert_cash_bank_only(
        &books,
        &screen.picker_items(&books.masters),
        "contra line 1",
    );
    screen.set_ledger_text(0, "Cash");
    screen.submit(&books.masters);
    // Through the first line's amount to the second line's ledger.
    screen.set_amount_text(0, "100");
    for _ in 0..3 {
        if matches!(
            screen.focus(),
            ledgerly_app::screens::voucher_entry::Field::Ledger(1)
        ) {
            break;
        }
        screen.submit(&books.masters);
    }
    assert!(matches!(
        screen.focus(),
        ledgerly_app::screens::voucher_entry::Field::Ledger(1)
    ));
    let listed = names(&books, &screen.picker_items(&books.masters));
    assert!(!listed.is_empty(), "contra line 2 lists the bank ledgers");
    for (name, cash_bank) in &listed {
        assert!(cash_bank, "contra line 2: only cash and bank, found {name}");
    }
}
