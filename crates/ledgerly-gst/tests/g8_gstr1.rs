//! Card G8 tests: the GSTR-1 builder and its exception buckets (spec 6.3).
//!
//! Provenance. Placement rules are derived from the GSTR-1 table definitions: a sale to a
//! registered recipient is a B2B invoice (Table 4A); a sale to an unregistered recipient is B2CL
//! (Table 5) when it is inter-state and the invoice value is above the threshold, otherwise B2CS
//! (Table 7); a credit note to a registered recipient is a CDNR note (Table 9B); Table 12 sums
//! taxable value and tax by HSN, unit quantity code and rate; Table 13 lists runs of document
//! numbers with the cancelled ones counted. Slab percentages come from `packs/rates-slabs.toml`
//! (GST 2.0, 18 percent from 22 Sep 2025, research 04-gst 2.2 `[V]`). The B2CL threshold and the
//! credit-note HSN treatment are parameters the tests put in their own fixture pack, so the
//! expected values below hold for those fixture parameters and say nothing about the real
//! threshold. Every expected figure is hand arithmetic written next to it; none is copied from
//! the engine.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::format_push_string,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_core::{
    CompanyId, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::GstError;
use ledgerly_gst::gstr1::{Bucket, BucketedVoucher, build_gstr1};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_gst_formats::gstr1::{B2bInvoiceType, DocRange, DocType, Gstr1, ItemTax, NoteKind};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstClassification, GstHistory,
    GstLine, GstRegistration, InventoryLine, Ledger, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem, StoredGst,
    TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use time::macros::date;

const KARNATAKA: u8 = 29;
const GUJARAT: u8 = 24;

/// Fictional GSTIN from the core test vectors (Karnataka): the company's own registration.
const OWN_GSTIN: &str = "29ABCDE1234F1ZW";
/// Fictional GSTIN from the formats test vectors (Gujarat): a registered recipient.
const BUYER_GSTIN: &str = "24AAACB1111C1ZT";

const ITEM: StockItemId = StockItemId(1);
const SALES_LEDGER: LedgerId = LedgerId(10);
const REGISTERED: LedgerId = LedgerId(11);
const REGISTERED_NO_GSTIN: LedgerId = LedgerId(12);
const UNREGISTERED: LedgerId = LedgerId(13);
const IGST_LEDGER: LedgerId = LedgerId(14);

const SALES: VoucherTypeId = VoucherTypeId(1);
const CREDIT_NOTE: VoucherTypeId = VoucherTypeId(2);
const PURCHASE: VoucherTypeId = VoucherTypeId(3);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn gstin(text: &str) -> Gstin {
    Gstin::parse(text).unwrap()
}

fn january() -> Period {
    Period::new(date!(2026 - 01 - 01), date!(2026 - 01 - 31)).unwrap()
}

fn paise(n: i64) -> Paise {
    Paise::new(n)
}

fn rate(percent: i64) -> Rate {
    Rate(Decimal::new(percent, 0))
}

// ---------------------------------------------------------------------------------------------
// Fixtures

/// The real slab pack plus the two GSTR-1 entries, each only when asked for.
fn pack(threshold_rupees: Option<&str>, credit_note_treatment: Option<&str>) -> RulePack {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join("rates-slabs.toml");
    let mut text = fs::read_to_string(path).unwrap();
    if let Some(t) = threshold_rupees {
        text.push_str(&format!(
            "\n[[entry]]\nid = \"gstr1.b2cl_threshold\"\neffective_from = \"2024-08-01\"\nlegal_ref = \"fixture for the G8 tests\"\n[entry.params]\ninvoice_value_rupees = \"{t}\"\n"
        ));
    }
    if let Some(t) = credit_note_treatment {
        text.push_str(&format!(
            "\n[[entry]]\nid = \"gstr1.hsn_credit_notes\"\neffective_from = \"2024-08-01\"\nlegal_ref = \"fixture for the G8 tests\"\n[entry.params]\ntreatment = \"{t}\"\n"
        ));
    }
    load_pack(&text).unwrap()
}

