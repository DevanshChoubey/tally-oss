//! Card E2 tests: inventory and cost master commands.
//!
//! Provenance for every expected value is `derived`, by hand, from the rule each test names:
//! spec section 6.4 (units, items, godowns, batches), research R3 "Data-model" (compound units are
//! acyclic and immutable once used; a batch's expiry is not before its manufacturing date; item
//! GST history is effective-dated), engineering rule A1 (an error or a value, never a guess) and
//! the audit rule of spec 6.6 (every create, alter and delete is one event, nothing else is
//! written). The expected result of each case is the error class or the event list that the
//! stated rule dictates; no value was copied from the engine's output.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, CompanyId, CostCategoryId, CostCentreId, Date, Decimal, DrCr, GodownId, LedgerId,
    Outcome, Paise, Qty, Rate, SignedAmount, StockGroupId, StockItemId, UnitId, VoucherId,
    VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_engine::masters_stock::{
    alter_batch, alter_cost_category, alter_cost_centre, alter_godown, alter_stock_group,
    alter_stock_item, alter_unit, create_batch, create_cost_category, create_cost_centre,
    create_godown, create_stock_group, create_stock_item, create_unit, delete_batch,
    delete_cost_category, delete_cost_centre, delete_godown, delete_stock_group, delete_stock_item,
    delete_unit,
};
use ledgerly_model::{
    Address, AltUnit, Batch, Books, Company, CostAllocation, CostCategory, CostCentre, EntityKind,
    Features, Godown, GodownAllocation, GstHistory, InventoryLine, LedgerLine, MasterSnapshot,
    Masters, NegativeStockPolicy, OpeningStock, StockGroup, StockItem, StockMove, Taxability, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_store::Event;
use time::macros::date;

// ---------------------------------------------------------------------------------------------
// Fixtures (fictional data)
// ---------------------------------------------------------------------------------------------

fn base() -> Books {
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: ledgerly_core::StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2025 - 04 - 01),
            books_begin: date!(2025 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters: Masters::default(),
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn unit(id: u64, symbol: &str) -> Unit {
    Unit {
        id: UnitId(id),
        symbol: symbol.to_owned(),
        formal_name: symbol.to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    }
}

fn compound(id: u64, symbol: &str, first: u64, factor: i64, second: u64) -> Unit {
    Unit {
        kind: UnitKind::Compound {
            first: UnitId(first),
            factor: Decimal::from(factor),
            second: UnitId(second),
        },
        ..unit(id, symbol)
    }
}

fn group(id: u64, name: &str, parent: Option<u64>) -> StockGroup {
    StockGroup {
        id: StockGroupId(id),
        name: name.to_owned(),
        parent: parent.map(StockGroupId),
        tally: None,
    }
}

fn item(id: u64, name: &str, unit_id: u64) -> StockItem {
    StockItem {
        id: StockItemId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UnitId(unit_id),
        alt_unit: None,
        hsn: None,
        gst_history: Vec::new(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn godown(id: u64, name: &str, parent: Option<u64>) -> Godown {
    Godown {
        id: GodownId(id),
        name: name.to_owned(),
        parent: parent.map(GodownId),
        tally: None,
    }
}

fn batch(id: u64, item_id: u64, name: &str) -> Batch {
    Batch {
        id: BatchId(id),
        item: StockItemId(item_id),
        name: name.to_owned(),
        mfg: None,
        expiry: None,
    }
}

fn category(id: u64, name: &str) -> CostCategory {
    CostCategory {
        id: CostCategoryId(id),
        name: name.to_owned(),
    }
}

fn centre(id: u64, name: &str, category_id: u64, parent: Option<u64>) -> CostCentre {
    CostCentre {
        id: CostCentreId(id),
        name: name.to_owned(),
        category: CostCategoryId(category_id),
        parent: parent.map(CostCentreId),
    }
}

fn qty(n: i64, scale: u32) -> Qty {
    Qty(Decimal::new(n, scale))
}

fn opening(godown_id: u64, batch_id: Option<u64>, q: Qty) -> OpeningStock {
    OpeningStock {
        godown: GodownId(godown_id),
        batch: batch_id.map(BatchId),
        qty: q,
        rate: Rate(Decimal::from(10)),
    }
}

fn put_unit(b: &mut Books, u: Unit) {
    b.masters.units.insert(u.id, u);
}
fn put_group(b: &mut Books, g: StockGroup) {
    b.masters.stock_groups.insert(g.id, g);
}
fn put_item(b: &mut Books, i: StockItem) {
    b.masters.stock_items.insert(i.id, i);
}
fn put_godown(b: &mut Books, g: Godown) {
    b.masters.godowns.insert(g.id, g);
}
fn put_batch(b: &mut Books, x: Batch) {
    b.masters.batches.insert(x.id, x);
}
fn put_category(b: &mut Books, c: CostCategory) {
    b.masters.cost_categories.insert(c.id, c);
}
fn put_centre(b: &mut Books, c: CostCentre) {
    b.masters.cost_centres.insert(c.id, c);
}

fn voucher(inventory: Vec<InventoryLine>, ledger: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: date!(2025 - 05 - 01),
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
        ledger_lines: ledger,
        inventory_lines: inventory,
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn inv_line(
    item_id: u64,
    unit_id: u64,
    godown_id: Option<u64>,
    batch_id: Option<u64>,
) -> InventoryLine {
    InventoryLine {
        item: StockItemId(item_id),
        qty: qty(1, 0),
        unit: UnitId(unit_id),
        rate: Rate(Decimal::from(10)),
        discount_percent: None,
        amount: Paise::new(1000),
        ledger: None,
        godowns: godown_id
            .map(|g| GodownAllocation {
                godown: GodownId(g),
                batch: batch_id.map(BatchId),
                qty: qty(1, 0),
                amount: Paise::new(1000),
            })
            .into_iter()
            .collect(),
    }
}

fn cost_line(category_id: u64, centre_id: u64) -> LedgerLine {
    LedgerLine {
        ledger: LedgerId(1),
        amount: SignedAmount {
            side: DrCr::Dr,
            amount: Paise::new(1000),
        },
        bills: Vec::new(),
        cost: vec![CostAllocation {
            category: CostCategoryId(category_id),
            centre: CostCentreId(centre_id),
            amount: Paise::new(1000),
        }],
        bank: None,
    }
}

fn put_voucher(b: &mut Books, v: Voucher) {
    b.vouchers.insert(v.id, v);
}

fn is_invalid<T: std::fmt::Debug>(r: &Result<T, EngineError>) -> bool {
    matches!(r, Err(EngineError::Invalid { .. }))
}

fn only(events: Outcome<Vec<Event>>) -> Event {
    let mut v = events.value().unwrap();
    assert_eq!(v.len(), 1, "exactly one event");
    v.remove(0)
}

// ---------------------------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------------------------

#[test]
fn compound_unit_referencing_itself_errors() {
    // provenance: derived - R3 Data-model invariant "compound graphs are acyclic": 1 box = 12 box
    // where the box is the unit being defined is a cycle of length one.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    assert!(is_invalid(&create_unit(compound(2, "box", 2, 12, 1), &b)));
    assert!(is_invalid(&create_unit(compound(2, "box", 1, 12, 2), &b)));
}

#[test]
fn compound_unit_cycle_through_other_units_errors() {
    // provenance: derived - R3 invariant "compound graphs are acyclic". Set up: A = 10 x B (B, C
    // simple). Altering B into "1 B = 2 x A" would give A -> B -> A, a cycle of length two.
    let mut b = base();
    put_unit(&mut b, unit(2, "B"));
    put_unit(&mut b, unit(3, "C"));
    put_unit(&mut b, compound(1, "A", 2, 10, 3));
    // C also refers to nothing, so only the B alteration closes the loop.
    let r = alter_unit(compound(2, "B", 1, 2, 3), &b);
    assert!(is_invalid(&r), "{r:?}");
}

#[test]
fn compound_unit_with_bad_parts_errors() {
    // provenance: derived - a compound unit "1 first = factor x second" needs a factor above zero
    // (a zero or negative count of pieces is meaningless), two different units, and both must exist.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    put_unit(&mut b, unit(2, "kg"));
    assert!(is_invalid(&create_unit(compound(3, "box", 2, 0, 1), &b)));
    assert!(is_invalid(&create_unit(compound(3, "box", 2, -12, 1), &b)));
    assert!(is_invalid(&create_unit(compound(3, "box", 1, 12, 1), &b)));
    assert!(matches!(
        create_unit(compound(3, "box", 2, 12, 99), &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn compound_units_may_nest() {
    // provenance: derived - R3 "Compound units ... can be nested to many levels": "box of 12 pcs"
    // is (box, 12, pcs); "carton of 4 boxes of 12" is (carton, 4, that compound unit). An acyclic
    // chain of two levels is valid.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    put_unit(&mut b, unit(2, "box"));
    put_unit(&mut b, unit(3, "carton"));
    put_unit(&mut b, compound(4, "box of 12", 2, 12, 1));
    assert!(create_unit(compound(5, "carton of 4 boxes", 3, 4, 4), &b).is_ok());
}

#[test]
fn create_unit_yields_one_created_event() {
    // provenance: derived - spec 6.6, one event per create, carrying the new master.
    let b = base();
    let u = unit(1, "pcs");
    let out = create_unit(u.clone(), &b).unwrap();
    assert_eq!(only(out), Event::MasterCreated(MasterSnapshot::Unit(u)));
}

#[test]
fn unit_symbols_are_unique_ignoring_case_and_padding() {
    // provenance: derived - a symbol names a unit on screen and in the HSN summary; "PCS" and
    // " pcs " read as the same name, so the second is a duplicate. Same id twice is a duplicate too.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    assert!(matches!(
        create_unit(unit(2, " PCS "), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(matches!(
        create_unit(unit(1, "kg"), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(is_invalid(&create_unit(unit(3, "  "), &b)));
}

#[test]
fn alter_unit_rename_ok_and_unchanged_yields_no_event() {
    // provenance: derived - spec 6.6: an alter is one MasterAltered event with before and after
    // from the books; an alter that changes nothing writes nothing.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    let mut renamed = unit(1, "nos");
    renamed.formal_name = "Numbers".to_owned();
    let out = alter_unit(renamed.clone(), &b).unwrap();
    assert_eq!(
        only(out),
        Event::MasterAltered {
            before: MasterSnapshot::Unit(unit(1, "pcs")),
            after: MasterSnapshot::Unit(renamed),
        }
    );
    assert_eq!(
        alter_unit(unit(1, "pcs"), &b).unwrap(),
        Outcome::Value(Vec::new())
    );
    assert!(matches!(
        alter_unit(unit(9, "x"), &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn used_unit_cannot_change_kind_or_shrink_decimals() {
    // provenance: derived - R3 "A compound unit is immutable once used": item 1 uses unit 1 (pcs),
    // so turning it compound, or cutting its decimals below those the stock was entered with, is
    // refused; growing the decimals is harmless.
    let mut b = base();
    let mut kg = unit(1, "kg");
    kg.decimals = 3;
    put_unit(&mut b, kg.clone());
    put_unit(&mut b, unit(2, "g"));
    put_unit(&mut b, unit(3, "mg"));
    put_item(&mut b, item(1, "Rice", 1));
    assert!(is_invalid(&alter_unit(compound(1, "kg", 2, 1000, 3), &b)));
    let mut fewer = kg.clone();
    fewer.decimals = 2;
    assert!(is_invalid(&alter_unit(fewer, &b)));
    let mut more = kg;
    more.decimals = 4;
    assert!(alter_unit(more, &b).is_ok());
}

#[test]
fn delete_unit_in_use_errors_and_unused_deletes() {
    // provenance: derived - spec 6.4 / rule A1: a master that items, compound units or voucher
    // lines refer to cannot disappear; an unused one is deleted with one MasterDeleted event.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    put_unit(&mut b, unit(2, "doz"));
    put_unit(&mut b, unit(3, "box"));
    put_unit(&mut b, compound(4, "case", 3, 12, 1));
    put_unit(&mut b, unit(5, "spare"));
    put_unit(&mut b, unit(6, "tray"));
    put_item(&mut b, item(1, "Pen", 2));
    assert!(
        is_invalid(&delete_unit(UnitId(2), &b)),
        "base unit of an item"
    );
    assert!(
        is_invalid(&delete_unit(UnitId(1), &b)),
        "part of a compound unit"
    );
    let mut with_alt = item(2, "Ink", 5);
    with_alt.alt_unit = Some(AltUnit {
        unit: UnitId(6),
        base_per_alt: Decimal::from(6),
    });
    put_item(&mut b, with_alt);
    assert!(
        is_invalid(&delete_unit(UnitId(6), &b)),
        "alternate unit of an item"
    );
    put_voucher(
        &mut b,
        voucher(vec![inv_line(1, 5, None, None)], Vec::new()),
    );
    assert_eq!(
        delete_unit(UnitId(5), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    assert!(matches!(
        delete_unit(UnitId(77), &b),
        Err(EngineError::NotFound { .. })
    ));
    let mut clean = base();
    put_unit(&mut clean, unit(1, "pcs"));
    assert_eq!(
        only(delete_unit(UnitId(1), &clean).unwrap()),
        Event::MasterDeleted(MasterSnapshot::Unit(unit(1, "pcs")))
    );
}

// ---------------------------------------------------------------------------------------------
// Stock groups
// ---------------------------------------------------------------------------------------------

#[test]
fn stock_group_cannot_be_its_own_ancestor() {
    // provenance: derived - a group tree has no cycles. Chain 1 <- 2 <- 3 (3 inside 2 inside 1):
    // moving 1 under 3 closes the loop; a group under itself is the shortest loop.
    let mut b = base();
    put_group(&mut b, group(1, "Goods", None));
    put_group(&mut b, group(2, "Raw", Some(1)));
    put_group(&mut b, group(3, "Grain", Some(2)));
    assert_eq!(
        alter_stock_group(group(1, "Goods", Some(3)), &b),
        Err(EngineError::GroupCycle)
    );
    assert_eq!(
        alter_stock_group(group(2, "Raw", Some(2)), &b),
        Err(EngineError::GroupCycle)
    );
    assert_eq!(
        create_stock_group(group(4, "Own", Some(4)), &b),
        Err(EngineError::GroupCycle)
    );
    assert!(alter_stock_group(group(3, "Grain", Some(1)), &b).is_ok());
}

#[test]
fn stock_group_names_unique_and_parent_must_exist() {
    // provenance: derived - names identify masters (case-insensitive, trimmed); a parent must exist.
    let mut b = base();
    put_group(&mut b, group(1, "Goods", None));
    assert!(matches!(
        create_stock_group(group(2, "goods ", None), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(matches!(
        create_stock_group(group(2, "Raw", Some(9)), &b),
        Err(EngineError::NotFound { .. })
    ));
    assert!(is_invalid(&create_stock_group(group(2, "", None), &b)));
    assert_eq!(
        only(create_stock_group(group(2, "Raw", Some(1)), &b).unwrap()),
        Event::MasterCreated(MasterSnapshot::StockGroup(group(2, "Raw", Some(1))))
    );
}

#[test]
fn stock_group_with_children_or_items_cannot_be_deleted() {
    // provenance: derived - deleting a group that still holds sub-groups or items would orphan them.
    let mut b = base();
    put_unit(&mut b, unit(1, "pcs"));
    put_group(&mut b, group(1, "Goods", None));
    put_group(&mut b, group(2, "Raw", Some(1)));
    let mut it = item(1, "Pen", 1);
    it.group = Some(StockGroupId(2));
    put_item(&mut b, it);
    assert!(is_invalid(&delete_stock_group(StockGroupId(1), &b)));
    assert!(is_invalid(&delete_stock_group(StockGroupId(2), &b)));
    b.masters.stock_items.clear();
    assert!(delete_stock_group(StockGroupId(2), &b).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Stock items
// ---------------------------------------------------------------------------------------------

fn books_with_units() -> Books {
    let mut b = base();
    let mut pcs = unit(1, "pcs");
    pcs.decimals = 2;
    put_unit(&mut b, pcs);
    put_unit(&mut b, unit(2, "box"));
    put_godown(&mut b, godown(1, "Main", None));
    b
}

#[test]
fn item_names_and_aliases_share_one_namespace() {
    // provenance: derived - R3: barcodes live in the alias field and a scan selects one item, so a
    // name or alias that another item already answers to would make a scan ambiguous.
    let mut b = books_with_units();
    let mut pen = item(1, "Pen", 1);
    pen.aliases = vec!["8901234567890".to_owned()];
    put_item(&mut b, pen);
    assert!(matches!(
        create_stock_item(item(2, "pen", 1), &b),
        Err(EngineError::Duplicate { .. })
    ));
    let mut by_alias = item(2, "Pencil", 1);
    by_alias.aliases = vec!["8901234567890".to_owned()];
    assert!(matches!(
        create_stock_item(by_alias, &b),
        Err(EngineError::Duplicate { .. })
    ));
    let mut twice = item(2, "Pencil", 1);
    twice.aliases = vec!["P1".to_owned(), "p1".to_owned()];
    assert!(matches!(
        create_stock_item(twice, &b),
        Err(EngineError::Duplicate { .. })
    ));
    let mut own_name = item(2, "Pencil", 1);
    own_name.aliases = vec!["PENCIL".to_owned()];
    assert!(matches!(
        create_stock_item(own_name, &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(create_stock_item(item(2, "Pencil", 1), &b).is_ok());
    // Altering an item keeps its own aliases without clashing with itself.
    let mut same = item(1, "Pen", 1);
    same.aliases = vec!["8901234567890".to_owned(), "Biro".to_owned()];
    assert!(alter_stock_item(same, &b).is_ok());
}

#[test]
fn item_references_must_exist_and_alt_unit_must_make_sense() {
    // provenance: derived - spec 6.4: an item has a base unit and an optional alternate unit with
    // a conversion "base_per_alt base units make one alternate unit"; that factor is above zero and
    // the alternate unit differs from the base unit.
    let b = books_with_units();
    assert!(matches!(
        create_stock_item(item(1, "Pen", 9), &b),
        Err(EngineError::NotFound { .. })
    ));
    let mut grouped = item(1, "Pen", 1);
    grouped.group = Some(StockGroupId(9));
    assert!(matches!(
        create_stock_item(grouped, &b),
        Err(EngineError::NotFound { .. })
    ));
    for (alt, per, ok) in [(2, 12, true), (1, 12, false), (2, 0, false), (2, -1, false)] {
        let mut it = item(1, "Pen", 1);
        it.alt_unit = Some(AltUnit {
            unit: UnitId(alt),
            base_per_alt: Decimal::from(per),
        });
        assert_eq!(create_stock_item(it, &b).is_ok(), ok, "alt {alt} per {per}");
    }
    let mut missing_alt = item(1, "Pen", 1);
    missing_alt.alt_unit = Some(AltUnit {
        unit: UnitId(9),
        base_per_alt: Decimal::from(2),
    });
    assert!(matches!(
        create_stock_item(missing_alt, &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn item_gst_history_is_dated_in_order_and_taxable_needs_a_rate() {
    // provenance: derived - rule A8 / spec 5: GST details are effective-dated, one entry per date
    // in date order; a taxable entry needs a rate in 0..=100 percent, a non-taxable one has none.
    let b = books_with_units();
    let entry = |from: Date, t: Taxability, r: Option<i64>| GstHistory {
        effective_from: from,
        taxability: t,
        rate_percent: r.map(|n| Rate(Decimal::from(n))),
    };
    let with = |h: Vec<GstHistory>| {
        let mut it = item(1, "Pen", 1);
        it.gst_history = h;
        create_stock_item(it, &b)
    };
    let d1 = date!(2025 - 04 - 01);
    let d2 = date!(2025 - 09 - 22);
    assert!(
        with(vec![
            entry(d1, Taxability::Taxable, Some(12)),
            entry(d2, Taxability::Taxable, Some(5)),
        ])
        .is_ok()
    );
    assert!(with(vec![entry(d1, Taxability::Exempt, None)]).is_ok());
    assert!(is_invalid(&with(vec![
        entry(d2, Taxability::Taxable, Some(5)),
        entry(d1, Taxability::Taxable, Some(12)),
    ])));
    assert!(is_invalid(&with(vec![
        entry(d1, Taxability::Taxable, Some(5)),
        entry(d1, Taxability::Taxable, Some(12)),
    ])));
    assert!(is_invalid(&with(vec![entry(
        d1,
        Taxability::Taxable,
        None
    )])));
    assert!(is_invalid(&with(vec![entry(
        d1,
        Taxability::Taxable,
        Some(101)
    )])));
    assert!(is_invalid(&with(vec![entry(
        d1,
        Taxability::Taxable,
        Some(-1)
    )])));
    assert!(is_invalid(&with(vec![entry(
        d1,
        Taxability::NilRated,
        Some(0)
    )])));
    assert!(is_invalid(&with(vec![entry(
        d1,
        Taxability::NonGst,
        Some(5)
    )])));
}

#[test]
fn item_opening_stock_is_checked() {
    // provenance: derived - opening stock is entered per godown (and batch) in base units: the
    // godown must exist, a quantity cannot be negative, one line per godown and batch, and a
    // quantity cannot be finer than the base unit's decimals (unit 1 has 2 places: 1.25 ok, 1.255 not;
    // 1.250 is the same number as 1.25).
    let b = books_with_units();
    let with = |o: Vec<OpeningStock>| {
        let mut it = item(1, "Pen", 1);
        it.opening = o;
        create_stock_item(it, &b)
    };
    assert!(with(vec![opening(1, None, qty(125, 2))]).is_ok());
    assert!(with(vec![opening(1, None, qty(1250, 3))]).is_ok());
    assert!(is_invalid(&with(vec![opening(1, None, qty(1255, 3))])));
    assert!(is_invalid(&with(vec![opening(1, None, qty(-1, 0))])));
    assert!(is_invalid(&with(vec![
        opening(1, None, qty(1, 0)),
        opening(1, None, qty(2, 0)),
    ])));
    assert!(matches!(
        with(vec![opening(9, None, qty(1, 0))]),
        Err(EngineError::NotFound { .. })
    ));
    assert!(matches!(
        with(vec![opening(1, Some(9), qty(1, 0))]),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn used_item_cannot_change_units_and_batch_tracking_needs_no_batches() {
    // provenance: derived - a voucher line records its quantity in the item's unit, so changing the
    // unit under existing lines would silently change what the stored quantity means; an item
    // cannot stop tracking batches while batches of it exist.
    let mut b = books_with_units();
    put_item(&mut b, item(1, "Pen", 1));
    assert!(
        alter_stock_item(item(1, "Pen", 2), &b).is_ok(),
        "unused: may change unit"
    );
    put_voucher(
        &mut b,
        voucher(vec![inv_line(1, 1, Some(1), None)], Vec::new()),
    );
    assert!(is_invalid(&alter_stock_item(item(1, "Pen", 2), &b)));
    assert!(alter_stock_item(item(1, "Pen renamed", 1), &b).is_ok());

    let mut tracked = item(2, "Tablet", 1);
    tracked.batches_enabled = true;
    put_item(&mut b, tracked);
    put_batch(&mut b, batch(1, 2, "B1"));
    assert!(is_invalid(&alter_stock_item(item(2, "Tablet", 1), &b)));
}

#[test]
fn item_alter_event_carries_before_and_after_from_books() {
    // provenance: derived - spec 6.6.
    let mut b = books_with_units();
    put_item(&mut b, item(1, "Pen", 1));
    let after = item(1, "Pen (blue)", 1);
    assert_eq!(
        only(alter_stock_item(after.clone(), &b).unwrap()),
        Event::MasterAltered {
            before: MasterSnapshot::StockItem(item(1, "Pen", 1)),
            after: MasterSnapshot::StockItem(after),
        }
    );
}

#[test]
fn delete_item_blocked_by_vouchers_moves_and_batches() {
    // provenance: derived - an item that vouchers, stock movements or batches refer to cannot go.
    let mut b = books_with_units();
    let mut tracked = item(1, "Tablet", 1);
    tracked.batches_enabled = true;
    put_item(&mut b, tracked);
    put_item(&mut b, item(2, "Pen", 1));
    put_item(&mut b, item(3, "Ink", 1));
    put_item(&mut b, item(4, "Free", 1));
    put_batch(&mut b, batch(1, 1, "B1"));
    assert!(is_invalid(&delete_stock_item(StockItemId(1), &b)), "batch");
    put_voucher(
        &mut b,
        voucher(vec![inv_line(2, 1, None, None)], Vec::new()),
    );
    assert_eq!(
        delete_stock_item(StockItemId(2), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    b.stock_moves.push(StockMove {
        voucher: VoucherId(9),
        date: date!(2025 - 05 - 01),
        item: StockItemId(3),
        godown: GodownId(1),
        batch: None,
        qty_in: qty(1, 0),
        qty_out: Qty::ZERO,
        value: Paise::new(1000),
    });
    assert!(is_invalid(&delete_stock_item(StockItemId(3), &b)), "move");
    assert!(matches!(
        only(delete_stock_item(StockItemId(4), &b).unwrap()),
        Event::MasterDeleted(MasterSnapshot::StockItem(_))
    ));
}

// ---------------------------------------------------------------------------------------------
// Godowns
// ---------------------------------------------------------------------------------------------

#[test]
fn deleting_a_godown_with_stock_errors() {
    // provenance: derived - R3 "Godowns": stock lives in godowns; deleting one that holds opening
    // stock, voucher allocations or stock movements would orphan that stock.
    let mut b = books_with_units();
    put_godown(&mut b, godown(2, "Shop", None));
    put_godown(&mut b, godown(3, "Van", None));
    put_godown(&mut b, godown(4, "Yard", None));
    let mut pen = item(1, "Pen", 1);
    pen.opening = vec![opening(2, None, qty(5, 0))];
    put_item(&mut b, pen);
    assert!(is_invalid(&delete_godown(GodownId(2), &b)), "opening stock");

    put_voucher(
        &mut b,
        voucher(vec![inv_line(1, 1, Some(3), None)], Vec::new()),
    );
    assert_eq!(
        delete_godown(GodownId(3), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );

    b.stock_moves.push(StockMove {
        voucher: VoucherId(9),
        date: date!(2025 - 05 - 01),
        item: StockItemId(1),
        godown: GodownId(4),
        batch: None,
        qty_in: qty(1, 0),
        qty_out: Qty::ZERO,
        value: Paise::new(1000),
    });
    assert!(is_invalid(&delete_godown(GodownId(4), &b)), "movement");
}

#[test]
fn empty_godown_deletes_with_one_event_and_children_block() {
    // provenance: derived - spec 6.6 one event; a godown with a sub-godown cannot be deleted.
    let mut b = books_with_units();
    put_godown(&mut b, godown(2, "Shop", Some(1)));
    put_godown(&mut b, godown(3, "Spare", None));
    assert!(is_invalid(&delete_godown(GodownId(1), &b)));
    assert_eq!(
        only(delete_godown(GodownId(3), &b).unwrap()),
        Event::MasterDeleted(MasterSnapshot::Godown(godown(3, "Spare", None)))
    );
    assert!(matches!(
        delete_godown(GodownId(99), &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn godown_hierarchy_is_acyclic_and_names_unique() {
    // provenance: derived - R3: godowns are hierarchical; no godown is inside itself.
    let mut b = books_with_units();
    put_godown(&mut b, godown(2, "Shop", Some(1)));
    assert_eq!(
        alter_godown(godown(1, "Main", Some(2)), &b),
        Err(EngineError::GroupCycle)
    );
    assert_eq!(
        create_godown(godown(5, "Loop", Some(5)), &b),
        Err(EngineError::GroupCycle)
    );
    assert!(matches!(
        create_godown(godown(3, "main", None), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(matches!(
        create_godown(godown(3, "Annex", Some(9)), &b),
        Err(EngineError::NotFound { .. })
    ));
    assert!(create_godown(godown(3, "Annex", Some(2)), &b).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Batches
// ---------------------------------------------------------------------------------------------

fn books_with_batch_item() -> Books {
    let mut b = books_with_units();
    let mut tablet = item(1, "Tablet", 1);
    tablet.batches_enabled = true;
    put_item(&mut b, tablet);
    put_item(&mut b, item(2, "Pen", 1));
    b
}

#[test]
fn batch_needs_a_batch_tracking_item_and_sane_dates() {
    // provenance: derived - R3 "Batches": expiry is not before manufacturing date (equal is fine);
    // batches exist only for items that track them; one name per item (another item may reuse it).
    let mut b = books_with_batch_item();
    let mut dated = batch(1, 1, "B1");
    dated.mfg = Some(date!(2025 - 06 - 01));
    dated.expiry = Some(date!(2025 - 05 - 31));
    assert!(is_invalid(&create_batch(dated.clone(), &b)));
    dated.expiry = Some(date!(2025 - 06 - 01));
    assert!(create_batch(dated, &b).is_ok());
    assert!(
        is_invalid(&create_batch(batch(1, 2, "B1"), &b)),
        "untracked item"
    );
    assert!(matches!(
        create_batch(batch(1, 9, "B1"), &b),
        Err(EngineError::NotFound { .. })
    ));
    assert!(is_invalid(&create_batch(batch(1, 1, " "), &b)));
    put_batch(&mut b, batch(1, 1, "B1"));
    assert!(matches!(
        create_batch(batch(2, 1, "b1"), &b),
        Err(EngineError::Duplicate { .. })
    ));
    let mut other = item(3, "Syrup", 1);
    other.batches_enabled = true;
    put_item(&mut b, other);
    assert!(create_batch(batch(2, 3, "B1"), &b).is_ok());
}

#[test]
fn batch_never_moves_to_another_item() {
    // provenance: derived - stock allocated to a batch belongs to that batch's item.
    let mut b = books_with_batch_item();
    put_batch(&mut b, batch(1, 1, "B1"));
    let mut other = item(3, "Syrup", 1);
    other.batches_enabled = true;
    put_item(&mut b, other);
    assert!(is_invalid(&alter_batch(batch(1, 3, "B1"), &b)));
    let mut dated = batch(1, 1, "B1");
    dated.expiry = Some(date!(2026 - 01 - 31));
    assert!(alter_batch(dated, &b).is_ok());
}

#[test]
fn batch_with_stock_cannot_be_deleted() {
    // provenance: derived - a batch that opening stock, voucher allocations or stock movements use
    // holds stock; an unused batch deletes with one event.
    let mut b = books_with_batch_item();
    put_batch(&mut b, batch(1, 1, "B1"));
    put_batch(&mut b, batch(2, 1, "B2"));
    put_batch(&mut b, batch(3, 1, "B3"));
    put_batch(&mut b, batch(4, 1, "B4"));
    let mut tablet = item(1, "Tablet", 1);
    tablet.batches_enabled = true;
    tablet.opening = vec![opening(1, Some(1), qty(5, 0))];
    put_item(&mut b, tablet);
    assert!(is_invalid(&delete_batch(BatchId(1), &b)), "opening");
    put_voucher(
        &mut b,
        voucher(vec![inv_line(1, 1, Some(1), Some(2))], Vec::new()),
    );
    assert_eq!(
        delete_batch(BatchId(2), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    b.stock_moves.push(StockMove {
        voucher: VoucherId(9),
        date: date!(2025 - 05 - 01),
        item: StockItemId(1),
        godown: GodownId(1),
        batch: Some(BatchId(3)),
        qty_in: qty(1, 0),
        qty_out: Qty::ZERO,
        value: Paise::new(1000),
    });
    assert!(is_invalid(&delete_batch(BatchId(3), &b)), "movement");
    assert_eq!(
        only(delete_batch(BatchId(4), &b).unwrap()),
        Event::MasterDeleted(MasterSnapshot::Batch(batch(4, 1, "B4")))
    );
}

#[test]
fn opening_stock_may_name_a_batch_of_the_item_only() {
    // provenance: derived - a batch named in opening stock must be a batch of that item and the
    // item must track batches.
    let mut b = books_with_batch_item();
    put_batch(&mut b, batch(1, 1, "B1"));
    let mut tablet = item(1, "Tablet", 1);
    tablet.batches_enabled = true;
    tablet.opening = vec![opening(1, Some(1), qty(5, 0))];
    assert!(alter_stock_item(tablet, &b).is_ok());
    let mut pen = item(2, "Pen", 1);
    pen.opening = vec![opening(1, Some(1), qty(5, 0))];
    assert!(matches!(
        alter_stock_item(pen, &b),
        Err(EngineError::NotFound { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// Cost categories and centres
// ---------------------------------------------------------------------------------------------

#[test]
fn cost_category_names_unique_and_delete_blocked_by_centres_and_vouchers() {
    // provenance: derived - a category with centres cannot be deleted; one that voucher lines
    // allocate to cannot be deleted either.
    let mut b = base();
    put_category(&mut b, category(1, "Department"));
    put_category(&mut b, category(2, "Project"));
    put_category(&mut b, category(3, "Region"));
    put_category(&mut b, category(4, "Spare"));
    put_centre(&mut b, centre(1, "Sales", 1, None));
    assert!(matches!(
        create_cost_category(category(9, "department "), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(is_invalid(&create_cost_category(category(9, ""), &b)));
    assert!(is_invalid(&delete_cost_category(CostCategoryId(1), &b)));
    put_voucher(&mut b, voucher(Vec::new(), vec![cost_line(2, 7)]));
    assert_eq!(
        delete_cost_category(CostCategoryId(2), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    assert_eq!(
        only(delete_cost_category(CostCategoryId(4), &b).unwrap()),
        Event::MasterDeleted(MasterSnapshot::CostCategory(category(4, "Spare")))
    );
    assert!(
        alter_cost_category(category(3, "Zone"), &b).is_ok(),
        "rename is fine"
    );
}

#[test]
fn cost_centre_parent_must_share_category_and_never_loop() {
    // provenance: derived - R1 "Cost centres": hierarchical within a category, so a parent lives
    // in the same category; no centre is inside itself.
    let mut b = base();
    put_category(&mut b, category(1, "Department"));
    put_category(&mut b, category(2, "Project"));
    put_centre(&mut b, centre(1, "Sales", 1, None));
    put_centre(&mut b, centre(2, "North", 1, Some(1)));
    put_centre(&mut b, centre(3, "Alpha", 2, None));
    assert!(is_invalid(&create_cost_centre(
        centre(4, "Beta", 2, Some(1)),
        &b
    )));
    assert!(create_cost_centre(centre(4, "Beta", 2, Some(3)), &b).is_ok());
    assert_eq!(
        alter_cost_centre(centre(1, "Sales", 1, Some(2)), &b),
        Err(EngineError::GroupCycle)
    );
    assert_eq!(
        create_cost_centre(centre(5, "Loop", 1, Some(5)), &b),
        Err(EngineError::GroupCycle)
    );
    assert!(matches!(
        create_cost_centre(centre(4, "Beta", 9, None), &b),
        Err(EngineError::NotFound { .. })
    ));
    assert!(matches!(
        create_cost_centre(centre(4, "Beta", 1, Some(9)), &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn cost_centre_names_unique_within_category_only() {
    // provenance: derived - centres are told apart inside their category, so "Alpha" may exist in
    // two categories but not twice in one.
    let mut b = base();
    put_category(&mut b, category(1, "Department"));
    put_category(&mut b, category(2, "Project"));
    put_centre(&mut b, centre(1, "Alpha", 1, None));
    assert!(matches!(
        create_cost_centre(centre(2, "ALPHA", 1, None), &b),
        Err(EngineError::Duplicate { .. })
    ));
    assert!(create_cost_centre(centre(2, "Alpha", 2, None), &b).is_ok());
}

#[test]
fn cost_centre_delete_and_category_change_blocked_by_use() {
    // provenance: derived - a centre with sub-centres or voucher allocations cannot be deleted;
    // moving a centre used by vouchers to another category would change what those lines mean.
    let mut b = base();
    put_category(&mut b, category(1, "Department"));
    put_category(&mut b, category(2, "Project"));
    put_centre(&mut b, centre(1, "Sales", 1, None));
    put_centre(&mut b, centre(2, "North", 1, Some(1)));
    put_centre(&mut b, centre(3, "Ops", 1, None));
    put_centre(&mut b, centre(4, "Spare", 1, None));
    assert!(
        is_invalid(&delete_cost_centre(CostCentreId(1), &b)),
        "has sub-centre"
    );
    assert!(
        is_invalid(&alter_cost_centre(centre(1, "Sales", 2, None), &b)),
        "sub-centres stay behind"
    );
    put_voucher(&mut b, voucher(Vec::new(), vec![cost_line(1, 3)]));
    assert_eq!(
        delete_cost_centre(CostCentreId(3), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    assert_eq!(
        alter_cost_centre(centre(3, "Ops", 2, None), &b),
        Err(EngineError::InUse { vouchers: 1 })
    );
    assert!(alter_cost_centre(centre(3, "Operations", 1, None), &b).is_ok());
    assert_eq!(
        only(delete_cost_centre(CostCentreId(4), &b).unwrap()),
        Event::MasterDeleted(MasterSnapshot::CostCentre(centre(4, "Spare", 1, None)))
    );
}
