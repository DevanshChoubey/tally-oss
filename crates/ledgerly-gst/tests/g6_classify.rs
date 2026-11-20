//! Card G6 tests: supply classification (IGST Act 2017 s.2(5), s.16; CGST Act 2017 s.9, s.11,
//! s.147).
//!
//! Every expected value is `derived`: the statutory rule is written next to it. The supply option
//! (`gst_supply_option` in `Voucher::extra`) is our own convention for the s.16(3) choice, which
//! the model has no field for; the tests check that the choice is never defaulted.
//! Nothing here is copied from the engine's output. Cases whose law is not confirmed
//! (services to overseas recipients, exempt supplies to SEZ or overseas recipients) expect
//! `Unsupported`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::{
    Date, Decimal, GroupId, Hsn, LedgerId, Outcome, Paise, Qty, Rate, SignedAmount, StockItemId,
    UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::classify::{SUPPLY_OPTION_KEY, SupplyKind, classify_line};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, GstClassification, GstHistory,
    InventoryLine, Ledger, LedgerLine, Masters, NumberingConfig, NumberingMethod, PartyDetails,
    RegistrationType, RestartPeriod, StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const ITEM: StockItemId = StockItemId(1);
const SALES_LEDGER: LedgerId = LedgerId(10);
const PARTY: LedgerId = LedgerId(11);
const SEZ_PARTY: LedgerId = LedgerId(13);
const OVERSEAS_PARTY: LedgerId = LedgerId(14);
const UIN_PARTY: LedgerId = LedgerId(15);
const COMPOSITION_PARTY: LedgerId = LedgerId(16);
const IGST_LEDGER: LedgerId = LedgerId(17);
const NON_GST_LEDGER: LedgerId = LedgerId(18);
const UNCLASSIFIED_LEDGER: LedgerId = LedgerId(19);
const SERVICE_LEDGER: LedgerId = LedgerId(20);
const BANK: LedgerId = LedgerId(21);
const NO_DETAILS_PARTY: LedgerId = LedgerId(22);

fn rules_on(d: Date) -> RuleSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let pack = load_pack(&fs::read_to_string(path).unwrap()).unwrap();
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn history(from: Date, taxability: Taxability, percent: Option<i64>) -> GstHistory {
    GstHistory {
        effective_from: from,
        taxability,
        rate_percent: percent.map(|p| Rate(Decimal::new(p, 0))),
    }
}

fn item_with(gst_history: Vec<GstHistory>, hsn: Option<&str>) -> StockItem {
    StockItem {
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        gst_history,
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        tally: None,
    }
}

fn ledger(
    id: LedgerId,
    registration: Option<RegistrationType>,
    tax: Option<TaxDetails>,
    gst: Option<GstClassification>,
) -> Ledger {
    Ledger {
        id,
        name: format!("Ledger {}", id.0),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: registration.map(|registration| PartyDetails {
            registration,
            gstin: None,
            pan: None,
            state: None,
            address: Address::default(),
        }),
        bank: None,
        tax,
        gst,
        reserved: None,
        tally: None,
    }
}

fn class(taxability: Taxability, percent: Option<i64>, hsn: Option<&str>) -> GstClassification {
    GstClassification {
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        taxability,
        rate_percent: percent.map(|p| Rate(Decimal::new(p, 0))),
    }
}