fn full_pack() -> RulePack {
    pack(Some("100000"), Some("net"))
}

fn company() -> Company {
    Company {
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(KARNATAKA),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin(OWN_GSTIN),
            state: state(KARNATAKA),
            effective_from: date!(2017 - 07 - 01),
            composition: false,
        }],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2025 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn party_ledger(
    id: LedgerId,
    registration: RegistrationType,
    gstin_text: Option<&str>,
    code: u8,
) -> Ledger {
    Ledger {
        id,
        name: format!("Party {}", id.0),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: Some(PartyDetails {
            registration,
            gstin: gstin_text.map(gstin),
            pan: None,
            state: Some(state(code)),
            address: Address::default(),
        }),
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn voucher_type(id: VoucherTypeId, base: BaseVoucherKind, name: &str) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.to_owned(),
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
    }
}

fn masters() -> Masters {
    let mut m = Masters::default();
    m.voucher_types
        .insert(SALES, voucher_type(SALES, BaseVoucherKind::Sales, "Sales"));
    m.voucher_types.insert(
        CREDIT_NOTE,
        voucher_type(CREDIT_NOTE, BaseVoucherKind::CreditNote, "Credit Note"),
    );
    m.voucher_types.insert(
        PURCHASE,
        voucher_type(PURCHASE, BaseVoucherKind::Purchase, "Purchase"),
    );
    m.units.insert(
        UnitId(1),
        Unit {
            id: UnitId(1),
            symbol: "pcs".to_owned(),
            formal_name: "Pieces".to_owned(),
            decimals: 0,
            kind: UnitKind::Simple,
            uqc: Some("PCS".to_owned()),
            tally: None,
        },
    );
    m.stock_items.insert(
        ITEM,
        StockItem {
            id: ITEM,
            name: "Widget".to_owned(),
            aliases: vec![],
            group: None,
            unit: UnitId(1),
            alt_unit: None,
            hsn: Some(Hsn::parse("8471").unwrap()),
            gst_history: vec![GstHistory {
                effective_from: date!(2025 - 09 - 22),
                taxability: Taxability::Taxable,
                rate_percent: Some(rate(18)),
            }],
            valuation: ValuationMethod::AverageCost,
            batches_enabled: false,
            opening: vec![],
            standard_costs: vec![],
            tally: None,
        },
    );
    m.ledgers.insert(
        SALES_LEDGER,
        Ledger {
            gst: Some(GstClassification {
                hsn: None,
                taxability: Taxability::Taxable,
                rate_percent: Some(rate(5)),
            }),
            party: None,
            ..party_ledger(SALES_LEDGER, RegistrationType::Regular, None, KARNATAKA)
        },
    );
    m.ledgers.insert(
        REGISTERED,
        party_ledger(
            REGISTERED,
            RegistrationType::Regular,
            Some(BUYER_GSTIN),
            GUJARAT,
        ),
    );
    m.ledgers.insert(
        REGISTERED_NO_GSTIN,
        party_ledger(
            REGISTERED_NO_GSTIN,
            RegistrationType::Regular,
            None,
            GUJARAT,
        ),
    );
    m.ledgers.insert(
        UNREGISTERED,
        party_ledger(UNREGISTERED, RegistrationType::Unregistered, None, GUJARAT),
    );
    m
}

fn books(vouchers: Vec<Voucher>) -> Books {
    Books {
        company: company(),
        masters: masters(),
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn voucher(id: u64, type_id: VoucherTypeId, number: &str, party: LedgerId, pos: u8) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: Some(number.to_owned()),
        series: None,
        date: date!(2026 - 01 - 15),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::ItemInvoice,
        party: Some(party),
        place_of_supply: Some(state(pos)),
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

fn inv_line(amount_paise: i64) -> InventoryLine {
    InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::ONE),
        unit: UnitId(1),
        rate: Rate(Decimal::new(amount_paise, 2)),
        discount_percent: None,
        amount: paise(amount_paise),
        ledger: Some(SALES_LEDGER),
        godowns: vec![],
    }
}

