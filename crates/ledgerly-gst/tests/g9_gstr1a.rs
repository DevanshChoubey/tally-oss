//! Card G9 tests: GSTR-1A amendments (spec 6.3).
//!
//! Provenance. The filed return is built by the G8 builder from the books as they stood at
//! filing; the books are then altered and GSTR-1A is asked for. Expected amounts are hand
//! arithmetic: 18 percent from 22 Sep 2025 (`packs/rates-slabs.toml`, research 04-gst 2.2 `[V]`),
//! inter-state supply is IGST only (IGST Act s.5). Table 9A (B2BA) names the original invoice by
//! number and date and carries the amended one (research 04-gst, amendments, `[K]`). Which
//! differences GSTR-1A can hold is the data shape `Gstr1a` (B2BA only); the others must come back
//! `Unsupported`, which is the rule of this crate, not a figure from the portal.

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
use ledgerly_gst::gstr1::build_gstr1;
use ledgerly_gst::gstr1a::build_gstr1a;
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_gst_formats::gstr1::{Gstr1, Gstr1a, ItemTax};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstClassification, GstHistory,
    GstRegistration, InventoryLine, Ledger, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem, Taxability, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
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

fn filed(books: &Books, pack: &RulePack) -> Gstr1 {
    match build_gstr1(books, &gstin(OWN_GSTIN), january(), pack).unwrap() {
        Outcome::Value((r, _)) => r,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn amend(books: &Books, filed: &Gstr1, pack: &RulePack) -> Outcome<Gstr1a> {
    build_gstr1a(books, &gstin(OWN_GSTIN), january(), filed, pack).unwrap()
}

fn amend_value(books: &Books, filed: &Gstr1, pack: &RulePack) -> Gstr1a {
    match amend(books, filed, pack) {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn unsupported_reason(out: Outcome<Gstr1a>) -> String {
    match out {
        Outcome::Unsupported(u) => format!("{} / {}", u.what, u.reason),
        Outcome::Value(v) => panic!("expected Unsupported, got {v:?}"),
    }
}

#[test]
fn unchanged_books_give_an_empty_amendment() {
    // provenance: derived. Nothing differs between the filed return and the books, so there is
    // nothing to amend: Table 9A is empty.
    let p = full_pack();
    let b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&b, &p);
    let a = amend_value(&b, &f, &p);
    assert_eq!(a.b2ba, [] as [ledgerly_gst_formats::gstr1::B2baParty; 0]);
    assert_eq!(a.gstin, gstin(OWN_GSTIN));
    assert_eq!(a.period, january());
}

#[test]
fn altering_a_filed_invoice_gives_a_1a_entry_and_leaves_gstr1_alone() {
    // provenance: derived. Filed: INV-1, taxable 1,000,000 paise, IGST 180,000, value 1,180,000.
    // Altered to taxable 2,000,000 paise: IGST = 2,000,000 x 18 percent = 360,000; value
    // 2,360,000. B2BA names the original as INV-1 dated 15 Jan 2026 and carries the new figures.
    let p = full_pack();
    let before = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&before, &p);
    let f_copy = f.clone();
    let after = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[2_000_000])]);
    let a = amend_value(&after, &f, &p);
    // The filed return is untouched, and differs from what the books now say.
    assert_eq!(f, f_copy);
    assert_ne!(filed(&after, &p), f);
    assert_eq!(a.b2ba.len(), 1);
    let party = a.b2ba.first().unwrap();
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    assert_eq!(party.invoices.len(), 1);
    let entry = party.invoices.first().unwrap();
    assert_eq!(entry.original_num, "INV-1");
    assert_eq!(entry.original_date, date!(2026 - 01 - 15));
    assert_eq!(entry.amended.num, "INV-1");
    assert_eq!(entry.amended.value, paise(2_360_000));
    let item = entry.amended.items.first().unwrap();
    assert_eq!(item.taxable, paise(2_000_000));
    assert_eq!(item.tax, ItemTax::Igst(paise(360_000)));
}

#[test]
fn only_the_altered_invoice_is_amended() {
    // provenance: derived. Two filed invoices; only INV-2 changes (500,000 to 600,000 paise
    // taxable, IGST 18 percent = 108,000, value 708,000). INV-1 is identical and has no entry.
    let p = full_pack();
    let before = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        sale(2, "INV-2", REGISTERED, GUJARAT, &[500_000]),
    ]);
    let f = filed(&before, &p);
    let after = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        sale(2, "INV-2", REGISTERED, GUJARAT, &[600_000]),
    ]);
    let a = amend_value(&after, &f, &p);
    assert_eq!(a.b2ba.len(), 1);
    let invoices = &a.b2ba.first().unwrap().invoices;
    assert_eq!(invoices.len(), 1);
    let entry = invoices.first().unwrap();
    assert_eq!(entry.original_num, "INV-2");
    assert_eq!(entry.amended.value, paise(708_000));
}

