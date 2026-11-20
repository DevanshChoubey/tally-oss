//! Card T5 tests: negative stock detection and the Block / Warn / Allow policy (spec 6.4).
//!
//! Every expected value is `derived`: the running quantity is opening + receipts - issues in
//! `(date, voucher)` order, below zero means strictly less than zero (exactly zero is fine), and
//! the arithmetic is written in each test. Spec 6.4: "Negative-stock policy per company/voucher
//! type: Block | Warn | Allow"; Block rejects the voucher, Warn saves with a warning, Allow saves
//! silently.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GodownId, Paise, Qty, Rate, StateCode, StockItemId, UnitId,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, AltUnit, BaseVoucherKind, Books, Company, EntityKind, Features, InventoryLine,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, OpeningStock, RestartPeriod,
    StockItem, StockMove, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_stock::StockError;
use ledgerly_stock::negative::{
    StockWarning, check_policy, first_negative, first_negative_with_opening,
};
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);
const SALES: VoucherTypeId = VoucherTypeId(1);
const PURCHASE: VoucherTypeId = VoucherTypeId(2);
const STOCK_JOURNAL: VoucherTypeId = VoucherTypeId(3);

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn mv(v: u64, on: Date, item: StockItemId, qty_in: i64, qty_out: i64) -> StockMove {
    StockMove {
        voucher: VoucherId(v),
        date: on,
        item,
        godown: GodownId(1),
        batch: None,
        qty_in: Qty(d(qty_in)),
        qty_out: Qty(d(qty_out)),
        value: Paise::new(100),
    }
}

fn stock_item(id: StockItemId, opening: i64) -> StockItem {
    StockItem {
        id,
        name: format!("Item {}", id.0),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: Some(AltUnit {
            unit: UnitId(2),
            base_per_alt: d(12),
        }),
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![OpeningStock {
            godown: GodownId(1),
            batch: None,
            qty: Qty(d(opening)),
            rate: Rate(d(10)),
        }],
        standard_costs: vec![],
        tally: None,
    }
}

fn vtype(id: VoucherTypeId, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: format!("{base:?}"),
        base,
        abbreviation: "T".to_owned(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 1,
            restart: RestartPeriod::Yearly,
            series: vec![],
        },
        active: true,
        tally: None,
    }
}

/// Books with items 1 (opening 10) and 2 (opening 0), and the given stock moves.
fn books(moves: Vec<StockMove>) -> Books {
    let mut masters = Masters::default();
    for (id, base) in [
        (SALES, BaseVoucherKind::Sales),
        (PURCHASE, BaseVoucherKind::Purchase),
        (STOCK_JOURNAL, BaseVoucherKind::StockJournal),
    ] {
        masters.voucher_types.insert(id, vtype(id, base));
    }
    masters.stock_items.insert(ITEM, stock_item(ITEM, 10));
    masters
        .stock_items
        .insert(StockItemId(2), stock_item(StockItemId(2), 0));
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
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
        stock_moves: moves,
    }
}

fn line(item: StockItemId, qty: i64, unit: UnitId) -> InventoryLine {
    InventoryLine {
        item,
        qty: Qty(d(qty)),
        unit,
        rate: Rate(d(10)),
        discount_percent: None,
        amount: Paise::new(100),
        ledger: None,
        godowns: vec![],
    }
}