/// A sales voucher with one inventory line per amount (in paise).
fn sale(id: u64, number: &str, party: LedgerId, pos: u8, amounts: &[i64]) -> Voucher {
    Voucher {
        inventory_lines: amounts.iter().map(|a| inv_line(*a)).collect(),
        ..voucher(id, SALES, number, party, pos)
    }
}

fn build(books: &Books, pack: &RulePack) -> (Gstr1, Vec<BucketedVoucher>) {
    match build_gstr1(books, &gstin(OWN_GSTIN), january(), pack).unwrap() {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn bucket_of(buckets: &[BucketedVoucher], id: u64) -> &BucketedVoucher {
    buckets
        .iter()
        .find(|b| b.voucher == VoucherId(id))
        .unwrap_or_else(|| panic!("voucher {id} has no bucket"))
}

fn assert_needs_correction(buckets: &[BucketedVoucher], id: u64, reason_part: &str) {
    let b = bucket_of(buckets, id);
    assert_eq!(b.bucket, Bucket::NeedsCorrection, "voucher {id}: {b:?}");
    let reason = b.reason.as_deref().unwrap_or("");
    assert!(
        reason.contains(reason_part),
        "voucher {id}: reason `{reason}` lacks `{reason_part}`"
    );
}

// ---------------------------------------------------------------------------------------------
// Tests

#[test]
fn b2b_invoice_lands_in_table_4a_and_hsn() {
    // provenance: derived. Sale to a registered recipient in another state: Table 4A, IGST only
    // (IGST Act s.5). Rs 10,000.00 = 1,000,000 paise at 18 percent: IGST = 180,000 paise, invoice
    // value 1,180,000 paise, already whole rupees. Table 12: one row, qty 1, taxable 1,000,000,
    // IGST 180,000, value 1,180,000.
    let b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(buckets.len(), 1);
    assert_eq!(bucket_of(&buckets, 1).bucket, Bucket::Ready);
    assert_eq!(bucket_of(&buckets, 1).reason, None);
    assert_eq!(r.b2b.len(), 1);
    let party = r.b2b.first().unwrap();
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    let inv = party.invoices.first().unwrap();
    assert_eq!(inv.num, "INV-1");
    assert_eq!(inv.date, date!(2026 - 01 - 15));
    assert_eq!(inv.value, paise(1_180_000));
    assert_eq!(inv.pos, state(GUJARAT));
    assert!(!inv.reverse_charge);
    assert_eq!(inv.inv_type, B2bInvoiceType::Regular);
    let item = inv.items.first().unwrap();
    assert_eq!(item.rate, rate(18));
    assert_eq!(item.taxable, paise(1_000_000));
    assert_eq!(item.tax, ItemTax::Igst(paise(180_000)));
    assert_eq!(item.cess, Paise::ZERO);
    assert!(r.b2cl.is_empty() && r.b2cs.is_empty() && r.cdnr.is_empty());
    assert_eq!(r.hsn.len(), 1);
    let h = r.hsn.first().unwrap();
    assert_eq!(h.hsn.as_str(), "8471");
    assert_eq!(h.uqc, "PCS");
    assert_eq!(h.qty, Qty(Decimal::ONE));
    assert_eq!(h.rate, rate(18));
    assert_eq!(h.taxable, paise(1_000_000));
    assert_eq!(h.igst, paise(180_000));
    assert_eq!(h.value, paise(1_180_000));
}

#[test]
fn intra_state_b2b_invoice_splits_cgst_and_sgst() {
    // provenance: derived. Intra-state supply is CGST + SGST (CGST Act s.9); 18 percent is 9 + 9.
    // 1,000,000 paise x 9 percent = 90,000 paise each; value 1,180,000.
    let b = books(vec![sale(1, "INV-1", REGISTERED, KARNATAKA, &[1_000_000])]);
    let (r, _) = build(&b, &full_pack());
    let item = r
        .b2b
        .first()
        .unwrap()
        .invoices
        .first()
        .unwrap()
        .items
        .first()
        .unwrap();
    assert_eq!(
        item.tax,
        ItemTax::CgstSgst {
            cgst: paise(90_000),
            sgst: paise(90_000)
        }
    );
}

#[test]
fn inter_state_unregistered_above_threshold_is_b2cl() {
    // provenance: derived. Inter-state sale to an unregistered recipient with invoice value above
    // the threshold is B2CL (Table 5). Fixture threshold Rs 100,000. Taxable Rs 100,000.00 =
    // 10,000,000 paise; IGST 18 percent = 1,800,000; value 11,800,000 paise = Rs 118,000 > Rs 100,000.
    let b = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[10_000_000])]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(bucket_of(&buckets, 1).bucket, Bucket::Ready);
    assert!(r.b2cs.is_empty() && r.b2b.is_empty());
    let g = r.b2cl.first().unwrap();
    assert_eq!(g.pos, state(GUJARAT));
    let inv = g.invoices.first().unwrap();
    assert_eq!(inv.num, "INV-1");
    assert_eq!(inv.value, paise(11_800_000));
    assert_eq!(
        inv.items.first().unwrap().tax,
        ItemTax::Igst(paise(1_800_000))
    );
}

