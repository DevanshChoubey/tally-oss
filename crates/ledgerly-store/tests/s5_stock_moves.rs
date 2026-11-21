//! Card S5 tests: `stock_moves` projects inventory-line allocations into stock movements.
//!
//! Provenance. Every expected value is derived by hand from the fixture numbers below and from
//! the definitions in the doc comments of `ledgerly_model::{StockMove, InventoryLine,
//! GodownAllocation, BaseVoucherKind, UnitKind, AltUnit}` and the card text, never from output of
//! the code under test:
//! - An allocation is one movement; quantities and values are the allocation's own numbers
//!   ("value of the movement at voucher rate").
//! - Purchase, receipt note and rejection-in bring goods in; sales, delivery note and
//!   rejection-out send goods out (the voucher kind docs: "Goods received", "Goods delivered",
//!   "Goods returned by a customer", "Goods returned to a supplier").
//! - Unit conversion: `UnitKind::Compound { factor: 12 }` is "1 first = 12 second", so 2 boxes =
//!   2 x 12 = 24 pcs; `AltUnit { base_per_alt: 12 }` makes 3 alternate units = 3 x 12 = 36 pcs.
//! - Round 3 (documented behaviour, pages read 2026-11-21): a credit note with items is a sales
//!   return and brings goods in; optional and memorandum vouchers never count as actual stock; a
//!   physical count equal to the book quantity gives no movement. Cases whose stock effect is not
//!   documented (debit note with items, stock journal, post-dated, non-zero physical adjustment,
//!   physical count sharing a day with another movement) assert the typed
//!   `StoreError::Unsupported` naming the voucher; they have no expected number to check.
//! - Errors in the data itself (unknown ids, sums that disagree, unit not convertible) remain
//!   `StoreError::Encoding`.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{
    BatchId, Clock, CompanyId, Date, Decimal, GodownId, Gstin, Month, OffsetDateTime, Paise, Pan,
    Period, Qty, Rate, StateCode, StockItemId, UnitId, UserId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, AltUnit, BaseVoucherKind, Batch, Company, EntityKind, Features, Godown,
    GodownAllocation, GstRegistration, InventoryLine, MasterSnapshot, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, OpeningStock, RestartPeriod, StockItem, StockMove, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_store::stock_load::stock_moves;
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store, StoreError};

const SALES: u64 = 1;
const PURCHASE: u64 = 2;
const RECEIPT_NOTE: u64 = 3;
const SALES_ORDER: u64 = 4;
const STOCK_JOURNAL: u64 = 5;
const CREDIT_NOTE: u64 = 6;
const REJECTION_IN: u64 = 7;
const DELIVERY_NOTE: u64 = 8;
const JOURNAL: u64 = 9;
const DEBIT_NOTE: u64 = 10;
const MEMORANDUM: u64 = 11;
const PHYSICAL_STOCK: u64 = 12;

const PCS: u64 = 1;
const BOX: u64 = 2;
const DOZEN: u64 = 3;
const KG: u64 = 4;

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn qty(n: i64) -> Qty {
    Qty(Decimal::from(n))
}

fn company() -> Company {
    // Fictional data; same company as the S4 tests.
    Company {
        id: CompanyId(7),
        name: "Example Traders Pvt Ltd".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Sample Road".to_owned()],
            pincode: Some("560001".to_owned()),
            state: Some(StateCode::new(29).unwrap()),
            country: String::new(),
        },
        state: StateCode::new(29).unwrap(),
        pan: Some(Pan::parse("ABCDE1234F").unwrap()),
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date(2017, Month::July, 1),
            composition: false,
        }],
        fy_start: date(2026, Month::April, 1),
        books_begin: date(2026, Month::April, 1),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: None,
        app_version: "0.0.0-test".to_owned(),
    }
}

fn unit(id: u64, symbol: &str, kind: UnitKind) -> MasterSnapshot {
    MasterSnapshot::Unit(Unit {
        id: UnitId(id),
        symbol: symbol.to_owned(),
        formal_name: symbol.to_owned(),
        decimals: 0,
        kind,
        uqc: None,
        tally: None,
    })
}

