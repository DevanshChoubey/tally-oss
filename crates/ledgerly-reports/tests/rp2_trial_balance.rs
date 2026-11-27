//! Card RP2: trial balance tests. Hand-built books; every expected figure is derived in a comment.
//!
//! Chart used throughout (all names fictional):
//!
//! ```text
//! 1 Capital Account (primary)        ledger 10 Proprietor Capital
//! 2 Current Assets (primary)
//!   3 Cash-in-Hand                   ledger 11 Cash
//!   4 Sundry Debtors                 ledger 12 Customer One
//! 5 Sales Accounts (primary)         ledger 13 Sales
//! 6 Current Liabilities (primary)
//!   7 Duties & Taxes                 ledger 14 CGST, ledger 15 SGST
//! ```
//!
//! `rs(n)` is `n` rupees.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, GroupId, LedgerId, MoneyError, Outcome, Paise, Period, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, Posting, RestartPeriod, Scenario,
    VoucherStatus, VoucherType,
};
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind, trial_balance::trial_balance,
};
use proptest::prelude::*;
use time::macros::date;

const CAPITAL: u64 = 10;
const CASH: u64 = 11;
const DEBTOR: u64 = 12;
const SALES: u64 = 13;
const CGST: u64 = 14;
const SGST: u64 = 15;

const JOURNAL_TYPE: u64 = 1;
const MEMO_TYPE: u64 = 2;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.to_string(),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved: None,
        nature,
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, name: &str, group: u64, opening: SignedAmount) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_string(),
        aliases: vec![],
        group: Some(GroupId(group)),
        opening,
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

fn voucher_type(id: u64, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id: VoucherTypeId(id),
        name: name.to_string(),
        base,
        abbreviation: name.to_string(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Never,
            series: vec![],
        },
        active: true,
        tally: None,
    }
}

fn zero() -> SignedAmount {
    SignedAmount::dr(Paise::ZERO)
}

