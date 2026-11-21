//! Card C2 tests: GST at posting and per invoice (spec 6.3, invariant 9).
//!
//! Provenance: every expected figure is `derived` by hand in the test from the amounts typed and
//! the 18 per cent slab (bundled pack entry `slab.eighteen`, Notification No. 9/2025-Central Tax
//! (Rate) dated 17 Sep 2025, read in the pack): CGST 9 and SGST 9 for an intra-state supply, IGST
//! 18 for an inter-state one (IGST Act s.7 and s.8 for intra and inter). Tax heads are summed over
//! the voucher and rounded once to the paisa, and the invoice value is rounded to the rupee, with
//! the difference as the round-off (spec 6.3). Which side tax is posted on follows the voucher
//! kind (sales credit tax, purchases debit it).
//!
//! Decided here, not by Tally: the model has no invoice-rounding ledger type (TallyPrime sets one
//! with the ledger type Invoice Rounding,
//! <https://help.tallysolutions.com/tally-prime/gst-master-setup/india-gst-expense-income-non-revenue-invoice-round-off-ledgers-tally/>,
//! read 2026-11-21), so an invoice with a non-zero round-off is `Unsupported` at the API.
//! GSTR-3B: the s.17(5) entries are `pending` in `packs/gstr-rules.toml` (CGST Act s.17(5) names no
//! HSN code, read at <https://cbic-gst.gov.in/pdf/CGST-Act-Updated-31082021.pdf> on 2026-11-21).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise,
    Period, Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
    gstin_check_character,
};
use ledgerly_engine::EngineError;
use ledgerly_gst::pack::load_pack;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Godown, GodownAllocation, GstHistory,
    GstRegistration, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy, PartyDetails,
    RegistrationType, ReservedGroup, StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
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

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
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
        party(KA_BUYER, "Ka Buyer", ReservedGroup::SundryDebtors, "29FGHIJ5678K1Z", 29),
        party(MH_BUYER, "Mh Buyer", ReservedGroup::SundryDebtors, BUYER_GSTIN, 27),
        party(SUPPLIER, "Supplier", ReservedGroup::SundryCreditors, SUPPLIER_GSTIN, 29),
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
    ok(api.execute(Command::CreateGodown(Godown {
        id: MAIN,
        name: "Main Location".to_owned(),
        parent: None,
        tally: None,
    })));
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
        other => panic!("expected a value, got {other:?}"),
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

