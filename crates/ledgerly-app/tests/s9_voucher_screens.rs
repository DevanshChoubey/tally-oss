//! Voucher entry screens against the mock (round s9): the sidebar entry that stays selected.
//!
//! Provenance: the mock's `NAV` (`design/mock-v2/index.html` line 888) has Payment and Receipt
//! rows, and `renderShell` (line 904) marks the row of the open screen with `aria-current`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_api::OpenBill;
use ledgerly_app::Screen;
use ledgerly_app::screens::voucher_entry::{BillDraft, Field, VoucherEntryScreen};
use ledgerly_core::{
    Date, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherTypeId,
};
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, BillRefKind, BillWiseConfig, Books, Company, EntityKind,
    Features, Group, Ledger, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod,
    ReservedGroup, RestartPeriod, VoucherType,
};
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(5);
const BANK: LedgerId = LedgerId(2);
const PARTY: LedgerId = LedgerId(6);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const GROUP: GroupId = GroupId(1);
const CASH_GROUP: GroupId = GroupId(2);
const BANK_GROUP: GroupId = GroupId(3);
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

/// S22: a payment's or receipt's account lists only cash and bank ledgers, so the fixture puts
/// `ledgers` under a reserved group (Cash-in-Hand, Bank Accounts) inside Current Assets, as in a
/// real company.
fn reserve(
    masters: &mut Masters,
    id: GroupId,
    name: &str,
    reserved: ReservedGroup,
    ledgers: &[LedgerId],
) {
    masters.groups.insert(
        id,
        Group {
            ineligible_itc: None,
            id,
            name: name.to_owned(),
            aliases: Vec::new(),
            parent: Some(GROUP),
            reserved: Some(reserved),
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    for ledger in ledgers {
        if let Some(l) = masters.ledgers.get_mut(ledger) {
            l.group = id;
        }
    }
}

/// Cash under Cash-in-Hand and the bank under Bank Accounts.
fn reserve_cash_and_bank(masters: &mut Masters) {
    reserve(
        masters,
        CASH_GROUP,
        "Cash-in-Hand",
        ReservedGroup::CashInHand,
        &[CASH],
    );
    reserve(
        masters,
        BANK_GROUP,
        "Bank Accounts",
        ReservedGroup::BankAccounts,
        &[BANK],
    );
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
    let mut bank = ledger(BANK, "Saptagiri Bank");
    bank.bank = Some(BankDetails {
        account_number: "XXXX 4471".to_owned(),
        ifsc: "SBIN0000001".to_owned(),
        bank_name: "Saptagiri".to_owned(),
        branch: "Main".to_owned(),
    });
    let mut party = ledger(PARTY, "Vardhaman Logistics");
    party.bill_wise = Some(BillWiseConfig {
        default_credit_days: None,
    });
    for l in [ledger(CASH, "Cash"), ledger(RENT, "Rent"), bank, party] {
        masters.ledgers.insert(l.id, l);
    }
    reserve_cash_and_bank(&mut masters);
    masters.voucher_types.insert(
        T_PAYMENT,
        VoucherType {
            id: T_PAYMENT,
            name: "Payment".to_owned(),
            base: BaseVoucherKind::Payment,
            abbreviation: "Pay".to_owned(),
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
        },
    );
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

fn open(b: &Books) -> VoucherEntryScreen {
    let fy = FinancialYear::april(2026).unwrap();
    match VoucherEntryScreen::open(b, T_PAYMENT, fy, TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

#[test]
fn the_payment_screen_keeps_the_payment_row_selected() {
    // derived: mock line 904 marks the row of the open screen; the payment screen is the
    // mock's `payment` screen, whose row is "Payment" (line 888).
    let b = books();
    let screen = open(&b);
    assert_eq!(
        ledgerly_app::shell::sidebar::selected_label(
            &screen.sidebar_title(&ledgerly_app::i18n::Tr::default()),
            &ledgerly_app::i18n::Tr::default(),
        ),
        Some("Payment")
    );
}

fn bill(name: &str, day: u8, rupees: i64) -> OpenBill {
    OpenBill {
        ledger: PARTY,
        name: name.to_owned(),
        date: Date::from_calendar_date(2026, time::Month::November, day).unwrap(),
        due: None,
        open: SignedAmount::cr(Paise::new(rupees * 100)),
    }
}

/// A payment from the bank to the party for 1,500, its bill-wise details open.
fn paying_the_party(b: &Books) -> VoucherEntryScreen {
    let mut s = open(b);
    let m = &b.masters;
    s.set_ledger_text(0, "saptagiri");
    s.submit(m);
    assert_eq!(s.focus(), Field::Mode);
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1));
    s.set_ledger_text(1, "vardhaman");
    s.submit(m);
    s.set_amount_text(1, "1500");
    s.submit(m);
    s
}

/// provenance: the mock's Mode field (design/mock-v2/index.html line 1394) follows the bank
/// account; the instruments are the model's own (`InstrumentKind`). derived: typing "ne" is the
/// start of NEFT only (Cheque, RTGS, IMPS, UPI, Card and Other start with other letters), so it
/// reads NEFT and goes on the bank line; a cash account has no mode stop, and "zzz" is none.
#[test]
fn the_mode_follows_a_bank_account_and_is_kept_on_its_line() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b);
    s.set_ledger_text(0, "cash");
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(1), "a cash account has no mode");
    let mut s = open(&b);
    s.set_ledger_text(0, "saptagiri");
    s.submit(m);
    assert_eq!(s.focus(), Field::Mode);
    s.draft_mut().mode_text = "zzz".to_owned();
    s.submit(m);
    assert!(s.error_text().unwrap().contains("none of"));
    assert_eq!(s.focus(), Field::Mode);
    s.draft_mut().mode_text = "ne".to_owned();
    s.submit(m);
    assert_eq!(s.draft().mode_text, "NEFT");
    assert_eq!(s.focus(), Field::Ledger(1));
    s.set_ledger_text(1, "rent");
    s.submit(m);
    s.set_amount_text(1, "100");
    s.submit(m);
    let fy = FinancialYear::april(2026).unwrap();
    let Outcome::Value(v) = s
        .draft()
        .build_voucher(m, fy, b.company.books_begin)
        .unwrap()
    else {
        panic!("builds");
    };
    let bank = v.ledger_lines[0].bank.as_ref().unwrap();
    assert_eq!(bank.instrument, ledgerly_model::InstrumentKind::Neft);
}

