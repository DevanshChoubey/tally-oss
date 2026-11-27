//! Card C8 tests: the headless end-to-end path through the facade alone (spec sections 4, 5, 6.2,
//! 6.3, 6.5 and 6.6).
//!
//! The path is: create a company file, create every master with id 0 so the API assigns the id,
//! post twenty vouchers (checking each GST invoice with [`Api::voucher_gst`] first), run the trial
//! balance, balance sheet and profit and loss, build GSTR-1, import a bank statement, reopen the
//! file, check that the statement is still held and the bank reconciliation is unchanged, run the
//! rebuild check, back the file up and verify the audit chain of the file and of the copy.
//!
//! Provenance. Every expected figure is `derived` by hand from the posting rules (a sale debits
//! the customer and credits sales and the output tax) and from the arithmetic written next to it.
//! None is copied from the engine. The voucher set and the report arithmetic are those of the W8
//! test (`w8_headless_e2e.rs`), re-derived here; the bank statement and its reconciliation are new
//! and derived in the comments on [`STATEMENT`] and [`check_brs`].
//!
//! Test inputs that are not law (recorded so that they are not mistaken for verified facts):
//! * The GSTR-1 pack entries `gstr1.b2cl_threshold` and `gstr1.hsn_credit_notes` are fixture
//!   parameters appended to the real slab pack, because the bundled pack does not carry them
//!   before card C2. Neither value is verified. Every voucher here is far below any threshold, so
//!   the threshold does not change a figure. The credit-note treatment `net` does change the HSN
//!   row, which is only as good as the fixture. Pending until a person checks the rule pack.
//! * The credit note brings the returned item back into stock at its own invoice value, because
//!   the engine takes the inward value from the line amount. The average-cost closing value
//!   (Rs 6,187.50) depends on that input and on the engine's average-cost formula. Tally's page
//!   for a sales return does not say how returned stock is valued. Pending.
//!
//! Pending, because the frozen contract (`docs/.../wave3/contracts.md`) names no fields:
//! * `Api::voucher_gst` returns a `GstBreakup`. This test checks only that each invoice gets a
//!   value rather than `Unsupported`. It does not compare the breakup's figures with the tax lines
//!   of the vouchers, because the type's fields are not frozen.
//! * Whether `PostVoucher` now also checks or fills GST on post (card C2) is not asserted. The
//!   vouchers carry hand-derived tax lines, as in W8.
//!
//! Law and Tally references: the Balance Sheet shows each group as one amount (Tally Balance Sheet
//! page, <https://help.tallysolutions.com/balance-sheet-tally/>, read 2026-11-21); the bank
//! reconciliation rule (balance per bank = balance per books - uncleared deposits + uncleared
//! payments) is the K3 rule from <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! (accessed 2026-11-21); GSTR-1 tables 4A, 7, 9B and 12 are the CBIC GSTR-1 table structure at
//! <https://tutorial.gst.gov.in/userguide/returns/index.htm> (accessed 2026-11-21).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ledgerly_api::{
    Api, ApiError, Brs, ChainProblem, Command, CommandResult, CreatedId, VerifyReport, verify_file,
};
use ledgerly_api::{Bucket, BucketedVoucher, Gstr1Return};
use ledgerly_bank::statement::StatementError;
use ledgerly_core::{
    BankStatementId, Clock, CompanyId, Date, Decimal, GodownId, GroupId, Gstin, Hsn, LedgerId,
    Outcome, Paise, Period, Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId,
    VoucherTypeId,
};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_gst_formats::gstr1::{B2bInvoice, B2bInvoiceType, Gstr1, ItemTax};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation,
    GstHistory, GstRegistration, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy,
    PartyDetails, RegistrationType, ReservedGroup, Scenario, StockItem, TaxDetails, TaxLedgerKind,
    Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use time::OffsetDateTime;
use time::macros::date;

/// Fictional GSTIN from the core test vectors (Karnataka): the company's own registration.
const OWN_GSTIN: &str = "29ABCDE1234F1ZW";
/// Fictional GSTIN from the formats test vectors (Gujarat): a registered buyer.
const BUYER_GSTIN: &str = "24AAACB1111C1ZT";
const KARNATAKA: u8 = 29;
const GUJARAT: u8 = 24;
/// The format registry id of the MT940 parser.
const MT940: &str = "generic-mt940";

/// A fictional MT940 statement for the bank ledger, covering July 2026. Its lines, in order:
///
/// - 2026-07-14 credit 3000,00 (the receipt, voucher 14)
/// - 2026-07-18 credit 1000,00 (the contra, voucher 16)
/// - 2026-07-25 credit 2000,00 (the receipt, voucher 18)
/// - 2026-07-30 debit 1000,00 (the journal, voucher 19)
/// - 2026-07-31 debit 50,00, "BANK CHARGES", which is in no voucher
///
/// Derived. The opening balance is the books' bank balance on 30 June 2026: 500,000 (voucher 1) -
/// 50,000 (voucher 2) - 5,000 (voucher 6) - 3,000 (voucher 8) = 442,000. Closing balance:
/// 442,000 + 3,000 + 1,000 + 2,000 - 1,000 - 50 = 446,950, which is the stated `:62F:`.
const STATEMENT: &str = "{1:F01TESTINBBAXXX0000000000}{2:O9400000000000TESTINBBXXXX00000000000000000000N}{4:\r\n\
:20:STMT0001\r\n\
:25:0123456789\r\n\
:28C:00001/001\r\n\
:60F:C260701INR442000,00\r\n\
:61:260714C3000,00NTRFNONREF//BK1\r\n\
:61:260718C1000,00NTRFNONREF//BK2\r\n\
:61:260725C2000,00NTRFNONREF//BK3\r\n\
:61:260730D1000,00NTRFNONREF//BK4\r\n\
:61:260731D50,00NTRFNONREF//BANKCHG\r\n\
:86:BANK CHARGES\r\n\
:62F:C260731INR446950,00\r\n\
-}\r\n";