/// Books with the chart above. `capital` and `cash` are the opening balances.
fn books(capital: SignedAmount, cash: SignedAmount) -> Books {
    let mut masters = Masters::default();
    for g in [
        group(1, "Capital Account", None, Some(Nature::Liability)),
        group(2, "Current Assets", None, Some(Nature::Asset)),
        group(3, "Cash-in-Hand", Some(2), None),
        group(4, "Sundry Debtors", Some(2), None),
        group(5, "Sales Accounts", None, Some(Nature::Income)),
        group(6, "Current Liabilities", None, Some(Nature::Liability)),
        group(7, "Duties & Taxes", Some(6), None),
    ] {
        masters.groups.insert(g.id, g);
    }
    for l in [
        ledger(CAPITAL, "Proprietor Capital", 1, capital),
        ledger(CASH, "Cash", 3, cash),
        ledger(DEBTOR, "Customer One", 4, zero()),
        ledger(SALES, "Sales", 5, zero()),
        ledger(CGST, "CGST", 7, zero()),
        ledger(SGST, "SGST", 7, zero()),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(JOURNAL_TYPE, "Journal", BaseVoucherKind::Journal),
        voucher_type(MEMO_TYPE, "Memorandum", BaseVoucherKind::Memorandum),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Test Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: vec![],
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn post(
    voucher: u64,
    on: Date,
    ledger: u64,
    amount: SignedAmount,
    status: VoucherStatus,
    type_id: u64,
) -> Posting {
    Posting {
        voucher: VoucherId(voucher),
        date: on,
        ledger: LedgerId(ledger),
        amount,
        status,
        type_id: VoucherTypeId(type_id),
    }
}

fn regular(voucher: u64, on: Date, ledger: u64, amount: SignedAmount) -> Posting {
    post(
        voucher,
        on,
        ledger,
        amount,
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    )
}

#[expect(clippy::too_many_arguments, reason = "flat test helper")]
/// A two-line voucher: Dr `dr_ledger`, Cr `cr_ledger`, same amount, same status and type.
fn two_line(
    b: &mut Books,
    voucher: u64,
    on: Date,
    dr_ledger: u64,
    cr_ledger: u64,
    amount: Paise,
    status: VoucherStatus,
    type_id: u64,
) {
    b.postings.push(post(
        voucher,
        on,
        dr_ledger,
        SignedAmount::dr(amount),
        status,
        type_id,
    ));
    b.postings.push(post(
        voucher,
        on,
        cr_ledger,
        SignedAmount::cr(amount),
        status,
        type_id,
    ));
}

/// The sale of the card: 10,000 + CGST 900 + SGST 900 = 11,800 receivable.
fn add_sale(b: &mut Books) {
    let on = date!(2026 - 04 - 10);
    b.postings
        .push(regular(1, on, DEBTOR, SignedAmount::dr(rs(11_800))));
    b.postings
        .push(regular(1, on, SALES, SignedAmount::cr(rs(10_000))));
    b.postings
        .push(regular(1, on, CGST, SignedAmount::cr(rs(900))));
    b.postings
        .push(regular(1, on, SGST, SignedAmount::cr(rs(900))));
}

fn params(as_of: Date, detail: Detail) -> ReportParams {
    ReportParams {
        kind: ReportKind::TrialBalance,
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap(),
        as_of,
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn run(b: &Books, p: &ReportParams) -> Report {
    match trial_balance(b, p).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn dr_cr(cells: &[Cell]) -> (Option<Paise>, Option<Paise>) {
    let side = |c: &Cell| match c {
        Cell::Amount(a) => Some(a.amount),
        Cell::Empty => None,
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. }
        | Cell::Unsupported(_) => panic!("unexpected cell {c:?}"),
    };
    (side(&cells[0]), side(&cells[1]))
}

fn row<'a>(r: &'a Report, label: &str) -> &'a ReportRow {
    r.rows
        .iter()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("no row {label}"))
}

fn has_row(r: &Report, label: &str) -> bool {
    r.rows.iter().any(|x| x.label == label)
}

fn totals(r: &Report) -> (Paise, Paise) {
    let (dr, cr) = dr_cr(&r.totals);
    (dr.unwrap(), cr.unwrap())
}

fn unsupported(b: &Books, p: &ReportParams) -> String {
    match trial_balance(b, p).unwrap() {
        Outcome::Unsupported(u) => format!("{} / {}", u.what, u.reason),
        Outcome::Value(_) => panic!("expected Unsupported"),
    }
}

fn card_books() -> Books {
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    add_sale(&mut b);
    b
}

#[test]
fn card_books_trial_balance_totals_equal() {
    // provenance: derived — the card's scenario worked by hand.
    // Opening: Capital Cr 1,00,000; Cash Dr 1,00,000 (balanced).
    // Sale 11,800 incl. GST at 18%: taxable 10,000; GST 18% x 10,000 = 1,800 = CGST 900 + SGST 900;
    // 10,000 + 900 + 900 = 11,800 receivable from the customer.
    // Closing: Debtor Dr 11,800; Sales Cr 10,000; CGST Cr 900; SGST Cr 900.
    // Dr side: Cash 1,00,000 + Debtor 11,800 = 1,11,800.
    // Cr side: Capital 1,00,000 + Sales 10,000 + CGST 900 + SGST 900 = 1,11,800.
    let r = run(
        &card_books(),
        &params(date!(2026 - 04 - 30), Detail::Detailed),
    );

    assert_eq!(totals(&r), (rs(111_800), rs(111_800)));
    assert!(r.rows.iter().all(|x| x.kind != RowKind::Difference));

    assert_eq!(dr_cr(&row(&r, "Cash").cells), (Some(rs(100_000)), None));
    assert_eq!(
        dr_cr(&row(&r, "Customer One").cells),
        (Some(rs(11_800)), None)
    );
    assert_eq!(
        dr_cr(&row(&r, "Proprietor Capital").cells),
        (None, Some(rs(100_000)))
    );
    assert_eq!(dr_cr(&row(&r, "Sales").cells), (None, Some(rs(10_000))));
    assert_eq!(dr_cr(&row(&r, "CGST").cells), (None, Some(rs(900))));
    assert_eq!(dr_cr(&row(&r, "SGST").cells), (None, Some(rs(900))));
}

#[test]
fn group_rows_hold_the_net_of_their_ledgers() {
    // provenance: derived — group balance = sum of signed ledger balances (Dr positive).
    // Current Assets = Cash Dr 1,00,000 + Customer One Dr 11,800 = Dr 1,11,800.
    // Duties & Taxes = CGST Cr 900 + SGST Cr 900 = Cr 1,800; its parent Current Liabilities is Cr 1,800.
    let r = run(
        &card_books(),
        &params(date!(2026 - 04 - 30), Detail::Detailed),
    );

    assert_eq!(
        dr_cr(&row(&r, "Current Assets").cells),
        (Some(rs(111_800)), None)
    );
    assert_eq!(
        dr_cr(&row(&r, "Cash-in-Hand").cells),
        (Some(rs(100_000)), None)
    );
    assert_eq!(
        dr_cr(&row(&r, "Duties & Taxes").cells),
        (None, Some(rs(1_800)))
    );
    assert_eq!(
        dr_cr(&row(&r, "Current Liabilities").cells),
        (None, Some(rs(1_800)))
    );

    // Depth: primary 0, sub-group 1, ledger under a sub-group 2.
    assert_eq!(row(&r, "Current Assets").depth, 0);
    assert_eq!(row(&r, "Cash-in-Hand").depth, 1);
    assert_eq!(row(&r, "Cash").depth, 2);
    assert_eq!(
        row(&r, "Current Assets").key,
        Some(DrillKey::Group(GroupId(2)))
    );
    assert_eq!(row(&r, "Cash").key, Some(DrillKey::Ledger(LedgerId(CASH))));
    assert_eq!(row(&r, "Cash").kind, RowKind::Ledger);
    assert_eq!(row(&r, "Current Assets").kind, RowKind::Group);
}

#[test]
fn a_group_with_debit_and_credit_ledgers_shows_both_sides_and_totals_are_gross() {
    // provenance: derived — Tally's own Trial Balance screenshots in
    // <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Financial_Statements/Display_Trial_Balance.htm>
    // (image 3._Trial_Balance-1.gif, read 2026-11-21): the Current Liabilities group row shows a
    // Debit amount (5,45,257.50) and a Credit amount (2,38,27,223.75) on the same row, and the
    // Grand Total adds both columns (credits 24,00,000 + 12,00,000 + 2,38,27,223.75 + 8,600 +
    // 3,90,55,008.75 + 2,26,300 = 6,67,17,132.50, the Grand Total shown). In the detailed image
    // (3._Trial_Balance-7.gif) every sub-group of Current Liabilities is Cr, so the debit side
    // comes from ledgers, not sub-groups: a group's columns are its debit-balance ledgers and its
    // credit-balance ledgers.
    // Here: an advance of 5,000 from Customer One into Cash. Cash Dr 1,05,000; Customer One
    // Cr 5,000 (both in Current Assets); Capital Cr 1,00,000.
    // Current Assets row: Dr 1,05,000 and Cr 5,000. Totals: Dr 1,05,000; Cr 5,000 + 1,00,000 =
    // 1,05,000. (Netting the group would give Dr 1,00,000 and totals of 1,00,000.)
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    two_line(
        &mut b,
        1,
        date!(2026 - 04 - 10),
        CASH,
        DEBTOR,
        rs(5_000),
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    );
    for detail in [Detail::Detailed, Detail::Condensed] {
        let r = run(&b, &params(date!(2026 - 04 - 30), detail));
        assert_eq!(
            dr_cr(&row(&r, "Current Assets").cells),
            (Some(rs(105_000)), Some(rs(5_000)))
        );
        assert_eq!(totals(&r), (rs(105_000), rs(105_000)));
    }
}

#[test]
fn condensed_lists_primary_groups_only() {
    // provenance: derived — ReportParams docs: Condensed = primary groups only.
    // Primary groups with a non-zero balance: Capital Account, Current Assets, Sales Accounts,
    // Current Liabilities. Totals equal the detailed report: 1,11,800 each side.
    let r = run(
        &card_books(),
        &params(date!(2026 - 04 - 30), Detail::Condensed),
    );

    let labels: Vec<&str> = r.rows.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Capital Account",
            "Current Assets",
            "Sales Accounts",
            "Current Liabilities"
        ]
    );
    assert!(
        r.rows
            .iter()
            .all(|x| x.depth == 0 && x.kind == RowKind::Group)
    );
    assert_eq!(totals(&r), (rs(111_800), rs(111_800)));
}

