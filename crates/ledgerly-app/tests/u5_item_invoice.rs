//! Card U5: item invoice entry. Every expected value is derived by hand and the derivation is
//! written next to it; nothing is copied from this crate's output. The books are a small
//! fictional fixture built here.
//!
//! The GST block is host-supplied (this crate cannot call the engine). The `GstBlock` built in
//! this file is a test double standing in for the engine's answer; its figures are derived by
//! hand from the 18% slab (CGST 9% + SGST 9%) and only check that the screen shows, checks and
//! passes on exactly what it is given. They say nothing about what the real engine returns.
//!
//! Amount-in-words wording ("Rupees ... Only") is card C1's convention, which C1 itself marks as
//! not sourced; these tests only check the number words, which are derived from the amount.
//!
//! Pending (no provenance, so the screen answers `Unsupported` and a test asserts that): godown
//! and batch allocation, bill references on a bill-wise party, rounding of a quantity x rate
//! product that has more than two decimals of a rupee.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assigning_clones,
    clippy::many_single_char_names,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::item_invoice::{
    Field, GstBlock, GstBlockLine, GstResult, GstStatus, GstTotal, InvoiceError, InvoiceKind,
    ItemInvoiceScreen, parse_place, place_label, search_items,
};
use ledgerly_app::screens::voucher_entry::{OpenError, format_entry_date};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Date, Decimal, FinancialYear, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, Unsupported, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillWiseConfig, Books, Company, EntityKind, Features, Godown,
    GstLine, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod,
    PartyDetails, RegistrationType, RestartPeriod, StockItem, StoredGst, TaxLedgerKind, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

// ---- fixture ---------------------------------------------------------------------------------

const SHARMA: LedgerId = LedgerId(1);
const SALES: LedgerId = LedgerId(2);
const CGST: LedgerId = LedgerId(3);
const SGST: LedgerId = LedgerId(4);
const KAVERI: LedgerId = LedgerId(5);

const T_SALES: VoucherTypeId = VoucherTypeId(1);
const T_PURCHASE: VoucherTypeId = VoucherTypeId(2);
const T_JOURNAL: VoucherTypeId = VoucherTypeId(3);
const T_STOCK: VoucherTypeId = VoucherTypeId(4);
const T_OFF: VoucherTypeId = VoucherTypeId(5);

const NOS: UnitId = UnitId(1);
const KG: UnitId = UnitId(2);
const WIDGET: StockItemId = StockItemId(1);
const BIG_WIDGET: StockItemId = StockItemId(2);
const TEA: StockItemId = StockItemId(3);
const BATCHED: StockItemId = StockItemId(4);

const TODAY: Date = date!(2026 - 11 - 21);

fn ledger(id: LedgerId, name: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: GroupId(1),
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

fn unit(id: UnitId, symbol: &str, decimals: u8) -> Unit {
    Unit {
        id,
        symbol: symbol.to_owned(),
        formal_name: symbol.to_owned(),
        decimals,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    }
}

fn item(id: StockItemId, name: &str, aliases: &[&str], unit: UnitId, batches: bool) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        group: None,
        unit,
        alt_unit: None,
        hsn: None,
        gst_history: Vec::new(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: batches,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn voucher_type(
    id: VoucherTypeId,
    name: &str,
    base: BaseVoucherKind,
    method: NumberingMethod,
    active: bool,
) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active,
        tally: None,
    }
}

fn add_ledgers(masters: &mut Masters) {
    let mut sharma = ledger(SHARMA, "Sharma Traders");
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        // A valid GSTIN from ledgerly-core's own tested vectors (gstin.rs `VALID`).
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    });
    let mut kaveri = ledger(KAVERI, "Kaveri Packaging");
    kaveri.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        sharma,
        ledger(SALES, "Sales - Local"),
        ledger(CGST, "Output CGST"),
        ledger(SGST, "Output SGST"),
        kaveri,
    ] {
        masters.ledgers.insert(l.id, l);
    }
}

fn add_stock(masters: &mut Masters) {
    for u in [unit(NOS, "Nos", 0), unit(KG, "Kg", 3)] {
        masters.units.insert(u.id, u);
    }
    for i in [
        item(WIDGET, "Widget Large", &["WL"], NOS, false),
        item(BIG_WIDGET, "Big Widget", &[], NOS, false),
        item(TEA, "Loose Tea", &["chai"], KG, false),
        item(BATCHED, "Syrup", &[], NOS, true),
    ] {
        masters.stock_items.insert(i.id, i);
    }
}