#[test]
fn invoice_value_equal_to_threshold_is_b2cs_and_one_paisa_below_is_b2cl() {
    // provenance: derived. B2CL needs the value to exceed the threshold, so a value equal to it
    // stays B2CS. Invoice value is 11,800,000 paise = Rs 118,000.00. Threshold Rs 118000: equal,
    // B2CS. Threshold Rs 117999.99: the value is one paisa above, B2CL.
    let b = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[10_000_000])]);
    let (r, _) = build(&b, &pack(Some("118000"), Some("net")));
    assert_eq!(r.b2cl, [] as [ledgerly_gst_formats::gstr1::B2clGroup; 0]);
    assert_eq!(r.b2cs.len(), 1);
    let (r, _) = build(&b, &pack(Some("117999.99"), Some("net")));
    assert_eq!(r.b2cl.len(), 1);
    assert_eq!(r.b2cs, [] as [ledgerly_gst_formats::gstr1::B2csRow; 0]);
}

#[test]
fn small_inter_state_unregistered_sale_is_b2cs_igst_row() {
    // provenance: derived. Value 1,180,000 paise is below the Rs 100,000 fixture threshold: B2CS,
    // one row by place of supply and rate, inter-state, IGST 180,000 on taxable 1,000,000.
    let b = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[1_000_000])]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(bucket_of(&buckets, 1).bucket, Bucket::Ready);
    assert_eq!(r.b2cl, [] as [ledgerly_gst_formats::gstr1::B2clGroup; 0]);
    let row = r.b2cs.first().unwrap();
    assert!(!row.intra_state);
    assert_eq!(row.pos, state(GUJARAT));
    assert_eq!(row.line.rate, rate(18));
    assert_eq!(row.line.taxable, paise(1_000_000));
    assert_eq!(row.line.tax, ItemTax::Igst(paise(180_000)));
}

#[test]
fn intra_state_unregistered_sale_is_never_b2cl() {
    // provenance: derived. B2CL covers inter-state supplies only. Rs 118,000 intra-state is B2CS:
    // taxable 10,000,000, CGST = SGST = 900,000.
    let b = books(vec![sale(
        1,
        "INV-1",
        UNREGISTERED,
        KARNATAKA,
        &[10_000_000],
    )]);
    let (r, _) = build(&b, &full_pack());
    assert_eq!(r.b2cl, [] as [ledgerly_gst_formats::gstr1::B2clGroup; 0]);
    let row = r.b2cs.first().unwrap();
    assert!(row.intra_state);
    assert_eq!(
        row.line.tax,
        ItemTax::CgstSgst {
            cgst: paise(900_000),
            sgst: paise(900_000)
        }
    );
}