/// provenance: the mock's bill-wise popup (index.html lines 1398-1403): the open bills, and per bill
/// what this payment settles. derived: bills of 1,000 and 800 are open on the party (credits); a
/// payment of 1,500 is offered the first bill's whole 1,000 (the less of 1,500 and 1,000); the
/// user types 500 against the second; 1,000 + 500 = 1,500 settles the line and the allocations are
/// Against each bill on the debit side (the side of the payment).
#[test]
fn a_payment_to_a_bill_wise_party_opens_its_bills_and_settles_them() {
    let b = books();
    let m = &b.masters;
    let mut s = paying_the_party(&b);
    assert_eq!(s.focus(), Field::Bill(0));
    s.set_open_bills(PARTY, vec![bill("VL/861", 3, 1000), bill("VL/870", 9, 800)]);
    assert_eq!(s.draft().rows[1].bills[0].name, "VL/861");
    assert_eq!(s.draft().rows[1].bills[0].amount_text, "1000.00");
    s.draft_mut().rows[1].bills.push(BillDraft {
        name: "VL/870".to_owned(),
        amount_text: "500".to_owned(),
    });
    s.submit(m);
    assert_eq!(s.focus(), Field::Bill(1));
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(2), "the details are accepted");
    let fy = FinancialYear::april(2026).unwrap();
    let Outcome::Value(v) = s
        .draft()
        .build_voucher(m, fy, b.company.books_begin)
        .unwrap()
    else {
        panic!("builds");
    };
    let bills = &v.ledger_lines[1].bills;
    assert_eq!(bills.len(), 2);
    assert_eq!(bills[0].kind, BillRefKind::Against);
    assert_eq!(bills[0].name, "VL/861");
    assert_eq!(bills[0].amount, SignedAmount::dr(Paise::new(100_000)));
    assert_eq!(bills[1].name, "VL/870");
    assert_eq!(bills[1].amount, SignedAmount::dr(Paise::new(50_000)));
}

/// derived: 1,000 settled of a 1,500 line leaves 500 unaccounted, so the details are refused and
/// stay open; with nothing typed at all the whole 1,500 is On Account (not linked to a bill).
#[test]
fn bills_that_do_not_add_up_to_the_line_are_refused_and_none_means_on_account() {
    let b = books();
    let m = &b.masters;
    let mut s = paying_the_party(&b);
    s.set_open_bills(PARTY, vec![bill("VL/861", 3, 1000)]);
    s.submit(m);
    assert!(s.error_text().unwrap().contains("1,000.00 of 1,500.00"));
    assert_eq!(s.focus(), Field::Bill(0));
    s.draft_mut().rows[1].bills[0].amount_text.clear();
    s.submit(m);
    assert_eq!(s.focus(), Field::Ledger(2));
    let fy = FinancialYear::april(2026).unwrap();
    let Outcome::Value(v) = s
        .draft()
        .build_voucher(m, fy, b.company.books_begin)
        .unwrap()
    else {
        panic!("builds");
    };
    let bills = &v.ledger_lines[1].bills;
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].kind, BillRefKind::OnAccount);
    assert_eq!(bills[0].amount, SignedAmount::dr(Paise::new(150_000)));
}
