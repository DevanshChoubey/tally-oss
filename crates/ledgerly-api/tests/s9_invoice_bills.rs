//! Round one of the sales/purchase parity work (s9): the invoice rounding ledger and bill-wise
//! allocation on an invoice's party line.
//!
//! Provenance. Tax figures are `derived` by hand in each test from the 18 per cent slab (bundled
//! pack entry `slab.eighteen`, Notification No. 9/2025-Central Tax (Rate) dated 17 Sep 2025) and the
//! rule the engine's own docs record (tax heads summed over the voucher and rounded once to the
//! paisa; the invoice value rounded to the rupee, the difference being the round-off).
//!
//! Bill-wise behaviour models TallyPrime's public help, read 2026-11-25:
//! * <https://help.tallysolutions.com/advances-received/>: an advance is recorded on the receipt
//!   with the reference type Advance; the sales invoice links to it by choosing the reference type
//!   Agst Ref in the invoice's Bill-wise Details.
//! * <https://help.tallysolutions.com/tally-prime/accounting-financial-reports/manage-outstanding-payables-tally/>
//!   and <https://help.tallysolutions.com/tally/manage-receivables-outstanding-tally/>: with bill-wise
//!   on in the party master, TallyPrime tracks the bill with the voucher number as the bill reference;
//!   the credit period comes from the party master (as seen in search results quoting these pages;
//!   the pages could not be re-fetched verbatim, so this test pins the behaviour the brief states:
//!   New Ref by default, named by the voucher number).
//! * <https://help.tallysolutions.com/set-up-receivables-and-payables-tracking-in-tallyprime/>:
//!   credit periods apply only when bill-wise tracking is on.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ledgerly_api::{Api, ApiError, Command, PartyBill};
use ledgerly_core::{
    Clock, CompanyId, Decimal, DrCr, GodownId, GroupId, Gstin, LedgerId, Outcome, Paise, Qty, Rate,
    SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company, EntityKind,
    Features, GodownAllocation, GstHistory, GstRegistration, InventoryLine, Ledger, LedgerLine,
    NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup, ReservedLedger, StockItem,
    TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus,
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

const ROUND: LedgerId = LedgerId(40);
const BILLED: LedgerId = LedgerId(41);
const ADVANCE_BILL: &str = "ADV-1";

/// A company file as in the GST tests, plus an invoice-rounding ledger and a bill-wise Karnataka
/// buyer with a 30 day credit period.
fn billed_file(with_rounding: bool) -> (tempfile::TempDir, Api) {
    let (dir, mut api) = company_file();
    if with_rounding {
        let mut round = plain(ROUND, "Round off", ReservedGroup::IndirectExpenses);
        round.reserved = Some(ReservedLedger::InvoiceRounding);
        ok(api.execute(Command::CreateLedger(round)));
    }
    let mut billed = party(
        BILLED,
        "Billed Buyer",
        ReservedGroup::SundryDebtors,
        "29FGHIJ5678K1Z",
        29,
    );
    billed.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    ok(api.execute(Command::CreateLedger(billed)));
    (dir, api)
}

fn unsupported_what<T: Debug>(out: Result<Outcome<T>, ApiError>) -> (String, String) {
    match out {
        Ok(Outcome::Unsupported(u)) => (u.what, u.reason),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

/// Derivation: Rs 100.01 taxable; CGST = SGST = 100.01 x 9% = 9.0009, summed and rounded to the
/// paisa = 9.00 each; value 100.01 + 9.00 + 9.00 = 118.01, rounded to the rupee 118.00, so the
/// round-off is -0.01. The party owes 118.00 (Dr 11,800 paise); sales Cr 10,001, CGST Cr 900,
/// SGST Cr 900 make 11,801 of credits, so the rounding ledger takes Dr 1 paisa to balance.
#[test]
fn a_round_off_goes_to_the_invoice_rounding_ledger() {
    let (_dir, api) = billed_file(true);
    let built = ok(api.invoice_gst(&item_sale(KA_BUYER, Some(29), 1, 10_001)));
    assert_eq!(built.invoice_total, Paise::new(11_800));
    let want = vec![
        dr(KA_BUYER, 11_800),
        cr(SALES, 10_001),
        cr(CGST, 900),
        cr(SGST, 900),
        dr(ROUND, 1),
    ];
    assert_eq!(built.ledger_lines, want);
    assert_eq!(built.stored.round_off, Paise::new(-1));
}

/// Derivation: Rs 100.00 taxable at 18% gives CGST 9.00 + SGST 9.00, value 118.00, a whole rupee:
/// no round-off, so no rounding line even though the ledger exists.
#[test]
fn a_whole_rupee_invoice_has_no_rounding_line() {
    let (_dir, api) = billed_file(true);
    let built = ok(api.invoice_gst(&item_sale(KA_BUYER, Some(29), 1, 10_000)));
    assert_eq!(built.ledger_lines.len(), 4);
    assert!(built.ledger_lines.iter().all(|l| l.ledger != ROUND));
}

/// Without a designated rounding ledger a round-off is still `Unsupported` (nothing to post to).
#[test]
fn a_round_off_without_a_rounding_ledger_is_still_unsupported() {
    let (_dir, api) = billed_file(false);
    let (what, _) = unsupported_what(api.invoice_gst(&item_sale(KA_BUYER, Some(29), 1, 10_001)));
    assert!(what.contains("round-off"), "{what}");
}

/// A posted invoice with a round-off is accepted by the engine and its books balance.
#[test]
fn an_invoice_with_a_round_off_posts() {
    let (_dir, mut api) = billed_file(true);
    let mut v = item_sale(KA_BUYER, Some(29), 1, 10_001);
    let built = ok(api.invoice_gst(&v));
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    assert_eq!(posted.gst.unwrap().round_off, Paise::new(-1));
}

/// Derivation: 10 Widgets, Rs 10,000 taxable, intra-state 18%: value Rs 11,800.00 (1,180,000
/// paise), a whole rupee. A bill-wise party with no references typed gets one New Ref for the
/// whole value, with the party master's 30 credit days. Its name is left empty here: the voucher
/// has no number yet, and the engine names it by the number when it posts.
#[test]
fn a_bill_wise_party_gets_one_new_ref_for_the_whole_value() {
    let (_dir, api) = billed_file(true);
    let built = ok(api.invoice_gst(&item_sale(BILLED, Some(29), 10, 1_000_000)));
    let party_line = &built.ledger_lines[0];
    assert_eq!(party_line.ledger, BILLED);
    assert_eq!(
        party_line.bills,
        vec![BillAllocation {
            kind: BillRefKind::New,
            name: String::new(),
            due_date: None,
            credit_days: Some(30),
            amount: SignedAmount::dr(Paise::new(1_180_000)),
        }]
    );
}

/// The default bill is named by the voucher number once the engine has numbered the voucher
/// (TallyPrime help, see the module note), and the outstanding bill carries the voucher's amount.
#[test]
fn posting_names_the_default_bill_by_the_voucher_number() {
    let (_dir, mut api) = billed_file(true);
    let mut v = item_sale(BILLED, Some(29), 10, 1_000_000);
    let built = ok(api.invoice_gst(&v));
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    let number = posted.number.clone().unwrap();
    let bills = &posted
        .ledger_lines
        .iter()
        .find(|l| l.ledger == BILLED)
        .unwrap()
        .bills;
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].name, number);
    assert_eq!(bills[0].kind, BillRefKind::New);
    assert_eq!(bills[0].amount, SignedAmount::dr(Paise::new(1_180_000)));
}

/// Derivation: value Rs 11,800.00 = Rs 4,000.00 against the advance ADV-1, Rs 2,800.00 on account
/// and the rest, Rs 5,000.00, as a new bill (the entry with no amount takes what is left:
/// 1,180,000 - 400,000 - 280,000 = 500,000 paise).
#[test]
fn typed_references_split_the_value_and_the_last_blank_takes_the_rest() {
    let (_dir, api) = billed_file(true);
    let refs = [
        PartyBill {
            kind: BillRefKind::Against,
            name: ADVANCE_BILL.to_owned(),
            amount: Some(Paise::new(400_000)),
            credit_days: None,
            due_date: None,
        },
        PartyBill {
            kind: BillRefKind::OnAccount,
            name: String::new(),
            amount: Some(Paise::new(280_000)),
            credit_days: None,
            due_date: None,
        },
        PartyBill {
            kind: BillRefKind::New,
            name: String::new(),
            amount: None,
            credit_days: None,
            due_date: None,
        },
    ];
    let built = ok(api.invoice_gst_with_bills(&item_sale(BILLED, Some(29), 10, 1_000_000), &refs));
    let got: Vec<(BillRefKind, i64)> = built.ledger_lines[0]
        .bills
        .iter()
        .map(|b| (b.kind, b.amount.amount.get()))
        .collect();
    assert_eq!(
        got,
        vec![
            (BillRefKind::Against, 400_000),
            (BillRefKind::OnAccount, 280_000),
            (BillRefKind::New, 500_000),
        ]
    );
}

/// References that do not add up to the value are `Unsupported` with both figures, never fitted.
#[test]
fn references_that_do_not_add_up_are_refused() {
    let (_dir, api) = billed_file(true);
    let refs = [PartyBill {
        kind: BillRefKind::Against,
        name: ADVANCE_BILL.to_owned(),
        amount: Some(Paise::new(400_000)),
        credit_days: None,
        due_date: None,
    }];
    let (what, reason) = unsupported_what(
        api.invoice_gst_with_bills(&item_sale(BILLED, Some(29), 10, 1_000_000), &refs),
    );
    assert!(what.contains("bill"), "{what}");
    assert!(
        reason.contains("4,000.00") && reason.contains("11,800.00"),
        "{reason}"
    );
}

/// A party that is not bill-wise takes no references.
#[test]
fn references_on_a_party_that_is_not_bill_wise_are_refused() {
    let (_dir, api) = billed_file(true);
    let refs = [PartyBill {
        kind: BillRefKind::New,
        name: "X".to_owned(),
        amount: None,
        credit_days: None,
        due_date: None,
    }];
    let (what, _) = unsupported_what(
        api.invoice_gst_with_bills(&item_sale(KA_BUYER, Some(29), 10, 1_000_000), &refs),
    );
    assert!(what.contains("bill"), "{what}");
}

/// The next number of an automatic voucher type is what the engine will allocate at posting; a
/// posted voucher moves it on.
#[test]
fn the_next_voucher_number_follows_the_books() {
    let (_dir, mut api) = billed_file(true);
    let ty = vtype(BaseVoucherKind::Sales);
    let first = match api.next_voucher_number(ty, date!(2026 - 10 - 05)).unwrap() {
        Outcome::Value(Some(n)) => n,
        other => panic!("{other:?}"),
    };
    let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    let built = ok(api.invoice_gst(&v));
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    assert_eq!(posted.number.as_deref(), Some(first.as_str()));
    let second = match api.next_voucher_number(ty, date!(2026 - 10 - 05)).unwrap() {
        Outcome::Value(Some(n)) => n,
        other => panic!("{other:?}"),
    };
    assert_ne!(first, second);
}

/// Derivation (TallyPrime: advance on the receipt with reference type Advance, the invoice links
/// to it with Agst Ref, <https://help.tallysolutions.com/advances-received/>): a receipt of
/// Rs 4,000.00 from the buyer opens bill ADV-1 on the credit side; the invoice of Rs 11,800.00 then
/// settles Rs 4,000.00 of it (Agst Ref, debit side, opposite to the advance) and opens the other
/// Rs 7,800.00 as a new bill named by the voucher number. The engine accepts the post.
#[test]
fn an_invoice_settles_an_advance_and_opens_the_rest() {
    let (_dir, mut api) = billed_file(true);
    let cash = LedgerId(42);
    ok(api.execute(Command::CreateLedger(plain(
        cash,
        "Petty cash",
        ReservedGroup::CashInHand,
    ))));
    let mut credit = cr(BILLED, 400_000);
    credit.bills = vec![BillAllocation {
        kind: BillRefKind::Advance,
        name: ADVANCE_BILL.to_owned(),
        due_date: None,
        credit_days: None,
        amount: SignedAmount::cr(Paise::new(400_000)),
    }];
    let mut receipt = invoice(
        BaseVoucherKind::Receipt,
        VoucherMode::AsVoucher,
        BILLED,
        None,
        vec![dr(cash, 400_000), credit],
    );
    receipt.party = None;
    receipt.date = date!(2026 - 10 - 01);
    receipt.id = VoucherId(2);
    ok(api.execute(Command::PostVoucher(receipt)));

    let refs = [
        PartyBill {
            kind: BillRefKind::Against,
            name: ADVANCE_BILL.to_owned(),
            amount: Some(Paise::new(400_000)),
            credit_days: None,
            due_date: None,
        },
        PartyBill {
            kind: BillRefKind::New,
            name: String::new(),
            amount: None,
            credit_days: None,
            due_date: None,
        },
    ];
    let mut v = item_sale(BILLED, Some(29), 10, 1_000_000);
    let built = ok(api.invoice_gst_with_bills(&v, &refs));
    v.ledger_lines = built.ledger_lines;
    v.gst = Some(built.stored);
    let posted = ok(api.execute(Command::PostVoucher(v))).posted.unwrap();
    let bills = &posted.ledger_lines[0].bills;
    assert_eq!(bills.len(), 2);
    assert_eq!(bills[0].kind, BillRefKind::Against);
    assert_eq!(bills[1].name, posted.number.clone().unwrap());
    assert_eq!(bills[1].amount, SignedAmount::dr(Paise::new(780_000)));
}

/// Derivation: one sale of 10 Widgets (Rs 10,000 taxable, 18% intra-state) leaves the buyer owing
/// Rs 11,800.00 (1,180,000 paise) Dr. A second identical draft would take it to 2 x 11,800.00 =
/// Rs 23,600.00 (2,360,000 paise) Dr. Both figures are the engine's, not an addition in the screen.
#[test]
fn the_balance_after_a_draft_is_the_trial_balances() {
    let (_dir, mut api) = billed_file(true);
    let post = |api: &mut Api, id: u64| {
        let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
        v.id = VoucherId(id);
        let built = ok(api.invoice_gst(&v));
        v.ledger_lines = built.ledger_lines;
        v.gst = Some(built.stored);
        v
    };
    let first = post(&mut api, 5);
    ok(api.execute(Command::PostVoucher(first)));
    let second = post(&mut api, 6);
    let change = ok(api.balance_change(KA_BUYER, &second));
    assert_eq!(change.now, SignedAmount::dr(Paise::new(1_180_000)));
    assert_eq!(change.after, SignedAmount::dr(Paise::new(2_360_000)));
}

/// A ledger with no balance in the trial balance gives `Unsupported`, never an assumed zero.
#[test]
fn a_ledger_without_a_balance_row_gives_unsupported() {
    let (_dir, api) = billed_file(true);
    let mut v = item_sale(KA_BUYER, Some(29), 10, 1_000_000);
    let built = ok(api.invoice_gst(&v));
    v.ledger_lines = built.ledger_lines;
    let (what, _) = unsupported_what(api.balance_change(KA_BUYER, &v));
    assert!(what.contains("balance"), "{what}");
}