/// The ids the API assigned to the masters.
#[derive(Debug, Clone, Copy)]
struct Ids {
    cash: LedgerId,
    bank: LedgerId,
    capital: LedgerId,
    sales: LedgerId,
    purchase: LedgerId,
    rent: LedgerId,
    electricity: LedgerId,
    interest: LedgerId,
    input_igst: LedgerId,
    output_igst: LedgerId,
    output_cgst: LedgerId,
    output_sgst: LedgerId,
    buyer: LedgerId,
    walk_in: LedgerId,
    ka_customer: LedgerId,
    supplier: LedgerId,
    item: StockItemId,
    unit: UnitId,
    main: GodownId,
}

// ---------------------------------------------------------------------------------------------
// Small builders

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn rate(percent: i64) -> Rate {
    Rate(Decimal::new(percent, 0))
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn gstin(text: &str) -> Gstin {
    Gstin::parse(text).unwrap()
}

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn fy_period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn july() -> Period {
    Period::new(date!(2026 - 07 - 01), date!(2026 - 07 - 31)).unwrap()
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn type_of(kind: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
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
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// The result of a command that committed a value.
fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Masters, all created with id 0 so that the API assigns the id (contract C1)

/// A ledger with id 0 (unassigned), the zero opening balance and no party, tax or bank details.
fn plain(name: &str, reserved: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(0),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group(reserved)),
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

fn tax_ledger(name: &str, kind: TaxLedgerKind, percent: i64) -> Ledger {
    Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: Some(rate(percent)),
        }),
        ..plain(name, ReservedGroup::DutiesAndTaxes)
    }
}

fn party_ledger(
    name: &str,
    reserved: ReservedGroup,
    registration: RegistrationType,
    gstin_text: Option<&str>,
    code: u8,
) -> Ledger {
    Ledger {
        party: Some(PartyDetails {
            registration,
            gstin: gstin_text.map(gstin),
            pan: None,
            state: Some(state(code)),
            address: Address::default(),
        }),
        ..plain(name, reserved)
    }
}

fn bank_ledger() -> Ledger {
    Ledger {
        bank: Some(BankDetails {
            account_number: "0123456789".to_owned(),
            ifsc: "TEST0000001".to_owned(),
            bank_name: "Test Bank".to_owned(),
            branch: "Main".to_owned(),
        }),
        ..plain("Bank", ReservedGroup::BankAccounts)
    }
}

/// Creates a ledger with id 0 and returns the id the API assigned.
fn create_ledger(api: &mut Api, ledger: Ledger) -> LedgerId {
    assert_eq!(ledger.id, LedgerId(0));
    let result = value(api.execute(Command::CreateLedger(ledger)));
    match result.created {
        Some(CreatedId::Ledger(id)) => id,
        other => panic!("expected an assigned ledger id, got {other:?}"),
    }
}

/// Creates the sixteen ledgers, the unit, the stock item and the godown. Ledger names differ from
/// every seeded group name, because report rows are read by label.
fn create_masters(api: &mut Api) -> Ids {
    let cash = create_ledger(api, plain("Cash", ReservedGroup::CashInHand));
    let bank = create_ledger(api, bank_ledger());
    let capital = create_ledger(api, plain("Capital", ReservedGroup::CapitalAccount));
    let sales = create_ledger(api, plain("Sales", ReservedGroup::SalesAccounts));
    let purchase = create_ledger(api, plain("Purchase", ReservedGroup::PurchaseAccounts));
    let rent = create_ledger(api, plain("Rent", ReservedGroup::IndirectExpenses));
    let electricity = create_ledger(api, plain("Electricity", ReservedGroup::IndirectExpenses));
    let interest = create_ledger(
        api,
        plain("Interest Received", ReservedGroup::IndirectIncomes),
    );
    let input_igst = create_ledger(api, tax_ledger("Input IGST", TaxLedgerKind::Igst, 18));
    let output_igst = create_ledger(api, tax_ledger("Output IGST", TaxLedgerKind::Igst, 18));
    let central_out = create_ledger(api, tax_ledger("Output CGST", TaxLedgerKind::Cgst, 9));
    let state_out = create_ledger(api, tax_ledger("Output SGST", TaxLedgerKind::Sgst, 9));
    let buyer = create_ledger(
        api,
        party_ledger(
            "Gujarat Buyer",
            ReservedGroup::SundryDebtors,
            RegistrationType::Regular,
            Some(BUYER_GSTIN),
            GUJARAT,
        ),
    );
    let walk_in = create_ledger(
        api,
        party_ledger(
            "Walk-in Gujarat",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            GUJARAT,
        ),
    );
    let ka_customer = create_ledger(
        api,
        party_ledger(
            "Karnataka Customer",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            KARNATAKA,
        ),
    );
    let supplier = create_ledger(
        api,
        party_ledger(
            "Gujarat Supplier",
            ReservedGroup::SundryCreditors,
            RegistrationType::Unregistered,
            None,
            GUJARAT,
        ),
    );
    let (unit, item, main) = create_stock_masters(api);
    Ids {
        cash,
        bank,
        capital,
        sales,
        purchase,
        rent,
        electricity,
        interest,
        input_igst,
        output_igst,
        output_cgst: central_out,
        output_sgst: state_out,
        buyer,
        walk_in,
        ka_customer,
        supplier,
        item,
        unit,
        main,
    }
}