fn masters(base: BaseVoucherKind, gst_history: Vec<GstHistory>, hsn: Option<&str>) -> Masters {
    let mut m = Masters::default();
    m.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Type".to_owned(),
            base,
            abbreviation: "Ty".to_owned(),
            numbering: NumberingConfig {
                method: NumberingMethod::Automatic,
                prevent_renumbering: true,
                prefix: String::new(),
                suffix: String::new(),
                width: 0,
                restart: RestartPeriod::Yearly,
                series: vec![],
            },
            active: true,
            tally: None,
        },
    );
    m.units.insert(
        UnitId(1),
        Unit {
            id: UnitId(1),
            symbol: "pcs".to_owned(),
            formal_name: "Pieces".to_owned(),
            decimals: 0,
            kind: UnitKind::Simple,
            uqc: None,
            tally: None,
        },
    );
    m.stock_items.insert(ITEM, item_with(gst_history, hsn));
    let ledgers = [
        ledger(
            SALES_LEDGER,
            None,
            None,
            Some(class(Taxability::Taxable, Some(18), Some("8501"))),
        ),
        ledger(
            SERVICE_LEDGER,
            None,
            None,
            Some(class(Taxability::Taxable, Some(18), Some("9983"))),
        ),
        ledger(
            NON_GST_LEDGER,
            None,
            None,
            Some(class(Taxability::NonGst, None, None)),
        ),
        ledger(UNCLASSIFIED_LEDGER, None, None, None),
        ledger(PARTY, Some(RegistrationType::Regular), None, None),
        ledger(SEZ_PARTY, Some(RegistrationType::Sez), None, None),
        ledger(OVERSEAS_PARTY, Some(RegistrationType::Overseas), None, None),
        ledger(UIN_PARTY, Some(RegistrationType::Uin), None, None),
        ledger(
            COMPOSITION_PARTY,
            Some(RegistrationType::Composition),
            None,
            None,
        ),
        ledger(NO_DETAILS_PARTY, None, None, None),
        ledger(BANK, None, None, None),
        ledger(
            IGST_LEDGER,
            None,
            Some(TaxDetails {
                kind: TaxLedgerKind::Igst,
                rate_percent: None,
            }),
            None,
        ),
    ];
    for l in ledgers {
        m.ledgers.insert(l.id, l);
    }
    m
}

fn taxable18() -> Vec<GstHistory> {
    vec![history(
        date!(2025 - 09 - 22),
        Taxability::Taxable,
        Some(18),
    )]
}

fn base_voucher(mode: VoucherMode, party: LedgerId) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("1".to_owned()),
        series: None,
        date: date!(2026 - 01 - 15),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party: Some(party),
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn inv_line() -> InventoryLine {
    InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(1_000_000, 2)),
        discount_percent: None,
        amount: Paise::new(1_000_000),
        ledger: Some(SALES_LEDGER),
        godowns: vec![],
    }
}

fn item_voucher(party: LedgerId) -> Voucher {
    Voucher {
        inventory_lines: vec![inv_line()],
        ..base_voucher(VoucherMode::ItemInvoice, party)
    }
}

fn with_option(mut v: Voucher, option: &str) -> Voucher {
    v.extra
        .insert(SUPPLY_OPTION_KEY.to_owned(), option.to_owned());
    v
}

fn ledger_line(ledger: LedgerId, paise: i64) -> LedgerLine {
    LedgerLine {
        ledger,
        amount: SignedAmount::cr(Paise::new(paise)),
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

fn accounting_voucher(party: LedgerId, ledger: LedgerId) -> Voucher {
    Voucher {
        ledger_lines: vec![ledger_line(ledger, 1_000_000)],
        ..base_voucher(VoucherMode::AccountingInvoice, party)
    }
}

fn run(v: &Voucher, line: usize, m: &Masters) -> Result<Outcome<SupplyKind>, GstError> {
    let tos = v.supply_date.unwrap_or(v.date);
    classify_line(v, line, m, &rules_on(tos))
}

fn kind(v: &Voucher, line: usize, m: &Masters) -> SupplyKind {
    match run(v, line, m).unwrap() {
        Outcome::Value(k) => k,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn assert_unsupported(v: &Voucher, line: usize, m: &Masters) {
    match run(v, line, m).unwrap() {
        Outcome::Unsupported(_) => {}
        Outcome::Value(k) => panic!("expected Unsupported, got {k:?}"),
    }
}

fn sales(history: Vec<GstHistory>, hsn: Option<&str>) -> Masters {
    masters(BaseVoucherKind::Sales, history, hsn)
}

// ---- every SupplyKind is reachable ---------------------------------------------------------

#[test]
fn taxable_domestic_supply_is_taxable() {
    // provenance: derived — CGST Act s.9(1): tax is levied on intra-State supplies; the item is
    // taxable at 18% on the supply date, the recipient is a registered domestic party.
    let m = sales(taxable18(), Some("8501"));
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Taxable);
}

#[test]
fn exempt_item_is_exempt() {
    // provenance: derived — CGST Act s.11 (exemption by notification): the item master says
    // exempt on the supply date, the recipient is domestic.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::Exempt, None)],
        Some("8501"),
    );
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Exempt);
}

#[test]
fn nil_rated_item_is_nil_rated() {
    // provenance: derived — a nil-rated supply is a supply on which the rate of tax is nil; the
    // item master says nil-rated.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::NilRated, None)],
        Some("8501"),
    );
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::NilRated);
}

