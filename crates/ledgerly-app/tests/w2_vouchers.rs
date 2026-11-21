//! Wave 2 (S-H): the receipt and payment screens in single-entry layout.
//!
//! Every expected value is derived by hand and the derivation is written next to it. Provenance:
//! `derived` for amounts and sides, the mock `design/mock-v2/index.html` (payment and receipt
//! screen layout, lines 1380-1411) for the layout, and `crates/ledgerly-app/assets/keymap-classic.toml`
//! (lines 20-22 for the voucher keys, line 40 for save) for the keys. The single-entry model
//! follows the mock; the Tally documentation for receipt single entry is not yet checked (the
//! receipt claim is unverified, see the report).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_app::Screen;
use ledgerly_app::keymap::{self, Action};
use ledgerly_app::screens::voucher_entry::{
    DraftRow, EntryKind, Field, VoucherDraft, VoucherEntryScreen,
};
use ledgerly_core::{
    Date, DrCr, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
    VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillWiseConfig, Books, Company, EntityKind, Features, Group, Ledger,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, RestartPeriod, VoucherType,
};
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(5);
const SHREE: LedgerId = LedgerId(4);
const BILLWISE: LedgerId = LedgerId(6);

const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const T_RECEIPT: VoucherTypeId = VoucherTypeId(7);
const GROUP: GroupId = GroupId(1);

const TODAY: Date = date!(2026 - 11 - 21);

fn ledger(id: LedgerId, name: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: GROUP,
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters.groups.insert(
        GROUP,
        Group {
            ineligible_itc: None,
            id: GROUP,
            name: "Current Assets".to_owned(),
            aliases: Vec::new(),
            parent: None,
            reserved: None,
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    let mut billwise = ledger(BILLWISE, "Kaveri Packaging");
    billwise.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        ledger(CASH, "Cash"),
        ledger(SHREE, "Shree Stationers"),
        ledger(RENT, "Rent"),
        billwise,
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_JOURNAL, "Journal", BaseVoucherKind::Journal),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment),
        voucher_type(T_RECEIPT, "Receipt", BaseVoucherKind::Receipt),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn fy() -> FinancialYear {
    FinancialYear::april(2026).unwrap()
}

fn open(books: &Books, t: VoucherTypeId) -> VoucherEntryScreen {
    match VoucherEntryScreen::open(books, t, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn row(ledger: LedgerId, side: DrCr, amount: &str) -> DraftRow {
    DraftRow {
        ledger: Some(ledgerly_app::screens::voucher_entry::LedgerRef::Chosen(
            ledger,
        )),
        ledger_text: String::new(),
        side,
        amount_text: amount.to_owned(),
    }
}

#[test]
fn payment_account_is_credited_and_receipt_account_debited() {
    // derived: a payment takes money out of the cash or bank account, so that account is
    // credited; a receipt brings money in, so it is debited.
    let pay = VoucherDraft::new(T_PAYMENT, EntryKind::Payment, "15-4".to_owned());
    assert!(pay.is_single_entry());
    assert_eq!(pay.rows[0].side, DrCr::Cr);

    let rec = VoucherDraft::new(T_RECEIPT, EntryKind::Receipt, "15-4".to_owned());
    assert!(rec.is_single_entry());
    assert_eq!(rec.rows[0].side, DrCr::Dr);

    // Journal stays double entry: the first line is a debit and there is no account line.
    let jn = VoucherDraft::new(T_JOURNAL, EntryKind::Journal, "15-4".to_owned());
    assert!(!jn.is_single_entry());
    assert_eq!(jn.rows[0].side, DrCr::Dr);
}

#[test]
fn the_account_amount_is_the_total_of_the_particulars() {
    // derived: 1000.00 + 250.50 = 1250.50 rupees = 125050 paise. The account line holds no
    // amount of its own; its amount is the particulars total.
    let d = VoucherDraft {
        type_id: T_PAYMENT,
        kind: EntryKind::Payment,
        date_text: "15-4".to_owned(),
        narration: String::new(),
        rows: vec![
            DraftRow::empty(DrCr::Cr),
            row(RENT, DrCr::Dr, "1000"),
            row(SHREE, DrCr::Dr, "250.50"),
        ],
    };
    assert_eq!(d.particulars_total(), Ok(Some(Paise::new(125_050))));
    assert_eq!(d.line_amount(0), Ok(Some(Paise::new(125_050))));
    // The account is credited with that total, so the lines balance.
    assert_eq!(
        d.balance(),
        Ok(ledgerly_app::screens::voucher_entry::Balance::Balanced)
    );
}

#[test]
fn enter_after_the_account_goes_to_the_particulars() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_PAYMENT);
    assert_eq!(s.focus(), Field::Ledger(0));

    s.set_ledger_text(0, "cash");
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1));
    assert_eq!(s.error_text(), None);
}

#[test]
fn enter_on_an_empty_first_particular_is_an_error() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_PAYMENT);
    s.set_ledger_text(0, "cash");
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1));

    // Nothing typed on the first particular: Enter must refuse, not skip to the narration.
    s.submit(m);
    assert!(s.error_text().is_some());
    assert_eq!(s.focus(), Field::Ledger(1));
}

#[test]
fn save_is_ctrl_a_in_the_classic_keymap() {
    // keymap-classic.toml line 40: save = ["Ctrl+A", "Ctrl+S"]. The screen reads the key from the
    // keymap; this checks the label the page head shows.
    let map = keymap::classic();
    assert_eq!(map.shortcut_label(Action::Save), Some("Ctrl+A".to_owned()));
}

#[test]
fn the_payment_subtitle_says_single_entry_with_its_key() {
    // keymap-classic.toml line 21: new-payment = ["F5"].
    let b = books();
    let s = open(&b, T_PAYMENT);
    assert_eq!(
        s.subtitle(),
        Some("F5 · single entry: pick the cash or bank account once, then the ledgers".to_owned())
    );
    assert_eq!(s.page_title(), "Payment");
}

#[test]
fn a_bill_wise_particular_stays_unsupported_in_single_entry() {
    // Bill allocation is not built yet (E-6): a bill-wise ledger is Unsupported, never saved
    // without its allocations. derived: any draft with that ledger is Unsupported.
    let b = books();
    let d = VoucherDraft {
        type_id: T_PAYMENT,
        kind: EntryKind::Payment,
        date_text: "15-4".to_owned(),
        narration: String::new(),
        rows: vec![row(CASH, DrCr::Cr, "10"), row(BILLWISE, DrCr::Dr, "10")],
    };
    let out = d
        .build_voucher(&b.masters, fy(), b.company.books_begin)
        .unwrap();
    assert!(out.is_unsupported());
}