#[test]
fn unbalanced_opening_shows_explicit_difference_row() {
    // provenance: derived — spec invariant 4: an unbalanced opening trial balance is allowed and
    // the difference is an explicit line.
    // Opening: Capital Cr 1,00,000; Cash Dr 90,000. Cr exceeds Dr by 10,000, so the difference line
    // is a Dr of 10,000 and both columns total 1,00,000.
    let b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(90_000)));
    let r = run(&b, &params(date!(2026 - 04 - 01), Detail::Detailed));

    let diff = row(&r, "Difference in opening balances");
    assert_eq!(diff.kind, RowKind::Difference);
    assert_eq!(diff.key, None);
    assert_eq!(dr_cr(&diff.cells), (Some(rs(10_000)), None));
    assert_eq!(totals(&r), (rs(100_000), rs(100_000)));
}

#[test]
fn opening_difference_on_credit_side_when_debits_exceed() {
    // provenance: derived — mirror of the case above: Capital Cr 80,000; Cash Dr 1,00,000.
    // Dr exceeds Cr by 20,000, so the difference line is a Cr of 20,000; totals 1,00,000 each.
    let b = books(SignedAmount::cr(rs(80_000)), SignedAmount::dr(rs(100_000)));
    let r = run(&b, &params(date!(2026 - 04 - 01), Detail::Detailed));

    let diff = row(&r, "Difference in opening balances");
    assert_eq!(dr_cr(&diff.cells), (None, Some(rs(20_000))));
    assert_eq!(totals(&r), (rs(100_000), rs(100_000)));
}

