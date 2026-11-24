//! Home figures from the engine: cash and bank, receivables and payables with their late bills,
//! and sales by month before GST.
//!
//! Provenance: every expected figure is `derived` by hand from the vouchers posted in this file,
//! with the arithmetic next to it. The definitions are ours (Home's tile sub-lines in the design
//! mock say "bills overdue more than 30 days" and "supplier bills past due"): a bill is overdue by
//! `as_of - due date` whole days, a customer bill counts above 30 days, a supplier bill above 0.
//! "Sales before GST" is the net credit to the Sales Accounts group (credits less debits) over the
//! voucher dates of the month, so tax ledgers, which sit under Duties and Taxes, are left out.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, CashAndBank, Command, CommandResult, LateBills, MonthSales};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company, EntityKind,
    Features, Ledger, LedgerLine, NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode,
    VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const BANK_A: LedgerId = LedgerId(2);
const BANK_B: LedgerId = LedgerId(3);
const CUSTOMER: LedgerId = LedgerId(4);
const SUPPLIER: LedgerId = LedgerId(5);
const SALES: LedgerId = LedgerId(6);
const EXPENSE: LedgerId = LedgerId(7);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn ledger(
    id: LedgerId,
    name: &str,
    kind: ReservedGroup,
    opening: SignedAmount,
    bills: bool,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening,
        bill_wise: bills.then_some(BillWiseConfig {
            default_credit_days: None,
        }),
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills,
        cost: Vec::new(),
        bank: None,
    }
}

fn bill(kind: BillRefKind, name: &str, due: Option<Date>, amount: SignedAmount) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.to_owned(),
        due_date: due,
        credit_days: None,
        amount,
    }
}

fn journal(id: u64, on: Date, status: VoucherStatus, lines: Vec<LedgerLine>) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A sale of `rupees` to the customer: Dr customer (new bill `name`, due `due`), Cr sales.
fn sale(id: u64, on: Date, name: &str, due: Option<Date>, rupees: i64) -> Voucher {
    let amount = rs(rupees);
    journal(
        id,
        on,
        VoucherStatus::Regular,
        vec![
            line(
                CUSTOMER,
                SignedAmount::dr(amount),
                vec![bill(BillRefKind::New, name, due, SignedAmount::dr(amount))],
            ),
            line(SALES, SignedAmount::cr(amount), Vec::new()),
        ],
    )
}

/// A purchase of `rupees` from the supplier: Dr expense, Cr supplier (new bill).
fn purchase(id: u64, on: Date, name: &str, due: Date, rupees: i64) -> Voucher {
    let amount = rs(rupees);
    journal(
        id,
        on,
        VoucherStatus::Regular,
        vec![
            line(EXPENSE, SignedAmount::dr(amount), Vec::new()),
            line(
                SUPPLIER,
                SignedAmount::cr(amount),
                vec![bill(
                    BillRefKind::New,
                    name,
                    Some(due),
                    SignedAmount::cr(amount),
                )],
            ),
        ],
    )
}

fn masters(api: &mut Api, bank_b: SignedAmount, customer_bills: bool) {
    let list = [
        ledger(
            CASH,
            "Cash",
            ReservedGroup::CashInHand,
            SignedAmount::dr(rs(1_000)),
            false,
        ),
        ledger(
            BANK_A,
            "Bank A",
            ReservedGroup::BankAccounts,
            SignedAmount::dr(rs(5_000)),
            false,
        ),
        ledger(BANK_B, "Bank B", ReservedGroup::BankAccounts, bank_b, false),
        ledger(
            CUSTOMER,
            "Customer",
            ReservedGroup::SundryDebtors,
            SignedAmount::dr(Paise::ZERO),
            customer_bills,
        ),
        ledger(
            SUPPLIER,
            "Supplier",
            ReservedGroup::SundryCreditors,
            SignedAmount::dr(Paise::ZERO),
            true,
        ),
        ledger(
            SALES,
            "Sales",
            ReservedGroup::SalesAccounts,
            SignedAmount::dr(Paise::ZERO),
            false,
        ),
        ledger(
            EXPENSE,
            "Expense",
            ReservedGroup::IndirectExpenses,
            SignedAmount::dr(Paise::ZERO),
            false,
        ),
    ];
    for l in list {
        committed(api.execute(Command::CreateLedger(l)));
    }
}

