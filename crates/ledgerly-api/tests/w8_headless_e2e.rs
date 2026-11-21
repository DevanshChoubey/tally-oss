//! Card W8 tests: the headless end-to-end path (spec sections 4, 5, 6.2, 6.3 and 6.6).
//!
//! The path is: create a company file, create its masters through [`Command`], post twenty
//! vouchers, run the trial balance, balance sheet and profit and loss, build GSTR-1 on the books
//! loaded from the store, back the file up, verify the audit chain of the original and the copy,
//! reopen both with [`Api::open`] and compare the reports.
//!
//! Provenance. Every expected figure is `derived` by hand from the posting rules (a sale debits
//! the customer and credits sales and the output tax) and from the arithmetic written next to
//! it. None is copied from the engine. Amounts are in rupees in the test and converted to paise
//! by [`rs`]. The arithmetic is shown in the comments on each check.
//!
//! Test inputs that are not law (recorded so that they are not mistaken for verified facts):
//! * The GSTR-1 pack entries `gstr1.b2cl_threshold` and `gstr1.hsn_credit_notes` are fixture
//!   parameters appended to the real slab pack, as the G8 tests do. Neither value is verified.
//!   Every voucher in this test is far below any threshold, so the threshold does not change a
//!   figure here. The credit-note treatment `net` does change the HSN row; that row is only as
//!   good as the fixture. Both stay pending until a person checks the rule pack.
//! * The credit note brings the returned item back into stock at its own invoice value, because
//!   the engine takes the inward value from the line amount. The average-cost closing value
//!   (Rs 6,187.50) therefore depends on that input and on the engine's average-cost formula.
//!   The page for a sales return does not say how Tally values returned stock. Pending.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ledgerly_api::{Api, ApiError, Command, CommandResult};
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GodownId, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise,
    Period, Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_gst::gstr1::{Bucket, BucketedVoucher, build_gstr1};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_gst_formats::gstr1::{B2bInvoice, B2bInvoiceType, Gstr1, ItemTax};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, GodownAllocation, GstHistory,
    GstRegistration, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy, PartyDetails,
    RegistrationType, ReservedGroup, Scenario, StockItem, TaxDetails, TaxLedgerKind, Taxability,
    Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use ledgerly_store::verify::{ChainProblem, VerifyReport, verify_chain};
use ledgerly_store::{SqliteStore, Store};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const BANK: LedgerId = LedgerId(2);
const CAPITAL: LedgerId = LedgerId(3);
const SALES: LedgerId = LedgerId(4);
const PURCHASE: LedgerId = LedgerId(5);
const RENT: LedgerId = LedgerId(6);
const ELECTRICITY: LedgerId = LedgerId(7);
const INTEREST: LedgerId = LedgerId(8);
const INPUT_IGST: LedgerId = LedgerId(9);
const OUTPUT_IGST: LedgerId = LedgerId(10);
const OUTPUT_CGST: LedgerId = LedgerId(11);
const OUTPUT_SGST: LedgerId = LedgerId(12);
const GUJARAT_BUYER: LedgerId = LedgerId(13);
const WALK_IN: LedgerId = LedgerId(14);
const KA_CUSTOMER: LedgerId = LedgerId(15);
const SUPPLIER: LedgerId = LedgerId(16);

const ITEM: StockItemId = StockItemId(1);
const UNIT: UnitId = UnitId(1);
const MAIN: GodownId = GodownId(1);

/// Fictional GSTIN from the core test vectors (Karnataka): the company's own registration.
const OWN_GSTIN: &str = "29ABCDE1234F1ZW";
/// Fictional GSTIN from the formats test vectors (Gujarat): a registered buyer.
const BUYER_GSTIN: &str = "24AAACB1111C1ZT";
const KARNATAKA: u8 = 29;
const GUJARAT: u8 = 24;

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

// ---------------------------------------------------------------------------------------------
// Masters