#[test]
fn non_gst_item_is_non_gst() {
    // provenance: derived — goods outside the GST base (for example alcohol for human
    // consumption, Constitution Art. 366(12A) read with CGST Act s.2(52)); the item master
    // says non-GST.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::NonGst, None)],
        None,
    );
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::NonGst);
}

#[test]
fn export_of_goods_with_payment() {
    // provenance: derived — IGST Act s.2(5) (export of goods) and s.16(1)(a) (zero-rated);
    // s.16(3)(b): the exporter may supply on payment of IGST and claim a refund. The voucher
    // states `with_payment`.
    let m = sales(taxable18(), Some("8501"));
    let v = with_option(item_voucher(OVERSEAS_PARTY), "with_payment");
    assert_eq!(kind(&v, 0, &m), SupplyKind::ExportWithPayment);
}

#[test]
fn export_of_goods_under_lut() {
    // provenance: derived — IGST Act s.16(1)(a) and s.16(3)(a): zero-rated, supplied under bond
    // or letter of undertaking without payment of IGST. The voucher states `under_lut`.
    let m = sales(taxable18(), Some("8501"));
    let v = with_option(item_voucher(OVERSEAS_PARTY), "under_lut");
    assert_eq!(kind(&v, 0, &m), SupplyKind::ExportUnderLut);
}

#[test]
fn supply_to_sez_with_payment() {
    // provenance: derived — IGST Act s.16(1)(b) (supply to an SEZ unit or developer is zero-rated)
    // and s.16(3)(b) (with payment of IGST).
    let m = sales(taxable18(), Some("8501"));
    let v = with_option(item_voucher(SEZ_PARTY), "with_payment");
    assert_eq!(kind(&v, 0, &m), SupplyKind::SezWithPayment);
}

#[test]
fn supply_to_sez_under_lut() {
    // provenance: derived — IGST Act s.16(1)(b) and s.16(3)(a) (under bond or LUT).
    let m = sales(taxable18(), Some("8501"));
    let v = with_option(item_voucher(SEZ_PARTY), "under_lut");
    assert_eq!(kind(&v, 0, &m), SupplyKind::SezUnderLut);
}

#[test]
fn service_to_sez_is_also_zero_rated() {
    // provenance: derived — IGST Act s.16(1)(b): "supply of goods or services or both to a
    // Special Economic Zone developer or a Special Economic Zone unit". The line is a service
    // (SAC 9983), accounting invoice, under LUT.
    let m = sales(taxable18(), None);
    let v = with_option(accounting_voucher(SEZ_PARTY, SERVICE_LEDGER), "under_lut");
    assert_eq!(kind(&v, 0, &m), SupplyKind::SezUnderLut);
}

#[test]
fn deemed_export_to_regular_recipient() {
    // provenance: derived — CGST Act s.147: supplies of notified goods to a registered person
    // (EOU, EPCG holder and similar) are deemed exports. The voucher declares `deemed_export`;
    // the line is taxable, the recipient a regular taxpayer.
    let m = sales(taxable18(), Some("8501"));
    let v = with_option(item_voucher(PARTY), "deemed_export");
    assert_eq!(kind(&v, 0, &m), SupplyKind::DeemedExport);
}