#[test]
fn difference_row_stays_after_activity() {
    // provenance: derived — the sale (balanced by itself) does not change the opening difference.
    // Opening Cr 1,00,000 vs Dr 90,000: difference Dr 10,000. After the sale of 11,800:
    // Dr = 90,000 + 11,800 + 10,000 (difference) = 1,11,800; Cr = 1,00,000 + 11,800 = 1,11,800.
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(90_000)));
    add_sale(&mut b);
    let r = run(&b, &params(date!(2026 - 04 - 30), Detail::Detailed));

    let diff = row(&r, "Difference in opening balances");
    assert_eq!(dr_cr(&diff.cells), (Some(rs(10_000)), None));
    assert_eq!(totals(&r), (rs(111_800), rs(111_800)));
}

#[test]
fn balanced_opening_has_no_difference_row() {
    // provenance: derived — Cr 1,00,000 = Dr 1,00,000, so there is nothing to show.
    let b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    let r = run(&b, &params(date!(2026 - 04 - 01), Detail::Detailed));
    assert!(r.rows.iter().all(|x| x.kind != RowKind::Difference));
    assert_eq!(totals(&r), (rs(100_000), rs(100_000)));
}

#[test]
fn as_of_date_cuts_off_later_postings() {
    // provenance: derived — a balance as of 5 April includes only postings dated on or before it.
    // The sale is on 10 April, so at 5 April only the opening balances remain; on 10 April it counts.
    let b = card_books();
    let before = run(&b, &params(date!(2026 - 04 - 05), Detail::Detailed));
    assert_eq!(totals(&before), (rs(100_000), rs(100_000)));
    assert!(!has_row(&before, "Customer One"));
    let on_day = run(&b, &params(date!(2026 - 04 - 10), Detail::Detailed));
    assert_eq!(totals(&on_day), (rs(111_800), rs(111_800)));
}

#[test]
fn zero_balance_ledgers_are_not_listed_and_do_not_change_totals() {
    // provenance: derived — Cash opens Dr 500 and is paid out Cr 500: closing 0, so no Cash row.
    // Customer One Dr 500; Capital opens Cr 500 so the opening is balanced.
    // Closing: Customer One Dr 500, Capital Cr 500, Cash 0, so both columns total 500.
    let mut b = books(SignedAmount::cr(rs(500)), SignedAmount::dr(rs(500)));
    two_line(
        &mut b,
        1,
        date!(2026 - 04 - 02),
        DEBTOR,
        CASH,
        rs(500),
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    );
    let r = run(&b, &params(date!(2026 - 04 - 30), Detail::Detailed));
    assert!(!has_row(&r, "Cash") && !has_row(&r, "Cash-in-Hand"));
    assert_eq!(totals(&r), (rs(500), rs(500)));
}