fn plain(id: LedgerId, name: &str, reserved: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(reserved),
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

fn tax_ledger(id: LedgerId, name: &str, kind: TaxLedgerKind, percent: i64) -> Ledger {
    Ledger {
        tax: Some(TaxDetails {
            kind,
            rate_percent: Some(rate(percent)),
        }),
        ..plain(id, name, ReservedGroup::DutiesAndTaxes)
    }
}

fn party_ledger(
    id: LedgerId,
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
        ..plain(id, name, reserved)
    }
}

/// The sixteen ledgers. Names differ from every seeded group name, because rows are read by label.
fn ledgers() -> Vec<Ledger> {
    vec![
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(BANK, "Bank", ReservedGroup::BankAccounts),
        plain(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        plain(SALES, "Sales", ReservedGroup::SalesAccounts),
        plain(PURCHASE, "Purchase", ReservedGroup::PurchaseAccounts),
        plain(RENT, "Rent", ReservedGroup::IndirectExpenses),
        plain(ELECTRICITY, "Electricity", ReservedGroup::IndirectExpenses),
        plain(
            INTEREST,
            "Interest Received",
            ReservedGroup::IndirectIncomes,
        ),
        tax_ledger(INPUT_IGST, "Input IGST", TaxLedgerKind::Igst, 18),
        tax_ledger(OUTPUT_IGST, "Output IGST", TaxLedgerKind::Igst, 18),
        tax_ledger(OUTPUT_CGST, "Output CGST", TaxLedgerKind::Cgst, 9),
        tax_ledger(OUTPUT_SGST, "Output SGST", TaxLedgerKind::Sgst, 9),
        party_ledger(
            GUJARAT_BUYER,
            "Gujarat Buyer",
            ReservedGroup::SundryDebtors,
            RegistrationType::Regular,
            Some(BUYER_GSTIN),
            GUJARAT,
        ),
        party_ledger(
            WALK_IN,
            "Walk-in Gujarat",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            GUJARAT,
        ),
        party_ledger(
            KA_CUSTOMER,
            "Karnataka Customer",
            ReservedGroup::SundryDebtors,
            RegistrationType::Unregistered,
            None,
            KARNATAKA,
        ),
        party_ledger(
            SUPPLIER,
            "Gujarat Supplier",
            ReservedGroup::SundryCreditors,
            RegistrationType::Unregistered,
            None,
            GUJARAT,
        ),
    ]
}

/// One stock item, Widget, HSN 8471, 18 percent from 22 September 2025 (the rate in the GST 2.0
/// schedule; the item's own history is the input, as in the G8 tests).
fn widget() -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: ITEM,
        name: "Widget".to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UNIT,
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
    }
}

