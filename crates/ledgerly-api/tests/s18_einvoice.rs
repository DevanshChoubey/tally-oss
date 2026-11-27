//! S18: a sales item invoice as the IRP e-invoice JSON (INV-01).
//!
//! Provenance: derived by hand. 10 Widgets for Rs 10,000.00 taxable at 18% (bundled slab entry
//! `slab.eighteen`, Notification No. 9/2025-Central Tax (Rate), as in `c2_voucher_gst.rs`). Intra-state
//! (Karnataka to Karnataka, 29): CGST 9% = 900.00 and SGST 9% = 900.00, invoice value 11,800.00,
//! round-off nil. Inter-state (Karnataka 29 to Maharashtra 27): IGST 18% = 1,800.00. Field names,
//! key order and the date shape DD/MM/YYYY are the NIC schema page's
//! (<https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26): `DocDtls.Dt`
//! is "05/10/2026" for 5 Oct 2026, `SlNo` is the string "1", `Qty` 10, `UnitPrice` 1000.000
//! (Rs 10,000.00 over 10), `TotAmt` = `AssAmt` 10000.00 with no discount, `TotItemVal` 11800.00.
//! The address rule (first line `Addr1`, last line `Loc`, lines between `Addr2`) is a product choice:
//! the schema has no city field in the model, so the choice is written here.

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
    Clock, CompanyId, Decimal, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation, GstHistory,
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
        group: Some(group(kind)),
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
            address: Address {
                lines: vec!["7 Buyer Lane".to_owned(), "Bengaluru".to_owned()],
                pincode: Some("560001".to_owned()),
                ..Address::default()
            },
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
        address: Address {
            lines: vec![
                "12 Test Road".to_owned(),
                "Industrial Area".to_owned(),
                "Mysuru".to_owned(),
            ],
            pincode: Some("570001".to_owned()),
            ..Address::default()
        },
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
        hsn: Some(ledgerly_core::Hsn::parse("8501").unwrap()),
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

/// Saves `draft` with its engine-built GST block and returns the stored voucher's id and number.
fn save(api: &mut Api, draft: Voucher) -> VoucherId {
    let built = ok(api.invoice_gst(&draft));
    let mut v = draft;
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    v.number = Some("SI/0001".to_owned());
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    posted.id
}

fn json_of(api: &Api, id: VoucherId) -> serde_json::Value {
    let json = ok(api.einvoice_json(id));
    serde_json::from_str(&json).unwrap()
}

#[test]
fn an_intra_state_sale_maps_to_cgst_and_sgst_items() {
    let (_dir, mut api) = company_file();
    let id = save(&mut api, item_sale(KA_BUYER, Some(29), 10, 1_000_000));
    let v = json_of(&api, id);
    assert_eq!(v["Version"], "1.1");
    assert_eq!(v["TranDtls"]["SupTyp"], "B2B");
    assert_eq!(v["DocDtls"]["Typ"], "INV");
    assert_eq!(v["DocDtls"]["No"], "SI/0001");
    assert_eq!(v["DocDtls"]["Dt"], "05/10/2026");
    // Seller: the company (29), address lines first / middle / last.
    assert_eq!(v["SellerDtls"]["Addr1"], "12 Test Road");
    assert_eq!(v["SellerDtls"]["Addr2"], "Industrial Area");
    assert_eq!(v["SellerDtls"]["Loc"], "Mysuru");
    assert_eq!(v["SellerDtls"]["Pin"], 570_001);
    assert_eq!(v["SellerDtls"]["Stcd"], "29");
    assert_eq!(
        v["BuyerDtls"]["Gstin"],
        "29FGHIJ5678K1Z".to_owned() + &check("29FGHIJ5678K1Z")
    );
    assert_eq!(v["BuyerDtls"]["Pos"], "29");
    assert_eq!(v["BuyerDtls"]["Loc"], "Bengaluru");
}

#[test]
fn an_intra_state_sale_lists_its_item_and_totals() {
    let (_dir, mut api) = company_file();
    let id = save(&mut api, item_sale(KA_BUYER, Some(29), 10, 1_000_000));
    let v = json_of(&api, id);
    let item = &v["ItemList"][0];
    assert_eq!(item["SlNo"], "1");
    assert_eq!(item["HsnCd"], "8501");
    assert_eq!(item["Unit"], "PCS");
    assert_eq!(item["IsServc"], "N");
    assert_eq!(item["GstRt"], 18.0);
    assert_eq!(item["AssAmt"], 10_000.0);
    assert_eq!(item["CgstAmt"], 900.0);
    assert_eq!(item["SgstAmt"], 900.0);
    assert_eq!(item["IgstAmt"], 0.0);
    assert_eq!(item["TotItemVal"], 11_800.0);
    assert_eq!(v["ValDtls"]["AssVal"], 10_000.0);
    assert_eq!(v["ValDtls"]["TotInvVal"], 11_800.0);
}

#[test]
fn an_inter_state_sale_maps_to_igst() {
    let (_dir, mut api) = company_file();
    let id = save(&mut api, item_sale(MH_BUYER, Some(27), 10, 1_000_000));
    let v = json_of(&api, id);
    assert_eq!(v["BuyerDtls"]["Pos"], "27");
    let item = &v["ItemList"][0];
    assert_eq!(item["IgstAmt"], 1_800.0);
    assert_eq!(item["CgstAmt"], 0.0);
    assert_eq!(item["TotItemVal"], 11_800.0);
    assert_eq!(v["ValDtls"]["IgstVal"], 1_800.0);
}

#[test]
fn a_draft_is_checked_with_a_stand_in_number_and_an_unregistered_buyer_says_b2c() {
    let (_dir, mut api) = company_file();
    let _id = save(&mut api, item_sale(KA_BUYER, Some(29), 10, 1_000_000));
    assert!(matches!(
        api.einvoice_check(&item_sale(KA_BUYER, Some(29), 10, 1_000_000))
            .unwrap(),
        Outcome::Value(())
    ));
    let mut cash = plain(LedgerId(40), "Cash sale", ReservedGroup::SundryDebtors);
    cash.party = Some(PartyDetails {
        registration: RegistrationType::Consumer,
        gstin: None,
        pan: None,
        state: Some(state(29)),
        address: Address::default(),
    });
    ok(api.execute(Command::CreateLedger(cash)));
    let Ok(Outcome::Unsupported(u)) =
        api.einvoice_check(&item_sale(LedgerId(40), Some(29), 10, 1_000_000))
    else {
        panic!("a consumer sale is not an e-invoice");
    };
    assert!(u.reason.contains("B2C"), "{}", u.reason);
}

#[test]
fn a_buyer_without_a_pin_is_unsupported_and_says_so() {
    let (_dir, mut api) = company_file();
    let mut mh = party(
        LedgerId(41),
        "No Pin Buyer",
        ReservedGroup::SundryDebtors,
        BUYER_GSTIN,
        27,
    );
    mh.party.as_mut().unwrap().address = Address {
        lines: vec!["1 Road".to_owned(), "Pune".to_owned()],
        ..Address::default()
    };
    ok(api.execute(Command::CreateLedger(mh)));
    let Ok(Outcome::Unsupported(u)) =
        api.einvoice_check(&item_sale(LedgerId(41), Some(27), 10, 1_000_000))
    else {
        panic!("a buyer with no PIN cannot be written");
    };
    assert_eq!(
        u.reason,
        "the buyer No Pin Buyer has no PIN code (Pin is required)"
    );
}

fn check(first14: &str) -> String {
    gstin_check_character(first14).unwrap().to_string()
}