fn voucher_type(id: u64, base: BaseVoucherKind) -> MasterSnapshot {
    MasterSnapshot::VoucherType(VoucherType {
        id: VoucherTypeId(id),
        name: format!("type {id}"),
        base,
        abbreviation: String::new(),
        numbering: NumberingConfig {
            method: NumberingMethod::Manual,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Never,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    })
}

fn godown(id: u64) -> MasterSnapshot {
    MasterSnapshot::Godown(Godown {
        id: GodownId(id),
        name: format!("godown {id}"),
        parent: None,
        tally: None,
    })
}

fn masters_events() -> Vec<Event> {
    let item = StockItem {
        id: StockItemId(1),
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UnitId(PCS),
        alt_unit: Some(AltUnit {
            unit: UnitId(DOZEN),
            base_per_alt: Decimal::from(12),
        }),
        hsn: None,
        gst_history: Vec::new(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: true,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    };
    let batch = Batch {
        id: BatchId(1),
        item: StockItemId(1),
        name: "B-1".to_owned(),
        mfg: None,
        expiry: None,
    };
    let kinds = [
        (SALES, BaseVoucherKind::Sales),
        (PURCHASE, BaseVoucherKind::Purchase),
        (RECEIPT_NOTE, BaseVoucherKind::ReceiptNote),
        (SALES_ORDER, BaseVoucherKind::SalesOrder),
        (STOCK_JOURNAL, BaseVoucherKind::StockJournal),
        (CREDIT_NOTE, BaseVoucherKind::CreditNote),
        (REJECTION_IN, BaseVoucherKind::RejectionIn),
        (DELIVERY_NOTE, BaseVoucherKind::DeliveryNote),
        (JOURNAL, BaseVoucherKind::Journal),
        (DEBIT_NOTE, BaseVoucherKind::DebitNote),
        (MEMORANDUM, BaseVoucherKind::Memorandum),
        (PHYSICAL_STOCK, BaseVoucherKind::PhysicalStock),
    ];
    let mut events = vec![
        Event::MasterCreated(unit(PCS, "pcs", UnitKind::Simple)),
        Event::MasterCreated(unit(
            BOX,
            "box",
            UnitKind::Compound {
                first: UnitId(BOX),
                factor: Decimal::from(12),
                second: UnitId(PCS),
            },
        )),
        Event::MasterCreated(unit(DOZEN, "doz", UnitKind::Simple)),
        Event::MasterCreated(unit(KG, "kg", UnitKind::Simple)),
        Event::MasterCreated(godown(1)),
        Event::MasterCreated(godown(2)),
        Event::MasterCreated(MasterSnapshot::StockItem(item)),
        Event::MasterCreated(MasterSnapshot::Batch(batch)),
    ];
    for (id, base) in kinds {
        events.push(Event::MasterCreated(voucher_type(id, base)));
    }
    events
}

fn new_store(dir: &Path) -> SqliteStore {
    let path = dir.join("books.ledgerly");
    let mut store =
        SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    store.commit(&masters_events(), &meta()).unwrap();
    store
}

fn alloc(godown: u64, batch: Option<u64>, n: i64, paise: i64) -> GodownAllocation {
    GodownAllocation {
        godown: GodownId(godown),
        batch: batch.map(BatchId),
        qty: qty(n),
        amount: Paise::new(paise),
    }
}

fn inv_line(unit: u64, n: i64, paise: i64, godowns: Vec<GodownAllocation>) -> InventoryLine {
    InventoryLine {
        item: StockItemId(1),
        qty: qty(n),
        unit: UnitId(unit),
        rate: Rate(Decimal::ONE),
        discount_percent: None,
        amount: Paise::new(paise),
        ledger: None,
        godowns,
    }
}

fn voucher(id: u64, type_id: u64, on: Date, lines: Vec<InventoryLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(type_id),
        number: Some(format!("V{id}")),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::ItemInvoice,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: Vec::new(),
        inventory_lines: lines,
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn post(store: &mut SqliteStore, vouchers: Vec<Voucher>) {
    let events: Vec<Event> = vouchers.into_iter().map(Event::VoucherPosted).collect();
    store.commit(&events, &meta()).unwrap();
}

fn assert_error_names(result: &Result<Vec<StockMove>, StoreError>, voucher: u64, needle: &str) {
    let want_id = format!("VoucherId({voucher})");
    assert!(
        matches!(result, Err(StoreError::Encoding(m)) if m.contains(&want_id) && m.contains(needle)),
        "{result:?}"
    );
}

/// Asserts a typed `StoreError::Unsupported` naming `voucher` whose reason contains `needle`.
fn assert_unsupported(result: &Result<Vec<StockMove>, StoreError>, voucher: u64, needle: &str) {
    assert!(
        matches!(
            result,
            Err(StoreError::Unsupported { voucher: v, reason })
                if *v == VoucherId(voucher) && reason.contains(needle)
        ),
        "{result:?}"
    );
}

#[test]
fn empty_log_has_no_moves() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(dir.path());
    assert_eq!(stock_moves(&store, None).unwrap(), Vec::<StockMove>::new());
}

#[test]
fn item_invoice_with_two_godown_allocations_gives_two_out_moves() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; fixture is 10 pcs at Rs 50 = Rs 500 (50,000 paise) split 6 + 4 over
    // two godowns: 6 x Rs 50 = Rs 300 = 30,000 paise and 4 x Rs 50 = Rs 200 = 20,000 paise.
    let line = inv_line(
        PCS,
        10,
        50_000,
        vec![alloc(1, None, 6, 30_000), alloc(2, Some(1), 4, 20_000)],
    );
    post(
        &mut store,
        vec![voucher(1, SALES, date(2026, Month::April, 5), vec![line])],
    );
    let want = vec![
        StockMove {
            voucher: VoucherId(1),
            date: date(2026, Month::April, 5),
            item: StockItemId(1),
            godown: GodownId(1),
            batch: None,
            qty_in: Qty::ZERO,
            qty_out: qty(6),
            value: Paise::new(30_000),
            status: VoucherStatus::Regular,
        },
        StockMove {
            voucher: VoucherId(1),
            date: date(2026, Month::April, 5),
            item: StockItemId(1),
            godown: GodownId(2),
            batch: Some(BatchId(1)),
            qty_in: Qty::ZERO,
            qty_out: qty(4),
            value: Paise::new(20_000),
            status: VoucherStatus::Regular,
        },
    ];
    assert_eq!(stock_moves(&store, None).unwrap(), want);
}

#[test]
fn receipt_note_is_qty_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; a receipt note is "Goods received", so 5 pcs go to qty_in, qty_out 0.
    let line = inv_line(PCS, 5, 25_000, vec![alloc(1, None, 5, 25_000)]);
    post(
        &mut store,
        vec![voucher(
            1,
            RECEIPT_NOTE,
            date(2026, Month::April, 6),
            vec![line],
        )],
    );
    let moves = stock_moves(&store, None).unwrap();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].qty_in, qty(5));
    assert_eq!(moves[0].qty_out, Qty::ZERO);
    assert_eq!(moves[0].value, Paise::new(25_000));
}