fn voucher(
    id: u64,
    type_id: VoucherTypeId,
    on: Date,
    lines: Vec<InventoryLine>,
    status: VoucherStatus,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::ItemInvoice,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: lines,
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn sale(id: u64, on: Date, qty: i64) -> Voucher {
    voucher(
        id,
        SALES,
        on,
        vec![line(ITEM, qty, UnitId(1))],
        VoucherStatus::Regular,
    )
}

#[test]
fn first_negative_is_the_date_the_running_quantity_drops_below_zero() {
    // derived: zero opening; receipt 5 on 5-Apr (running 5); issue 8 on 10-Apr (running -3).
    let moves = vec![
        mv(1, date!(2026 - 04 - 05), ITEM, 5, 0),
        mv(2, date!(2026 - 04 - 10), ITEM, 0, 8),
    ];
    assert_eq!(first_negative(ITEM, &moves), Some(date!(2026 - 04 - 10)));
}

#[test]
fn exactly_zero_is_not_negative() {
    // derived: receipt 5, issue 5: running 0, which is not below zero.
    let moves = vec![
        mv(1, date!(2026 - 04 - 05), ITEM, 5, 0),
        mv(2, date!(2026 - 04 - 10), ITEM, 0, 5),
    ];
    assert_eq!(first_negative(ITEM, &moves), None);
}

#[test]
fn first_negative_ignores_other_items_and_sorts_by_date_and_voucher() {
    // derived: the issue is listed first but dated after the receipt: sorted, running 5 then 0,
    // never negative. Item 2's issue of 99 is not item 1's.
    let moves = vec![
        mv(2, date!(2026 - 04 - 10), ITEM, 0, 5),
        mv(3, date!(2026 - 04 - 08), StockItemId(2), 0, 99),
        mv(1, date!(2026 - 04 - 05), ITEM, 5, 0),
    ];
    assert_eq!(first_negative(ITEM, &moves), None);
    assert_eq!(
        first_negative(StockItemId(2), &moves),
        Some(date!(2026 - 04 - 08))
    );
}

#[test]
fn same_day_order_is_by_voucher_id() {
    // derived: on 5-Apr voucher 1 issues 3 then voucher 2 receives 10: running -3 after voucher 1,
    // so 5-Apr is negative. With the vouchers swapped (receipt first) it never goes negative.
    let day = date!(2026 - 04 - 05);
    let issue_first = vec![mv(1, day, ITEM, 0, 3), mv(2, day, ITEM, 10, 0)];
    assert_eq!(first_negative(ITEM, &issue_first), Some(day));
    let receipt_first = vec![mv(1, day, ITEM, 10, 0), mv(2, day, ITEM, 0, 3)];
    assert_eq!(first_negative(ITEM, &receipt_first), None);
}

#[test]
fn opening_stock_counts_in_the_checked_variant() {
    // derived: opening 10; issue 15 on 10-Apr: running -5. A later issue after a purchase is
    // irrelevant to the first date. Through 9-Apr the issue is not yet counted: no negative.
    let it = stock_item(ITEM, 10);
    let moves = vec![mv(1, date!(2026 - 04 - 10), ITEM, 0, 15)];
    assert_eq!(
        first_negative_with_opening(&it, &moves, date!(2026 - 04 - 30)).unwrap(),
        Some(date!(2026 - 04 - 10))
    );
    assert_eq!(
        first_negative_with_opening(&it, &moves, date!(2026 - 04 - 09)).unwrap(),
        None
    );
}

#[test]
fn block_rejects_a_sale_that_takes_stock_below_zero() {
    // derived: opening 10, sale 15 on 10-Apr: running -5 on 10-Apr.
    let b = books(vec![]);
    let err = check_policy(
        &sale(10, date!(2026 - 04 - 10), 15),
        &b,
        NegativeStockPolicy::Block,
    )
    .unwrap_err();
    assert_eq!(
        err,
        StockError::NegativeStockBlocked {
            item: ITEM,
            date: date!(2026 - 04 - 10)
        }
    );
}

#[test]
fn warn_returns_a_warning_and_allow_returns_none() {
    // derived: as above, sale 15 against 10 in stock.
    let b = books(vec![]);
    let v = sale(10, date!(2026 - 04 - 10), 15);
    let warnings = check_policy(&v, &b, NegativeStockPolicy::Warn).unwrap();
    assert_eq!(
        warnings,
        vec![StockWarning {
            item: ITEM,
            date: date!(2026 - 04 - 10),
            voucher: VoucherId(10)
        }]
    );
    let none = check_policy(&v, &b, NegativeStockPolicy::Allow).unwrap();
    assert_eq!(none, vec![]);
}

#[test]
fn selling_exactly_what_is_in_stock_passes_under_block() {
    // derived: opening 10, sale 10: running 0, not negative.
    let b = books(vec![]);
    let r = check_policy(
        &sale(10, date!(2026 - 04 - 10), 10),
        &b,
        NegativeStockPolicy::Block,
    )
    .unwrap();
    assert_eq!(r, vec![]);
}

#[test]
fn earlier_receipts_cover_the_sale() {
    // derived: opening 10 + receipt 5 on 5-Apr = 15; sale 15 on 10-Apr: running 0.
    let b = books(vec![mv(1, date!(2026 - 04 - 05), ITEM, 5, 0)]);
    let r = check_policy(
        &sale(10, date!(2026 - 04 - 10), 15),
        &b,
        NegativeStockPolicy::Block,
    )
    .unwrap();
    assert_eq!(r, vec![]);
}

#[test]
fn a_backdated_sale_is_caught_where_a_later_issue_goes_negative() {
    // derived: opening 10; existing issue of 8 on 20-Apr (voucher 2). New sale of 5 on 10-Apr:
    // running 5 on 10-Apr, then 5 - 8 = -3 on 20-Apr. The warning date is 20-Apr.
    let b = books(vec![mv(2, date!(2026 - 04 - 20), ITEM, 0, 8)]);
    let w = check_policy(
        &sale(10, date!(2026 - 04 - 10), 5),
        &b,
        NegativeStockPolicy::Warn,
    )
    .unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].date, date!(2026 - 04 - 20));
}

