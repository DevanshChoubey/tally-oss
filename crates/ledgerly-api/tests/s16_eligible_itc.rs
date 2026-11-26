//! Sprint 16: eligible input tax credit of a purchase draft, from the engine.
//!
//! Rules, read 2026-11-26: CGST Act s.16(1) lets a registered person take credit of tax charged on
//! supplies used in the course or furtherance of business; s.17(5) blocks listed supplies. The
//! accounting product's page on ineligible credit
//! (<https://help.tallysolutions.com/docs/te9rel66/Tax_India/gst/gst_accounting_for_ineligible_itc.htm>)
//! says: set "Is ineligible for input credit?" to Yes in the group/ledger or stock group/item, or
//! at the company level; the invoice is "pre-filled from the master" and can be changed while
//! recording. The draft figure is the same rule as GSTR-3B 4(A) (`itc_eligibility`): the tax of
//! each purchase line the rule says is eligible, summed per head unrounded then rounded to the
//! paisa. What a draft cannot know is the s.16(2) conditions (the supplier's bill in 2B, receipt,
//! payment within 180 days), so the figure is eligible before those are met.
//!
//! Provenance: `derived`, the arithmetic in each test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    dead_code,
    reason = "test code: rule B2 lets tests unwrap and panic; the shared fixture has helpers some files do not use"
)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation, GstClassification,
    GstHistory, GstRegistration, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy,
    PartyDetails, RegistrationType, ReservedGroup, StockItem, TaxDetails, TaxLedgerKind,
    Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const COMPANY: &str = "29ABCDE1234F1Z";
const BUYER_GSTIN: &str = "27RSTUV3456W1Z";
const SUPPLIER_GSTIN: &str = "29LMNOP9012Q1Z";

const KA_BUYER: LedgerId = LedgerId(10);
const MH_BUYER: LedgerId = LedgerId(11);
const SUPPLIER: LedgerId = LedgerId(12);
const SALES: LedgerId = LedgerId(20);
const PURCHASE: LedgerId = LedgerId(21);
const CGST: LedgerId = LedgerId(30);
const SGST: LedgerId = LedgerId(31);
const IGST: LedgerId = LedgerId(32);
const ITEM: StockItemId = StockItemId(1);
const UNIT: UnitId = UnitId(1);
const MAIN: GodownId = GodownId(1);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn gstin_of(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn vtype(base: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == base)
        .unwrap()
        .id
}

fn plain(id: LedgerId, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
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

fn party(id: LedgerId, name: &str, kind: ReservedGroup, gstin: &str, code: u8) -> Ledger {
    Ledger {
        party: Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(gstin_of(gstin)),
            pan: None,
            state: Some(state(code)),
            address: Address::default(),
        }),
        ..plain(id, name, kind)
    }
}

fn tax(id: LedgerId, name: &str, kind: TaxLedgerKind) -> Ledger {
    Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: None,
        }),
        ..plain(id, name, ReservedGroup::DutiesAndTaxes)
    }
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(29),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin_of(COMPANY),
            state: state(29),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A company file with the masters: a Karnataka buyer (state 29, intra-state for the company), a
