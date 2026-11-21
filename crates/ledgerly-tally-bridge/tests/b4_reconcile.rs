//! Card B4: reconciliation report tests.
//!
//! Provenance. Every figure is `derived` by hand from the scenario below; none is taken from our
//! engine's output. The Tally side is hand-built `TReportRow`s (and one recalled-layout XML
//! document), NOT an observed capture: the Tally export layout is `pending` an `observed`
//! capture (plan Prerequisite 4), so these tests say nothing about what TallyPrime writes.
//!
//! Scenario (all names fictional), the card's sale on 10-Apr-2026:
//!
//! ```text
//! opening: Proprietor Capital Cr 1,00,000; Cash Dr 1,00,000
//! sale 10,000 + CGST 900 + SGST 900 = 11,800 receivable (18% of 10,000 = 1,800 = 900 + 900)
//! closing Dr: Cash 1,00,000 + Customer One 11,800            = 1,11,800
//! closing Cr: Capital 1,00,000 + Sales 10,000 + CGST 900 + SGST 900 = 1,11,800
//! ```
//!
//! Chart: 1 Capital Account (primary, ledger 10), 2 Current Assets (primary) with 3 Cash-in-Hand
//! (ledger 11) and 4 Sundry Debtors (ledger 12), 5 Sales Accounts (primary, ledger 13),
//! 6 Current Liabilities (primary) with 7 Duties & Taxes (ledgers 14 CGST, 15 SGST).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, GroupId, LedgerId, Paise, Period, SignedAmount, StateCode, TallyRef,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, Posting, RestartPeriod, VoucherStatus,
    VoucherType,
};
use ledgerly_tally_bridge::reconcile::{
    LineKind, LineStatus, ReconAmount, ReconLine, SectionStatus, reconcile,
};
use ledgerly_tally_xml::reports::{
    TReportAmount, TReportRow, TallyReportKind, TallyReports, parse_report_xml,
};
use time::macros::date;