#[test]
fn editing_a_voucher_replaces_its_own_earlier_movements() {
    // derived: opening 10. The books hold voucher 10's old issue of 15 (would be -5). The edited
    // voucher 10 issues 5: running 5. Counting the old and new would give 10 - 15 - 5 = -10.
    let b = books(vec![mv(10, date!(2026 - 04 - 10), ITEM, 0, 15)]);
    let r = check_policy(
        &sale(10, date!(2026 - 04 - 10), 5),
        &b,
        NegativeStockPolicy::Block,
    )
    .unwrap();
    assert_eq!(r, vec![]);
}

#[test]
fn a_negative_stretch_that_ended_before_the_voucher_is_not_its_fault() {
    // derived: item 2 opens at 0. Existing issue 3 on 5-Apr (running -3), receipt 10 on 6-Apr
    // (running 7). New sale of 2 on 10-Apr: running 5. The earlier dip is not reported.
    let b = books(vec![
        mv(1, date!(2026 - 04 - 05), StockItemId(2), 0, 3),
        mv(2, date!(2026 - 04 - 06), StockItemId(2), 10, 0),
    ]);
    let v = voucher(
        10,
        SALES,
        date!(2026 - 04 - 10),
        vec![line(StockItemId(2), 2, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert_eq!(
        check_policy(&v, &b, NegativeStockPolicy::Block).unwrap(),
        vec![]
    );
}

#[test]
fn alternate_unit_lines_are_converted_to_base_units() {
    // derived: 1 alternate unit = 12 base units (AltUnit.base_per_alt). Opening 10; a sale of 1
    // alternate unit issues 12: running -2 on 10-Apr.
    let b = books(vec![]);
    let v = voucher(
        10,
        SALES,
        date!(2026 - 04 - 10),
        vec![line(ITEM, 1, UnitId(2))],
        VoucherStatus::Regular,
    );
    let w = check_policy(&v, &b, NegativeStockPolicy::Warn).unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].date, date!(2026 - 04 - 10));
    // an unknown unit cannot be converted: an error, not a guess
    let bad = voucher(
        11,
        SALES,
        date!(2026 - 04 - 10),
        vec![line(ITEM, 1, UnitId(9))],
        VoucherStatus::Regular,
    );
    assert!(matches!(
        check_policy(&bad, &b, NegativeStockPolicy::Warn).unwrap_err(),
        StockError::Unit { .. }
    ));
}

#[test]
fn block_names_the_lowest_item_id_and_warn_lists_items_in_id_order() {
    // derived: item 1 opens at 10, item 2 at 0. A sale of 11 of item 1 and 1 of item 2 takes both
    // below zero on 10-Apr. Block reports item 1 first; Warn reports 1 then 2.
    let b = books(vec![]);
    let v = voucher(
        10,
        SALES,
        date!(2026 - 04 - 10),
        vec![
            line(StockItemId(2), 1, UnitId(1)),
            line(ITEM, 11, UnitId(1)),
        ],
        VoucherStatus::Regular,
    );
    assert_eq!(
        check_policy(&v, &b, NegativeStockPolicy::Block).unwrap_err(),
        StockError::NegativeStockBlocked {
            item: ITEM,
            date: date!(2026 - 04 - 10)
        }
    );
    let w = check_policy(&v, &b, NegativeStockPolicy::Warn).unwrap();
    let items: Vec<StockItemId> = w.iter().map(|x| x.item).collect();
    assert_eq!(items, vec![ITEM, StockItemId(2)]);
}

#[test]
fn receipts_and_unposted_vouchers_are_never_flagged() {
    // derived: a purchase only adds stock. An optional or cancelled sale is not in the books, so it
    // takes nothing out. Item 2 has 0 in stock, so a regular sale of it would be flagged.
    let b = books(vec![]);
    let on = date!(2026 - 04 - 10);
    let purchase = voucher(
        10,
        PURCHASE,
        on,
        vec![line(StockItemId(2), 5, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert_eq!(
        check_policy(&purchase, &b, NegativeStockPolicy::Block).unwrap(),
        vec![]
    );
    for status in [VoucherStatus::Optional, VoucherStatus::Cancelled] {
        let v = voucher(
            11,
            SALES,
            on,
            vec![line(StockItemId(2), 5, UnitId(1))],
            status,
        );
        assert_eq!(
            check_policy(&v, &b, NegativeStockPolicy::Block).unwrap(),
            vec![]
        );
    }
    let regular = voucher(
        12,
        SALES,
        on,
        vec![line(StockItemId(2), 5, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert!(check_policy(&regular, &b, NegativeStockPolicy::Block).is_err());
}

#[test]
fn a_voucher_without_inventory_lines_is_fine() {
    // derived: no stock lines, no stock effect.
    let b = books(vec![]);
    let v = voucher(
        10,
        SALES,
        date!(2026 - 04 - 10),
        vec![],
        VoucherStatus::Regular,
    );
    assert_eq!(
        check_policy(&v, &b, NegativeStockPolicy::Block).unwrap(),
        vec![]
    );
}

#[test]
fn undecidable_stock_effect_and_unknown_masters_are_errors() {
    // derived: a stock journal's direction per line is not known from the base kind, so it is
    // refused rather than guessed; a missing voucher type or item is corrupt input.
    let b = books(vec![]);
    let on = date!(2026 - 04 - 10);
    let journal = voucher(
        10,
        STOCK_JOURNAL,
        on,
        vec![line(ITEM, 1, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert!(matches!(
        check_policy(&journal, &b, NegativeStockPolicy::Block).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let no_type = voucher(
        11,
        VoucherTypeId(99),
        on,
        vec![line(ITEM, 1, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert!(matches!(
        check_policy(&no_type, &b, NegativeStockPolicy::Block).unwrap_err(),
        StockError::Invalid { .. }
    ));
    let no_item = voucher(
        12,
        SALES,
        on,
        vec![line(StockItemId(99), 1, UnitId(1))],
        VoucherStatus::Regular,
    );
    assert!(matches!(
        check_policy(&no_item, &b, NegativeStockPolicy::Block).unwrap_err(),
        StockError::Invalid { .. }
    ));
}