#[test]
fn scenario_actual_excludes_optional_cancelled_and_memorandum() {
    // provenance: derived — spec invariant 3: optional, memorandum and (future) post-dated vouchers
    // never affect base balances; VoucherStatus::Cancelled has "no amounts".
    // Only the regular sale counts: totals stay 1,11,800 whatever else is posted.
    let mut b = card_books();
    let on = date!(2026 - 04 - 11);
    two_line(
        &mut b,
        2,
        on,
        CASH,
        SALES,
        rs(700),
        VoucherStatus::Optional,
        JOURNAL_TYPE,
    );
    two_line(
        &mut b,
        3,
        on,
        CASH,
        SALES,
        rs(300),
        VoucherStatus::Cancelled,
        JOURNAL_TYPE,
    );
    two_line(
        &mut b,
        4,
        on,
        CASH,
        SALES,
        rs(200),
        VoucherStatus::Regular,
        MEMO_TYPE,
    );

    let r = run(&b, &params(date!(2026 - 04 - 30), Detail::Detailed));
    assert_eq!(totals(&r), (rs(111_800), rs(111_800)));
    assert_eq!(dr_cr(&row(&r, "Cash").cells), (Some(rs(100_000)), None));
    assert_eq!(dr_cr(&row(&r, "Sales").cells), (None, Some(rs(10_000))));
}

#[test]
fn scenario_including_optional_adds_optional_vouchers() {
    // provenance: derived — Scenario::Including(statuses) = Actual plus those statuses (model docs).
    // Optional voucher Dr Cash 700 / Cr Sales 700 is added: Cash Dr 1,00,700; Sales Cr 10,700;
    // totals 1,11,800 + 700 = 1,12,500 each side. The cancelled voucher stays out.
    let mut b = card_books();
    let on = date!(2026 - 04 - 11);
    two_line(
        &mut b,
        2,
        on,
        CASH,
        SALES,
        rs(700),
        VoucherStatus::Optional,
        JOURNAL_TYPE,
    );
    two_line(
        &mut b,
        3,
        on,
        CASH,
        SALES,
        rs(300),
        VoucherStatus::Cancelled,
        JOURNAL_TYPE,
    );

    let mut p = params(date!(2026 - 04 - 30), Detail::Detailed);
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let r = run(&b, &p);
    assert_eq!(dr_cr(&row(&r, "Cash").cells), (Some(rs(100_700)), None));
    assert_eq!(dr_cr(&row(&r, "Sales").cells), (None, Some(rs(10_700))));
    assert_eq!(totals(&r), (rs(112_500), rs(112_500)));
    // The report declares its scenario.
    assert!(
        r.notes.iter().any(|n| n.contains("Optional")),
        "notes: {:?}",
        r.notes
    );
}

#[test]
fn actual_scenario_is_declared_in_notes() {
    // provenance: derived — spec 6.2 / invariant 3: reports declare their scenario.
    let r = run(
        &card_books(),
        &params(date!(2026 - 04 - 30), Detail::Detailed),
    );
    assert!(
        r.notes.iter().any(|n| n.contains("Actual")),
        "notes: {:?}",
        r.notes
    );
}

#[test]
fn post_dated_voucher_counts_only_once_its_date_is_reached() {
    // provenance: derived — model docs: "Post-dated: enters the books on its date".
    // Post-dated Dr Cash 400 / Cr Sales 400 on 20 April. As of 15 April it is out; as of 20 April in.
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    two_line(
        &mut b,
        5,
        date!(2026 - 04 - 20),
        CASH,
        SALES,
        rs(400),
        VoucherStatus::PostDated,
        JOURNAL_TYPE,
    );

    let early = run(&b, &params(date!(2026 - 04 - 15), Detail::Detailed));
    assert_eq!(dr_cr(&row(&early, "Cash").cells), (Some(rs(100_000)), None));
    let reached = run(&b, &params(date!(2026 - 04 - 20), Detail::Detailed));
    assert_eq!(
        dr_cr(&row(&reached, "Cash").cells),
        (Some(rs(100_400)), None)
    );
    assert_eq!(totals(&reached), (rs(100_400), rs(100_400)));
}