/// Maharashtra buyer (27, inter-state), a Karnataka supplier, one ledger per tax head, and a
/// Widget item taxed at 18 per cent.
fn company_file() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    let ledgers = vec![
        party(
            KA_BUYER,
            "Ka Buyer",
            ReservedGroup::SundryDebtors,
            "29FGHIJ5678K1Z",
            29,
        ),
        party(
            MH_BUYER,
            "Mh Buyer",
            ReservedGroup::SundryDebtors,
            BUYER_GSTIN,
            27,
        ),
        party(
            SUPPLIER,
            "Supplier",
            ReservedGroup::SundryCreditors,
            SUPPLIER_GSTIN,
            29,
        ),
        plain(SALES, "Sales", ReservedGroup::SalesAccounts),
        plain(PURCHASE, "Purchases", ReservedGroup::PurchaseAccounts),
        tax(CGST, "CGST", TaxLedgerKind::Cgst),
        tax(SGST, "SGST", TaxLedgerKind::Sgst),
        tax(IGST, "IGST", TaxLedgerKind::Igst),
    ];
    for l in ledgers {
        ok(api.execute(Command::CreateLedger(l)));
    }
    ok(api.execute(Command::CreateUnit(Unit {
        id: UNIT,
        symbol: "pcs".to_owned(),
        formal_name: "Pieces".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: Some("PCS".to_owned()),
        tally: None,
    })));
    ok(api.execute(Command::CreateStockItem(widget())));
    // Main Location is Tally's default godown and every company file holds it from creation as
    // godown 1 (seed_godowns; TallyPrime godowns FAQ, accessed 2026-11-21), so this test no
    // longer creates it by hand (updated 2026-11-21 on the owner's "follow Tally" instruction).
    assert_eq!(MAIN, MAIN_LOCATION);
    (dir, api)
}

fn widget() -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UNIT,
        alt_unit: None,
        hsn: None,
        gst_history: vec![GstHistory {
            effective_from: date!(2025 - 09 - 22),
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn ok<T: Debug>(out: Result<Outcome<T>, ApiError>) -> T {
    match out {
        Ok(Outcome::Value(v)) => v,
        other @ (Ok(Outcome::Unsupported(_)) | Err(_)) => panic!("expected a value, got {other:?}"),
    }
}

fn line(ledger: LedgerId, side: DrCr, paise: i64) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: SignedAmount {
            amount: Paise::new(paise),
            side,
        },
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn dr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, DrCr::Dr, paise)
}

fn cr(ledger: LedgerId, paise: i64) -> LedgerLine {
    line(ledger, DrCr::Cr, paise)
}

/// An accounting-invoice voucher dated 5 Oct 2026 (the sale and purchase ledgers carry no GST
/// classification here, so the taxable base of an accounting invoice is the classified ledger; the
/// helper classifies them through [`classify`] before use).
fn invoice(
    base: BaseVoucherKind,
    mode: VoucherMode,
    party: LedgerId,
    pos: Option<u8>,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: vtype(base),
        number: None,
        series: None,
        date: date!(2026 - 10 - 05),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party: Some(party),
        place_of_supply: pos.map(state),
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Item invoice lines: `qty` Widgets, `amount` in paise, posted to `ledger`.
fn items(qty: i64, amount: i64, ledger: LedgerId) -> Vec<InventoryLine> {
    vec![InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::new(qty, 0)),
        unit: UNIT,
        rate: Rate(Decimal::new(amount, 2) / Decimal::new(qty, 0)),
        discount_percent: None,
        amount: Paise::new(amount),
        ledger: Some(ledger),
        godowns: vec![GodownAllocation {
            godown: MAIN,
            batch: None,
            qty: Qty(Decimal::new(qty, 0)),
            amount: Paise::new(amount),
        }],
    }]
}

/// An item invoice sold to `party` for `qty` Widgets worth `amount` paise, no ledger lines.
fn item_sale(party: LedgerId, pos: Option<u8>, qty: i64, amount: i64) -> Voucher {
    Voucher {
        inventory_lines: items(qty, amount, SALES),
        ..invoice(
            BaseVoucherKind::Sales,
            VoucherMode::ItemInvoice,
            party,
            pos,
            Vec::new(),
        )
    }
}

const GOODS: LedgerId = LedgerId(23);

/// A purchase ledger, taxable at 18 per cent, with the ineligible flag as given.
fn purchase_ledger(api: &mut Api, ineligible: Option<bool>) {
    ok(api.execute(Command::CreateLedger(Ledger {
        ineligible_itc: ineligible,
        gst: Some(GstClassification {
            hsn: None,
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(18))),
        }),
        ..plain(GOODS, "Purchases (goods)", ReservedGroup::PurchaseAccounts)
    })));
}