fn add_voucher_types(masters: &mut Masters) {
    for t in [
        voucher_type(
            T_SALES,
            "Sales",
            BaseVoucherKind::Sales,
            NumberingMethod::Automatic,
            true,
        ),
        voucher_type(
            T_PURCHASE,
            "Purchase",
            BaseVoucherKind::Purchase,
            NumberingMethod::Manual,
            true,
        ),
        voucher_type(
            T_JOURNAL,
            "Journal",
            BaseVoucherKind::Journal,
            NumberingMethod::Automatic,
            true,
        ),
        voucher_type(
            T_STOCK,
            "Stock Journal",
            BaseVoucherKind::StockJournal,
            NumberingMethod::Automatic,
            true,
        ),
        voucher_type(
            T_OFF,
            "Credit Note",
            BaseVoucherKind::CreditNote,
            NumberingMethod::Automatic,
            false,
        ),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
}

fn company() -> Company {
    Company {
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
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    add_ledgers(&mut masters);
    add_stock(&mut masters);
    add_voucher_types(&mut masters);
    Books {
        company: company(),
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn fy() -> FinancialYear {
    FinancialYear::april(2026).unwrap()
}

fn open(b: &Books, t: VoucherTypeId) -> ItemInvoiceScreen {
    match ItemInvoiceScreen::open(b, t, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

/// Types a complete one-line sales invoice with the keyboard flow: 100 Nos of "Widget Large" at
/// 100.00 to Sharma Traders, place of supply 27, sales ledger "Sales - Local".
fn fill(s: &mut ItemInvoiceScreen, m: &Masters) {
    assert_eq!(s.focus(), Field::Party);
    s.set_party_text("sharma");
    s.submit(m);
    assert_eq!(s.focus(), Field::Place);
    s.draft_mut().place_text = "27".to_owned();
    s.submit(m);
    assert_eq!(s.focus(), Field::Account);
    s.set_account_text("sales");
    s.submit(m);
    assert_eq!(s.focus(), Field::Reference);
    s.submit(m);
    assert_eq!(s.focus(), Field::Item(0));
    s.set_item_text(0, "widget");
    s.submit(m);
    assert_eq!(s.focus(), Field::Qty(0));
    s.draft_mut().rows[0].qty_text = "100".to_owned();
    s.submit(m);
    assert_eq!(s.focus(), Field::Rate(0));
    s.draft_mut().rows[0].rate_text = "100".to_owned();
    s.submit(m);
    assert_eq!(s.focus(), Field::Discount(0));
    s.submit(m);
}

fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// Stand-in for the engine's answer for the filled invoice (100 x 100.00 = 10,000.00 taxable).
///
/// provenance: derived. The 18% slab is CGST 9% + SGST 9%. Taxable 10,000.00; CGST = 10,000 x 9%
/// = 900.00; SGST = 900.00; no round-off because the total is already a whole rupee; invoice
/// total = 10,000 + 900 + 900 = 11,800.00. Postings: party Dr 11,800, sales Cr 10,000, CGST Cr 900,
/// SGST Cr 900 (Dr total 11,800 = Cr total 11,800).
fn block_for_10000() -> GstBlock {
    let gst_line = |ledger, kind, tax| GstLine {
        ledger,
        kind,
        taxable: rupees(10_000),
        rate_percent: Rate(Decimal::from(9)),
        tax,
    };
    GstBlock {
        pack_version: "test-pack".to_owned(),
        lines: vec![GstBlockLine {
            taxable: rupees(10_000),
            rate_percent: Decimal::from(18),
        }],
        taxable_total: rupees(10_000),
        heads: vec![
            GstTotal {
                head: "CGST".to_owned(),
                amount: rupees(900),
            },
            GstTotal {
                head: "SGST".to_owned(),
                amount: rupees(900),
            },
        ],
        round_off: Paise::ZERO,
        invoice_total: rupees(11_800),
        ledger_lines: vec![
            line(SHARMA, SignedAmount::dr(rupees(11_800))),
            line(SALES, SignedAmount::cr(rupees(10_000))),
            line(CGST, SignedAmount::cr(rupees(900))),
            line(SGST, SignedAmount::cr(rupees(900))),
        ],
        stored: StoredGst {
            pack_version: "test-pack".to_owned(),
            lines: vec![
                gst_line(CGST, TaxLedgerKind::Cgst, rupees(900)),
                gst_line(SGST, TaxLedgerKind::Sgst, rupees(900)),
            ],
            round_off: Paise::ZERO,
        },
    }
}

/// A filled screen with the stand-in engine answer already handed in.
fn answered(b: &Books) -> (ItemInvoiceScreen, Voucher) {
    let m = &b.masters;
    let mut s = open(b, T_SALES);
    fill(&mut s, m);
    let v = s.take_compute_request(m).unwrap();
    s.set_gst(v.clone(), GstResult::Computed(block_for_10000()));
    (s, v)
}

// ---- opening ---------------------------------------------------------------------------------

#[test]
fn sales_purchase_and_notes_open_every_other_kind_is_unsupported() {
    let b = books();
    // provenance: design (plan card U5: item invoice entry for the four invoice kinds).
    assert!(
        ItemInvoiceScreen::open(&b, T_SALES, fy(), TODAY)
            .unwrap()
            .value()
            .is_some()
    );
    assert!(
        ItemInvoiceScreen::open(&b, T_PURCHASE, fy(), TODAY)
            .unwrap()
            .value()
            .is_some()
    );
    for t in [T_JOURNAL, T_STOCK] {
        let out = ItemInvoiceScreen::open(&b, t, fy(), TODAY).unwrap();
        assert!(out.is_unsupported(), "type {t:?} must be unsupported");
    }
    assert_eq!(
        ItemInvoiceScreen::open(&b, VoucherTypeId(99), fy(), TODAY).err(),
        Some(OpenError::UnknownVoucherType(VoucherTypeId(99)))
    );
    assert!(matches!(
        ItemInvoiceScreen::open(&b, T_OFF, fy(), TODAY),
        Err(OpenError::Inactive(_))
    ));
    assert!(matches!(
        InvoiceKind::from_base(BaseVoucherKind::DebitNote),
        Outcome::Value(InvoiceKind::DebitNote)
    ));
}

#[test]
fn title_names_the_voucher_type() {
    let b = books();
    assert_eq!(open(&b, T_SALES).title(), "Sales invoice");
}

// ---- line amounts (arithmetic on typed input only) --------------------------------------------

fn amount_of(
    b: &Books,
    qty: &str,
    rate: &str,
    discount: &str,
    typed: &str,
    item: StockItemId,
) -> Result<Paise, InvoiceError> {
    let m = &b.masters;
    let mut s = open(b, T_SALES);
    fill(&mut s, m);
    let row = &mut s.draft_mut().rows[0];
    row.item = Some(item);
    row.qty_text = qty.to_owned();
    row.rate_text = rate.to_owned();
    row.discount_text = discount.to_owned();
    row.amount_text = typed.to_owned();
    let v = s.draft().build_voucher(m, fy())?;
    Ok(v.inventory_lines[0].amount)
}

#[test]
fn line_amount_is_quantity_times_rate() {
    let b = books();
    // provenance: derived. 100 x 100.00 = 10,000.00 rupees = 1,000,000 paise.
    assert_eq!(
        amount_of(&b, "100", "100", "", "", WIDGET),
        Ok(Paise::new(1_000_000))
    );
    // provenance: derived. 2.5 kg x 120.00 = 300.00 rupees = 30,000 paise (Kg allows 3 decimals).
    assert_eq!(
        amount_of(&b, "2.5", "120", "", "", TEA),
        Ok(Paise::new(30_000))
    );
}

#[test]
fn discount_reduces_the_line() {
    let b = books();
    // provenance: derived. 100 x 100.00 = 10,000.00; 10% discount leaves 90% = 9,000.00 rupees =
    // 900,000 paise.
    assert_eq!(
        amount_of(&b, "100", "100", "10", "", WIDGET),
        Ok(Paise::new(900_000))
    );
    // provenance: derived. 100% discount leaves nothing.
    assert_eq!(
        amount_of(&b, "100", "100", "100", "", WIDGET),
        Ok(Paise::ZERO)
    );
}

#[test]
fn a_product_with_a_fraction_of_a_paisa_needs_a_typed_amount() {
    let b = books();
    // provenance: derived. 3 x 33.333 = 99.999 rupees: three decimals, so no paisa value exists
    // without choosing a rounding rule, and none has provenance for line amounts (pending). The
    // screen refuses to pick one.
    assert!(matches!(
        amount_of(&b, "3", "33.333", "", "", WIDGET),
        Err(InvoiceError::NeedsAmount { line: 1 })
    ));
    // With the amount typed, that is the amount: 100 rupees = 10,000 paise.
    assert_eq!(
        amount_of(&b, "3", "33.333", "", "100", WIDGET),
        Ok(Paise::new(10_000))
    );
}

#[test]
fn bad_quantities_rates_and_discounts_are_errors_not_numbers() {
    let b = books();
    // provenance: design (rule A1). Nos allows 0 decimals (unit master), Kg allows 3.
    for (qty, rate, disc, item) in [
        ("2.5", "10", "", WIDGET),
        ("2.5555", "10", "", TEA),
        ("0", "10", "", WIDGET),
        ("-1", "10", "", WIDGET),
        ("abc", "10", "", WIDGET),
        ("", "10", "", WIDGET),
        ("1", "", "", WIDGET),
        ("1", "1,000", "", WIDGET),
        ("1", "10", "101", WIDGET),
        ("1", "10", "x", WIDGET),
    ] {
        assert!(
            matches!(
                amount_of(&b, qty, rate, disc, "", item),
                Err(InvoiceError::Line { line: 1, .. })
            ),
            "{qty:?} x {rate:?} less {disc:?} on {item:?} must be a line error"
        );
    }
}

// ---- place of supply -------------------------------------------------------------------------

#[test]
fn place_of_supply_is_read_from_a_code_a_name_or_a_unique_prefix() {
    // provenance: derived from ledgerly-core's table: 27 = Maharashtra, 29 = Karnataka.
    assert_eq!(parse_place("27").unwrap().get(), 27);
    assert_eq!(parse_place(" maharashtra ").unwrap().get(), 27);
    assert_eq!(parse_place("karn").unwrap().get(), 29);
    assert_eq!(parse_place("27 Maharashtra").unwrap().get(), 27);
    assert_eq!(place_label(StateCode::new(29).unwrap()), "29 Karnataka");
    // "ma" starts Maharashtra, Manipur and Madhya Pradesh: ambiguous, never guessed.
    assert!(matches!(parse_place("ma"), Err(InvoiceError::Place(_))));
    // Not a code in the table, empty, unknown name.
    for bad in ["98", "", "  ", "atlantis", "300"] {
        assert!(
            matches!(parse_place(bad), Err(InvoiceError::Place(_))),
            "{bad:?}"
        );
    }
    // Pre-merger code 25 is not reachable by name ("daman" would otherwise pick it).
    assert!(matches!(parse_place("daman"), Err(InvoiceError::Place(_))));
}

// ---- item type-ahead -------------------------------------------------------------------------

#[test]
fn item_search_ranks_prefix_before_word_before_substring_and_finds_aliases() {
    let b = books();
    let m = &b.masters;
    // provenance: design (same ranking rule as the ledger type-ahead): "widget" is a prefix of
    // "Widget Large" (rank 1) and starts the second word of "Big Widget" (rank 2).
    let names: Vec<String> = search_items(m, "widget")
        .into_iter()
        .map(|h| h.name)
        .collect();
    assert_eq!(names, ["Widget Large", "Big Widget"]);
    // Alias match is reported.
    let hits = search_items(m, "chai");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Loose Tea");
    assert_eq!(hits[0].via_alias.as_deref(), Some("chai"));
    assert_eq!(hits[0].unit.as_deref(), Some("Kg"));
    // Empty query lists everything; no match lists nothing.
    assert_eq!(search_items(m, "").len(), 4);
    // An empty query lists items by name. Nothing was typed, so no alias can have matched: derived
    // from the doc contract "an empty query lists every item", not from engine output.
    assert!(search_items(m, "").iter().all(|h| h.via_alias.is_none()));
    assert_eq!(
        search_items(m, "zzz"),
        [] as [ledgerly_app::screens::item_invoice::ItemHit; 0]
    );
}

// ---- building the voucher --------------------------------------------------------------------

#[test]
fn the_filled_invoice_becomes_an_item_invoice_voucher_without_accounting_or_gst() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    // Enter after the last line's discount opens a new blank line.
    assert_eq!(s.focus(), Field::Item(1));
    assert_eq!(s.draft().rows.len(), 2);
    assert_eq!(s.draft().place_text, "27 Maharashtra");
    let v = s.draft().build_voucher(m, fy()).unwrap();
    assert_eq!(v.mode, VoucherMode::ItemInvoice);
    assert_eq!(v.status, VoucherStatus::Regular);
    assert_eq!(v.party, Some(SHARMA));
    assert_eq!(v.place_of_supply.map(StateCode::get), Some(27));
    assert_eq!(v.date, TODAY);
    assert_eq!(v.type_id, T_SALES);
    // Blank trailing line is skipped; the one line carries the item, qty, rate and sales ledger.
    assert_eq!(v.inventory_lines.len(), 1);
    let l = &v.inventory_lines[0];
    assert_eq!(l.item, WIDGET);
    assert_eq!(l.unit, NOS);
    assert_eq!(l.qty.0, Decimal::from(100));
    assert_eq!(l.rate.0, Decimal::from(100));
    assert_eq!(l.ledger, Some(SALES));
    assert_eq!(l.discount_percent, None);
    // The tax engine owns these.
    assert_eq!(v.ledger_lines, [] as [ledgerly_model::LedgerLine; 0]);
    assert!(v.gst.is_none());
}

#[test]
fn missing_pieces_are_named_and_nothing_is_defaulted() {
    let b = books();
    let m = &b.masters;
    // Nothing entered: the party is the first thing missing.
    let s = open(&b, T_SALES);
    assert_eq!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Missing { field: "Customer" })
    );
    // Party and lines but no place of supply: never guessed from the party or the company.
    let mut s = open(&b, T_SALES);
    s.set_party_text("sharma");
    s.submit(m);
    let r = s.draft().build_voucher(m, fy());
    assert!(matches!(r, Err(InvoiceError::Place(_))), "{r:?}");
    // No sales ledger.
    s.draft_mut().place_text = "27".to_owned();
    assert_eq!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Missing {
            field: "Sales ledger"
        })
    );
    // Manual numbering (the Purchase type) needs the number before anything else.
    let p = open(&b, T_PURCHASE);
    assert_eq!(
        p.draft().build_voucher(m, fy()),
        Err(InvoiceError::Missing {
            field: "voucher number"
        })
    );
    assert!(p.draft().number_required);
    assert!(!s.draft().number_required);
}

#[test]
fn no_items_is_an_error() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    s.set_item_text(0, "");
    s.draft_mut().rows[0].qty_text.clear();
    s.draft_mut().rows[0].rate_text.clear();
    assert_eq!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Missing { field: "an item" })
    );
}