#[test]
fn direction_follows_the_voucher_kind() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; kind docs: Purchase and RejectionIn bring goods in, DeliveryNote sends
    // goods out. One pcs each, vouchers 1 (purchase), 2 (rejection in), 3 (delivery note).
    let one = |v: u64, t: u64| {
        voucher(
            v,
            t,
            date(2026, Month::April, 7),
            vec![inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)])],
        )
    };
    post(
        &mut store,
        vec![
            one(1, PURCHASE),
            one(2, REJECTION_IN),
            one(3, DELIVERY_NOTE),
        ],
    );
    let got: Vec<(u64, Qty, Qty)> = stock_moves(&store, None)
        .unwrap()
        .iter()
        .map(|m| (m.voucher.0, m.qty_in, m.qty_out))
        .collect();
    let want = vec![
        (1, qty(1), Qty::ZERO),
        (2, qty(1), Qty::ZERO),
        (3, Qty::ZERO, qty(1)),
    ];
    assert_eq!(got, want);
}

#[test]
fn quantities_are_converted_to_the_base_unit() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; compound unit box = 12 pcs, so 2 boxes = 2 x 12 = 24 pcs (voucher 1).
    // Alternate unit dozen has base_per_alt 12, so 3 dozen = 3 x 12 = 36 pcs (voucher 2).
    // Values are unchanged by the unit.
    let boxes = inv_line(BOX, 2, 4_800, vec![alloc(1, None, 2, 4_800)]);
    let dozens = inv_line(DOZEN, 3, 7_200, vec![alloc(1, None, 3, 7_200)]);
    post(
        &mut store,
        vec![
            voucher(1, PURCHASE, date(2026, Month::April, 8), vec![boxes]),
            voucher(2, PURCHASE, date(2026, Month::April, 9), vec![dozens]),
        ],
    );
    let got: Vec<(u64, Qty, i64)> = stock_moves(&store, None)
        .unwrap()
        .iter()
        .map(|m| (m.voucher.0, m.qty_in, m.value.get()))
        .collect();
    assert_eq!(got, vec![(1, qty(24), 4_800), (2, qty(36), 7_200)]);
}