#[test]
fn including_scenario_with_memorandum_postings_is_unsupported() {
    // provenance: derived — no rule says whether a named scenario brings in memorandum vouchers,
    // so the report must not guess (rules A1, A5): it is Unsupported, not a number.
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    two_line(
        &mut b,
        4,
        date!(2026 - 04 - 11),
        CASH,
        SALES,
        rs(200),
        VoucherStatus::Regular,
        MEMO_TYPE,
    );
    let mut p = params(date!(2026 - 04 - 30), Detail::Detailed);
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    assert!(unsupported(&b, &p).contains("Memorandum"));
}

#[test]
fn comparative_and_monthly_columns_are_unsupported() {
    // provenance: derived — this card renders the single closing-balance pair only.
    let b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    let mut p = params(date!(2026 - 04 - 30), Detail::Detailed);
    p.columns = ColumnSpec::Monthly;
    assert!(unsupported(&b, &p).contains("columns"));
    p.columns = ColumnSpec::Comparative(vec![
        Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap(),
    ]);
    assert!(unsupported(&b, &p).contains("columns"));
}

#[test]
fn filters_and_exceptions_only_are_unsupported() {
    // provenance: derived — a filtered trial balance would not balance, and exceptions have no
    // definition for this report yet, so neither may produce a figure.
    let b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    let base = params(date!(2026 - 04 - 30), Detail::Detailed);

    let mut p = base.clone();
    p.filter.ledgers = vec![LedgerId(CASH)];
    assert!(unsupported(&b, &p).contains("filter"));

    let mut p = base.clone();
    p.filter.groups = vec![GroupId(2)];
    assert!(unsupported(&b, &p).contains("filter"));

    let mut p = base.clone();
    p.filter.voucher_types = vec![VoucherTypeId(1)];
    assert!(unsupported(&b, &p).contains("filter"));

    let mut p = base.clone();
    p.filter.items = vec![ledgerly_core::StockItemId(1)];
    assert!(unsupported(&b, &p).contains("filter"));

    let mut p = base;
    p.exceptions_only = true;
    assert!(unsupported(&b, &p).contains("exceptions"));
}

#[test]
fn wrong_report_kind_is_an_error() {
    // provenance: derived — calling the TB engine for another report is a caller bug, not a figure.
    let b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    let mut p = params(date!(2026 - 04 - 30), Detail::Detailed);
    p.kind = ReportKind::DayBook;
    assert!(matches!(
        trial_balance(&b, &p),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn as_of_before_books_begin_is_an_error() {
    // provenance: derived — opening balances are as at books_begin (invariant 6), so a balance
    // before that date does not exist.
    let b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    let p = params(date!(2026 - 03 - 31), Detail::Detailed);
    assert!(matches!(
        trial_balance(&b, &p),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn posting_to_unknown_ledger_is_not_found() {
    // provenance: derived — a posting whose ledger is not in the masters cannot be placed under a
    // group; showing the others would silently drop an amount.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    two_line(
        &mut b,
        1,
        date!(2026 - 04 - 02),
        999,
        CASH,
        rs(5),
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::NotFound { .. })
    ));
}

#[test]
fn posting_with_unknown_voucher_type_is_not_found() {
    // provenance: derived — memorandum status depends on the voucher type, which cannot be read.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    two_line(
        &mut b,
        1,
        date!(2026 - 04 - 02),
        CASH,
        SALES,
        rs(5),
        VoucherStatus::Regular,
        77,
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::NotFound { .. })
    ));
}