#[test]
fn a_half_filled_line_is_an_error_not_skipped() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    s.draft_mut().rows[1].qty_text = "5".to_owned();
    assert!(matches!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Line { line: 2, .. })
    ));
}

#[test]
fn bill_wise_party_godowns_and_batches_are_unsupported_not_guessed() {
    let b = books();
    let m = &b.masters;
    // pending: bill references are not entered here.
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    s.draft_mut().party = Some(KAVERI);
    assert!(matches!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Unsupported(_))
    ));
    // pending: godown allocation.
    let mut with_godown = books();
    with_godown.masters.godowns.insert(
        GodownId(1),
        Godown {
            id: GodownId(1),
            name: "Main".to_owned(),
            parent: None,
            tally: None,
        },
    );
    let mut s = open(&with_godown, T_SALES);
    fill(&mut s, &with_godown.masters);
    assert!(matches!(
        s.draft().build_voucher(&with_godown.masters, fy()),
        Err(InvoiceError::Unsupported(_))
    ));
    // pending: batch allocation.
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    s.draft_mut().rows[0].item = Some(BATCHED);
    assert!(matches!(
        s.draft().build_voucher(m, fy()),
        Err(InvoiceError::Unsupported(_))
    ));
}

// ---- the GST block shows engine output -------------------------------------------------------