/// The unit Pieces, the item Widget (HSN 8471, 18 percent from 22 September 2025, the rate in the
/// GST 2.0 schedule; the item's own history is the input) and the godown Main Location.
fn create_stock_masters(api: &mut Api) -> (UnitId, StockItemId, GodownId) {
    let pieces = Unit {
        id: UnitId(0),
        symbol: "pcs".to_owned(),
        formal_name: "Pieces".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: Some("PCS".to_owned()),
        tally: None,
    };
    let unit = match value(api.execute(Command::CreateUnit(pieces))).created {
        Some(CreatedId::Unit(id)) => id,
        other => panic!("expected an assigned unit id, got {other:?}"),
    };
    let widget = StockItem {
        ineligible_itc: None,
        id: StockItemId(0),
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit,
        alt_unit: None,
        hsn: Some(Hsn::parse("8471").unwrap()),
        gst_history: vec![GstHistory {
            effective_from: date!(2025 - 09 - 22),
            taxability: Taxability::Taxable,
            rate_percent: Some(rate(18)),
        }],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    };
    let item = match value(api.execute(Command::CreateStockItem(widget))).created {
        Some(CreatedId::StockItem(id)) => id,
        other => panic!("expected an assigned stock item id, got {other:?}"),
    };
    // Main Location is Tally's default godown and every company file holds it from creation as
    // godown 1 (seed_godowns; TallyPrime godowns FAQ, accessed 2026-11-21), so this test no
    // longer creates it by hand (updated 2026-11-21 on the owner's "follow Tally" instruction).
    let main = MAIN_LOCATION;
    (unit, item, main)
}

// ---------------------------------------------------------------------------------------------
// Vouchers

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

fn dr(ledger: LedgerId, rupees: i64) -> LedgerLine {
    line(ledger, SignedAmount::dr(rs(rupees)))
}

fn cr(ledger: LedgerId, rupees: i64) -> LedgerLine {
    line(ledger, SignedAmount::cr(rs(rupees)))
}

/// One stock line: `qty` units at `rate_rupees` each, valued at `amount_rupees`, all in Main
/// Location. The sales or purchase ledger is the line's `ledger`.
fn item(
    ids: &Ids,
    qty: i64,
    rate_rupees: i64,
    amount_rupees: i64,
    ledger: LedgerId,
) -> InventoryLine {
    let amount = rs(amount_rupees);
    InventoryLine {
        item: ids.item,
        qty: Qty(Decimal::new(qty, 0)),
        unit: ids.unit,
        rate: Rate(Decimal::new(rate_rupees * 100, 2)),
        discount_percent: None,
        amount,
        ledger: Some(ledger),
        godowns: vec![GodownAllocation {
            godown: ids.main,
            batch: None,
            qty: Qty(Decimal::new(qty, 0)),
            amount,
        }],
    }
}

fn plain_voucher(id: u64, kind: BaseVoucherKind, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: type_of(kind),
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
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

/// An item invoice: accounting lines (the books post only these) and the stock lines.
fn invoice(
    id: u64,
    kind: BaseVoucherKind,
    on: Date,
    party: LedgerId,
    pos: u8,
    (lines, items): (Vec<LedgerLine>, Vec<InventoryLine>),
) -> Voucher {
    Voucher {
        party: Some(party),
        place_of_supply: Some(state(pos)),
        mode: VoucherMode::ItemInvoice,
        inventory_lines: items,
        ..plain_voucher(id, kind, on, lines)
    }
}

/// Vouchers 1 to 8: capital, a contra, two purchases (ten and five units at Rs 1,000), and the
/// payments and receipts of April.
fn purchases_and_payments(ids: &Ids) -> Vec<Voucher> {
    vec![
        plain_voucher(
            1,
            BaseVoucherKind::Journal,
            date!(2026 - 04 - 01),
            vec![dr(ids.bank, 500_000), cr(ids.capital, 500_000)],
        ),
        plain_voucher(
            2,
            BaseVoucherKind::Contra,
            date!(2026 - 04 - 02),
            vec![dr(ids.cash, 50_000), cr(ids.bank, 50_000)],
        ),
        invoice(
            3,
            BaseVoucherKind::Purchase,
            date!(2026 - 04 - 05),
            ids.supplier,
            GUJARAT,
            (
                vec![
                    dr(ids.purchase, 10_000),
                    dr(ids.input_igst, 1_800),
                    cr(ids.supplier, 11_800),
                ],
                vec![item(ids, 10, 1_000, 10_000, ids.purchase)],
            ),
        ),
        plain_voucher(
            4,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 10),
            vec![dr(ids.rent, 20_000), cr(ids.cash, 20_000)],
        ),
        plain_voucher(
            5,
            BaseVoucherKind::Receipt,
            date!(2026 - 04 - 12),
            vec![dr(ids.cash, 5_000), cr(ids.interest, 5_000)],
        ),
        plain_voucher(
            6,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 15),
            vec![dr(ids.supplier, 5_000), cr(ids.bank, 5_000)],
        ),
        invoice(
            7,
            BaseVoucherKind::Purchase,
            date!(2026 - 04 - 20),
            ids.supplier,
            GUJARAT,
            (
                vec![
                    dr(ids.purchase, 5_000),
                    dr(ids.input_igst, 900),
                    cr(ids.supplier, 5_900),
                ],
                vec![item(ids, 5, 1_000, 5_000, ids.purchase)],
            ),
        ),
        plain_voucher(
            8,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 22),
            vec![dr(ids.electricity, 3_000), cr(ids.bank, 3_000)],
        ),
    ]
}