#[test]
fn changed_invoice_date_keeps_the_filed_date_as_original() {
    // provenance: derived. The invoice is re-dated from 15 Jan to 20 Jan 2026 (same period,
    // same figures). B2BA identifies the original by its filed number and date (15 Jan) and the
    // amended invoice carries the new date.
    let p = full_pack();
    let before = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&before, &p);
    let mut moved = sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]);
    moved.date = date!(2026 - 01 - 20);
    let a = amend_value(&books(vec![moved]), &f, &p);
    let entry = a.b2ba.first().unwrap().invoices.first().unwrap();
    assert_eq!(entry.original_date, date!(2026 - 01 - 15));
    assert_eq!(entry.amended.date, date!(2026 - 01 - 20));
}

#[test]
fn an_invoice_not_in_the_filed_return_is_unsupported() {
    // provenance: derived. B2BA amends filed invoices only; a new invoice cannot be written, so
    // the whole amendment is Unsupported rather than silently missing it.
    let p = full_pack();
    let before = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&before, &p);
    let after = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        sale(2, "INV-2", REGISTERED, GUJARAT, &[500_000]),
    ]);
    let reason = unsupported_reason(amend(&after, &f, &p));
    assert!(reason.contains("INV-2"), "{reason}");
    assert!(reason.contains("not in the filed return"), "{reason}");
}

#[test]
fn a_filed_invoice_missing_from_the_books_is_unsupported() {
    // provenance: derived. INV-1 was filed but is gone from the books: B2BA cannot express a
    // deletion or a renumbering, so the result is Unsupported and names the invoice.
    let p = full_pack();
    let before = books(vec![
        sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000]),
        sale(2, "INV-2", REGISTERED, GUJARAT, &[500_000]),
    ]);
    let f = filed(&before, &p);
    let after = books(vec![sale(2, "INV-2", REGISTERED, GUJARAT, &[500_000])]);
    let reason = unsupported_reason(amend(&after, &f, &p));
    assert!(reason.contains("INV-1"), "{reason}");
    assert!(reason.contains("not in the books"), "{reason}");
}

#[test]
fn a_change_outside_b2b_is_unsupported_and_names_the_section() {
    // provenance: derived. A B2CS (Table 7) contribution changes from 1,000,000 to 2,000,000
    // paise taxable. B2CSA is not modelled, so the amendment is Unsupported and names b2cs.
    let p = full_pack();
    let before = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&before, &p);
    let after = books(vec![sale(1, "INV-1", UNREGISTERED, GUJARAT, &[2_000_000])]);
    let reason = unsupported_reason(amend(&after, &f, &p));
    assert!(reason.contains("b2cs"), "{reason}");
}

#[test]
fn an_unresolved_voucher_in_the_books_is_unsupported() {
    // provenance: derived. A registered party without a GSTIN is NeedsCorrection in the G8
    // builder, so the invoice would vanish from the current return; the amendment refuses
    // instead of reading that as a deletion.
    let p = full_pack();
    let before = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let f = filed(&before, &p);
    let after = books(vec![sale(
        1,
        "INV-1",
        REGISTERED_NO_GSTIN,
        GUJARAT,
        &[1_000_000],
    )]);
    let reason = unsupported_reason(amend(&after, &f, &p));
    assert!(reason.contains("voucher 1"), "{reason}");
}

#[test]
fn filed_return_of_another_gstin_or_period_is_an_error() {
    // provenance: derived. A filed return for a different GSTIN or period is not the return
    // being amended: a typed error, not an empty or wrong amendment.
    let p = full_pack();
    let b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let mut other_gstin = filed(&b, &p);
    other_gstin.gstin = gstin(BUYER_GSTIN);
    assert!(matches!(
        build_gstr1a(&b, &gstin(OWN_GSTIN), january(), &other_gstin, &p),
        Err(GstError::Invalid { .. })
    ));
    let mut other_period = filed(&b, &p);
    other_period.period = Period::new(date!(2025 - 12 - 01), date!(2025 - 12 - 31)).unwrap();
    assert!(matches!(
        build_gstr1a(&b, &gstin(OWN_GSTIN), january(), &other_period, &p),
        Err(GstError::Invalid { .. })
    ));
}

#[test]
fn invalid_period_for_the_rebuild_is_unsupported() {
    // provenance: derived. A period that is neither a month nor a quarter is not a return
    // period; the G8 builder says Unsupported and 1A passes that on. The filed return must
    // carry the same period to get that far.
    let p = full_pack();
    let b = books(vec![sale(1, "INV-1", REGISTERED, GUJARAT, &[1_000_000])]);
    let odd = Period::new(date!(2026 - 01 - 05), date!(2026 - 01 - 20)).unwrap();
    let mut f = filed(&b, &p);
    f.period = odd;
    let out = build_gstr1a(&b, &gstin(OWN_GSTIN), odd, &f, &p).unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)));
}