#[test]
fn moves_are_sorted_by_date_then_voucher_keeping_line_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let one = |v: u64, on: Date| {
        voucher(
            v,
            PURCHASE,
            on,
            vec![inv_line(
                PCS,
                2,
                200,
                vec![alloc(2, None, 1, 100), alloc(1, None, 1, 100)],
            )],
        )
    };
    // provenance: derived; ids 1 (12 Apr), 3 (5 Apr), 2 (5 Apr) committed in that order. Sorted by
    // (date, voucher): 2, 3 (both 5 Apr), then 1; within a voucher the allocation order of the
    // line is kept: godown 2 first, then godown 1.
    post(
        &mut store,
        vec![
            one(1, date(2026, Month::April, 12)),
            one(3, date(2026, Month::April, 5)),
            one(2, date(2026, Month::April, 5)),
        ],
    );
    let got: Vec<(u64, u64)> = stock_moves(&store, None)
        .unwrap()
        .iter()
        .map(|m| (m.voucher.0, m.godown.0))
        .collect();
    assert_eq!(got, vec![(2, 2), (2, 1), (3, 2), (3, 1), (1, 2), (1, 1)]);
}

#[test]
fn period_keeps_only_vouchers_dated_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let one = |v: u64, on: Date| {
        voucher(
            v,
            PURCHASE,
            on,
            vec![inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)])],
        )
    };
    // provenance: derived; the period 1-10 Apr (inclusive) contains 5 Apr and 10 Apr, not 11 Apr.
    post(
        &mut store,
        vec![
            one(1, date(2026, Month::April, 5)),
            one(2, date(2026, Month::April, 11)),
            one(3, date(2026, Month::April, 10)),
        ],
    );
    let period = Period::new(date(2026, Month::April, 1), date(2026, Month::April, 10)).unwrap();
    let got: Vec<u64> = stock_moves(&store, Some(period))
        .unwrap()
        .iter()
        .map(|m| m.voucher.0)
        .collect();
    assert_eq!(got, vec![1, 3]);
}

#[test]
fn orders_and_vouchers_without_items_do_not_move_stock() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; an order is a commitment, not a movement; a voucher with no inventory
    // lines has nothing to project.
    let order = voucher(
        1,
        SALES_ORDER,
        date(2026, Month::April, 5),
        vec![inv_line(PCS, 3, 300, vec![alloc(1, None, 3, 300)])],
    );
    let plain = voucher(2, SALES, date(2026, Month::April, 5), Vec::new());
    post(&mut store, vec![order, plain]);
    assert_eq!(stock_moves(&store, None).unwrap(), Vec::<StockMove>::new());
}