fn receipt_voucher(bills: Vec<BillAllocation>, party_line_side_cr: bool) -> Voucher {
    let amount = if party_line_side_cr {
        SignedAmount::cr(Paise::new(500_000))
    } else {
        SignedAmount::dr(Paise::new(500_000))
    };
    Voucher {
        ledger_lines: vec![
            LedgerLine {
                ledger: BANK,
                amount: SignedAmount::dr(Paise::new(500_000)),
                bills: vec![],
                cost: vec![],
                bank: None,
            },
            LedgerLine {
                ledger: PARTY,
                amount,
                bills,
                cost: vec![],
                bank: None,
            },
        ],
        ..base_voucher(VoucherMode::AsVoucher, PARTY)
    }
}

fn bill(kind: BillRefKind) -> BillAllocation {
    BillAllocation {
        kind,
        name: "ADV-1".to_owned(),
        due_date: None,
        credit_days: None,
        amount: SignedAmount::cr(Paise::new(500_000)),
    }
}

#[test]
fn receipt_with_advance_allocation_is_advance() {
    // provenance: derived — CGST Act s.13(2) / s.12(2): an advance is money received before the
    // supply; the receipt voucher's party line carries an `Advance` bill allocation and credits
    // the party. The kind records only that it is an advance (not whether tax is due).
    let m = masters(BaseVoucherKind::Receipt, vec![], None);
    let v = receipt_voucher(vec![bill(BillRefKind::Advance)], true);
    assert_eq!(kind(&v, 1, &m), SupplyKind::Advance);
}

// ---- unknown combinations are Unsupported --------------------------------------------------

#[test]
fn zero_rated_supply_without_the_choice_is_unsupported() {
    // provenance: derived — IGST Act s.16(3): the supplier chooses between payment of IGST and
    // bond or LUT; a voucher that does not say which must not be defaulted.
    let m = sales(taxable18(), Some("8501"));
    assert_unsupported(&item_voucher(OVERSEAS_PARTY), 0, &m);
    assert_unsupported(&item_voucher(SEZ_PARTY), 0, &m);
}

#[test]
fn service_to_overseas_recipient_is_unsupported() {
    // provenance: derived — export of services needs the five conditions of IGST Act s.2(6)
    // (place of supply outside India under s.13, payment in convertible foreign exchange, ...),
    // which are not modelled; a SAC line to an overseas party is not classified.
    let m = sales(taxable18(), None);
    let v = with_option(
        accounting_voucher(OVERSEAS_PARTY, SERVICE_LEDGER),
        "with_payment",
    );
    assert_unsupported(&v, 0, &m);
}

#[test]
fn overseas_supply_of_unknown_nature_is_unsupported() {
    // provenance: derived — goods and services to an overseas recipient follow different rules
    // (s.2(5) against s.2(6)); an item with no HSN or SAC cannot be told apart.
    let m = sales(taxable18(), None);
    let v = with_option(item_voucher(OVERSEAS_PARTY), "with_payment");
    assert_unsupported(&v, 0, &m);
}

#[test]
fn exempt_supply_to_overseas_or_sez_is_unsupported() {
    // provenance: derived — whether an exempt supply to a zero-rated recipient is zero-rated or
    // exempt is not decided by this card; it must not be guessed.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::Exempt, None)],
        Some("8501"),
    );
    assert_unsupported(
        &with_option(item_voucher(OVERSEAS_PARTY), "with_payment"),
        0,
        &m,
    );
    assert_unsupported(&with_option(item_voucher(SEZ_PARTY), "under_lut"), 0, &m);
}

#[test]
fn non_gst_line_to_overseas_recipient_stays_non_gst() {
    // provenance: derived — a supply outside the GST base is outside GST whoever the recipient
    // is, so there is no s.16(3) choice to make.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::NonGst, None)],
        Some("8501"),
    );
    assert_eq!(
        kind(&item_voucher(OVERSEAS_PARTY), 0, &m),
        SupplyKind::NonGst
    );
}