fn create_masters(api: &mut Api) {
    for ledger in ledgers() {
        value(api.execute(Command::CreateLedger(ledger)));
    }
    let pieces = Unit {
        id: UNIT,
        symbol: "pcs".to_owned(),
        formal_name: "Pieces".to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: Some("PCS".to_owned()),
        tally: None,
    };
    value(api.execute(Command::CreateUnit(pieces)));
    value(api.execute(Command::CreateStockItem(widget())));
    // Main Location is Tally's default godown and every company file holds it from creation as
    // godown 1 (seed_godowns; TallyPrime godowns FAQ, accessed 2026-11-21), so this test no
    // longer creates it by hand (updated 2026-11-21 on the owner's "follow Tally" instruction).
    assert_eq!(MAIN, MAIN_LOCATION);
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
fn item(qty: i64, rate_rupees: i64, amount_rupees: i64, ledger: LedgerId) -> InventoryLine {
    let amount = rs(amount_rupees);
    InventoryLine {
        item: ITEM,
        qty: Qty(Decimal::new(qty, 0)),
        unit: UNIT,
        rate: Rate(Decimal::new(rate_rupees * 100, 2)),
        discount_percent: None,
        amount,
        ledger: Some(ledger),
        godowns: vec![GodownAllocation {
            godown: MAIN,
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
    lines: Vec<LedgerLine>,
    items: Vec<InventoryLine>,
) -> Voucher {
    Voucher {
        party: Some(party),
        place_of_supply: Some(state(pos)),
        mode: VoucherMode::ItemInvoice,
        inventory_lines: items,
        ..plain_voucher(id, kind, on, lines)
    }
}

/// Vouchers 1 to 8: capital, a contra, two purchases (ten and five units at Rs 1,000), and
/// the payments and receipts of April.
fn purchases_and_payments() -> Vec<Voucher> {
    vec![
        plain_voucher(
            1,
            BaseVoucherKind::Journal,
            date!(2026 - 04 - 01),
            vec![dr(BANK, 500_000), cr(CAPITAL, 500_000)],
        ),
        plain_voucher(
            2,
            BaseVoucherKind::Contra,
            date!(2026 - 04 - 02),
            vec![dr(CASH, 50_000), cr(BANK, 50_000)],
        ),
        invoice(
            3,
            BaseVoucherKind::Purchase,
            date!(2026 - 04 - 05),
            SUPPLIER,
            GUJARAT,
            vec![
                dr(PURCHASE, 10_000),
                dr(INPUT_IGST, 1_800),
                cr(SUPPLIER, 11_800),
            ],
            vec![item(10, 1_000, 10_000, PURCHASE)],
        ),
        plain_voucher(
            4,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 10),
            vec![dr(RENT, 20_000), cr(CASH, 20_000)],
        ),
        plain_voucher(
            5,
            BaseVoucherKind::Receipt,
            date!(2026 - 04 - 12),
            vec![dr(CASH, 5_000), cr(INTEREST, 5_000)],
        ),
        plain_voucher(
            6,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 15),
            vec![dr(SUPPLIER, 5_000), cr(BANK, 5_000)],
        ),
        invoice(
            7,
            BaseVoucherKind::Purchase,
            date!(2026 - 04 - 20),
            SUPPLIER,
            GUJARAT,
            vec![
                dr(PURCHASE, 5_000),
                dr(INPUT_IGST, 900),
                cr(SUPPLIER, 5_900),
            ],
            vec![item(5, 1_000, 5_000, PURCHASE)],
        ),
        plain_voucher(
            8,
            BaseVoucherKind::Payment,
            date!(2026 - 04 - 22),
            vec![dr(ELECTRICITY, 3_000), cr(BANK, 3_000)],
        ),
    ]
}

/// Vouchers 9 to 13: four sales and one credit note with items, all in July 2026.
fn july_sales() -> Vec<Voucher> {
    vec![
        // Taxable 4 x 1,500 = 6,000; IGST 18 percent = 1,080; invoice 7,080.
        invoice(
            9,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 03),
            GUJARAT_BUYER,
            GUJARAT,
            vec![
                dr(GUJARAT_BUYER, 7_080),
                cr(SALES, 6_000),
                cr(OUTPUT_IGST, 1_080),
            ],
            vec![item(4, 1_500, 6_000, SALES)],
        ),
        // Taxable 1,500; IGST 270; invoice 1,770 to an unregistered buyer in another state.
        invoice(
            10,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 05),
            WALK_IN,
            GUJARAT,
            vec![dr(WALK_IN, 1_770), cr(SALES, 1_500), cr(OUTPUT_IGST, 270)],
            vec![item(1, 1_500, 1_500, SALES)],
        ),
        // Taxable 2 x 1,000 = 2,000; CGST 9 percent = 180; SGST 180; invoice 2,360, intra-state.
        invoice(
            11,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 08),
            KA_CUSTOMER,
            KARNATAKA,
            vec![
                dr(KA_CUSTOMER, 2_360),
                cr(SALES, 2_000),
                cr(OUTPUT_CGST, 180),
                cr(OUTPUT_SGST, 180),
            ],
            vec![item(2, 1_000, 2_000, SALES)],
        ),
        // Taxable 3 x 1,500 = 4,500; IGST 810; invoice 5,310.
        invoice(
            12,
            BaseVoucherKind::Sales,
            date!(2026 - 07 - 10),
            GUJARAT_BUYER,
            GUJARAT,
            vec![
                dr(GUJARAT_BUYER, 5_310),
                cr(SALES, 4_500),
                cr(OUTPUT_IGST, 810),
            ],
            vec![item(3, 1_500, 4_500, SALES)],
        ),
        // One unit returned by the buyer of voucher 9: taxable 1,500; IGST 270; note 1,770.
        // The stock comes back at 1,500 (the line amount is the inward value; see module note).
        invoice(
            13,
            BaseVoucherKind::CreditNote,
            date!(2026 - 07 - 12),
            GUJARAT_BUYER,
            GUJARAT,
            vec![
                dr(SALES, 1_500),
                dr(OUTPUT_IGST, 270),
                cr(GUJARAT_BUYER, 1_770),
            ],
            vec![item(1, 1_500, 1_500, SALES)],
        ),
    ]
}