#[test]
fn cancelled_voucher_has_no_moves_and_altered_voucher_shows_only_the_new_ones() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let first = voucher(
        1,
        PURCHASE,
        date(2026, Month::April, 5),
        vec![inv_line(PCS, 4, 400, vec![alloc(1, None, 4, 400)])],
    );
    let second = voucher(
        2,
        PURCHASE,
        date(2026, Month::April, 6),
        vec![inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)])],
    );
    let second_after = voucher(
        2,
        PURCHASE,
        date(2026, Month::April, 6),
        vec![inv_line(PCS, 7, 700, vec![alloc(2, None, 7, 700)])],
    );
    post(&mut store, vec![first.clone(), second.clone()]);
    store
        .commit(
            &[
                Event::VoucherCancelled { before: first },
                Event::VoucherAltered {
                    before: second,
                    after: second_after,
                },
            ],
            &meta(),
        )
        .unwrap();
    // provenance: derived; "number kept, amounts removed" for the cancelled voucher 1; the altered
    // voucher 2 is replaced by `after`: 7 pcs into godown 2 for 700 paise.
    let got: Vec<(u64, u64, Qty, i64)> = stock_moves(&store, None)
        .unwrap()
        .iter()
        .map(|m| (m.voucher.0, m.godown.0, m.qty_in, m.value.get()))
        .collect();
    assert_eq!(got, vec![(2, 2, qty(7), 700)]);
}

#[test]
fn allocation_quantities_that_do_not_add_up_are_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; model doc: allocation quantities sum to the line quantity: 3 + 4 != 10.
    let line = inv_line(
        PCS,
        10,
        1_000,
        vec![alloc(1, None, 3, 300), alloc(2, None, 4, 700)],
    );
    post(
        &mut store,
        vec![voucher(1, SALES, date(2026, Month::April, 5), vec![line])],
    );
    assert_error_names(&stock_moves(&store, None), 1, "sum to");
}

#[test]
fn allocation_amounts_that_do_not_add_up_are_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; quantities 3 + 7 = 10 agree but amounts 300 + 600 != 1,000.
    let line = inv_line(
        PCS,
        10,
        1_000,
        vec![alloc(1, None, 3, 300), alloc(2, None, 7, 600)],
    );
    post(
        &mut store,
        vec![voucher(1, SALES, date(2026, Month::April, 5), vec![line])],
    );
    assert_error_names(&stock_moves(&store, None), 1, "amounts sum to");
}

#[test]
fn a_line_without_godown_allocation_is_an_error_not_a_default_godown() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let line = inv_line(PCS, 10, 1_000, Vec::new());
    post(
        &mut store,
        vec![voucher(1, SALES, date(2026, Month::April, 5), vec![line])],
    );
    assert_error_names(&stock_moves(&store, None), 1, "no godown allocation");
}

#[test]
fn unknown_unit_item_godown_batch_and_voucher_type_are_errors() {
    // Unit kg exists but is neither the base unit, the alternate unit nor a compound of pcs.
    let kg = inv_line(KG, 1, 100, vec![alloc(1, None, 1, 100)]);
    let missing_godown = inv_line(PCS, 1, 100, vec![alloc(99, None, 1, 100)]);
    let missing_batch = inv_line(PCS, 1, 100, vec![alloc(1, Some(99), 1, 100)]);
    let mut missing_item = inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)]);
    missing_item.item = StockItemId(99);
    let on = date(2026, Month::April, 5);
    let cases = [
        (1, SALES, kg, "cannot be converted"),
        (2, SALES, missing_godown, "godown"),
        (3, SALES, missing_batch, "batch"),
        (4, SALES, missing_item, "stock item"),
        (
            5,
            99,
            inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)]),
            "voucher type",
        ),
    ];
    for (id, type_id, line, needle) in cases {
        let dir2 = tempfile::tempdir().unwrap();
        let mut one = new_store(dir2.path());
        post(&mut one, vec![voucher(id, type_id, on, vec![line])]);
        assert_error_names(&stock_moves(&one, None), id, needle);
    }
}