/// Vouchers 9 to 13: four sales and one credit note with items, all in July 2026.
fn july_sales(ids: &Ids) -> Vec<Voucher> {
    vec![
        // Taxable 4 x 1,500 = 6,000; IGST 18 percent = 1,080; invoice 7,080.
        invoice(
            9,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 03),
            ids.buyer,
            GUJARAT,
            (
                vec![
                    dr(ids.buyer, 7_080),
                    cr(ids.sales, 6_000),
                    cr(ids.output_igst, 1_080),
                ],
                vec![item(ids, 4, 1_500, 6_000, ids.sales)],
            ),
        ),
        // Taxable 1,500; IGST 270; invoice 1,770 to an unregistered buyer in another state.
        invoice(
            10,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 05),
            ids.walk_in,
            GUJARAT,
            (
                vec![
                    dr(ids.walk_in, 1_770),
                    cr(ids.sales, 1_500),
                    cr(ids.output_igst, 270),
                ],
                vec![item(ids, 1, 1_500, 1_500, ids.sales)],
            ),
        ),
        // Taxable 2 x 1,000 = 2,000; CGST 9 percent = 180; SGST 180; invoice 2,360, intra-state.
        invoice(
            11,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 08),
            ids.ka_customer,
            KARNATAKA,
            (
                vec![
                    dr(ids.ka_customer, 2_360),
                    cr(ids.sales, 2_000),
                    cr(ids.output_cgst, 180),
                    cr(ids.output_sgst, 180),
                ],
                vec![item(ids, 2, 1_000, 2_000, ids.sales)],
            ),
        ),
        // Taxable 3 x 1,500 = 4,500; IGST 810; invoice 5,310.
        invoice(
            12,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 10),
            ids.buyer,
            GUJARAT,
            (
                vec![
                    dr(ids.buyer, 5_310),
                    cr(ids.sales, 4_500),
                    cr(ids.output_igst, 810),
                ],
                vec![item(ids, 3, 1_500, 4_500, ids.sales)],
            ),
        ),
        // One unit returned by the buyer of voucher 9: taxable 1,500; IGST 270; note 1,770. The
        // stock comes back at 1,500 (the line amount is the inward value; see the module note).
        invoice(
            13,
            BaseVoucherKind::CreditNote,
            date!(2026 - 07 - 12),
            ids.buyer,
            GUJARAT,
            (
                vec![
                    dr(ids.sales, 1_500),
                    dr(ids.output_igst, 270),
                    cr(ids.buyer, 1_770),
                ],
                vec![item(ids, 1, 1_500, 1_500, ids.sales)],
            ),
        ),
    ]
}

/// Vouchers 14 to 20: receipts, a contra, a reclassifying journal, a payment and a capital
/// journal. None of them is a GSTR-1 document. Vouchers 14, 16, 18 and 19 touch the bank.
fn july_settlements(ids: &Ids) -> Vec<Voucher> {
    vec![
        plain_voucher(
            14,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 14),
            vec![dr(ids.bank, 3_000), cr(ids.buyer, 3_000)],
        ),
        plain_voucher(
            15,
            BaseVoucherKind::Journal,
            date!(2026 - 07 - 15),
            vec![dr(ids.electricity, 1_000), cr(ids.rent, 1_000)],
        ),
        plain_voucher(
            16,
            BaseVoucherKind::Contra,
            date!(2026 - 07 - 18),
            vec![dr(ids.bank, 1_000), cr(ids.cash, 1_000)],
        ),
        plain_voucher(
            17,
            BaseVoucherKind::Payment,
            date!(2026 - 07 - 22),
            vec![dr(ids.supplier, 4_000), cr(ids.cash, 4_000)],
        ),
        plain_voucher(
            18,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 25),
            vec![dr(ids.bank, 2_000), cr(ids.interest, 2_000)],
        ),
        plain_voucher(
            19,
            BaseVoucherKind::Journal,
            date!(2026 - 07 - 30),
            vec![dr(ids.capital, 1_000), cr(ids.bank, 1_000)],
        ),
        plain_voucher(
            20,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 31),
            vec![dr(ids.cash, 1_770), cr(ids.walk_in, 1_770)],
        ),
    ]
}

/// Vouchers whose GST the engine must refuse to compute: vouchers 3 and 7 are purchases from
/// Gujarat Supplier, an unregistered supplier, who charges no GST (reverse charge applies only in notified cases), so
/// the engine says `Unsupported` and each voucher posts with its typed lines and no GST block.
const GST_NOT_COMPUTED: [u64; 2] = [3, 7];

/// Posts every voucher in order. Each GST invoice (a voucher with stock lines) is first run
/// through [`Api::voucher_gst`], which must give a value, except the vouchers in
/// [`GST_NOT_COMPUTED`], which must be `Unsupported`. Each post must commit.
fn post_all(api: &mut Api, vouchers: Vec<Voucher>) {
    for v in vouchers {
        if !v.inventory_lines.is_empty() {
            let refused = GST_NOT_COMPUTED.contains(&v.id.0);
            match api.voucher_gst(&v) {
                Ok(Outcome::Value(_breakup)) if !refused => {}
                Ok(Outcome::Unsupported(u)) if refused => {
                    assert!(u.what.contains("Unregistered supplier"), "{u:?}");
                }
                Ok(Outcome::Value(_breakup)) => {
                    panic!(
                        "voucher {}: voucher_gst gave a value for an unregistered purchase",
                        v.id.0
                    )
                }
                Ok(Outcome::Unsupported(u)) => {
                    panic!("voucher {}: voucher_gst is Unsupported: {u:?}", v.id.0)
                }
                Err(e) => panic!("voucher {}: voucher_gst failed: {e:?}", v.id.0),
            }
        }
        value(api.execute(Command::PostVoucher(v)));
    }
}