/// Vouchers 14 to 20: receipts, a contra, a reclassifying journal, a payment and a capital
/// journal. None of them is a GSTR-1 document.
fn july_settlements() -> Vec<Voucher> {
    vec![
        plain_voucher(
            14,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 14),
            vec![dr(BANK, 3_000), cr(GUJARAT_BUYER, 3_000)],
        ),
        plain_voucher(
            15,
            BaseVoucherKind::Journal,
            date!(2026 - 07 - 15),
            vec![dr(ELECTRICITY, 1_000), cr(RENT, 1_000)],
        ),
        plain_voucher(
            16,
            BaseVoucherKind::Contra,
            date!(2026 - 07 - 18),
            vec![dr(BANK, 1_000), cr(CASH, 1_000)],
        ),
        plain_voucher(
            17,
            BaseVoucherKind::Payment,
            date!(2026 - 07 - 22),
            vec![dr(SUPPLIER, 4_000), cr(CASH, 4_000)],
        ),
        plain_voucher(
            18,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 25),
            vec![dr(BANK, 2_000), cr(INTEREST, 2_000)],
        ),
        plain_voucher(
            19,
            BaseVoucherKind::Journal,
            date!(2026 - 07 - 30),
            vec![dr(CAPITAL, 1_000), cr(BANK, 1_000)],
        ),
        plain_voucher(
            20,
            BaseVoucherKind::Receipt,
            date!(2026 - 07 - 31),
            vec![dr(CASH, 1_770), cr(WALK_IN, 1_770)],
        ),
    ]
}

/// Posts every voucher in order. Each must commit.
fn post_all(api: &mut Api, vouchers: Vec<Voucher>) {
    for v in vouchers {
        value(api.execute(Command::PostVoucher(v)));
    }
}

// ---------------------------------------------------------------------------------------------
// Results and reports

/// The result of a command that committed a value.
fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

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
/// Decided behaviour (owner instruction 2026-11-21, "take how tally does"): Tally's Balance Sheet
/// shows each group as one amount (screenshot in <https://help.tallysolutions.com/balance-sheet-tally/>,
/// image View-Balance-sheet.gif, read 2026-11-21: Current Liabilities is a single figure). So the
/// Input IGST debit of 2,700, which sits in Duties & Taxes under Current Liabilities, nets against
/// that group and is not moved to the asset side. The first version of this test moved it and
/// expected 509,950 on both sides; that expectation was changed for this reason, not to match
/// output.
///
/// Liabilities: capital 499,000 + Current Liabilities (supplier 8,700 + output tax 2,250 - input
/// IGST 2,700 = 8,250) = Cr 507,250. Assets: cash 31,770 + bank 447,000 + buyer 7,620 + karnataka
/// customer 2,360 + stock 6,187.50 = 494,937.50, plus the loss of 12,312.50 shown on the asset
/// side = Dr 507,250.
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
// GSTR-1 on the books loaded from the store