#[test]
fn under_lut_with_igst_posted_is_unsupported() {
    // provenance: derived — IGST Act s.16(3)(a): a supply under bond or LUT is made without
    // payment of IGST, so a voucher marked `under_lut` that posts IGST contradicts itself.
    let m = sales(taxable18(), None);
    let mut v = with_option(accounting_voucher(SEZ_PARTY, SALES_LEDGER), "under_lut");
    v.ledger_lines.push(ledger_line(IGST_LEDGER, 180_000));
    assert_unsupported(&v, 0, &m);
    // the same voucher marked with_payment is consistent: IGST was paid (s.16(3)(b))
    let v = with_option(
        {
            let mut v = accounting_voucher(SEZ_PARTY, SALES_LEDGER);
            v.ledger_lines.push(ledger_line(IGST_LEDGER, 180_000));
            v
        },
        "with_payment",
    );
    assert_eq!(kind(&v, 0, &m), SupplyKind::SezWithPayment);
}

#[test]
fn option_that_does_not_fit_the_supply_is_unsupported() {
    // provenance: derived — s.16(3) applies only to zero-rated supplies (exports, SEZ), and
    // s.147 deemed exports go to registered persons; anywhere else the option is meaningless.
    let m = sales(taxable18(), Some("8501"));
    assert_unsupported(&with_option(item_voucher(PARTY), "with_payment"), 0, &m);
    assert_unsupported(&with_option(item_voucher(PARTY), "under_lut"), 0, &m);
    assert_unsupported(
        &with_option(item_voucher(COMPOSITION_PARTY), "deemed_export"),
        0,
        &m,
    );
    assert_unsupported(
        &with_option(item_voucher(OVERSEAS_PARTY), "deemed_export"),
        0,
        &m,
    );
    assert_unsupported(&with_option(item_voucher(PARTY), "lut"), 0, &m);
}

#[test]
fn deemed_export_of_an_exempt_line_is_unsupported() {
    // provenance: derived — only taxable supplies are classified as deemed exports here.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::Exempt, None)],
        Some("8501"),
    );
    assert_unsupported(&with_option(item_voucher(PARTY), "deemed_export"), 0, &m);
}

#[test]
fn taxable_line_with_zero_rate_is_unsupported() {
    // provenance: derived — a 0% rate is a nil-rated supply; a master that says taxable at 0%
    // is ambiguous and the user must pick nil-rated or give the rate.
    let m = sales(
        vec![history(date!(2025 - 04 - 01), Taxability::Taxable, Some(0))],
        Some("8501"),
    );
    assert_unsupported(&item_voucher(PARTY), 0, &m);
}

#[test]
fn uin_recipient_is_unsupported() {
    // provenance: derived — supplies to UIN holders (embassies, UN bodies) have their own
    // refund rules and are not modelled.
    let m = sales(taxable18(), Some("8501"));
    assert_unsupported(&item_voucher(UIN_PARTY), 0, &m);
}

#[test]
fn cancelled_reverse_charge_and_unclassified_vouchers_are_unsupported() {
    // provenance: derived — a cancelled voucher has no supply; reverse charge (CGST Act s.9(3))
    // changes who pays and is not classified here; a plain Dr/Cr voucher has no supply lines.
    let m = sales(taxable18(), Some("8501"));
    let mut v = item_voucher(PARTY);
    v.status = VoucherStatus::Cancelled;
    assert_unsupported(&v, 0, &m);
    let mut v = item_voucher(PARTY);
    v.reverse_charge = true;
    assert_unsupported(&v, 0, &m);
    let mut v = item_voucher(PARTY);
    v.mode = VoucherMode::AsVoucher;
    assert_unsupported(&v, 0, &m);
}

#[test]
fn non_supply_voucher_kinds_are_unsupported() {
    // provenance: derived — a sales order, payment or journal is not a supply.
    for base in [
        BaseVoucherKind::SalesOrder,
        BaseVoucherKind::Payment,
        BaseVoucherKind::Journal,
        BaseVoucherKind::DeliveryNote,
    ] {
        let m = masters(base, taxable18(), Some("8501"));
        assert_unsupported(&item_voucher(PARTY), 0, &m);
    }
}

#[test]
fn payment_before_invoice_without_supply_date_is_unsupported() {
    // provenance: derived — CGST Act s.12/s.13 time of supply can be the payment date when it
    // precedes the invoice; this card does not decide it (card G12 does).
    let m = sales(taxable18(), Some("8501"));
    let mut v = item_voucher(PARTY);
    v.payment_date = Some(date!(2026 - 01 - 10));
    assert_unsupported(&v, 0, &m);
    // with an explicit supply date the classification proceeds
    v.supply_date = Some(date!(2026 - 01 - 15));
    assert_eq!(kind(&v, 0, &m), SupplyKind::Taxable);
}