#[test]
fn the_block_is_incomplete_then_not_computed_then_shows_exactly_the_engine_answer() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    assert!(matches!(s.gst_status(m), GstStatus::Incomplete(_)));
    assert!(s.take_compute_request(m).is_none());
    fill(&mut s, m);
    assert_eq!(s.gst_status(m), GstStatus::NotComputedYet);
    assert!(s.invoice_total_in_words(m).is_none());
    // One request per exact voucher.
    let v = s.take_compute_request(m).unwrap();
    assert!(s.take_compute_request(m).is_none());
    s.set_gst(v, GstResult::Computed(block_for_10000()));
    let GstStatus::Computed(block) = s.gst_status(m) else {
        panic!("expected the engine's answer");
    };
    // provenance: derived, see `block_for_10000`: 10,000 + 900 + 900 = 11,800.
    assert_eq!(block.taxable_total, rupees(10_000));
    assert_eq!(block.invoice_total, rupees(11_800));
    assert_eq!(block.heads[0].amount, rupees(900));
    assert_eq!(block.heads[1].amount, rupees(900));
    assert_eq!(block.round_off, Paise::ZERO);
}

#[test]
fn amount_in_words_comes_from_the_engines_total() {
    let b = books();
    let (s, _) = answered(&b);
    let words = s.invoice_total_in_words(&b.masters).unwrap();
    // provenance: derived. 11,800 = eleven thousand eight hundred. The "Rupees ... Only" wrapper is
    // card C1's convention, which C1 marks as unsourced, so only the number words are asserted.
    assert!(words.contains("Eleven Thousand"), "{words}");
    assert!(words.contains("Eight Hundred"), "{words}");
    assert!(words.ends_with("Only"), "{words}");
}