#[test]
fn unbalanced_postings_are_an_error_not_a_difference() {
    // provenance: derived — invariant 1 (sum Dr = sum Cr per voucher). Postings that do not balance
    // mean corrupt input; the opening-difference line must never absorb them.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    b.postings.push(regular(
        1,
        date!(2026 - 04 - 02),
        CASH,
        SignedAmount::dr(rs(5)),
    ));
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn posting_before_books_begin_is_an_error() {
    // provenance: derived — invariant 6: voucher date >= books_begin.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    two_line(
        &mut b,
        1,
        date!(2026 - 03 - 15),
        CASH,
        SALES,
        rs(5),
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn ledger_in_missing_group_is_an_error() {
    // provenance: derived — a ledger whose group is missing from the masters has no place in the
    // report; dropping it would make the totals wrong.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    b.masters.ledgers.insert(
        LedgerId(50),
        ledger(50, "Orphan", 999, SignedAmount::dr(rs(1))),
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn group_cycle_is_an_error() {
    // provenance: derived — groups 20 and 21 are each other's parent; a ledger in 20 cannot be
    // reached from any primary group.
    let mut b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    b.masters
        .groups
        .insert(GroupId(20), group(20, "Loop A", Some(21), None));
    b.masters
        .groups
        .insert(GroupId(21), group(21, "Loop B", Some(20), None));
    b.masters.ledgers.insert(
        LedgerId(51),
        ledger(51, "Looped", 20, SignedAmount::dr(rs(1))),
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 30), Detail::Detailed)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn overflow_is_a_typed_error() {
    // provenance: derived — two debits of i64::MAX paise cannot be summed (checked arithmetic, A3).
    let b = books(
        SignedAmount::dr(Paise::new(i64::MAX)),
        SignedAmount::dr(Paise::new(i64::MAX)),
    );
    assert_eq!(
        trial_balance(&b, &params(date!(2026 - 04 - 01), Detail::Detailed)),
        Err(ReportError::Money(MoneyError::Overflow))
    );
}

#[test]
fn negative_stored_amount_is_an_error() {
    // provenance: derived — SignedAmount docs: the amount is never negative for a well-formed value.
    let b = books(
        SignedAmount::cr(Paise::new(-5)),
        SignedAmount::dr(Paise::ZERO),
    );
    assert!(matches!(
        trial_balance(&b, &params(date!(2026 - 04 - 01), Detail::Detailed)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn report_metadata() {
    // provenance: derived — two columns, Debit then Credit; one total cell per column.
    let b = books(SignedAmount::cr(rs(100)), SignedAmount::dr(rs(100)));
    let p = params(date!(2026 - 04 - 30), Detail::Detailed);
    let r = run(&b, &p);
    let labels: Vec<&str> = r.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Debit", "Credit"]);
    assert_eq!(r.totals.len(), 2);
    assert_eq!(r.title, "Trial Balance");
    assert_eq!(r.params, p);
}

#[test]
fn deterministic_output() {
    // provenance: derived — rule A7: same input, same output.
    let b = card_books();
    let p = params(date!(2026 - 04 - 30), Detail::Detailed);
    assert_eq!(trial_balance(&b, &p), trial_balance(&b, &p));
}

const LEDGERS: [u64; 6] = [CAPITAL, CASH, DEBTOR, SALES, CGST, SGST];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // provenance: derived — spec invariants 1 and 4: every voucher balances, and the opening
    // difference is an explicit line, so for any openings and any balanced vouchers the two
    // columns total the same.
    #[test]
    fn totals_always_equal(
        openings in proptest::collection::vec((0usize..6, any::<bool>(), 0i64..1_000_000), 0..8),
        vouchers in proptest::collection::vec((0usize..6, 0usize..6, 1i64..1_000_000, 1u8..28), 0..12),
    ) {
        let mut b = books(SignedAmount::dr(Paise::ZERO), SignedAmount::dr(Paise::ZERO));
        for (idx, is_dr, amt) in &openings {
            let id = LedgerId(LEDGERS[*idx]);
            let amount = Paise::new(*amt);
            if let Some(l) = b.masters.ledgers.get_mut(&id) {
                l.opening = if *is_dr { SignedAmount::dr(amount) } else { SignedAmount::cr(amount) };
            }
        }
        for (n, (a, c, amt, day)) in vouchers.iter().enumerate() {
            let on = Date::from_calendar_date(2026, time::Month::April, *day).unwrap();
            let v = u64::try_from(n).unwrap() + 1;
            two_line(&mut b, v, on, LEDGERS[*a], LEDGERS[*c], Paise::new(*amt),
                VoucherStatus::Regular, JOURNAL_TYPE);
        }
        for detail in [Detail::Condensed, Detail::Detailed] {
            let r = run(&b, &params(date!(2026 - 04 - 30), detail));
            let (dr, cr) = totals(&r);
            prop_assert_eq!(dr, cr);
        }
    }
}