#[test]
fn item_without_gst_details_on_the_supply_date_is_unsupported() {
    // provenance: derived — rule A1: no GST details effective on the time of supply means the
    // taxability is unknown, never a default. History starts after the voucher date.
    let m = sales(
        vec![history(
            date!(2026 - 02 - 01),
            Taxability::Taxable,
            Some(18),
        )],
        Some("8501"),
    );
    assert_unsupported(&item_voucher(PARTY), 0, &m);
}

#[test]
fn item_history_is_selected_by_time_of_supply() {
    // provenance: derived — rule A8: effective-dated by time of supply. Taxable from 22 Sep 2025,
    // exempt from 1 Jan 2026; a supply dated 15 Jan 2026 takes the later entry, a supply dated
    // 15 Dec 2025 the earlier one.
    let hist = vec![
        history(date!(2025 - 09 - 22), Taxability::Taxable, Some(18)),
        history(date!(2026 - 01 - 01), Taxability::Exempt, None),
    ];
    let m = sales(hist, Some("8501"));
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Exempt);
    let mut v = item_voucher(PARTY);
    v.date = date!(2025 - 12 - 15);
    assert_eq!(kind(&v, 0, &m), SupplyKind::Taxable);
}

#[test]
fn accounting_invoice_uses_the_ledger_classification() {
    // provenance: derived — an accounting invoice line takes its taxability from the ledger.
    let m = sales(taxable18(), None);
    assert_eq!(
        kind(&accounting_voucher(PARTY, SALES_LEDGER), 0, &m),
        SupplyKind::Taxable
    );
    assert_eq!(
        kind(&accounting_voucher(PARTY, NON_GST_LEDGER), 0, &m),
        SupplyKind::NonGst
    );
    assert_unsupported(&accounting_voucher(PARTY, UNCLASSIFIED_LEDGER), 0, &m);
}

#[test]
fn credit_note_is_classified_like_a_sale() {
    // provenance: derived — a credit note reverses a supply of the same kind (CGST Act s.34), so
    // an exempt line stays exempt.
    let m = masters(
        BaseVoucherKind::CreditNote,
        vec![history(date!(2025 - 04 - 01), Taxability::Exempt, None)],
        Some("8501"),
    );
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Exempt);
}

#[test]
fn purchase_from_regular_supplier_is_classified_by_taxability() {
    // provenance: derived — an inward supply from a regular taxpayer carries the taxability of
    // the item: taxable 18% is taxable, exempt is exempt.
    let m = masters(BaseVoucherKind::Purchase, taxable18(), Some("8501"));
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Taxable);
    let m = masters(
        BaseVoucherKind::Purchase,
        vec![history(date!(2025 - 04 - 01), Taxability::Exempt, None)],
        Some("8501"),
    );
    assert_eq!(kind(&item_voucher(PARTY), 0, &m), SupplyKind::Exempt);
}

#[test]
fn purchase_from_other_suppliers_is_unsupported() {
    // provenance: derived — a composition supplier issues a bill of supply (CGST Act s.10(4)),
    // an unregistered supplier may trigger reverse charge (s.9(4)), an overseas supplier is an
    // import and an SEZ supplier is inter-State (IGST Act s.7(5)(b)); none is classified here.
    let m = masters(BaseVoucherKind::Purchase, taxable18(), Some("8501"));
    for party in [
        COMPOSITION_PARTY,
        OVERSEAS_PARTY,
        SEZ_PARTY,
        UIN_PARTY,
        NO_DETAILS_PARTY,
    ] {
        assert_unsupported(&item_voucher(party), 0, &m);
    }
    let mut v = item_voucher(PARTY);
    v.party = None;
    assert_unsupported(&v, 0, &m);
    assert_unsupported(&with_option(item_voucher(PARTY), "with_payment"), 0, &m);
}