#[test]
fn an_edit_makes_the_answer_stale_and_nothing_old_is_shown_or_saved() {
    let b = books();
    let m = &b.masters;
    let (mut s, _) = answered(&b);
    assert!(matches!(s.gst_status(m), GstStatus::Computed(_)));
    // Change the quantity: 100 -> 101. The held answer was for 100.
    s.draft_mut().rows[0].qty_text = "101".to_owned();
    assert_eq!(s.gst_status(m), GstStatus::NotComputedYet);
    assert!(s.invoice_total_in_words(m).is_none());
    s.save(m);
    assert!(s.error_text().unwrap().contains("not been computed"));
    assert!(s.take_save_request().is_none());
    // A new request is made for the new voucher.
    assert!(s.take_compute_request(m).is_some());
    // Changing the place of supply, party or date is stale too.
    let (mut s, _) = answered(&b);
    s.draft_mut().place_text = "29".to_owned();
    assert_eq!(s.gst_status(m), GstStatus::NotComputedYet);
    let (mut s, _) = answered(&b);
    s.draft_mut().date_text = format_entry_date(date!(2026 - 11 - 22));
    assert_eq!(s.gst_status(m), GstStatus::NotComputedYet);
    let (mut s, _) = answered(&b);
    s.draft_mut().narration = "x".to_owned();
    assert_eq!(s.gst_status(m), GstStatus::NotComputedYet);
}