// ---------------------------------------------------------------------------------------------
// Reports

fn params(kind: ReportKind, as_of: Date) -> ReportParams {
    ReportParams {
        kind,
        period: fy_period(),
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn run(api: &Api, kind: ReportKind, as_of: Date) -> Report {
    api.report(&params(kind, as_of)).unwrap().value().unwrap()
}

/// The three reports the test compares, run over the same books.
#[derive(Debug, PartialEq)]
struct Reports {
    trial_balance: Report,
    balance_sheet: Report,
    profit_and_loss: Report,
}

fn reports(api: &Api) -> Reports {
    Reports {
        trial_balance: run(api, ReportKind::TrialBalance, date!(2026 - 12 - 31)),
        balance_sheet: run(api, ReportKind::BalanceSheet, date!(2026 - 12 - 31)),
        profit_and_loss: run(api, ReportKind::ProfitAndLoss, date!(2027 - 03 - 31)),
    }
}

fn row_cells(report: &Report, label: &str) -> Option<Vec<Cell>> {
    report
        .rows
        .iter()
        .find(|row| row.label == label)
        .map(|row| row.cells.clone())
}

/// A trial-balance row with one balance on the given side.
fn assert_tb_row(tb: &Report, label: &str, debit: bool, rupees: i64) {
    let cells = if debit {
        vec![Cell::Amount(SignedAmount::dr(rs(rupees))), Cell::Empty]
    } else {
        vec![Cell::Empty, Cell::Amount(SignedAmount::cr(rs(rupees)))]
    };
    assert_eq!(
        row_cells(tb, label),
        Some(cells),
        "trial balance row {label}"
    );
}

/// Derived. Each ledger's closing balance, summed from the vouchers above (Dr positive):
/// Cash 50,000 - 20,000 + 5,000 - 1,000 - 4,000 + 1,770 = Dr 31,770. Bank 500,000 - 50,000 -
/// 5,000 - 3,000 + 3,000 + 1,000 + 2,000 - 1,000 = Dr 447,000. Capital 500,000 - 1,000 =
/// Cr 499,000. Purchase 10,000 + 5,000 = Dr 15,000. Rent 20,000 - 1,000 = Dr 19,000.
/// Electricity 3,000 + 1,000 = Dr 4,000. Interest 5,000 + 2,000 = Cr 7,000. Input IGST 1,800 +
/// 900 = Dr 2,700. Output IGST 1,080 + 270 + 810 - 270 = Cr 1,890. Output CGST 180 and SGST 180
/// (Cr). Sales 6,000 + 1,500 + 2,000 + 4,500 - 1,500 = Cr 12,500. Gujarat Buyer 7,080 + 5,310 -
/// 1,770 - 3,000 = Dr 7,620. Walk-in 1,770 - 1,770 = nil. Karnataka Customer Dr 2,360.
/// Gujarat Supplier 11,800 + 5,900 - 5,000 - 4,000 = Cr 8,700.
/// Totals: Dr 31,770 + 447,000 + 15,000 + 19,000 + 4,000 + 2,700 + 7,620 + 2,360 = 529,450;
/// Cr 499,000 + 12,500 + 7,000 + 1,890 + 180 + 180 + 8,700 = 529,450.
fn check_trial_balance(tb: &Report) {
    assert_tb_row(tb, "Cash", true, 31_770);
    assert_tb_row(tb, "Bank", true, 447_000);
    assert_tb_row(tb, "Capital", false, 499_000);
    assert_tb_row(tb, "Sales", false, 12_500);
    assert_tb_row(tb, "Purchase", true, 15_000);
    assert_tb_row(tb, "Rent", true, 19_000);
    assert_tb_row(tb, "Electricity", true, 4_000);
    assert_tb_row(tb, "Interest Received", false, 7_000);
    assert_tb_row(tb, "Input IGST", true, 2_700);
    assert_tb_row(tb, "Output IGST", false, 1_890);
    assert_tb_row(tb, "Output CGST", false, 180);
    assert_tb_row(tb, "Output SGST", false, 180);
    assert_tb_row(tb, "Gujarat Buyer", true, 7_620);
    assert_tb_row(tb, "Karnataka Customer", true, 2_360);
    assert_tb_row(tb, "Gujarat Supplier", false, 8_700);
    assert_eq!(
        tb.totals,
        vec![
            Cell::Amount(SignedAmount::dr(rs(529_450))),
            Cell::Amount(SignedAmount::cr(rs(529_450))),
        ]
    );
}

/// Derived. Closing stock is 6 units. The average-cost pool holds 10 units bought at 1,000
/// (10,000), 5 at 1,000 (5,000) and the returned unit at 1,500 (1,500): value 16,500 over 16
/// units. Closing value = 6 x 16,500 / 16 = 6,187.50.
///
/// Tally's Balance Sheet shows each group as one amount (see the module note), so the Input IGST
/// debit of 2,700, which sits in Duties & Taxes under Current Liabilities, nets against that group
/// and is not moved to the asset side. Liabilities: capital 499,000 + Current Liabilities
/// (supplier 8,700 + output tax 2,250 - input IGST 2,700 = 8,250) = Cr 507,250. Assets: cash
/// 31,770 + bank 447,000 + buyer 7,620 + karnataka customer 2,360 + stock 6,187.50 = 494,937.50,
/// plus the loss of 12,312.50 shown on the asset side = Dr 507,250.
fn check_balance_sheet(bs: &Report) {
    assert_eq!(
        bs.totals,
        vec![
            Cell::Amount(SignedAmount::cr(rs(507_250))),
            Cell::Amount(SignedAmount::dr(rs(507_250))),
        ]
    );
}

/// Derived. Gross profit = sales 12,500 + closing stock 6,187.50 - purchases 15,000 = 3,687.50.
/// Net profit = gross 3,687.50 + interest 7,000 - rent 19,000 - electricity 4,000 = a loss of
/// 12,312.50, shown negative. Paise: 368,750 and -1,231,250.
fn check_profit_and_loss(pl: &Report) {
    assert_eq!(
        row_cells(pl, "Gross profit"),
        Some(vec![Cell::Signed(Paise::new(368_750))])
    );
    assert_eq!(
        row_cells(pl, "Net profit"),
        Some(vec![Cell::Signed(Paise::new(-1_231_250))])
    );
}

// ---------------------------------------------------------------------------------------------
// GSTR-1 through the facade

/// The real slab pack, plus the two GSTR-1 fixture parameters (see the module note).
fn fixture_pack() -> RulePack {
    let mut text = include_str!("../../ledgerly-gst/packs/rates-slabs.toml").to_owned();
    text.push_str(
        "\n[[entry]]\nid = \"gstr1.b2cl_threshold\"\neffective_from = \"2024-08-01\"\n\
         legal_ref = \"fixture for the C8 test; not verified\"\n\
         [entry.params]\ninvoice_value_rupees = \"100000\"\n",
    );
    text.push_str(
        "\n[[entry]]\nid = \"gstr1.hsn_credit_notes\"\neffective_from = \"2024-08-01\"\n\
         legal_ref = \"fixture for the C8 test; not verified\"\n\
         [entry.params]\ntreatment = \"net\"\n",
    );
    load_pack(&text).unwrap()
}

fn gstr1_july(api: &Api) -> Gstr1Return {
    match api
        .gstr1_with_pack(&gstin(OWN_GSTIN), july(), &fixture_pack())
        .unwrap()
    {
        Outcome::Value(ret) => ret,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

/// Vouchers 9 to 20 fall in July. Sales 9 to 12 and the credit note 13 are `Ready`. The receipts,
/// contra, journals and payment are `NotRelevant`.
fn check_buckets(buckets: &[BucketedVoucher]) {
    let expected: [(u64, Bucket); 12] = [
        (9, Bucket::Ready),
        (10, Bucket::Ready),
        (11, Bucket::Ready),
        (12, Bucket::Ready),
        (13, Bucket::Ready),
        (14, Bucket::NotRelevant),
        (15, Bucket::NotRelevant),
        (16, Bucket::NotRelevant),
        (17, Bucket::NotRelevant),
        (18, Bucket::NotRelevant),
        (19, Bucket::NotRelevant),
        (20, Bucket::NotRelevant),
    ];
    assert_eq!(buckets.len(), expected.len());
    for (id, bucket) in expected {
        let found = buckets
            .iter()
            .find(|b| b.voucher == VoucherId(id))
            .unwrap_or_else(|| panic!("voucher {id} has no bucket"));
        assert_eq!(
            found.bucket, bucket,
            "voucher {id}: reason {:?}",
            found.reason
        );
    }
}

fn find_invoice(invoices: &[B2bInvoice], value: i64) -> &B2bInvoice {
    invoices
        .iter()
        .find(|inv| inv.value == rs(value))
        .unwrap_or_else(|| panic!("no B2B invoice of value {value}"))
}

/// Derived. Voucher 9: taxable 6,000 at 18 percent, IGST 1,080, value 7,080. Voucher 12: taxable
/// 4,500, IGST 810, value 5,310. Both go to the registered Gujarat buyer under Table 4A.
fn check_b2b(gstr1: &Gstr1) {
    assert_eq!(gstr1.b2b.len(), 1);
    let party = &gstr1.b2b[0];
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    assert_eq!(party.invoices.len(), 2);
    for (invoice_value, taxable, igst) in [(7_080, 6_000, 1_080), (5_310, 4_500, 810)] {
        let inv = find_invoice(&party.invoices, invoice_value);
        assert_eq!(inv.pos, state(GUJARAT));
        assert_eq!(inv.inv_type, B2bInvoiceType::Regular);
        let line = inv.items.first().unwrap();
        assert_eq!(line.rate, rate(18));
        assert_eq!(line.taxable, rs(taxable));
        assert_eq!(line.tax, ItemTax::Igst(rs(igst)));
        assert_eq!(line.cess, Paise::ZERO);
    }
}

/// Derived. Voucher 10 is an inter-state sale to an unregistered buyer below the threshold:
/// taxable 1,500, IGST 270. Voucher 11 is intra-state: taxable 2,000, CGST 180 and SGST 180.
fn check_b2cs(gstr1: &Gstr1) {
    assert_eq!(gstr1.b2cs.len(), 2);
    let inter = gstr1.b2cs.iter().find(|r| !r.intra_state).unwrap();
    assert_eq!(inter.pos, state(GUJARAT));
    assert_eq!(inter.line.rate, rate(18));
    assert_eq!(inter.line.taxable, rs(1_500));
    assert_eq!(inter.line.tax, ItemTax::Igst(rs(270)));
    let intra = gstr1.b2cs.iter().find(|r| r.intra_state).unwrap();
    assert_eq!(intra.pos, state(KARNATAKA));
    assert_eq!(intra.line.rate, rate(18));
    assert_eq!(intra.line.taxable, rs(2_000));
    assert_eq!(
        intra.line.tax,
        ItemTax::CgstSgst {
            cgst: rs(180),
            sgst: rs(180),
        }
    );
}

/// Derived. Voucher 13 is a credit note to the registered buyer: taxable 1,500, IGST 270, note
/// value 1,770, one note under Table 9B.
fn check_cdnr(gstr1: &Gstr1) {
    assert_eq!(gstr1.cdnr.len(), 1);
    let party = &gstr1.cdnr[0];
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    assert_eq!(party.notes.len(), 1);
    let note = &party.notes[0];
    assert_eq!(note.value, rs(1_770));
    let line = note.items.first().unwrap();
    assert_eq!(line.taxable, rs(1_500));
    assert_eq!(line.tax, ItemTax::Igst(rs(270)));
}

/// Derived, with the credit note netted (fixture treatment `net`). Taxable: 6,000 + 1,500 + 2,000 +
/// 4,500 - 1,500 = 12,500. IGST: 1,080 + 270 + 810 - 270 = 1,890. CGST 180, SGST 180. Value =
/// 12,500 + 1,890 + 180 + 180 = 14,750. One HSN, unit PCS, rate 18.
fn check_hsn(gstr1: &Gstr1) {
    assert_eq!(gstr1.hsn.len(), 1);
    let h = &gstr1.hsn[0];
    assert_eq!(h.hsn.as_str(), "8471");
    assert_eq!(h.uqc, "PCS");
    assert_eq!(h.rate, rate(18));
    assert_eq!(h.taxable, rs(12_500));
    assert_eq!(h.igst, rs(1_890));
    assert_eq!(h.cgst, rs(180));
    assert_eq!(h.sgst, rs(180));
    assert_eq!(h.value, rs(14_750));
}

fn check_gstr1(ret: &Gstr1Return) {
    check_buckets(&ret.buckets);
    assert!(ret.is_complete());
    check_b2b(&ret.gstr1);
    check_b2cs(&ret.gstr1);
    check_cdnr(&ret.gstr1);
    check_hsn(&ret.gstr1);
}

// ---------------------------------------------------------------------------------------------
// Bank statement and reconciliation

fn import_statement(api: &mut Api, ids: &Ids) -> BankStatementId {
    match api
        .import_bank_statement(STATEMENT.as_bytes(), MT940, ids.bank)
        .unwrap()
    {
        Outcome::Value(id) => id,
        other @ Outcome::Unsupported(_) => panic!("expected an imported statement, got {other:?}"),
    }
}

fn brs_at(api: &Api, ids: &Ids, as_of: Date) -> Brs {
    api.brs(ids.bank, as_of).unwrap().value().unwrap()
}

/// Derived (rule: balance per bank = balance per books - uncleared deposits + uncleared payments).
/// On 30 July 2026, no bank date is set on any line, so every bank line is uncleared. Books
/// balance: 447,000 (the trial balance row, every bank voucher is dated on or before 30 July).
/// Uncleared deposits (debits to the bank): voucher 1 (500,000), 14 (3,000), 16 (1,000), 18
/// (2,000) = 506,000, listed by date. Uncleared payments (credits): voucher 2 (50,000), 6
/// (5,000), 8 (3,000), 19 (1,000) = 59,000. Balance per bank = 447,000 - 506,000 + 59,000 = 0.
/// Statement balance on 30 July = opening 442,000 + 3,000 + 1,000 + 2,000 - 1,000 (the lines
/// dated up to 30 July; the 31 July charge is later) = 447,000. The statement covers 14 July up
/// to 31 July, so 30 July is covered. Difference = 447,000 - 0 = 447,000: the statement is not
/// reconciled because no match is confirmed here.
fn check_brs(brs: &Brs) {
    assert_eq!(brs.books_balance, rs(447_000));
    let deposits: Vec<(u64, Paise)> = brs
        .uncleared_deposits
        .iter()
        .map(|i| (i.voucher.0, i.amount))
        .collect();
    assert_eq!(
        deposits,
        vec![
            (1, rs(500_000)),
            (14, rs(3_000)),
            (16, rs(1_000)),
            (18, rs(2_000))
        ]
    );
    let payments: Vec<(u64, Paise)> = brs
        .uncleared_payments
        .iter()
        .map(|i| (i.voucher.0, i.amount))
        .collect();
    assert_eq!(
        payments,
        vec![
            (2, rs(50_000)),
            (6, rs(5_000)),
            (8, rs(3_000)),
            (19, rs(1_000))
        ]
    );
    assert_eq!(brs.uncleared_deposits_total, rs(506_000));
    assert_eq!(brs.uncleared_payments_total, rs(59_000));
    assert_eq!(brs.balance_as_per_bank, Paise::ZERO);
    assert_eq!(brs.statement_balance.clone().value(), Some(rs(447_000)));
    assert_eq!(brs.difference.clone().value(), Some(rs(447_000)));
    assert_eq!(brs.is_reconciled().value(), Some(false));
}

/// The statement is held for the bank ledger under `id`, with the lines and balances of
/// [`STATEMENT`]. Derived: five lines; opening 442,000; closing 446,950; first line 14 July, last
/// line 31 July.
fn check_statement_held(api: &Api, ids: &Ids, id: BankStatementId) {
    let held = api.bank_statements(ids.bank);
    assert_eq!(held.len(), 1, "one statement is held for the bank ledger");
    let (held_id, statement) = held[0];
    assert_eq!(held_id, id);
    assert_eq!(statement.ledger(), ids.bank);
    assert_eq!(statement.lines().len(), 5);
    assert_eq!(statement.opening(), rs(442_000));
    assert_eq!(statement.closing(), rs(446_950));
    assert_eq!(
        statement.period(),
        Some((date!(2026 - 07 - 14), date!(2026 - 07 - 31)))
    );
}

// ---------------------------------------------------------------------------------------------
// Audit chain and the rebuild check

fn assert_chain_ok(report: &VerifyReport) {
    assert_eq!(report.problems, Vec::<ChainProblem>::new());
    assert!(report.is_ok());
    assert!(report.events_checked > 0);
}

fn assert_rebuild_matches(api: &Api) {
    let report = api.rebuild_check().unwrap();
    assert!(
        report.is_match(),
        "the stored projections must equal the rebuild: {report:?}"
    );
}

/// A company file with every master created by id 0 and the twenty vouchers posted.
fn books_with_twenty_vouchers() -> (tempfile::TempDir, PathBuf, Api, Ids) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    let ids = create_masters(&mut api);
    post_all(&mut api, purchases_and_payments(&ids));
    post_all(&mut api, july_sales(&ids));
    post_all(&mut api, july_settlements(&ids));
    (dir, path, api, ids)
}

// ---------------------------------------------------------------------------------------------
// The tests

#[test]
fn masters_created_with_id_zero_get_distinct_assigned_ids_that_the_books_hold() {
    let (_dir, _path, api, ids) = books_with_twenty_vouchers();
    let ledger_ids = [
        ids.cash,
        ids.bank,
        ids.capital,
        ids.sales,
        ids.purchase,
        ids.rent,
        ids.electricity,
        ids.interest,
        ids.input_igst,
        ids.output_igst,
        ids.output_cgst,
        ids.output_sgst,
        ids.buyer,
        ids.walk_in,
        ids.ka_customer,
        ids.supplier,
    ];
    // Derived: sixteen ledgers were created, so sixteen different ids, none of them the
    // unassigned 0 (contract C1: an id of 0 gets the next free id).
    let distinct: BTreeSet<u64> = ledger_ids.iter().map(|id| id.0).collect();
    assert_eq!(distinct.len(), 16);
    assert!(ledger_ids.iter().all(|id| id.0 != 0));
    assert_ne!(ids.item.0, 0);
    assert_ne!(ids.unit.0, 0);
    assert_ne!(ids.main.0, 0);
    // The books hold each master under its assigned id, with the name it was created with.
    let books = api.books().unwrap().value().unwrap();
    for (id, name) in [
        (ids.cash, "Cash"),
        (ids.bank, "Bank"),
        (ids.buyer, "Gujarat Buyer"),
        (ids.supplier, "Gujarat Supplier"),
    ] {
        assert_eq!(books.masters.ledgers.get(&id).unwrap().name, name);
    }
    let widget = books.masters.stock_items.get(&ids.item).unwrap();
    assert_eq!(widget.unit, ids.unit);
}

#[test]
fn twenty_vouchers_reports_and_gstr1_agree_with_hand_derivation() {
    let (_dir, _path, api, _ids) = books_with_twenty_vouchers();
    let now = reports(&api);
    check_trial_balance(&now.trial_balance);
    check_balance_sheet(&now.balance_sheet);
    check_profit_and_loss(&now.profit_and_loss);
    check_gstr1(&gstr1_july(&api));
}

#[test]
fn company_from_masters_to_backup_keeps_statement_reports_and_chain() {
    let (dir, path, mut api, ids) = books_with_twenty_vouchers();
    let before = reports(&api);
    check_trial_balance(&before.trial_balance);
    check_balance_sheet(&before.balance_sheet);
    check_profit_and_loss(&before.profit_and_loss);
    let gstr1_before = gstr1_july(&api);
    check_gstr1(&gstr1_before);

    // Import the bank statement; the books side of the reconciliation does not change.
    let statement_id = import_statement(&mut api, &ids);
    check_statement_held(&api, &ids, statement_id);
    let brs_before = brs_at(&api, &ids, date!(2026 - 07 - 30));
    check_brs(&brs_before);
    assert_rebuild_matches(&api);
    drop(api);

    // Reopen: the statement is still there, the reconciliation and every report are unchanged.
    let reopened = Api::open(&path).unwrap();
    check_statement_held(&reopened, &ids, statement_id);
    assert_eq!(brs_at(&reopened, &ids, date!(2026 - 07 - 30)), brs_before);
    assert_eq!(reports(&reopened), before);
    assert_eq!(gstr1_july(&reopened), gstr1_before);
    assert_rebuild_matches(&reopened);

    // Back up, then verify the chain of the file, the copy, and the copy as a file.
    assert_chain_ok(&reopened.verify().unwrap());
    let backup = reopened.backup_to(&dir.path().join("backups")).unwrap();
    assert_chain_ok(&verify_file(&backup).unwrap());
    assert_chain_ok(&verify_file(&path).unwrap());
    drop(reopened);
    let from_backup = Api::open(&backup).unwrap();
    assert_eq!(reports(&from_backup), before);
    check_statement_held(&from_backup, &ids, statement_id);
    assert_eq!(
        brs_at(&from_backup, &ids, date!(2026 - 07 - 30)),
        brs_before
    );
    assert_rebuild_matches(&from_backup);
}

#[test]
fn a_second_import_of_the_same_statement_file_is_refused_after_reopen() {
    let (_dir, path, mut api, ids) = books_with_twenty_vouchers();
    import_statement(&mut api, &ids);
    drop(api);
    let mut reopened = Api::open(&path).unwrap();
    // Derived: the file hash is already known, so importing the same bytes again must fail, and
    // nothing is added (a duplicate would double every line in the reconciliation).
    let again = reopened.import_bank_statement(STATEMENT.as_bytes(), MT940, ids.bank);
    assert!(
        matches!(
            again,
            Err(ApiError::StatementImport(StatementError::Duplicate { .. }))
        ),
        "expected a duplicate-statement refusal, got {again:?}"
    );
    assert_eq!(reopened.bank_statements(ids.bank).len(), 1);
}