#[test]
fn b2cs_rows_add_up_vouchers_of_one_place_and_rate() {
    // provenance: derived. Two intra-state unregistered sales at 18 percent: taxable 1,000,000 and
    // 500,000. CGST 9 percent: 90,000 + 45,000 = 135,000; SGST the same. Taxable 1,500,000. One row.
    let b = books(vec![
        sale(1, "INV-1", UNREGISTERED, KARNATAKA, &[1_000_000]),
        sale(2, "INV-2", UNREGISTERED, KARNATAKA, &[500_000]),
    ]);
    let (r, _) = build(&b, &full_pack());
    assert_eq!(r.b2cs.len(), 1);
    let row = r.b2cs.first().unwrap();
    assert_eq!(row.line.taxable, paise(1_500_000));
    assert_eq!(
        row.line.tax,
        ItemTax::CgstSgst {
            cgst: paise(135_000),
            sgst: paise(135_000)
        }
    );
}

#[test]
fn unregistered_sale_without_a_threshold_in_the_pack_needs_correction() {
    // provenance: derived. An inter-state sale to an unregistered recipient cannot be placed
    // between B2CL and B2CS without the threshold; with no verified pack entry the builder says
    // so and does not guess.
    let b = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[1_000_000])]);
    let (r, buckets) = build(&b, &pack(None, Some("net")));
    assert_needs_correction(&buckets, 1, "unsupported");
    assert!(r.b2cl.is_empty() && r.b2cs.is_empty() && r.hsn.is_empty());
}

#[test]
fn credit_note_to_registered_recipient_lands_in_9b() {
    // provenance: derived. A credit note to a registered recipient is a CDNR note (Table 9B).
    // Rs 5,000.00 = 500,000 paise inter-state at 18 percent: IGST 90,000; value 590,000.
    // The HSN table treatment is the fixture's `net`: the note reduces the invoice's row.
    // Invoice 1,000,000 taxable, IGST 180,000, qty 1; note 500,000, IGST 90,000, qty 1.
    // Net row: qty 0, taxable 500,000, IGST 90,000, value 590,000.
    let mut note = voucher(2, CREDIT_NOTE, "CN-1", REGISTERED, GUJARAT);
    note.inventory_lines = vec![inv_line(500_000)];
    let b = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        note,
    ]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(bucket_of(&buckets, 2).bucket, Bucket::Ready);
    let party = r.cdnr.first().unwrap();
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    let n = party.notes.first().unwrap();
    assert_eq!(n.kind, NoteKind::Credit);
    assert_eq!(n.num, "CN-1");
    assert_eq!(n.value, paise(590_000));
    assert_eq!(n.items.first().unwrap().tax, ItemTax::Igst(paise(90_000)));
    assert_eq!(r.b2b.first().unwrap().invoices.len(), 1);
    let h = r.hsn.first().unwrap();
    assert_eq!(h.qty, Qty(Decimal::ZERO));
    assert_eq!(h.taxable, paise(500_000));
    assert_eq!(h.igst, paise(90_000));
    assert_eq!(h.value, paise(590_000));
}

#[test]
fn credit_note_without_hsn_treatment_in_the_pack_needs_correction() {
    // provenance: derived. How a credit note enters Table 12 is a rule the pack must state; with
    // no entry the note is listed for correction and left out of the return.
    let mut note = voucher(2, CREDIT_NOTE, "CN-1", REGISTERED, GUJARAT);
    note.inventory_lines = vec![inv_line(500_000)];
    let b = books(vec![note]);
    let (r, buckets) = build(&b, &pack(Some("100000"), None));
    assert_needs_correction(&buckets, 2, "unsupported");
    assert_eq!(r.cdnr, [] as [ledgerly_gst_formats::gstr1::CdnrParty; 0]);
}