#[test]
fn an_engine_unsupported_reason_is_shown_and_blocks_save() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    let v = s.take_compute_request(m).unwrap();
    let reason = Unsupported {
        what: "GST cess".to_owned(),
        reason: "no verified cess rate is available".to_owned(),
    };
    s.set_gst(v, GstResult::Unsupported(reason.clone()));
    assert_eq!(s.gst_status(m), GstStatus::Unsupported(&reason));
    s.save(m);
    let e = s.error_text().unwrap();
    assert!(
        e.contains("GST cess") && e.contains("no verified cess rate"),
        "{e}"
    );
    assert!(s.take_save_request().is_none());
}

#[test]
fn an_engine_failure_is_shown_and_blocks_save() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    let v = s.take_compute_request(m).unwrap();
    s.set_gst(v, GstResult::Failed("rule pack missing".to_owned()));
    assert_eq!(s.gst_status(m), GstStatus::Failed("rule pack missing"));
    s.save(m);
    assert!(s.error_text().unwrap().contains("rule pack missing"));
}

#[test]
fn an_answer_that_does_not_describe_the_invoice_is_refused() {
    let b = books();
    let m = &b.masters;
    // Taxable value of the line differs from the invoice line (10,000.00): 9,999.00.
    let mut wrong_lines = block_for_10000();
    wrong_lines.lines[0].taxable = rupees(9_999);
    // The party line is not the invoice total: 11,799.00 against 11,800.00.
    let mut wrong_party = block_for_10000();
    wrong_party.ledger_lines[0] = line(SHARMA, SignedAmount::dr(rupees(11_799)));
    // Two lines for a one-line invoice.
    let mut extra_line = block_for_10000();
    extra_line.lines.push(extra_line.lines[0].clone());
    for block in [wrong_lines, wrong_party, extra_line] {
        let mut s = open(&b, T_SALES);
        fill(&mut s, m);
        let v = s.take_compute_request(m).unwrap();
        s.set_gst(v, GstResult::Computed(block));
        assert!(matches!(s.gst_status(m), GstStatus::Failed(_)));
        assert!(s.invoice_total_in_words(m).is_none());
    }
}