fn purchase_draft() -> Voucher {
    Voucher {
        inventory_lines: items(40, 4_000_000, GOODS),
        ..invoice(
            BaseVoucherKind::Purchase,
            VoucherMode::ItemInvoice,
            SUPPLIER,
            Some(29),
            Vec::new(),
        )
    }
}

/// Derivation: 40 Widgets, 40,000.00 taxable, local supply, 18 per cent: CGST 9% = 3,600.00 and
/// SGST 9% = 3,600.00. The purchase ledger is marked eligible (ineligible = No), so s.16(1)
/// applies and all 7,200.00 is eligible; nothing is not claimable.
#[test]
fn a_purchase_marked_eligible_gets_all_its_tax_as_eligible_credit() {
    let (_dir, mut api) = company_file();
    purchase_ledger(&mut api, Some(false));
    let built = ok(api.invoice_gst(&purchase_draft()));
    let itc = built.eligible_itc.expect("a purchase has an ITC figure");
    let itc = match itc {
        Outcome::Value(i) => i,
        other @ Outcome::Unsupported(_) => panic!("expected a value, got {other:?}"),
    };
    assert_eq!(itc.eligible, Paise::new(720_000));
    assert_eq!(itc.not_claimable, Paise::ZERO);
}

/// Derivation: the same purchase with the ledger marked ineligible (Yes): the whole 7,200.00 is
/// not claimable and the eligible credit is nil.
#[test]
fn a_purchase_marked_ineligible_gets_no_credit() {
    let (_dir, mut api) = company_file();
    purchase_ledger(&mut api, Some(true));
    let built = ok(api.invoice_gst(&purchase_draft()));
    let itc = match built.eligible_itc.expect("a purchase has an ITC figure") {
        Outcome::Value(i) => i,
        other @ Outcome::Unsupported(_) => panic!("expected a value, got {other:?}"),
    };
    assert_eq!(itc.eligible, Paise::ZERO);
    assert_eq!(itc.not_claimable, Paise::new(720_000));
}

/// Derivation: nothing marked and no HSN/SAC on the ledger or item, so s.17(5) cannot be ruled
/// out: the engine will not guess (module docs of `itc_eligibility`): Unsupported.
#[test]
fn an_unmarked_purchase_without_an_hsn_is_unsupported() {
    let (_dir, mut api) = company_file();
    purchase_ledger(&mut api, None);
    let built = ok(api.invoice_gst(&purchase_draft()));
    assert!(matches!(built.eligible_itc, Some(Outcome::Unsupported(_))));
}

/// Derivation: a sale has no input credit: the figure is absent, not zero.
#[test]
fn a_sale_has_no_credit_figure() {
    let (_dir, api) = company_file();
    let built = ok(api.invoice_gst(&item_sale(KA_BUYER, Some(29), 10, 1_000_000)));
    assert!(built.eligible_itc.is_none());
}

/// Derivation: an as-voucher purchase gets the same figure: 10,000.00 at 18 per cent, local,
/// CGST 900.00 + SGST 900.00 = 1,800.00 eligible with the ledger marked eligible.
#[test]
fn an_as_voucher_purchase_gets_the_credit_figure_too() {
    let (_dir, mut api) = company_file();
    purchase_ledger(&mut api, Some(false));
    let draft = invoice(
        BaseVoucherKind::Purchase,
        VoucherMode::AsVoucher,
        SUPPLIER,
        Some(29),
        vec![dr(GOODS, 1_000_000), dr(CGST, 0), dr(SGST, 0)],
    );
    let built = ok(api.invoice_gst(&draft));
    let itc = match built.eligible_itc.expect("a purchase has an ITC figure") {
        Outcome::Value(i) => i,
        other @ Outcome::Unsupported(_) => panic!("expected a value, got {other:?}"),
    };
    assert_eq!(itc.eligible, Paise::new(180_000));
}