#[test]
fn registered_party_without_gstin_needs_correction_with_voucher_id() {
    // provenance: derived. A B2B row is keyed by the recipient GSTIN, so a registered party with
    // none cannot be placed: NeedsCorrection, naming the voucher.
    let b = books(vec![sale(
        7,
        "INV-7",
        REGISTERED_NO_GSTIN,
        GUJARAT,
        &[1_000_000],
    )]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets.first().unwrap().voucher, VoucherId(7));
    assert_needs_correction(&buckets, 7, "GSTIN");
    assert!(r.b2b.is_empty() && r.hsn.is_empty());
}

#[test]
fn voucher_with_no_number_or_no_hsn_needs_correction() {
    // provenance: derived. Every document needs a number (CGST Rules 2017 r.46) and Table 12
    // needs an HSN or SAC code.
    let mut unnumbered = sale(1, "x", REGISTERED, GUJARAT, &[1_000_000]);
    unnumbered.number = None;
    let mut b = books(vec![
        unnumbered,
        sale(2, "INV-2", REGISTERED, GUJARAT, &[1_000_000]),
    ]);
    let (_, buckets) = build(&b, &full_pack());
    assert_needs_correction(&buckets, 1, "number");
    assert_eq!(bucket_of(&buckets, 2).bucket, Bucket::Ready);
    b.masters.stock_items.get_mut(&ITEM).unwrap().hsn = None;
    let (_, buckets) = build(&b, &full_pack());
    assert_needs_correction(&buckets, 2, "HSN");
}

#[test]
fn duplicate_document_numbers_need_correction() {
    // provenance: derived. Two invoices with one number cannot both be reported; both are listed.
    let b = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        sale(2, "INV-1", REGISTERED, GUJARAT, &[500_000]),
    ]);
    let (r, buckets) = build(&b, &full_pack());
    assert_needs_correction(&buckets, 1, "INV-1");
    assert_needs_correction(&buckets, 2, "INV-1");
    assert_eq!(r.b2b, [] as [ledgerly_gst_formats::gstr1::B2bParty; 0]);
}

#[test]
fn purchases_are_not_relevant_and_other_months_get_no_entry() {
    // provenance: derived. A purchase is an inward supply, not reported in GSTR-1. A voucher dated
    // in February is not part of the January return.
    let purchase = Voucher {
        inventory_lines: vec![inv_line(1_000_000)],
        ..voucher(1, PURCHASE, "P-1", REGISTERED, GUJARAT)
    };
    let mut february = sale(2, "INV-2", REGISTERED, GUJARAT, &[1_000_000]);
    february.date = date!(2026 - 02 - 02);
    let b = books(vec![purchase, february]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(buckets.len(), 1);
    assert_eq!(bucket_of(&buckets, 1).bucket, Bucket::NotRelevant);
    assert!(bucket_of(&buckets, 1).reason.is_some());
    assert_eq!(r.b2b, [] as [ledgerly_gst_formats::gstr1::B2bParty; 0]);
}

#[test]
fn stored_tax_that_disagrees_with_the_masters_is_a_mismatch() {
    // provenance: derived. The masters give IGST 180,000 paise on 1,000,000 at 18 percent. A
    // voucher that stores IGST of 170,000 disagrees with its own books: Mismatch, left out.
    let mut v = sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]);
    v.gst = Some(StoredGst {
        pack_version: "2026.1-slabs".to_owned(),
        lines: vec![GstLine {
            ledger: IGST_LEDGER,
            kind: TaxLedgerKind::Igst,
            taxable: paise(1_000_000),
            rate_percent: rate(18),
            tax: paise(170_000),
        }],
        round_off: Paise::ZERO,
    });
    let (r, buckets) = build(&books(vec![v]), &full_pack());
    let b = bucket_of(&buckets, 1);
    assert_eq!(b.bucket, Bucket::Mismatch);
    assert!(b.reason.is_some());
    assert_eq!(r.b2b, [] as [ledgerly_gst_formats::gstr1::B2bParty; 0]);
}