// ---- save ------------------------------------------------------------------------------------

#[test]
fn save_hands_the_host_the_voucher_with_the_engines_lines_and_gst() {
    let b = books();
    let m = &b.masters;
    let (mut s, requested) = answered(&b);
    s.save(m);
    let v = s.take_save_request().unwrap();
    assert!(s.take_save_request().is_none());
    // The accounting lines and stored GST are exactly the engine's; the rest is the request.
    let block = block_for_10000();
    assert_eq!(v.ledger_lines, block.ledger_lines);
    assert_eq!(v.gst, Some(block.stored));
    assert_eq!(v.inventory_lines, requested.inventory_lines);
    assert_eq!(v.party, requested.party);
    assert_eq!(v.place_of_supply, requested.place_of_supply);
    // provenance: derived. Dr 11,800 = Cr 10,000 + 900 + 900.
    let dr: i64 = v
        .ledger_lines
        .iter()
        .filter(|l| l.amount.side == ledgerly_core::DrCr::Dr)
        .map(|l| l.amount.amount.get())
        .sum();
    let cr: i64 = v
        .ledger_lines
        .iter()
        .filter(|l| l.amount.side == ledgerly_core::DrCr::Cr)
        .map(|l| l.amount.amount.get())
        .sum();
    assert_eq!((dr, cr), (1_180_000, 1_180_000));
}

#[test]
fn confirmed_save_starts_a_fresh_invoice_and_a_failed_save_keeps_the_screen() {
    let b = books();
    let m = &b.masters;
    let (mut s, _) = answered(&b);
    s.save(m);
    let _ = s.take_save_request().unwrap();
    s.save_failed("period is locked");
    assert!(s.error_text().unwrap().contains("period is locked"));
    assert!(!s.draft().is_blank());
    s.confirm_saved();
    assert!(s.draft().is_blank());
    assert_eq!(s.focus(), Field::Party);
    assert!(matches!(s.gst_status(m), GstStatus::Incomplete(_)));
    assert_eq!(s.info_text(), Some("Saved."));
}

#[test]
fn enter_on_narration_saves_when_the_answer_is_current() {
    let b = books();
    let m = &b.masters;
    let (mut s, _) = answered(&b);
    // Item(1) is blank: Enter drops it and goes to the narration.
    assert_eq!(s.focus(), Field::Item(1));
    s.submit(m);
    assert_eq!(s.focus(), Field::Narration);
    assert_eq!(s.draft().rows.len(), 1);
    s.submit(m);
    assert!(s.take_save_request().is_some());
}

// ---- keys ------------------------------------------------------------------------------------

#[test]
fn shift_enter_goes_back_one_field() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    s.set_party_text("sharma");
    s.submit(m);
    assert_eq!(s.focus(), Field::Place);
    s.step_back();
    assert_eq!(s.focus(), Field::Party);
}

#[test]
fn enter_refuses_a_bad_field_and_stays() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    s.set_party_text("zzz");
    s.submit(m);
    assert_eq!(s.focus(), Field::Party);
    assert!(s.error_text().unwrap().contains("No ledger matches"));
}

// ---- egui ------------------------------------------------------------------------------------

fn frame(
    ctx: &egui::Context,
    screen: &mut ItemInvoiceScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
    action
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

#[test]
fn the_screen_draws_with_and_without_a_company() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_SALES);
    let mut closed = AppCtx::default();
    assert!(matches!(
        frame(&ctx, &mut s, &mut closed, RawInput::default()),
        ScreenAction::Stay
    ));
    let mut app = AppCtx { books: Some(b) };
    assert!(matches!(
        frame(&ctx, &mut s, &mut app, RawInput::default()),
        ScreenAction::Stay
    ));
}