fn engine_refusal(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) -> String {
    match out {
        Err(ApiError::Engine(EngineError::Invalid { field, reason })) => {
            assert_eq!(field, "GST");
            reason
        }
        other => panic!("expected a GST refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// voucher_gst
// ---------------------------------------------------------------------------------------------

/// Derivation: 10 Widgets, Rs 10,000 taxable (1,000,000 paise), sold from Karnataka (29) to a
/// Karnataka buyer, place of supply 29: intra-state, so CGST 9% = 900.00 and SGST 9% = 900.00,
/// IGST nil. Value 11,800.00, a whole rupee, so the round-off is nil.
#[test]
fn voucher_gst_intra_state_item_invoice_is_cgst_plus_sgst() {
    let (_dir, api) = company_file();
    let b = ok(api.voucher_gst(&item_sale(KA_BUYER, Some(29), 10, 1_000_000)));
    assert_eq!(b.taxable_total, Paise::new(1_000_000));
    assert_eq!(b.totals.cgst, Paise::new(90_000));
    assert_eq!(b.totals.sgst, Paise::new(90_000));
    assert_eq!(b.totals.igst, Paise::ZERO);
    assert_eq!(b.round_off, Paise::ZERO);
    assert_eq!(b.total_before_round_off, Paise::new(1_180_000));
}

/// Derivation: same invoice to a Maharashtra buyer, place of supply 27: inter-state, so IGST 18% =
/// 1,800.00 and no CGST or SGST.
#[test]
fn voucher_gst_inter_state_item_invoice_is_igst() {
    let (_dir, api) = company_file();
    let b = ok(api.voucher_gst(&item_sale(MH_BUYER, Some(27), 10, 1_000_000)));
    assert_eq!(b.totals.igst, Paise::new(180_000));
    assert_eq!(b.totals.cgst, Paise::ZERO);
    assert_eq!(b.totals.sgst, Paise::ZERO);
}

/// Derivation: the place of supply is never guessed, so a draft without one is `Unsupported`.
#[test]
fn voucher_gst_without_place_of_supply_is_unsupported() {
    let (_dir, api) = company_file();
    let out = api
        .voucher_gst(&item_sale(KA_BUYER, None, 10, 1_000_000))
        .unwrap();
    match out {
        Outcome::Unsupported(u) => assert!(u.what.contains("place of supply"), "{u:?}"),
        other @ Outcome::Value(_) => panic!("expected Unsupported, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// invoice_gst and the post
// ---------------------------------------------------------------------------------------------

/// Derivation: intra-state, Rs 10,000 taxable: party Dr 11,800.00, Sales Cr 10,000.00, CGST Cr
/// 900.00, SGST Cr 900.00 (Dr 11,800 = Cr 11,800). The stored block has a CGST line at 9% and an
/// SGST line at 9%, each on taxable 10,000.00 with tax 900.00, and no round-off. Posting the
/// invoice with those lines succeeds and stores the block on the voucher.
#[test]
fn intra_state_item_invoice_posts_with_its_gst_block() {
    let (_dir, mut api) = company_file();
    let draft = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(built.invoice_total, Paise::new(1_180_000));
    let want = vec![
        dr(KA_BUYER, 1_180_000),
        cr(SALES, 1_000_000),
        cr(CGST, 90_000),
        cr(SGST, 90_000),
    ];
    assert_eq!(built.ledger_lines, want);
    assert_eq!(built.stored.lines.len(), 2);
    assert_eq!(built.stored.round_off, Paise::ZERO);

    let mut v = draft;
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    let result = ok(api.execute(Command::PostVoucher(v)));
    let posted = result.posted.unwrap();
    let stored = posted.gst.unwrap();
    assert_eq!(stored.pack_version, "2026.1-slabs");
    let by_ledger: Vec<_> = stored
        .lines
        .iter()
        .map(|l| (l.ledger, l.kind, l.taxable, l.rate_percent, l.tax))
        .collect();
    let nine = Rate(Decimal::from(9));
    assert_eq!(
        by_ledger,
        vec![
            (CGST, TaxLedgerKind::Cgst, Paise::new(1_000_000), nine, Paise::new(90_000)),
            (SGST, TaxLedgerKind::Sgst, Paise::new(1_000_000), nine, Paise::new(90_000)),
        ]
    );
}

/// Derivation: inter-state, Rs 10,000 taxable: party Dr 11,800.00, Sales Cr 10,000.00, IGST Cr
/// 1,800.00 at 18%. The post succeeds and stores one IGST line.
#[test]
fn inter_state_item_invoice_posts_with_igst() {
    let (_dir, mut api) = company_file();
    let draft = item_sale(MH_BUYER, Some(27), 10, 1_000_000);
    let built = ok(api.invoice_gst(&draft));
    assert_eq!(
        built.ledger_lines,
        vec![
            dr(MH_BUYER, 1_180_000),
            cr(SALES, 1_000_000),
            cr(IGST, 180_000)
        ]
    );
    let mut v = draft;
    v.ledger_lines = built.ledger_lines;
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    let stored = posted.gst.unwrap();
    assert_eq!(stored.lines.len(), 1);
    assert_eq!(stored.lines[0].kind, TaxLedgerKind::Igst);
    assert_eq!(stored.lines[0].rate_percent, Rate(Decimal::from(18)));
    assert_eq!(stored.lines[0].tax, Paise::new(180_000));
}

/// Derivation: Rs 100.01 taxable: CGST = SGST = 100.01 x 9% = 9.0009, rounded to 9.00 each; value
/// 100.01 + 9.00 + 9.00 = 118.01, which rounds to 118.00, so the round-off is -0.01. The model has
/// no invoice-rounding ledger, so there is nothing to post it to and the answer is `Unsupported`.
#[test]
fn an_invoice_with_a_round_off_is_unsupported_not_guessed() {
    let (_dir, api) = company_file();
    let out = api
        .invoice_gst(&item_sale(KA_BUYER, Some(29), 1, 10_001))
        .unwrap();
    match out {
        Outcome::Unsupported(u) => assert!(u.what.contains("round-off"), "{u:?}"),
        other @ Outcome::Value(_) => panic!("expected Unsupported, got {other:?}"),
    }
}

/// Derivation: a draft that already has ledger lines is not an item invoice to be built.
#[test]
fn an_invoice_that_already_has_lines_is_unsupported() {
    let (_dir, api) = company_file();
    let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    v.ledger_lines = vec![dr(KA_BUYER, 1)];
    assert!(matches!(
        api.invoice_gst(&v).unwrap(),
        Outcome::Unsupported(_)
    ));
}

// ---------------------------------------------------------------------------------------------
// Hand-typed tax
// ---------------------------------------------------------------------------------------------

/// Derivation: the engine computes CGST 900.00 on Rs 10,000 at 18% intra-state. Typed CGST of
/// 800.00 disagrees (the voucher balances: Dr buyer 11,700.00 = Cr Sales 10,000.00 + CGST 800.00 +
/// SGST 900.00), so the post is refused, naming CGST and both figures.
#[test]
fn a_typed_tax_line_that_disagrees_with_the_engine_is_refused() {
    let (_dir, mut api) = company_file();
    let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    v.ledger_lines = vec![
        dr(KA_BUYER, 1_170_000),
        cr(SALES, 1_000_000),
        cr(CGST, 80_000),
        cr(SGST, 90_000),
    ];
    let reason = engine_refusal(api.execute(Command::PostVoucher(v)));
    assert!(reason.contains("CGST"), "{reason}");
    assert!(reason.contains("800.00"), "{reason}");
    assert!(reason.contains("900.00"), "{reason}");
}

/// Derivation: an inter-state supply (place 27) is IGST only. Typing CGST and SGST of 900.00 each
/// (a balanced voucher) disagrees: the engine computes CGST 0.00. Refused.
#[test]
fn cgst_and_sgst_on_an_inter_state_sale_are_refused() {
    let (_dir, mut api) = company_file();
    let mut v = item_sale(MH_BUYER, Some(27), 10, 1_000_000);
    v.ledger_lines = vec![
        dr(MH_BUYER, 1_180_000),
        cr(SALES, 1_000_000),
        cr(CGST, 90_000),
        cr(SGST, 90_000),
    ];
    let reason = engine_refusal(api.execute(Command::PostVoucher(v)));
    assert!(reason.contains("CGST"), "{reason}");
    assert!(reason.contains("0.00"), "{reason}");
}

/// Derivation: a sale posts tax on the credit side. CGST typed as a debit is refused as the wrong
/// side. The lines balance: Dr buyer 11,800.00 + Dr CGST 900.00 = 12,700.00 and Cr Sales 11,800.00
/// + Cr SGST 900.00 = 12,700.00.
#[test]
fn tax_on_the_wrong_side_is_refused() {
    let (_dir, mut api) = company_file();
    let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    v.ledger_lines = vec![
        dr(KA_BUYER, 1_180_000),
        dr(CGST, 90_000),
        cr(SALES, 1_180_000),
        cr(SGST, 90_000),
    ];
    let reason = engine_refusal(api.execute(Command::PostVoucher(v)));
    assert!(reason.contains("wrong side"), "{reason}");
}

/// Derivation: a purchase debits input tax. Purchase of Rs 40,000 from a Karnataka supplier
/// (place 29), 18%: CGST 3,600 + SGST 3,600, bill 47,200. Typed correctly it posts and stores the
/// block with the Dr-side lines; the supplier line is a credit of 47,200.
#[test]
fn a_correct_purchase_posts_and_stores_the_block() {
    let (_dir, mut api) = company_file();
    let mut v = Voucher {
        inventory_lines: items(40, 4_000_000, PURCHASE),
        ..invoice(
            BaseVoucherKind::Purchase,
            VoucherMode::ItemInvoice,
            SUPPLIER,
            Some(29),
            Vec::new(),
        )
    };
    v.ledger_lines = vec![
        dr(PURCHASE, 4_000_000),
        dr(CGST, 360_000),
        dr(SGST, 360_000),
        cr(SUPPLIER, 4_720_000),
    ];
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    let stored = posted.gst.unwrap();
    assert_eq!(stored.lines.len(), 2);
    assert_eq!(stored.lines[0].tax, Paise::new(360_000));
}

// ---------------------------------------------------------------------------------------------
// GSTR-3B with a purchase
// ---------------------------------------------------------------------------------------------

/// Derivation: the packs now carry the three `itc.blocked.*` entries, all `pending` (CGST Act
/// s.17(5) names no HSN code). GSTR-3B on a period with a purchase therefore says which entry is
/// still pending, instead of showing a credit that s.17(5) may block.
#[test]
fn gstr3b_with_a_purchase_names_the_pending_itc_entry() {
    let (_dir, mut api) = company_file();
    let v = Voucher {
        inventory_lines: items(40, 4_000_000, PURCHASE),
        ledger_lines: vec![
            dr(PURCHASE, 4_000_000),
            dr(CGST, 360_000),
            dr(SGST, 360_000),
            cr(SUPPLIER, 4_720_000),
        ],
        ..invoice(
            BaseVoucherKind::Purchase,
            VoucherMode::ItemInvoice,
            SUPPLIER,
            Some(29),
            Vec::new(),
        )
    };
    ok(api.execute(Command::PostVoucher(v)));
    let slabs = load_pack(include_str!("../../ledgerly-gst/packs/rates-slabs.toml")).unwrap();
    let rules = load_pack(include_str!("../../ledgerly-gst/packs/gstr-rules.toml")).unwrap();
    let pack = slabs.merge(&rules).unwrap();
    let period = Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap();
    let out = api
        .gstr3b_with_pack(&gstin_of(COMPANY), period, &pack)
        .unwrap();
    match out {
        Outcome::Unsupported(u) => {
            assert!(u.reason.contains("itc.blocked.motor_vehicles"), "{u:?}");
            assert!(u.reason.contains("pending"), "{u:?}");
        }
        other @ Outcome::Value(_) => panic!("expected Unsupported, got {other:?}"),
    }
}
