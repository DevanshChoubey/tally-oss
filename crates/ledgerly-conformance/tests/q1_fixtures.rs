//! Card Q1 tests: the fixture company `books_small()` (fixtures/README.md).
//!
//! Counts and dates come from `fixtures/README.md`. Amounts are hand arithmetic written next to
//! each assertion; nothing is copied from engine output (rule A4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;

use ledgerly_conformance::fixtures::{books_small, ids, try_books_small};
use ledgerly_core::{
    Date, Decimal, DrCr, Gstin, LedgerId, Month, Paise, StateCode, StockItemId, VoucherId,
    gstin_check_character,
};
use ledgerly_model::{BillRefKind, Books, VoucherStatus};

fn date(m: Month, d: u8) -> Date {
    Date::from_calendar_date(2025, m, d).unwrap()
}

fn signed(side: DrCr, amount: Paise) -> i64 {
    match side {
        DrCr::Dr => amount.get(),
        DrCr::Cr => -amount.get(),
    }
}

fn net_of(books: &Books, ledger: LedgerId, voucher: u64) -> i64 {
    books
        .postings
        .iter()
        .filter(|p| p.ledger == ledger && p.voucher == VoucherId(voucher))
        .map(|p| signed(p.amount.side, p.amount.amount))
        .sum()
}

#[test]
fn try_books_small_succeeds_and_matches_books_small() {
    assert_eq!(try_books_small().unwrap(), books_small());
}

#[test]
fn counts_match_the_readme() {
    // provenance: derived. fixtures/README.md: 12 ledgers, 3 stock items, godowns Main and Pune,
    // 20 vouchers.
    let b = books_small();
    assert_eq!(b.masters.ledgers.len(), 12);
    assert_eq!(b.masters.stock_items.len(), 3);
    assert_eq!(b.masters.godowns.len(), 2);
    assert_eq!(b.vouchers.len(), 20);
    let godowns: Vec<&str> = b
        .masters
        .godowns
        .values()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(godowns, ["Main", "Pune"]);
}

#[test]
fn maharashtra_gstin_check_character_is_zero() {
    // provenance: derived. Hand derivation (same algorithm as ledgerly-core's test for
    // 29ABCDE1234F1ZW, with '7' in place of '9' at position 2, factor 2):
    //   that sum was 184 with 9x2=18 -> 18; here 7x2=14 -> 14, so the sum is 184 - 18 + 14 = 180;
    //   180 mod 36 = 0; check = (36 - 0) mod 36 = 0 -> '0'. The algorithm is recalled, not quoted.
    assert_eq!(gstin_check_character("27ABCDE1234F1Z"), Some('0'));
    let b = books_small();
    let gstins: Vec<&str> = b
        .company
        .registrations
        .iter()
        .map(|r| r.gstin.as_str())
        .collect();
    assert_eq!(gstins, ["27ABCDE1234F1Z0", "29ABCDE1234F1ZW"]);
    for r in &b.company.registrations {
        assert!(Gstin::parse(r.gstin.as_str()).is_ok());
        assert_eq!(r.gstin.pan_str(), "ABCDE1234F");
    }
}

#[test]
fn every_voucher_is_inside_the_financial_year() {
    // provenance: derived. README: FY 2025-26, books begin 1 April 2025 (FY ends 31 Mar 2026).
    let b = books_small();
    assert_eq!(b.company.books_begin, date(Month::April, 1));
    let end = Date::from_calendar_date(2026, Month::March, 31).unwrap();
    for v in b.vouchers.values() {
        assert!(
            v.date >= b.company.books_begin && v.date <= end,
            "{:?}",
            v.id
        );
    }
}