const CAPITAL: u64 = 10;
const CASH: u64 = 11;
const DEBTOR: u64 = 12;
const SALES: u64 = 13;
const CGST: u64 = 14;
const SGST: u64 = 15;
const JOURNAL_TYPE: u64 = 1;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn amt(debit: i64, credit: i64) -> ReconAmount {
    ReconAmount {
        debit: rs(debit),
        credit: rs(credit),
    }
}

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>) -> Group {
    Group {
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
        id: LedgerId(id),
        name: name.to_string(),
        aliases: vec![],
        group: GroupId(group),
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

fn journal_type() -> VoucherType {
    VoucherType {
        id: VoucherTypeId(JOURNAL_TYPE),
        name: "Journal".to_string(),
        base: BaseVoucherKind::Journal,
        abbreviation: "Jrnl".to_string(),
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

fn regular(voucher: u64, on: Date, ledger: u64, amount: SignedAmount) -> Posting {
    Posting {
        voucher: VoucherId(voucher),
        date: on,
        ledger: LedgerId(ledger),
        amount,
        status: VoucherStatus::Regular,
        type_id: VoucherTypeId(JOURNAL_TYPE),
    }
}

/// Books with the chart above and the given opening balances of Capital and Cash.
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
    masters
        .voucher_types
        .insert(VoucherTypeId(JOURNAL_TYPE), journal_type());
    Books {
        company: Company {
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

/// The card's books: opening balanced at 1,00,000 and the 11,800 sale.
fn card_books() -> Books {
    let mut b = books(SignedAmount::cr(rs(100_000)), SignedAmount::dr(rs(100_000)));
    let on = date!(2026 - 04 - 10);
    b.postings
        .push(regular(1, on, DEBTOR, SignedAmount::dr(rs(11_800))));
    b.postings
        .push(regular(1, on, SALES, SignedAmount::cr(rs(10_000))));
    b.postings
        .push(regular(1, on, CGST, SignedAmount::cr(rs(900))));
    b.postings
        .push(regular(1, on, SGST, SignedAmount::cr(rs(900))));
    b
}

fn period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap()
}

fn row(label: &str, debit: i64, credit: i64) -> TReportRow {
    TReportRow {
        label: label.to_string(),
        guid: None,
        party: None,
        date_text: None,
        is_total: false,
        amount: TReportAmount::Columns {
            debit: rs(debit),
            credit: rs(credit),
        },
    }
}

fn total(debit: i64, credit: i64) -> TReportRow {
    TReportRow {
        is_total: true,
        ..row("Grand Total", debit, credit)
    }
}

/// Tally's trial balance for the card's books, written from the derivation above.
fn tally_tb() -> Vec<TReportRow> {
    vec![
        row("Cash", 100_000, 0),
        row("Customer One", 11_800, 0),
        row("Proprietor Capital", 0, 100_000),
        row("Sales", 0, 10_000),
        row("CGST", 0, 900),
        row("SGST", 0, 900),
        total(111_800, 111_800),
    ]
}

fn reports(tb: Vec<TReportRow>) -> TallyReports {
    TallyReports {
        tb,
        ..TallyReports::default()
    }
}

fn find<'a>(lines: &'a [ReconLine], label: &str) -> &'a ReconLine {
    lines
        .iter()
        .find(|l| l.label == label)
        .unwrap_or_else(|| panic!("no line {label}"))
}

#[test]
fn equal_trial_balance_is_all_green() {
    // provenance: derived — the scenario in the module comment; every Tally row equals the
    // hand-derived closing balance, and the totals are 1,11,800 on each side.
    let r = reconcile(&card_books(), &reports(tally_tb()), period()).unwrap();

    assert_eq!(r.trial_balance.status, SectionStatus::Compared);
    assert!(r.is_green());
    assert!(r.mismatches().is_empty());
    // Matching rows are listed too: 6 ledgers + the grand total.
    let lines = &r.trial_balance.lines;
    assert_eq!(lines.len(), 7);
    assert!(lines.iter().all(|l| l.status == LineStatus::Match));
    let cash = find(lines, "Cash");
    assert_eq!(cash.kind, LineKind::Ledger);
    assert_eq!(cash.ours, Some(amt(100_000, 0)));
    assert_eq!(cash.theirs, Some(amt(100_000, 0)));
    let t = find(lines, "Grand Total");
    assert_eq!(t.kind, LineKind::Total);
    assert_eq!(t.ours, Some(amt(111_800, 111_800)));
}

#[test]
fn a_one_rupee_difference_is_listed_with_both_values() {
    // provenance: derived — ours Customer One is Dr 11,800.00 = 1,180,000 paise; Tally's row says
    // 11,801.00 = 1,180,100 paise. ours - theirs = 1,180,000 - 1,180,100 = -100 paise.
    let mut tb = tally_tb();
    tb[1] = row("Customer One", 11_801, 0);
    tb.pop(); // no totals row in this export
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    assert!(!r.is_green());
    let bad = r.mismatches();
    assert_eq!(bad.len(), 1);
    let l = bad[0];
    assert_eq!(l.label, "Customer One");
    assert_eq!(l.ours, Some(amt(11_800, 0)));
    assert_eq!(l.theirs, Some(amt(11_801, 0)));
    assert_eq!(
        l.status,
        LineStatus::Differs {
            debit_difference: Paise::new(-100),
            credit_difference: Paise::ZERO,
        }
    );
    // The other five rows are still listed, as matches.
    assert_eq!(r.trial_balance.lines.len(), 6);
}

#[test]
fn a_totals_difference_is_listed() {
    // provenance: derived — ours credit total 1,11,800.00; Tally's 1,11,801.00; difference
    // -100 paise on the credit side, none on the debit side.
    let mut tb = tally_tb();
    tb.pop();
    tb.push(total(111_800, 111_801));
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let l = find(&r.trial_balance.lines, "Grand Total");
    assert_eq!(
        l.status,
        LineStatus::Differs {
            debit_difference: Paise::ZERO,
            credit_difference: Paise::new(-100),
        }
    );
    assert!(!r.is_green());
}

#[test]
fn a_tally_row_with_no_master_is_listed_and_not_green() {
    // provenance: derived — "Ghost Ledger" is not a name in the chart above.
    let mut tb = tally_tb();
    tb.insert(0, row("Ghost Ledger", 5, 0));
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let l = find(&r.trial_balance.lines, "Ghost Ledger");
    assert_eq!(l.status, LineStatus::NotInOurBooks);
    assert_eq!(l.kind, LineKind::Unmatched);
    assert_eq!(l.ours, None);
    assert_eq!(l.theirs, Some(amt(5, 0)));
    assert!(!r.is_green());
}

#[test]
fn a_ledger_of_ours_that_tally_omits_is_listed() {
    // provenance: derived — SGST closes at Cr 900 (see the scenario); Tally's export leaves it
    // out and has no group row above it.
    let mut tb = tally_tb();
    tb.retain(|r| r.label != "SGST");
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let l = find(&r.trial_balance.lines, "SGST");
    assert_eq!(l.status, LineStatus::OnlyInOurs);
    assert_eq!(l.ours, Some(amt(0, 900)));
    assert_eq!(l.theirs, None);
    assert!(!r.is_green());
}

#[test]
fn a_group_row_accounts_for_the_ledgers_under_it() {
    // provenance: derived — Duties & Taxes holds CGST Cr 900 + SGST Cr 900 = Cr 1,800. A Tally
    // export that lists the group instead of its two ledgers is complete and equal.
    let mut tb = tally_tb();
    tb.retain(|r| r.label != "CGST" && r.label != "SGST");
    tb.insert(4, row("Duties & Taxes", 0, 1_800));
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let g = find(&r.trial_balance.lines, "Duties & Taxes");
    assert_eq!(g.kind, LineKind::Group);
    assert_eq!(g.ours, Some(amt(0, 1_800)));
    assert_eq!(g.status, LineStatus::Match);
    assert!(r.is_green());
}

#[test]
fn a_tally_amount_without_a_side_is_unreadable() {
    // provenance: derived — a trial balance row carrying one signed amount has no column that
    // names its side (X5: sign kept as written), so the side is not guessed.
    let mut tb = tally_tb();
    tb[0].amount = TReportAmount::Signed(rs(-100_000));
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let l = find(&r.trial_balance.lines, "Cash");
    assert!(matches!(l.status, LineStatus::Unreadable { .. }));
    assert_eq!(l.ours, Some(amt(100_000, 0)));
    assert_eq!(l.theirs, None);
    assert!(!r.is_green());
}

#[test]
fn a_name_matching_a_group_and_a_ledger_is_ambiguous() {
    // provenance: derived — with a group also named "Cash", the label "Cash" names two masters.
    let mut b = card_books();
    let g = group(8, "Cash", Some(2), None);
    b.masters.groups.insert(g.id, g);
    let r = reconcile(&b, &reports(tally_tb()), period()).unwrap();

    let l = find(&r.trial_balance.lines, "Cash");
    assert!(matches!(l.status, LineStatus::Ambiguous { .. }));
    assert!(!r.is_green());
}

#[test]
fn a_master_listed_twice_by_tally_is_ambiguous() {
    // provenance: derived — two Tally rows both resolve to ledger Cash.
    let mut tb = tally_tb();
    tb.insert(1, row("Cash", 100_000, 0));
    let r = reconcile(&card_books(), &reports(tb), period()).unwrap();

    let cash: Vec<&ReconLine> = r
        .trial_balance
        .lines
        .iter()
        .filter(|l| l.label == "Cash")
        .collect();
    assert_eq!(cash.len(), 2);
    assert!(
        cash.iter()
            .all(|l| matches!(l.status, LineStatus::Ambiguous { .. }))
    );
    assert!(!r.is_green());
}

#[test]
fn guid_match_beats_a_different_name() {
    // provenance: derived — the ledger carries Tally GUID g-cash; Tally's row names it
    // "Cash (renamed in Tally)" with the same GUID, so it is the same ledger.
    let mut b = card_books();
    if let Some(l) = b.masters.ledgers.get_mut(&LedgerId(CASH)) {
        l.tally = Some(TallyRef {
            guid: "g-cash".to_string(),
            alter_id: None,
        });
    }
    let mut tb = tally_tb();
    tb[0] = TReportRow {
        guid: Some("g-cash".to_string()),
        ..row("Cash (renamed in Tally)", 100_000, 0)
    };
    let r = reconcile(&b, &reports(tb), period()).unwrap();

    let l = find(&r.trial_balance.lines, "Cash (renamed in Tally)");
    assert_eq!(l.kind, LineKind::Ledger);
    assert_eq!(l.status, LineStatus::Match);
    assert!(r.is_green());
}

#[test]
fn unequal_opening_balances_reconcile_through_the_difference_row() {
    // provenance: derived — Capital Cr 90,000 and Cash Dr 1,00,000 open unequal; opening debits
    // exceed credits by 10,000, so the explicit difference line is a Cr 10,000 (spec invariant 4)
    // and both columns total 1,00,000.
    let b = books(SignedAmount::cr(rs(90_000)), SignedAmount::dr(rs(100_000)));
    let with_diff = vec![
        row("Cash", 100_000, 0),
        row("Proprietor Capital", 0, 90_000),
        row("Difference in opening balances", 0, 10_000),
        total(100_000, 100_000),
    ];
    let r = reconcile(&b, &reports(with_diff), period()).unwrap();
    let d = find(&r.trial_balance.lines, "Difference in opening balances");
    assert_eq!(d.kind, LineKind::OpeningDifference);
    assert_eq!(d.status, LineStatus::Match);
    assert!(r.is_green());

    // Tally's export without that line leaves our difference unaccounted for.
    let without = vec![
        row("Cash", 100_000, 0),
        row("Proprietor Capital", 0, 90_000),
    ];
    let r = reconcile(&b, &reports(without), period()).unwrap();
    let d = find(&r.trial_balance.lines, "Difference in opening balances");
    assert_eq!(d.status, LineStatus::OnlyInOurs);
    assert_eq!(d.ours, Some(amt(0, 10_000)));
    assert!(!r.is_green());
}

#[test]
fn exports_that_are_not_compared_are_unsupported_and_block_green() {
    // provenance: derived — the balance sheet rows are supplied, but their amounts are signed
    // with an unestablished side and our balance sheet needs valuations: nothing is compared,
    // and the report says so instead of passing.
    let mut rep = reports(tally_tb());
    rep.bs = vec![TReportRow {
        amount: TReportAmount::Signed(rs(100_000)),
        ..row("Cash", 0, 0)
    }];
    let r = reconcile(&card_books(), &rep, period()).unwrap();

    assert_eq!(r.trial_balance.status, SectionStatus::Compared);
    assert!(matches!(
        r.balance_sheet.status,
        SectionStatus::Unsupported(_)
    ));
    assert!(r.balance_sheet.lines.is_empty());
    assert_eq!(r.profit_loss.status, SectionStatus::NotSupplied);
    assert_eq!(r.stock.status, SectionStatus::NotSupplied);
    assert_eq!(r.outstanding.status, SectionStatus::NotSupplied);
    assert!(!r.is_green());
}

#[test]
fn nothing_supplied_is_not_green() {
    // provenance: derived — with no Tally trial balance nothing was compared, so "green" would
    // claim agreement nobody checked.
    let r = reconcile(&card_books(), &TallyReports::default(), period()).unwrap();
    assert_eq!(r.trial_balance.status, SectionStatus::NotSupplied);
    assert!(!r.is_green());
}

#[test]
fn a_failure_on_our_side_is_reported_not_hidden() {
    // provenance: derived — a balanced voucher dated 31-Mar-2026 precedes books_begin
    // (1-Apr-2026); the trial balance engine refuses it, so nothing may be compared.
    let mut b = card_books();
    let on = date!(2026 - 03 - 31);
    b.postings
        .push(regular(2, on, CASH, SignedAmount::dr(rs(10))));
    b.postings
        .push(regular(2, on, CAPITAL, SignedAmount::cr(rs(10))));
    let r = reconcile(&b, &reports(tally_tb()), period()).unwrap();

    assert!(matches!(
        r.trial_balance.status,
        SectionStatus::Failed { .. }
    ));
    assert!(r.trial_balance.lines.is_empty());
    assert!(!r.is_green());
}

#[test]
fn parsed_tally_xml_reconciles_green() {
    // provenance: derived — the XML is hand-built in the recalled layout (X5), debit written
    // negative (R8 2.3(e)); its amounts are the hand-derived closing balances above.
    let xml = r"<ENVELOPE>
  <DSPACCNAME><DSPDISPNAME>Cash</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA>-100000.00</DSPCLDRAMTA></DSPCLDRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Customer One</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA>-11800.00</DSPCLDRAMTA></DSPCLDRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Proprietor Capital</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLCRAMT><DSPCLCRAMTA>100000.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Sales</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLCRAMT><DSPCLCRAMTA>10000.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>CGST</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLCRAMT><DSPCLCRAMTA>900.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>SGST</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLCRAMT><DSPCLCRAMTA>900.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Grand Total</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA>-111800.00</DSPCLDRAMTA></DSPCLDRAMT>
    <DSPCLCRAMT><DSPCLCRAMTA>111800.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
</ENVELOPE>";
    let (rows, warnings) = parse_report_xml(xml, TallyReportKind::TrialBalance).unwrap();
    assert_eq!(warnings.len(), 0);
    let r = reconcile(&card_books(), &reports(rows), period()).unwrap();
    assert!(r.is_green());
    assert_eq!(r.trial_balance.lines.len(), 7);
}