// Replaces the round-2 default, under which these vouchers failed the load with StoreError::Encoding.
// Each case is refused with the typed StoreError::Unsupported because its stock effect is not
// documented or the model cannot express it. Provenance (derived, read 2026-11-21):
// - stock journal: a journal line has no source or destination side in `InventoryLine`, so the
//   stock-transfer page (https://help.tallysolutions.com/stock-transfer-between-godowns/) cannot be
//   applied to the stored data; interface request filed.
// - debit note with items: the purchase-return page
//   (https://help.tallysolutions.com/tally-prime/purchase-process/purchase-returns-tally/) names no
//   quantity effect.
// - post-dated: the post-dated page body was not returned by the fetch; no read page states its
//   stock effect.
// - journal (a non-stock kind): BaseVoucherKind docs say a journal does not move stock; a journal
//   with items is therefore refused rather than ignored.
#[test]
fn unsupported_vouchers_are_typed_unsupported_errors_that_name_the_voucher() {
    let on = date(2026, Month::April, 5);
    let line = || inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)]);
    let mut post_dated = voucher(4, SALES, on, vec![line()]);
    post_dated.status = VoucherStatus::PostDated;
    let cases = [
        (voucher(1, STOCK_JOURNAL, on, vec![line()]), "stock journal"),
        (voucher(2, DEBIT_NOTE, on, vec![line()]), "debit note"),
        (post_dated, "post-dated"),
        (voucher(5, JOURNAL, on, vec![line()]), "does not move stock"),
    ];
    for (v, needle) in cases {
        let id = v.id.0;
        let dir = tempfile::tempdir().unwrap();
        let mut store = new_store(dir.path());
        post(&mut store, vec![v]);
        assert_unsupported(&stock_moves(&store, None), id, needle);
    }
}

// provenance: derived. Replaces the round-2 default (credit note with items refused). The
// sales-return page (https://help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/,
// read 2026-11-21) states in its Q&A that a sales return increases stock by the returned quantity.
// The godown is the one the voucher allocates. 3 pcs returned for 1,000 paise: qty_in 3, value 1,000.
#[test]
fn credit_note_with_items_is_a_stock_in_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let line = inv_line(PCS, 3, 1_000, vec![alloc(1, None, 3, 1_000)]);
    post(
        &mut store,
        vec![voucher(1, CREDIT_NOTE, date(2026, Month::April, 5), vec![line])],
    );
    assert_eq!(
        stock_moves(&store, None).unwrap(),
        vec![StockMove {
            voucher: VoucherId(1),
            date: date(2026, Month::April, 5),
            item: StockItemId(1),
            godown: GodownId(1),
            batch: None,
            qty_in: qty(3),
            qty_out: Qty::ZERO,
            value: Paise::new(1_000),
            status: VoucherStatus::Regular,
        }]
    );
}

// provenance: derived. The optional-voucher page (https://help.tallysolutions.com/create-optional-voucher/,
// read 2026-11-21) says an optional voucher records transactions "without affecting" the books. It
// is silent on stock, so the exclusion from actual stock is this card's rule (owner brief), applied
// to that statement. Expected: purchase 5 pcs regular (qty_in 5, value 500) and optional sale 2 pcs
// (qty_out 2, value 200). The carried list has both; actual stock (load_books) has only the purchase.
#[test]
fn optional_sale_is_carried_but_is_not_actual_stock() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let purchase = voucher(
        1,
        PURCHASE,
        date(2026, Month::April, 4),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    let mut optional = voucher(
        2,
        SALES,
        date(2026, Month::April, 5),
        vec![inv_line(PCS, 2, 200, vec![alloc(1, None, 2, 200)])],
    );
    optional.status = VoucherStatus::Optional;
    post(&mut store, vec![purchase, optional]);

    let carried = stock_moves(&store, None).unwrap();
    assert_eq!(carried.len(), 2);
    assert_eq!(carried[1].voucher, VoucherId(2));
    assert_eq!(carried[1].status, VoucherStatus::Optional);
    assert_eq!(carried[1].qty_out, qty(2));
    assert_eq!(carried[1].value, Paise::new(200));

    let actual = store.load_books(None).unwrap().stock_moves;
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].voucher, VoucherId(1));
    assert_eq!(actual[0].status, VoucherStatus::Regular);
}