#[test]
fn receipt_lines_that_are_not_clean_advances_are_unsupported() {
    // provenance: derived — only a credit to the party with only advance allocations is an
    // advance; a line settling bills, mixing both, or on the debit side is not classified.
    let m = masters(BaseVoucherKind::Receipt, vec![], None);
    assert_unsupported(
        &receipt_voucher(vec![bill(BillRefKind::Against)], true),
        1,
        &m,
    );
    assert_unsupported(&receipt_voucher(vec![], true), 1, &m);
    assert_unsupported(
        &receipt_voucher(
            vec![bill(BillRefKind::Advance), bill(BillRefKind::Against)],
            true,
        ),
        1,
        &m,
    );
    assert_unsupported(
        &receipt_voucher(vec![bill(BillRefKind::Advance)], false),
        1,
        &m,
    );
    // the bank line has no advance allocation
    assert_unsupported(
        &receipt_voucher(vec![bill(BillRefKind::Advance)], true),
        0,
        &m,
    );
    // an option makes no sense on a receipt
    assert_unsupported(
        &with_option(
            receipt_voucher(vec![bill(BillRefKind::Advance)], true),
            "with_payment",
        ),
        1,
        &m,
    );
}

#[test]
fn advance_from_overseas_party_is_unsupported() {
    // provenance: derived — advances on zero-rated supplies follow the export rules, which are
    // not modelled.
    let m = masters(BaseVoucherKind::Receipt, vec![], None);
    let mut v = receipt_voucher(vec![bill(BillRefKind::Advance)], true);
    v.ledger_lines.get_mut(1).unwrap().ledger = OVERSEAS_PARTY;
    assert_unsupported(&v, 1, &m);
}

// ---- errors, not Unsupported ---------------------------------------------------------------

#[test]
fn line_past_the_end_is_an_error() {
    // provenance: derived — a caller bug, rule A1 "typed error": the voucher has one line.
    let m = sales(taxable18(), Some("8501"));
    assert!(matches!(
        run(&item_voucher(PARTY), 1, &m),
        Err(GstError::Invalid { .. })
    ));
    assert!(matches!(
        run(&accounting_voucher(PARTY, SALES_LEDGER), 5, &m),
        Err(GstError::Invalid { .. })
    ));
}

#[test]
fn missing_masters_are_errors() {
    // provenance: derived — a voucher that refers to a master that does not exist is corrupt
    // data, not an unsupported case.
    let m = sales(taxable18(), Some("8501"));
    let mut v = item_voucher(PARTY);
    v.type_id = VoucherTypeId(99);
    assert!(matches!(run(&v, 0, &m), Err(GstError::NotFound { .. })));
    let mut v = item_voucher(PARTY);
    v.party = Some(LedgerId(999));
    assert!(matches!(run(&v, 0, &m), Err(GstError::NotFound { .. })));
    let mut v = item_voucher(PARTY);
    v.inventory_lines.get_mut(0).unwrap().item = StockItemId(999);
    assert!(matches!(run(&v, 0, &m), Err(GstError::NotFound { .. })));
}

#[test]
fn rules_for_another_date_are_an_error() {
    // provenance: derived — rule A8: the rule set must be the one in force on the time of supply;
    // the voucher is dated 15 Jan 2026, the rules are selected for 10 Dec 2025.
    let m = sales(taxable18(), Some("8501"));
    let r = classify_line(
        &item_voucher(PARTY),
        0,
        &m,
        &rules_on(date!(2025 - 12 - 10)),
    );
    assert!(matches!(r, Err(GstError::Invalid { .. })));
}

#[test]
fn two_item_history_entries_from_one_date_are_an_error() {
    // provenance: derived — ambiguous master data (two entries effective the same day) is an
    // error, as in voucher tax computation.
    let m = sales(
        vec![
            history(date!(2025 - 09 - 22), Taxability::Taxable, Some(18)),
            history(date!(2025 - 09 - 22), Taxability::Exempt, None),
        ],
        Some("8501"),
    );
    assert!(matches!(
        run(&item_voucher(PARTY), 0, &m),
        Err(GstError::Invalid { .. })
    ));
}