#[test]
fn postings_and_moves_are_sorted_by_date_then_voucher() {
    let b = books_small();
    let keys: Vec<(Date, VoucherId)> = b.postings.iter().map(|p| (p.date, p.voucher)).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    let keys: Vec<(Date, VoucherId)> = b.stock_moves.iter().map(|m| (m.date, m.voucher)).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn every_voucher_balances_and_all_postings_sum_to_zero() {
    // provenance: derived. Double-entry: debits equal credits in each voucher.
    let b = books_small();
    let mut per_voucher: BTreeMap<VoucherId, i64> = BTreeMap::new();
    for p in &b.postings {
        *per_voucher.entry(p.voucher).or_default() += signed(p.amount.side, p.amount.amount);
    }
    for (id, net) in &per_voucher {
        assert_eq!(*net, 0, "voucher {id:?} does not balance");
    }
    assert_eq!(per_voucher.values().sum::<i64>(), 0);
}

#[test]
fn postings_mirror_ledger_lines_except_cancelled() {
    let b = books_small();
    for v in b.vouchers.values() {
        let n = b.postings.iter().filter(|p| p.voucher == v.id).count();
        match v.status {
            VoucherStatus::Cancelled => {
                assert_eq!(n, 0, "cancelled voucher has postings");
                assert!(v.ledger_lines.is_empty() && v.inventory_lines.is_empty());
            }
            VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {
                assert_eq!(n, v.ledger_lines.len(), "{:?}", v.id);
            }
        }
    }
}

#[test]
fn one_cancelled_and_one_optional_voucher() {
    let b = books_small();
    let count = |s: VoucherStatus| b.vouchers.values().filter(|v| v.status == s).count();
    assert_eq!(count(VoucherStatus::Cancelled), 1);
    assert_eq!(count(VoucherStatus::Optional), 1);
    let optional_ids: Vec<VoucherId> = b
        .vouchers
        .values()
        .filter(|v| v.status == VoucherStatus::Optional)
        .map(|v| v.id)
        .collect();
    for p in b
        .postings
        .iter()
        .filter(|p| optional_ids.contains(&p.voucher))
    {
        assert_eq!(p.status, VoucherStatus::Optional);
    }
}

#[test]
fn r3_movements_match_the_worked_data() {
    // provenance: derived. docs/research/03-inventory.md worked data with README opening
    // 100 @ 10: 10 Apr purchase 50 @ 13, 15 Apr sale 120, 20 May purchase 40 @ 15, 31 May sale 30.
    let b = books_small();
    let r3 = StockItemId(ids::ITEM_R3.0);
    let moves: Vec<_> = b.stock_moves.iter().filter(|m| m.item == r3).collect();
    let shape: Vec<(Date, i64, i64)> = moves
        .iter()
        .map(|m| {
            (
                m.date,
                i64::try_from(m.qty_in.0.mantissa()).unwrap(),
                i64::try_from(m.qty_out.0.mantissa()).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (date(Month::April, 10), 50, 0),
            (date(Month::April, 15), 0, 120),
            (date(Month::May, 20), 40, 0),
            (date(Month::May, 31), 0, 30),
        ]
    );
    // Purchase values: 50 x 13 = 650; 40 x 15 = 600.
    assert_eq!(moves[0].value, Paise::new(65_000));
    assert_eq!(moves[2].value, Paise::new(60_000));
    // Opening 100 @ 10 at Main.
    let item = &b.masters.stock_items[&r3];
    assert_eq!(item.opening.len(), 1);
    assert_eq!(item.opening[0].qty.0, Decimal::from(100));
    assert_eq!(item.opening[0].rate.0, Decimal::from(10));
}

#[test]
fn intra_and_inter_state_sales_of_10000_at_18_percent() {
    // provenance: derived. CGST Act s.9 / IGST Act s.5 rate 18% (the README fixes the rate):
    // 10,000 x 18% = 1,800 total; intra-state splits 900 CGST + 900 SGST; inter-state 1,800 IGST;
    // invoice value 11,800 either way.
    let b = books_small();
    let intra = [
        (ids::LEDGER_CGST, 90_000),
        (ids::LEDGER_SGST, 90_000),
        (ids::LEDGER_SALES_LOCAL, 1_000_000),
    ];
    for (ledger, cr) in intra {
        assert_eq!(net_of(&b, ledger, 7), -cr);
    }
    assert_eq!(net_of(&b, ids::LEDGER_SHREE_GANESH, 7), 1_180_000);
    assert_eq!(net_of(&b, ids::LEDGER_IGST, 8), -180_000);
    assert_eq!(net_of(&b, ids::LEDGER_SALES_INTERSTATE, 8), -1_000_000);
    assert_eq!(net_of(&b, ids::LEDGER_SHREE_GANESH, 8), 1_180_000);
    let inter = &b.vouchers[&VoucherId(8)];
    assert_eq!(inter.place_of_supply.map(StateCode::get), Some(29));
}

#[test]
fn rate_change_vouchers_straddle_22_september_2025() {
    // provenance: derived. README: one sale with supply on 21 Sep 2025 and invoice on 23 Sep 2025,
    // one with both on 23 Sep 2025. Tax lines are deliberately absent: the rate depends on the
    // rule pack (G12) and Q1 cannot derive it.
    let b = books_small();
    let a = &b.vouchers[&VoucherId(17)];
    assert_eq!(a.date, date(Month::September, 23));
    assert_eq!(a.supply_date, Some(date(Month::September, 21)));
    let c = &b.vouchers[&VoucherId(18)];
    assert_eq!(c.date, date(Month::September, 23));
    assert!(c.supply_date.is_none() || c.supply_date == Some(date(Month::September, 23)));
    for v in [a, c] {
        assert!(v.gst.is_none());
        assert!(v.payment_date.is_none());
    }
}

#[test]
fn receipt_settles_a_bill_that_an_earlier_sale_opened() {
    // provenance: derived. V7 opens bill INV/25-26/0003 for 11,800; V9 receipt settles it Against.
    let b = books_small();
    let sale = &b.vouchers[&VoucherId(7)];
    let receipt = &b.vouchers[&VoucherId(9)];
    let opened: Vec<_> = sale
        .ledger_lines
        .iter()
        .flat_map(|l| l.bills.iter())
        .filter(|a| a.kind == BillRefKind::New)
        .collect();
    let settled: Vec<_> = receipt
        .ledger_lines
        .iter()
        .flat_map(|l| l.bills.iter())
        .filter(|a| a.kind == BillRefKind::Against)
        .collect();
    assert_eq!(opened.len(), 1);
    assert_eq!(settled.len(), 1);
    assert_eq!(opened[0].name, settled[0].name);
    assert_eq!(opened[0].amount.amount, settled[0].amount.amount);
    assert_eq!(opened[0].amount.amount, Paise::new(1_180_000));
}

#[test]
fn bill_allocations_sum_to_the_line_on_bill_lines() {
    let b = books_small();
    for v in b.vouchers.values() {
        for line in v.ledger_lines.iter().filter(|l| !l.bills.is_empty()) {
            let sum: i64 = line
                .bills
                .iter()
                .map(|a| signed(a.amount.side, a.amount.amount))
                .sum();
            assert_eq!(
                sum,
                signed(line.amount.side, line.amount.amount),
                "voucher {:?}",
                v.id
            );
        }
    }
}

#[test]
fn inventory_amounts_are_quantity_times_rate_and_godowns_sum_to_quantity() {
    // provenance: derived. amount = qty x rate in rupees, e.g. 10 box x 600 = 6,000.00.
    let b = books_small();
    for v in b.vouchers.values() {
        for l in &v.inventory_lines {
            let rupees = l.qty.0 * l.rate.0;
            assert_eq!(l.amount.to_rupees(), rupees, "voucher {:?}", v.id);
            let alloc: Decimal = l.godowns.iter().map(|g| g.qty.0).sum();
            assert_eq!(alloc, l.qty.0, "voucher {:?}", v.id);
        }
    }
}

#[test]
fn widget_b_purchase_converts_boxes_to_pieces() {
    // provenance: derived. README: Widget B is a box of 12 pcs. 10 boxes = 120 pcs; sale of
    // 5 boxes = 60 pcs.
    let b = books_small();
    let moves: Vec<_> = b
        .stock_moves
        .iter()
        .filter(|m| m.item.0 == ids::ITEM_WIDGET_B.0)
        .collect();
    assert_eq!(moves.len(), 2);
    assert_eq!(moves[0].qty_in.0, Decimal::from(120));
    assert_eq!(moves[1].qty_out.0, Decimal::from(60));
    assert!(moves.iter().all(|m| m.godown == ids::GODOWN_PUNE));
}

#[test]
fn every_ledger_group_unit_and_item_reference_resolves() {
    let b = books_small();
    for l in b.masters.ledgers.values() {
        assert!(b.masters.groups.contains_key(&l.group), "{}", l.name);
    }
    for v in b.vouchers.values() {
        assert!(b.masters.voucher_types.contains_key(&v.type_id));
        for l in &v.ledger_lines {
            assert!(b.masters.ledgers.contains_key(&l.ledger));
        }
        for l in &v.inventory_lines {
            assert!(b.masters.stock_items.contains_key(&l.item));
            assert!(b.masters.units.contains_key(&l.unit));
        }
    }
    for m in &b.stock_moves {
        assert!(b.masters.godowns.contains_key(&m.godown));
    }
}

#[test]
fn voucher_numbers_are_unique() {
    let b = books_small();
    let mut seen = std::collections::BTreeSet::new();
    for v in b.vouchers.values() {
        let n = v.number.clone().unwrap();
        assert!(seen.insert(n.clone()), "duplicate number {n}");
    }
}