// provenance: derived. The memorandum page (https://help.tallysolutions.com/?p=193988, read
// 2026-11-21) says a memorandum voucher does not affect the books of accounts; it is silent on
// stock. Its direction is not decided, so it is neither carried nor counted: the load must succeed
// and actual stock must be empty.
#[test]
fn memorandum_with_items_is_excluded_and_does_not_fail_the_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let memo = voucher(
        1,
        MEMORANDUM,
        date(2026, Month::April, 5),
        vec![inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)])],
    );
    post(&mut store, vec![memo]);
    assert_eq!(stock_moves(&store, None).unwrap(), Vec::<StockMove>::new());
    assert!(store.load_books(None).unwrap().stock_moves.is_empty());
}

// provenance: derived. The physical-stock page (https://help.tallysolutions.com/physical-stock/,
// read 2026-11-21) says the books are adjusted to match the physical count. Book quantity: 5 pcs
// received on 4 Apr, no other movement. Count 5 pcs on 6 Apr: adjustment 5 - 5 = 0, so no move.
#[test]
fn physical_stock_equal_to_the_book_quantity_gives_no_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let purchase = voucher(
        1,
        PURCHASE,
        date(2026, Month::April, 4),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    let count = voucher(
        2,
        PHYSICAL_STOCK,
        date(2026, Month::April, 6),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    post(&mut store, vec![purchase, count]);
    let got: Vec<u64> = stock_moves(&store, None)
        .unwrap()
        .iter()
        .map(|m| m.voucher.0)
        .collect();
    assert_eq!(got, vec![1]);
}

// provenance: derived. Book quantity 5 pcs (receipt on 4 Apr), count 3 pcs on 6 Apr: the adjustment
// is 3 - 5 = -2 pcs. The physical-stock page does not state the value of an adjustment, so a
// non-zero adjustment is refused as unsupported rather than valued by a guessed rule.
#[test]
fn physical_stock_that_differs_from_the_book_is_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let purchase = voucher(
        1,
        PURCHASE,
        date(2026, Month::April, 4),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    let count = voucher(
        2,
        PHYSICAL_STOCK,
        date(2026, Month::April, 6),
        vec![inv_line(PCS, 3, 300, vec![alloc(1, None, 3, 300)])],
    );
    post(&mut store, vec![purchase, count]);
    assert_unsupported(&stock_moves(&store, None), 2, "adjustment");
}

// provenance: derived. The page does not say whether a count is applied before or after another
// movement on the same date, so a count sharing item and godown with another movement that day is
// refused. Purchase 5 pcs and count 5 pcs both on 4 Apr in godown 1.
#[test]
fn physical_stock_on_the_same_day_as_another_movement_is_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let purchase = voucher(
        1,
        PURCHASE,
        date(2026, Month::April, 4),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    let count = voucher(
        2,
        PHYSICAL_STOCK,
        date(2026, Month::April, 4),
        vec![inv_line(PCS, 5, 500, vec![alloc(1, None, 5, 500)])],
    );
    post(&mut store, vec![purchase, count]);
    assert_unsupported(&stock_moves(&store, None), 2, "same date");
}

// provenance: derived. Opening stock is the book quantity at books-begin. Opening 4 pcs in godown 1
// (set by an alter of the item), count 4 pcs on 5 Apr: adjustment 0, so no move. Without the opening
// stock the book would be 0 and the count would be refused, so this checks that opening is counted.
#[test]
fn physical_stock_counts_opening_stock_as_the_book_quantity() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    let before = store
        .load_books(None)
        .unwrap()
        .masters
        .stock_items
        .get(&StockItemId(1))
        .unwrap()
        .clone();
    let after = StockItem {
        opening: vec![OpeningStock {
            godown: GodownId(1),
            batch: None,
            qty: qty(4),
            rate: Rate(Decimal::ONE),
        }],
        ..before.clone()
    };
    store
        .commit(
            &[Event::MasterAltered {
                before: MasterSnapshot::StockItem(before),
                after: MasterSnapshot::StockItem(after),
            }],
            &meta(),
        )
        .unwrap();
    let count = voucher(
        1,
        PHYSICAL_STOCK,
        date(2026, Month::April, 5),
        vec![inv_line(PCS, 4, 400, vec![alloc(1, None, 4, 400)])],
    );
    post(&mut store, vec![count]);
    assert_eq!(stock_moves(&store, None).unwrap(), Vec::<StockMove>::new());
}