#[test]
fn the_screen_draws_every_gst_state() {
    let b = books();
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
    };
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    fill(&mut s, m);
    // not computed yet
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let v = s.take_compute_request(m).unwrap();
    s.set_gst(v.clone(), GstResult::Computed(block_for_10000()));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.set_gst(v.clone(), GstResult::Failed("boom".to_owned()));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.set_gst(
        v,
        GstResult::Unsupported(Unsupported {
            what: "x".to_owned(),
            reason: "y".to_owned(),
        }),
    );
    frame(&ctx, &mut s, &mut app, RawInput::default());
}

#[test]
fn ctrl_a_saves_and_ctrl_enter_does_not() {
    // provenance: spec 7 and plan card U2: save is Ctrl+A (and Ctrl+S); Ctrl+Enter alters the
    // master under the cursor, so it must not save.
    let b = books();
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
    };
    let (mut s, _) = answered(&b);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::COMMAND));
    assert!(s.take_save_request().is_none());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert!(s.take_save_request().is_some());
    // Ctrl+S too.
    let (mut s, _) = answered(&b);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::S, Modifiers::COMMAND));
    assert!(s.take_save_request().is_some());
}

#[test]
fn ctrl_a_on_an_incomplete_invoice_says_what_is_missing() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_SALES);
    let mut app = AppCtx { books: Some(b) };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert!(s.error_text().unwrap().contains("Customer is required"));
}

#[test]
fn escape_on_a_started_invoice_asks_then_y_closes_and_a_blank_one_is_left_to_the_shell() {
    let b = books();
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
    };
    // Blank: Esc is not consumed and no question is asked.
    let mut blank = open(&b, T_SALES);
    frame(&ctx, &mut blank, &mut app, RawInput::default());
    let action = frame(
        &ctx,
        &mut blank,
        &mut app,
        key(Key::Escape, Modifiers::NONE),
    );
    assert!(matches!(action, ScreenAction::Stay));
    assert!(!blank.is_confirming_discard());
    // Started: Esc asks, N keeps, Esc asks again, Y closes.
    let mut s = open(&b, T_SALES);
    s.draft_mut().narration = "x".to_owned();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(s.is_confirming_discard());
    frame(&ctx, &mut s, &mut app, key(Key::N, Modifiers::NONE));
    assert!(!s.is_confirming_discard());
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    let action = frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Pop));
}

#[test]
fn recent_ledgers_and_balances_reach_the_party_list() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_SALES);
    s.set_recents(vec![SALES]);
    s.set_balances(BTreeMap::from([(SHARMA, SignedAmount::dr(rupees(500)))]));
    let hits = s.ledger_choices(m, "");
    // provenance: design. Recents first on an empty query; a ledger with no supplied balance
    // shows none, and one with a balance shows it.
    assert!(hits.iter().find(|h| h.id == SALES).unwrap().recent);
    let sharma = hits.iter().find(|h| h.id == SHARMA).unwrap();
    assert_eq!(sharma.balance, Some(SignedAmount::dr(rupees(500))));
    assert_eq!(hits.iter().find(|h| h.id == CGST).unwrap().balance, None);
}

#[test]
fn a_typed_amount_with_a_fraction_of_a_paisa_is_refused_not_rounded() {
    let b = books();
    // provenance: derived. 100.005 rupees is 10_000.5 paise, so no whole-paisa value is exact.
    // Rounding it would store 100.01 with no notice, and no rounding rule for a typed line amount
    // has provenance (item_invoice/mod.rs), so the line is refused and says why.
    assert!(matches!(
        amount_of(&b, "100", "100", "", "100.005", WIDGET),
        Err(InvoiceError::Line { line: 1, .. })
    ));
    // provenance: derived. 100.50 rupees = 10_050 paise, exact, so it is still accepted.
    assert_eq!(
        amount_of(&b, "100", "100", "", "100.50", WIDGET),
        Ok(Paise::new(10_050))
    );
}

#[test]
fn shift_y_does_not_discard_a_started_invoice() {
    let b = books();
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(b.clone()),
    };
    let mut s = open(&b, T_SALES);
    s.draft_mut().narration = "x".to_owned();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(s.is_confirming_discard());
    // provenance: derived. The discard question is answered by a plain Y. Shift+Y is not that key,
    // so the question stays open and the invoice is not closed.
    let action = frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::SHIFT));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(s.is_confirming_discard());
}