#[test]
fn documents_issued_list_runs_with_cancelled_counted_and_gaps_split() {
    // provenance: derived from the Table 13 instructions: from and to serial number, total number
    // and cancelled number. INV-1, INV-2 (cancelled), INV-3 are one run of 3 with 1 cancelled;
    // INV-4 is missing, so INV-5 starts a new run of 1.
    let mut cancelled = sale(2, "INV-2", REGISTERED, GUJARAT, &[1_000_000]);
    cancelled.status = VoucherStatus::Cancelled;
    let b = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        cancelled,
        sale(3, "INV-3", REGISTERED, GUJARAT, &[1_000_000]),
        sale(5, "INV-5", REGISTERED, GUJARAT, &[1_000_000]),
    ]);
    let (r, buckets) = build(&b, &full_pack());
    assert_eq!(bucket_of(&buckets, 2).bucket, Bucket::NotRelevant);
    assert_eq!(r.doc_issue.len(), 1);
    let d = r.doc_issue.first().unwrap();
    assert_eq!(d.doc_type, DocType::OutwardInvoice);
    assert_eq!(
        d.ranges,
        vec![
            DocRange {
                from: "INV-1".to_owned(),
                to: "INV-3".to_owned(),
                total: 3,
                cancelled: 1
            },
            DocRange {
                from: "INV-5".to_owned(),
                to: "INV-5".to_owned(),
                total: 1,
                cancelled: 0
            },
        ]
    );
}

#[test]
fn a_period_that_is_not_a_month_or_quarter_is_unsupported() {
    // provenance: derived. GSTR-1 is filed monthly or quarterly; a 16-day period is neither.
    let b = books(vec![]);
    let p = Period::new(date!(2026 - 01 - 05), date!(2026 - 01 - 20)).unwrap();
    let out = build_gstr1(&b, &gstin(OWN_GSTIN), p, &full_pack()).unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)));
}

#[test]
fn a_gstin_that_is_not_a_registration_of_the_company_is_an_error() {
    // provenance: derived. Asking for another taxpayer's return from these books is a caller
    // error, not an empty return.
    let b = books(vec![]);
    let out = build_gstr1(&b, &gstin(BUYER_GSTIN), january(), &full_pack());
    assert!(matches!(out, Err(GstError::Invalid { .. })));
}

#[test]
fn quarter_period_is_accepted() {
    // provenance: derived. QRMP filers file GSTR-1 quarterly; Jan-Mar is one quarter.
    let b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let p = Period::new(date!(2026 - 01 - 01), date!(2026 - 03 - 31)).unwrap();
    let out = build_gstr1(&b, &gstin(OWN_GSTIN), p, &full_pack()).unwrap();
    let Outcome::Value((r, buckets)) = out else {
        panic!("quarter should be supported");
    };
    assert_eq!(buckets.len(), 1);
    assert_eq!(r.b2b.len(), 1);
}

#[test]
fn a_company_with_two_registrations_is_unsupported() {
    // provenance: derived. A voucher carries no registration, so with two GSTINs the same
    // vouchers would appear in both returns; the builder refuses instead of double counting.
    let mut b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    b.company.registrations.push(GstRegistration {
        gstin: gstin(BUYER_GSTIN),
        state: state(GUJARAT),
        effective_from: date!(2017 - 07 - 01),
        composition: false,
    });
    let out = build_gstr1(&b, &gstin(OWN_GSTIN), january(), &full_pack()).unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)));
}