#[test]
fn compound_unit_is_converted_only_when_it_is_defined_on_that_unit() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // Unit 5 is a compound record whose first unit is Dozen (1 Dozen = 12 pcs). It describes
    // dozens, not unit 5, so a line in unit 5 must not be converted with its factor.
    // provenance: derived; the record's own `first` must be the unit being converted (the BOX
    // fixture has first = BOX), so the conversion of unit 5 is not defined by the data.
    store
        .commit(
            &[Event::MasterCreated(unit(
                5,
                "xbox",
                UnitKind::Compound {
                    first: UnitId(DOZEN),
                    factor: Decimal::from(12),
                    second: UnitId(PCS),
                },
            ))],
            &meta(),
        )
        .unwrap();
    let line = inv_line(5, 2, 4_800, vec![alloc(1, None, 2, 4_800)]);
    post(
        &mut store,
        vec![voucher(
            1,
            PURCHASE,
            date(2026, Month::April, 5),
            vec![line],
        )],
    );
    assert_error_names(&stock_moves(&store, None), 1, "cannot be converted");
}

#[test]
fn load_books_reports_the_same_stock_moves_as_stock_moves() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; the item invoice fixture of `item_invoice_with_two_godown_allocations`:
    // 6 pcs from godown 1 (Rs 300 = 30,000 paise) and 4 pcs from godown 2 batch 1 (20,000 paise).
    let line = inv_line(
        PCS,
        10,
        50_000,
        vec![alloc(1, None, 6, 30_000), alloc(2, Some(1), 4, 20_000)],
    );
    post(
        &mut store,
        vec![voucher(1, SALES, date(2026, Month::April, 5), vec![line])],
    );
    let want = vec![
        StockMove {
            voucher: VoucherId(1),
            date: date(2026, Month::April, 5),
            item: StockItemId(1),
            godown: GodownId(1),
            batch: None,
            qty_in: Qty::ZERO,
            qty_out: qty(6),
            value: Paise::new(30_000),
            status: VoucherStatus::Regular,
        },
        StockMove {
            voucher: VoucherId(1),
            date: date(2026, Month::April, 5),
            item: StockItemId(1),
            godown: GodownId(2),
            batch: Some(BatchId(1)),
            qty_in: Qty::ZERO,
            qty_out: qty(4),
            value: Paise::new(20_000),
            status: VoucherStatus::Regular,
        },
    ];
    assert_eq!(stock_moves(&store, None).unwrap(), want);
    assert_eq!(store.load_books(None).unwrap().stock_moves, want);
}

#[test]
fn load_books_fails_when_a_voucher_cannot_be_projected_instead_of_leaving_its_stock_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = new_store(dir.path());
    // provenance: derived; a stock journal has no direction per line, so the projection is
    // unsupported (module documentation); load_books must fail naming it, not return no moves.
    let line = inv_line(PCS, 1, 100, vec![alloc(1, None, 1, 100)]);
    post(
        &mut store,
        vec![voucher(
            1,
            STOCK_JOURNAL,
            date(2026, Month::April, 5),
            vec![line],
        )],
    );
    let result = store.load_books(None);
    let message = match &result {
        Err(StoreError::Encoding(m)) => m.clone(),
        other => panic!("expected an encoding error, got {:?}", other.as_ref().err()),
    };
    assert!(
        message.contains("VoucherId(1)") && message.contains("stock journal"),
        "{message}"
    );
}