fn create(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

/// The shared company, as of 20 Nov 2026.
fn books(name: &str) -> Api {
    let mut api = create(name);
    masters(&mut api, SignedAmount::dr(rs(2_500)), true);
    let vouchers = [
        // Customer bills. Days overdue on 20 Nov 2026 are worked out below.
        // INV1 due 30 Sep: Sep 30 to Nov 20 = 31 (Oct) + 20 = 51 days, counted.
        sale(
            1,
            date!(2026 - 08 - 31),
            "INV1",
            Some(date!(2026 - 09 - 30)),
            10_000,
        ),
        // INV2 due 21 Oct: 10 (rest of Oct) + 20 = 30 days, not above 30.
        sale(
            2,
            date!(2026 - 09 - 05),
            "INV2",
            Some(date!(2026 - 10 - 21)),
            5_000,
        ),
        // INV3 due 20 Oct: 11 + 20 = 31 days, counted.
        sale(
            3,
            date!(2026 - 09 - 20),
            "INV3",
            Some(date!(2026 - 10 - 20)),
            7_000,
        ),
        // INV4 due 1 Dec: not yet due.
        sale(
            4,
            date!(2026 - 11 - 02),
            "INV4",
            Some(date!(2026 - 12 - 01)),
            3_000,
        ),
        // INV5 has no due date, so it is never aged.
        sale(5, date!(2026 - 11 - 15), "INV5", None, 2_000),
        // A sales return of 1,000 on account (not tied to a bill): Dr sales, Cr customer.
        journal(
            6,
            date!(2026 - 11 - 10),
            VoucherStatus::Regular,
            vec![
                line(SALES, SignedAmount::dr(rs(1_000)), Vec::new()),
                line(
                    CUSTOMER,
                    SignedAmount::cr(rs(1_000)),
                    vec![bill(
                        BillRefKind::OnAccount,
                        "",
                        None,
                        SignedAmount::cr(rs(1_000)),
                    )],
                ),
            ],
        ),
        // Supplier bills. B1 due 20 Nov: 0 days overdue, not past due. B2 due 19 Nov: 1 day.
        // B3 due 1 Nov: 19 days. B4 due 5 Dec: not due.
        purchase(7, date!(2026 - 10 - 20), "B1", date!(2026 - 11 - 20), 2_000),
        purchase(8, date!(2026 - 10 - 21), "B2", date!(2026 - 11 - 19), 4_000),
        purchase(9, date!(2026 - 10 - 02), "B3", date!(2026 - 11 - 01), 6_000),
        purchase(
            10,
            date!(2026 - 11 - 05),
            "B4",
            date!(2026 - 12 - 05),
            1_000,
        ),
        // An Optional voucher in October (never counts) and a post-dated one after the as-of
        // date (not yet in the books on 20 Nov).
        journal(
            11,
            date!(2026 - 10 - 12),
            VoucherStatus::Optional,
            vec![
                line(EXPENSE, SignedAmount::dr(rs(99_999)), Vec::new()),
                line(SALES, SignedAmount::cr(rs(99_999)), Vec::new()),
            ],
        ),
        journal(
            12,
            date!(2026 - 11 - 25),
            VoucherStatus::PostDated,
            vec![
                line(EXPENSE, SignedAmount::dr(rs(8_888)), Vec::new()),
                line(SALES, SignedAmount::cr(rs(8_888)), Vec::new()),
            ],
        ),
    ];
    for v in vouchers {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

const AS_OF: Date = date!(2026 - 11 - 20);

#[test]
fn cash_and_bank_is_one_figure_with_the_bank_count() {
    // provenance: derived. Cash 1,000 + Bank A 5,000 + Bank B 2,500 = 8,500; two ledgers sit
    // under Bank Accounts.
    let api = books("s9_home_cash.ledgerly");
    let figures = api.home_figures(AS_OF).unwrap();
    assert_eq!(
        figures.cash_and_bank,
        Outcome::Value(CashAndBank {
            total: rs(8_500),
            bank_accounts: 2
        })
    );
}

#[test]
fn an_overdrawn_bank_is_not_a_plain_figure() {
    // provenance: derived. Bank B opens with a credit of 7,500 (an overdraft) against Bank A's
    // debit of 5,000, so the group nets to 2,500 Cr, the side opposite to the usual one, and the
    // one-figure tile has no side marker: unsupported.
    let mut api = create("s9_home_overdraft.ledgerly");
    masters(&mut api, SignedAmount::cr(rs(7_500)), true);
    let figures = api.home_figures(AS_OF).unwrap();
    assert!(
        matches!(figures.cash_and_bank, Outcome::Unsupported(_)),
        "{:?}",
        figures.cash_and_bank
    );
}

#[test]
fn receivables_and_payables_are_the_group_balances() {
    // provenance: derived. Receivables: 10,000 + 5,000 + 7,000 + 3,000 + 2,000 = 27,000 of bills,
    // less the 1,000 return on account = 26,000 Dr. Payables: 2,000 + 4,000 + 6,000 + 1,000 =
    // 13,000 Cr.
    let api = books("s9_home_groups.ledgerly");
    let figures = api.home_figures(AS_OF).unwrap();
    assert_eq!(figures.receivables, Outcome::Value(rs(26_000)));
    assert_eq!(figures.payables, Outcome::Value(rs(13_000)));
}

#[test]
fn customer_bills_over_thirty_days_late_are_counted() {
    // provenance: derived. INV1 (51 days) and INV3 (31 days) are above 30; INV2 is exactly 30,
    // INV4 is not due and INV5 has no due date. Open: 10,000 + 7,000 = 17,000.
    let api = books("s9_home_late.ledgerly");
    let figures = api.home_figures(AS_OF).unwrap();
    assert_eq!(
        figures.receivables_overdue,
        Outcome::Value(LateBills {
            count: 2,
            open: rs(17_000)
        })
    );
}

#[test]
fn supplier_bills_past_due_are_counted() {
    // provenance: derived. B2 (1 day) and B3 (19 days) are past due; B1 is due on the as-of
    // date (0 days) and B4 is not due. Open: 4,000 + 6,000 = 10,000.
    let api = books("s9_home_past_due.ledgerly");
    let figures = api.home_figures(AS_OF).unwrap();
    assert_eq!(
        figures.payables_past_due,
        Outcome::Value(LateBills {
            count: 2,
            open: rs(10_000)
        })
    );
}

#[test]
fn sales_by_month_is_net_sales_before_gst_from_the_year_start() {
    // provenance: derived. Months run from April to the as-of month, November. Sales credits by
    // voucher date: Aug 10,000 (INV1); Sep 5,000 + 7,000 = 12,000; Nov 3,000 + 2,000 = 5,000
    // less the 1,000 return = 4,000. The Optional voucher (Oct, 99,999) is not in the books and
    // the post-dated one (25 Nov, 8,888) is after the as-of date. Other months are nil.
    let api = books("s9_home_sales.ledgerly");
    let figures = api.home_figures(AS_OF).unwrap();
    let months = match figures.sales_by_month {
        Outcome::Value(m) => m,
        other @ Outcome::Unsupported(_) => panic!("{other:?}"),
    };
    let expected = [
        (date!(2026 - 04 - 01), 0, false),
        (date!(2026 - 05 - 01), 0, false),
        (date!(2026 - 06 - 01), 0, false),
        (date!(2026 - 07 - 01), 0, false),
        (date!(2026 - 08 - 01), 10_000, false),
        (date!(2026 - 09 - 01), 12_000, false),
        (date!(2026 - 10 - 01), 0, false),
        (date!(2026 - 11 - 01), 4_000, true),
    ];
    let expected: Vec<MonthSales> = expected
        .iter()
        .map(|(month_start, rupees, to_date)| MonthSales {
            month_start: *month_start,
            sales: rs(*rupees),
            to_date: *to_date,
        })
        .collect();
    assert_eq!(months, expected);
}

#[test]
fn a_party_without_bill_tracking_makes_the_late_counts_unsupported() {
    // provenance: derived. The customer ledger does not track bills, so its sale cannot be aged:
    // a count over the supplier-only bills would leave it out. The balance is still a figure.
    let mut api = create("s9_home_untracked.ledgerly");
    masters(&mut api, SignedAmount::dr(rs(2_500)), false);
    committed(api.execute(Command::PostVoucher(journal(
        1,
        date!(2026 - 09 - 01),
        VoucherStatus::Regular,
        vec![
            line(CUSTOMER, SignedAmount::dr(rs(4_000)), Vec::new()),
            line(SALES, SignedAmount::cr(rs(4_000)), Vec::new()),
        ],
    ))));
    let figures = api.home_figures(AS_OF).unwrap();
    assert_eq!(figures.receivables, Outcome::Value(rs(4_000)));
    assert!(
        matches!(figures.receivables_overdue, Outcome::Unsupported(_)),
        "{:?}",
        figures.receivables_overdue
    );
}