/// The real slab pack, plus the two GSTR-1 fixture parameters (see the module note).
fn fixture_pack() -> RulePack {
    let mut text = include_str!("../../ledgerly-gst/packs/rates-slabs.toml").to_owned();
    text.push_str(
        "\n[[entry]]\nid = \"gstr1.b2cl_threshold\"\neffective_from = \"2024-08-01\"\n\
         legal_ref = \"fixture for the W8 test; not verified\"\n\
         [entry.params]\ninvoice_value_rupees = \"100000\"\n",
    );
    text.push_str(
        "\n[[entry]]\nid = \"gstr1.hsn_credit_notes\"\neffective_from = \"2024-08-01\"\n\
         legal_ref = \"fixture for the W8 test; not verified\"\n\
         [entry.params]\ntreatment = \"net\"\n",
    );
    load_pack(&text).unwrap()
}

fn build_july(path: &Path) -> (Gstr1, Vec<BucketedVoucher>) {
    let books = SqliteStore::open(path).unwrap().load_books(None).unwrap();
    let pack = fixture_pack();
    match build_gstr1(&books, &gstin(OWN_GSTIN), july(), &pack).unwrap() {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

/// Vouchers 9 to 20 fall in July. Sales 9 to 12 and the credit note 13 are `Ready`. The
/// receipts, contra, journals and payment are `NotRelevant`.
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
        let item = inv.items.first().unwrap();
        assert_eq!(item.rate, rate(18));
        assert_eq!(item.taxable, rs(taxable));
        assert_eq!(item.tax, ItemTax::Igst(rs(igst)));
        assert_eq!(item.cess, Paise::ZERO);
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

/// Derived. Voucher 13 is a credit note to the registered buyer: taxable 1,500, IGST 270,
/// note value 1,770, one note under Table 9B.
fn check_cdnr(gstr1: &Gstr1) {
    assert_eq!(gstr1.cdnr.len(), 1);
    let party = &gstr1.cdnr[0];
    assert_eq!(party.ctin, gstin(BUYER_GSTIN));
    assert_eq!(party.notes.len(), 1);
    let note = &party.notes[0];
    assert_eq!(note.value, rs(1_770));
    let item = note.items.first().unwrap();
    assert_eq!(item.taxable, rs(1_500));
    assert_eq!(item.tax, ItemTax::Igst(rs(270)));
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

fn check_gstr1(path: &Path) {
    let (gstr1, buckets) = build_july(path);
    check_buckets(&buckets);
    check_b2b(&gstr1);
    check_b2cs(&gstr1);
    check_cdnr(&gstr1);
    check_hsn(&gstr1);
}

// ---------------------------------------------------------------------------------------------
// Backup, audit chain and reopening

fn assert_chain_ok(report: &VerifyReport) {
    assert_eq!(report.problems, Vec::<ChainProblem>::new());
    assert!(report.is_ok());
    assert!(report.events_checked > 0);
}

/// Backs the file up with the store's function, then verifies the chain of the original and of
/// the copy. Returns the copy's path.
fn backup_and_verify(path: &Path, dir: &Path) -> PathBuf {
    let store = SqliteStore::open(path).unwrap();
    assert_chain_ok(&verify_chain(&store).unwrap());
    let backup = ledgerly_store::backup::backup_to(&store, &dir.join("backups"), clock()).unwrap();
    let copy = SqliteStore::open(&backup).unwrap();
    assert_chain_ok(&verify_chain(&copy).unwrap());
    backup
}

// ---------------------------------------------------------------------------------------------
// The test

#[test]
fn company_from_masters_to_backup_reopens_with_identical_reports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    create_masters(&mut api);
    post_all(&mut api, purchases_and_payments());
    post_all(&mut api, july_sales());
    post_all(&mut api, july_settlements());

    let before = reports(&api);
    check_trial_balance(&before.trial_balance);
    check_balance_sheet(&before.balance_sheet);
    check_profit_and_loss(&before.profit_and_loss);
    check_gstr1(&path);
    drop(api);

    let backup = backup_and_verify(&path, dir.path());
    let reopened = Api::open(&path).unwrap();
    assert_eq!(reports(&reopened), before);
    let from_backup = Api::open(&backup).unwrap();
    assert_eq!(reports(&from_backup), before);
}
