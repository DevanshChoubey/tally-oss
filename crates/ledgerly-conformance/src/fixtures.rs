//! Shared test fixtures. **Card Q1** implements this module.
//!
//! Frozen interface (plan card Q1):
//!
//! ```text
//! pub fn books_small() -> Books;   // the fixture company in fixtures/README.md
//! ```
//!
//! The company is **Neelgiri Traders (fictional)**: 12 ledgers, 3 stock items, 2 godowns and 20
//! vouchers for FY 2025-26, described in `fixtures/README.md`. Every name, GSTIN and amount is
//! fictional. The data is hand-written constants; the engine is not involved. Postings and stock
//! moves are derived here by a few lines of plain code (not by `ledgerly-engine`), so that a
//! bug in the engine cannot hide behind its own fixture.
//!
//! Choices the README leaves open, all recorded here so a reviewer can change them:
//!
//! - Voucher ids are `1..=20`. The ids that tests rely on are `V1` opening capital, `V7`
//!   intra-state sale, `V8` inter-state sale, `V9` receipt against `V7`, `V12` contra, `V13`
//!   receipt in cash, `V15` cancelled, `V16` optional, and `V17` and `V18`, the rate-change pair.
//! - The reserved groups are a small hand-made subset (11 groups), not `reserved_groups()` of card
//!   M1, so this card does not depend on another card's output.
//! - No voucher carries a stored GST block (`gst` is `None`) and the rate-change pair `V17`/`V18`
//!   carries no tax lines: the rate for those dates comes from the rule pack (card G12) and Q1
//!   cannot derive it. Only `V7` and `V8` post tax, at the 18% the README fixes.
//! - Item `R3` has opening 100 @ 10 (README), which differs from the research file's table that
//!   shows a 1 April purchase instead; the README wins for this fixture.
//! - Stock moves are derived for regular vouchers only.

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GodownId, GroupId, Gstin, GstinError, LedgerId, MoneyError, Paise,
    Pan, PanError, Qty, Rate, SignedAmount, StateCode, StateCodeError, StockGroupId, StockItemId,
    UnitId, VoucherId, VoucherTypeId, gstin_check_character,
};
use ledgerly_model::{
    Address, AltUnit, BankAllocation, BankDetails, BaseVoucherKind, BillAllocation, BillRefKind,
    BillWiseConfig, Books, Company, EntityKind, Features, Godown, GodownAllocation, Group,
    GstClassification, GstHistory, GstRegistration, InstrumentKind, InventoryLine, Ledger,
    LedgerLine, Masters, Nature, NegativeStockPolicy, NumberingConfig, NumberingMethod,
    OpeningStock, PartyDetails, Posting, RegistrationType, ReservedGroup, ReservedLedger,
    RestartPeriod, StockGroup, StockItem, StockMove, TaxDetails, TaxLedgerKind, Taxability, Unit,
    UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use thiserror::Error;
use time::macros::date;

/// Identifiers of the fixture's masters, for tests that look things up.
pub mod ids {
    use ledgerly_core::{GodownId, LedgerId, StockItemId, UnitId};

    /// Cash ledger.
    pub const LEDGER_CASH: LedgerId = LedgerId(1);
    /// HDFC Current A/c, the bank ledger.
    pub const LEDGER_HDFC: LedgerId = LedgerId(2);
    /// Capital ledger.
    pub const LEDGER_CAPITAL: LedgerId = LedgerId(3);
    /// Sales - Local.
    pub const LEDGER_SALES_LOCAL: LedgerId = LedgerId(4);
    /// Sales - Interstate.
    pub const LEDGER_SALES_INTERSTATE: LedgerId = LedgerId(5);
    /// Purchase.
    pub const LEDGER_PURCHASE: LedgerId = LedgerId(6);
    /// CGST output ledger (9%).
    pub const LEDGER_CGST: LedgerId = LedgerId(7);
    /// SGST output ledger (9%).
    pub const LEDGER_SGST: LedgerId = LedgerId(8);
    /// IGST output ledger (18%).
    pub const LEDGER_IGST: LedgerId = LedgerId(9);
    /// Rent.
    pub const LEDGER_RENT: LedgerId = LedgerId(10);
    /// Shree Ganesh Stores, a registered debtor in state 27.
    pub const LEDGER_SHREE_GANESH: LedgerId = LedgerId(11);
    /// Coastal Supplies, a registered creditor in state 29.
    pub const LEDGER_COASTAL: LedgerId = LedgerId(12);

    /// Widget A (pcs, 18%).
    pub const ITEM_WIDGET_A: StockItemId = StockItemId(1);
    /// Widget B (box of 12 pcs, 5%).
    pub const ITEM_WIDGET_B: StockItemId = StockItemId(2);
    /// Item R3, the valuation item of `docs/research/03-inventory.md`.
    pub const ITEM_R3: StockItemId = StockItemId(3);

    /// Main godown.
    pub const GODOWN_MAIN: GodownId = GodownId(1);
    /// Pune godown.
    pub const GODOWN_PUNE: GodownId = GodownId(2);

    /// Unit: pieces (the base unit of every item).
    pub const UNIT_PCS: UnitId = UnitId(1);
    /// Unit: box (12 pieces for Widget B).
    pub const UNIT_BOX: UnitId = UnitId(2);
}

use ids::{
    GODOWN_MAIN, GODOWN_PUNE, ITEM_R3, ITEM_WIDGET_A, ITEM_WIDGET_B, LEDGER_CAPITAL, LEDGER_CASH,
    LEDGER_CGST, LEDGER_COASTAL, LEDGER_HDFC, LEDGER_IGST, LEDGER_PURCHASE, LEDGER_RENT,
    LEDGER_SALES_INTERSTATE, LEDGER_SALES_LOCAL, LEDGER_SGST, LEDGER_SHREE_GANESH, UNIT_BOX,
    UNIT_PCS,
};

const G_CAPITAL: GroupId = GroupId(1);
const G_CURRENT_ASSETS: GroupId = GroupId(2);
const G_CURRENT_LIABILITIES: GroupId = GroupId(3);
const G_SALES: GroupId = GroupId(4);
const G_PURCHASES: GroupId = GroupId(5);
const G_INDIRECT_EXPENSES: GroupId = GroupId(6);
const G_CASH: GroupId = GroupId(7);
const G_BANK: GroupId = GroupId(8);
const G_DEBTORS: GroupId = GroupId(9);
const G_CREDITORS: GroupId = GroupId(10);
const G_TAXES: GroupId = GroupId(11);

const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PURCHASE: VoucherTypeId = VoucherTypeId(2);
const T_SALES: VoucherTypeId = VoucherTypeId(3);
const T_RECEIPT: VoucherTypeId = VoucherTypeId(4);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(5);
const T_CONTRA: VoucherTypeId = VoucherTypeId(6);
const T_CREDIT_NOTE: VoucherTypeId = VoucherTypeId(7);

/// Why the fixture company could not be built. It cannot happen with the constants in this file;
/// the type exists so that the builder never panics on a mistaken constant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FixtureError {
    /// A state code is not in the table.
    #[error("fixture state code: {0}")]
    State(#[from] StateCodeError),
    /// A GSTIN did not validate.
    #[error("fixture GSTIN: {0}")]
    Gstin(#[from] GstinError),
    /// A PAN did not validate.
    #[error("fixture PAN: {0}")]
    Pan(#[from] PanError),
    /// Money arithmetic failed.
    #[error("fixture money: {0}")]
    Money(#[from] MoneyError),
    /// The first 14 characters of a GSTIN have no check character.
    #[error("no check character for GSTIN prefix '{0}'")]
    CheckCharacter(String),
    /// A voucher names a stock item that is not a master.
    #[error("voucher {voucher} names a stock item that does not exist")]
    MissingItem {
        /// Voucher id.
        voucher: u64,
    },
    /// A voucher names a voucher type that is not a master.
    #[error("voucher {voucher} names a voucher type that does not exist")]
    MissingVoucherType {
        /// Voucher id.
        voucher: u64,
    },
    /// An inventory line is in a unit the item cannot convert to its base unit.
    #[error("voucher {voucher} has an inventory line in a unit the item has no conversion for")]
    Unit {
        /// Voucher id.
        voucher: u64,
    },
    /// Converting a quantity to base units overflowed.
    #[error("voucher {voucher}: quantity overflow")]
    Quantity {
        /// Voucher id.
        voucher: u64,
    },
    /// The voucher type moves stock in no single direction, so no stock move can be derived.
    #[error("voucher {voucher}: its voucher type has no fixed stock direction")]
    NoStockDirection {
        /// Voucher id.
        voucher: u64,
    },
}

/// The fixture company `Neelgiri Traders` (see `fixtures/README.md`).
///
/// This builds constants, so it cannot fail; the signature is the frozen interface of card Q1.
/// Use [`try_books_small`] where a `Result` is wanted.
///
/// # Panics
/// Only if a constant in this file is edited into an invalid value; a test
/// (`try_books_small_succeeds_and_matches_books_small`) guards against that.
#[expect(
    clippy::expect_used,
    reason = "frozen interface returns Books; the data is constants and a test asserts it builds"
)]
pub fn books_small() -> Books {
    try_books_small().expect("the fixture company is built from constants and always builds")
}

/// Builds the fixture company, reporting a mistake in the fixture constants as an error.
///
/// # Errors
/// A [`FixtureError`] if a constant in this file is invalid (never, unless the file is edited).
pub fn try_books_small() -> Result<Books, FixtureError> {
    let masters = masters()?;
    let all = vouchers()?;
    let postings = postings(&all);
    let stock_moves = stock_moves(&all, &masters)?;
    Ok(Books {
        company: company()?,
        masters,
        vouchers: all.into_iter().map(|v| (v.id, v)).collect(),
        postings,
        stock_moves,
    })
}

// ---------------------------------------------------------------- small constructors

/// Whole rupees as paise. Only called with small constants.
const fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

fn qty(n: i64) -> Qty {
    Qty(Decimal::from(n))
}

fn rate(n: i64) -> Rate {
    Rate(Decimal::from(n))
}

fn dr(r: i64) -> SignedAmount {
    SignedAmount::dr(rupees(r))
}

fn cr(r: i64) -> SignedAmount {
    SignedAmount::cr(rupees(r))
}

// ---------------------------------------------------------------- company

fn company() -> Result<Company, FixtureError> {
    let mh = gstin_with_check_character("27ABCDE1234F1Z")?;
    let ka = Gstin::parse("29ABCDE1234F1ZW")?;
    let start = date!(2025 - 04 - 01);
    let registration = |gstin: Gstin, state: StateCode| GstRegistration {
        gstin,
        state,
        effective_from: start,
        composition: false,
    };
    let state = StateCode::new(27)?;
    Ok(Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Neelgiri Traders".to_owned(),
        mailing_name: None,
        address: address("Fictional Estate, Pune", state),
        state,
        pan: Some(Pan::parse("ABCDE1234F")?),
        registrations: vec![
            registration(mh, state),
            registration(ka, StateCode::new(29)?),
        ],
        fy_start: start,
        books_begin: start,
        entity_kind: EntityKind::Proprietorship,
        features: Features {
            inventory: true,
            godowns: true,
            batches: false,
            cost_centres: false,
            bill_wise: true,
            gst: true,
            bank_reconciliation: true,
            optional_vouchers: true,
            post_dated: false,
        },
        negative_stock: NegativeStockPolicy::Warn,
    })
}

fn address(line: &str, state: StateCode) -> Address {
    Address {
        lines: vec![line.to_owned()],
        pincode: None,
        state: Some(state),
        country: String::new(),
    }
}

/// A valid GSTIN from its first 14 characters, with the check character computed by
/// `ledgerly_core::gstin_check_character`.
fn gstin_with_check_character(first14: &str) -> Result<Gstin, FixtureError> {
    let check = gstin_check_character(first14)
        .ok_or_else(|| FixtureError::CheckCharacter(first14.to_owned()))?;
    Ok(Gstin::parse(&format!("{first14}{check}"))?)
}

// ---------------------------------------------------------------- masters

fn masters() -> Result<Masters, FixtureError> {
    Ok(Masters {
        groups: groups().into_iter().map(|g| (g.id, g)).collect(),
        ledgers: ledgers()?.into_iter().map(|l| (l.id, l)).collect(),
        voucher_types: voucher_types().into_iter().map(|t| (t.id, t)).collect(),
        units: units().into_iter().map(|u| (u.id, u)).collect(),
        stock_groups: BTreeMap::from([(
            StockGroupId(1),
            StockGroup {
                ineligible_itc: None,
                id: StockGroupId(1),
                name: "Trading Goods".to_owned(),
                parent: None,
                tally: None,
            },
        )]),
        stock_items: stock_items().into_iter().map(|i| (i.id, i)).collect(),
        godowns: godowns().into_iter().map(|g| (g.id, g)).collect(),
        ..Masters::default()
    })
}

fn primary_group(
    id: GroupId,
    name: &str,
    reserved: ReservedGroup,
    nature: Nature,
    affects_gross_profit: bool,
) -> Group {
    Group {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: Some(reserved),
        nature: Some(nature),
        affects_gross_profit,
        tally: None,
    }
}

fn sub_group(id: GroupId, name: &str, parent: GroupId, reserved: ReservedGroup) -> Group {
    Group {
        ineligible_itc: None,
        parent: Some(parent),
        nature: None,
        ..primary_group(id, name, reserved, Nature::Asset, false)
    }
}

fn groups() -> Vec<Group> {
    use Nature as N;
    use ReservedGroup as R;
    vec![
        primary_group(
            G_CAPITAL,
            "Capital Account",
            R::CapitalAccount,
            N::Liability,
            false,
        ),
        primary_group(
            G_CURRENT_ASSETS,
            "Current Assets",
            R::CurrentAssets,
            N::Asset,
            false,
        ),
        primary_group(
            G_CURRENT_LIABILITIES,
            "Current Liabilities",
            R::CurrentLiabilities,
            N::Liability,
            false,
        ),
        primary_group(G_SALES, "Sales Accounts", R::SalesAccounts, N::Income, true),
        primary_group(
            G_PURCHASES,
            "Purchase Accounts",
            R::PurchaseAccounts,
            N::Expense,
            true,
        ),
        primary_group(
            G_INDIRECT_EXPENSES,
            "Indirect Expenses",
            R::IndirectExpenses,
            N::Expense,
            false,
        ),
        sub_group(G_CASH, "Cash-in-Hand", G_CURRENT_ASSETS, R::CashInHand),
        sub_group(G_BANK, "Bank Accounts", G_CURRENT_ASSETS, R::BankAccounts),
        sub_group(
            G_DEBTORS,
            "Sundry Debtors",
            G_CURRENT_ASSETS,
            R::SundryDebtors,
        ),
        sub_group(
            G_CREDITORS,
            "Sundry Creditors",
            G_CURRENT_LIABILITIES,
            R::SundryCreditors,
        ),
        sub_group(
            G_TAXES,
            "Duties & Taxes",
            G_CURRENT_LIABILITIES,
            R::DutiesAndTaxes,
        ),
    ]
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
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

fn ledgers() -> Result<Vec<Ledger>, FixtureError> {
    let mut all = plain_ledgers();
    all.extend(tax_ledgers());
    all.extend(party_ledgers()?);
    Ok(all)
}

fn plain_ledgers() -> Vec<Ledger> {
    let taxable = || GstClassification {
        hsn: None,
        taxability: Taxability::Taxable,
        rate_percent: None,
    };
    let mut cash = ledger(LEDGER_CASH, "Cash", G_CASH);
    cash.reserved = Some(ReservedLedger::Cash);
    let mut bank = ledger(LEDGER_HDFC, "HDFC Current A/c", G_BANK);
    bank.bank = Some(BankDetails {
        account_number: "0000000000".to_owned(),
        ifsc: "HDFC0000000".to_owned(),
        bank_name: "HDFC (fictional account)".to_owned(),
        branch: "Pune".to_owned(),
    });
    let mut sales_local = ledger(LEDGER_SALES_LOCAL, "Sales \u{2013} Local", G_SALES);
    sales_local.gst = Some(taxable());
    let mut sales_inter = ledger(
        LEDGER_SALES_INTERSTATE,
        "Sales \u{2013} Interstate",
        G_SALES,
    );
    sales_inter.gst = Some(taxable());
    let mut purchase = ledger(LEDGER_PURCHASE, "Purchase", G_PURCHASES);
    purchase.gst = Some(taxable());
    vec![
        cash,
        bank,
        ledger(LEDGER_CAPITAL, "Capital", G_CAPITAL),
        sales_local,
        sales_inter,
        purchase,
        ledger(LEDGER_RENT, "Rent", G_INDIRECT_EXPENSES),
    ]
}

fn tax_ledgers() -> Vec<Ledger> {
    let tax = |id: LedgerId, name: &str, kind: TaxLedgerKind, percent: i64| {
        let mut l = ledger(id, name, G_TAXES);
        l.tax = Some(TaxDetails {
            kind,
            rate_percent: Some(rate(percent)),
        });
        l
    };
    vec![
        tax(LEDGER_CGST, "CGST", TaxLedgerKind::Cgst, 9),
        tax(LEDGER_SGST, "SGST", TaxLedgerKind::Sgst, 9),
        tax(LEDGER_IGST, "IGST", TaxLedgerKind::Igst, 18),
    ]
}

fn party_ledgers() -> Result<Vec<Ledger>, FixtureError> {
    let party = |id: LedgerId,
                 name: &str,
                 parent: GroupId,
                 first14: &str,
                 state: u8|
     -> Result<Ledger, FixtureError> {
        let state = StateCode::new(state)?;
        let gstin = gstin_with_check_character(first14)?;
        let pan = Pan::parse(gstin.pan_str())?;
        let mut l = ledger(id, name, parent);
        l.bill_wise = Some(BillWiseConfig {
            default_credit_days: Some(30),
        });
        l.party = Some(PartyDetails {
            registration: RegistrationType::Regular,
            gstin: Some(gstin),
            pan: Some(pan),
            state: Some(state),
            address: address("Fictional Lane", state),
        });
        Ok(l)
    };
    Ok(vec![
        party(
            LEDGER_SHREE_GANESH,
            "Shree Ganesh Stores",
            G_DEBTORS,
            "27FGHIJ5678K1Z",
            27,
        )?,
        party(
            LEDGER_COASTAL,
            "Coastal Supplies",
            G_CREDITORS,
            "29PQRST3456U1Z",
            29,
        )?,
    ])
}

fn voucher_type(
    id: VoucherTypeId,
    name: &str,
    base: BaseVoucherKind,
    abbreviation: &str,
    prefix: &str,
) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: abbreviation.to_owned(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: true,
            prefix: prefix.to_owned(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn voucher_types() -> Vec<VoucherType> {
    use BaseVoucherKind as B;
    vec![
        voucher_type(T_JOURNAL, "Journal", B::Journal, "Jrnl", "JV/25-26/"),
        voucher_type(T_PURCHASE, "Purchase", B::Purchase, "Purc", "PUR/25-26/"),
        voucher_type(T_SALES, "Sales", B::Sales, "Sale", "INV/25-26/"),
        voucher_type(T_RECEIPT, "Receipt", B::Receipt, "Rcpt", "RCT/25-26/"),
        voucher_type(T_PAYMENT, "Payment", B::Payment, "Pymt", "PAY/25-26/"),
        voucher_type(T_CONTRA, "Contra", B::Contra, "Cntr", "CON/25-26/"),
        voucher_type(
            T_CREDIT_NOTE,
            "Credit Note",
            B::CreditNote,
            "CrNt",
            "CN/25-26/",
        ),
    ]
}

fn unit_master(id: UnitId, symbol: &str, formal_name: &str) -> Unit {
    Unit {
        id,
        symbol: symbol.to_owned(),
        formal_name: formal_name.to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    }
}

fn units() -> Vec<Unit> {
    vec![
        unit_master(UNIT_PCS, "pcs", "Pieces"),
        unit_master(UNIT_BOX, "box", "Box"),
    ]
}

fn godowns() -> Vec<Godown> {
    let godown = |id: GodownId, name: &str| Godown {
        id,
        name: name.to_owned(),
        parent: None,
        tally: None,
    };
    vec![godown(GODOWN_MAIN, "Main"), godown(GODOWN_PUNE, "Pune")]
}

fn stock_item(id: StockItemId, name: &str, gst_percent: Option<i64>) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(StockGroupId(1)),
        unit: UNIT_PCS,
        alt_unit: None,
        hsn: None,
        gst_history: gst_percent
            .map(|p| GstHistory {
                effective_from: date!(2025 - 04 - 01),
                taxability: Taxability::Taxable,
                rate_percent: Some(rate(p)),
            })
            .into_iter()
            .collect(),
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn stock_items() -> Vec<StockItem> {
    let opening = |pieces: i64, rupees_each: i64| OpeningStock {
        godown: GODOWN_MAIN,
        batch: None,
        qty: qty(pieces),
        rate: rate(rupees_each),
    };
    let mut widget_a = stock_item(ITEM_WIDGET_A, "Widget A", Some(18));
    widget_a.opening = vec![opening(200, 60)];
    let mut widget_b = stock_item(ITEM_WIDGET_B, "Widget B", Some(5));
    widget_b.alt_unit = Some(AltUnit {
        unit: UNIT_BOX,
        base_per_alt: Decimal::from(12),
    });
    let mut r3 = stock_item(ITEM_R3, "Item R3", None);
    r3.opening = vec![opening(100, 10)];
    vec![widget_a, widget_b, r3]
}

// ---------------------------------------------------------------- voucher building blocks

fn base(
    id: u64,
    type_id: VoucherTypeId,
    number: &str,
    on: Date,
    party: Option<LedgerId>,
    narration: &str,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: Some(number.to_owned()),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: narration.to_owned(),
        ledger_lines: Vec::new(),
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn plain(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn billed(ledger: LedgerId, amount: SignedAmount, kind: BillRefKind, name: &str) -> LedgerLine {
    let mut l = plain(ledger, amount);
    l.bills = vec![BillAllocation {
        kind,
        name: name.to_owned(),
        due_date: None,
        credit_days: None,
        amount,
    }];
    l
}

fn on_bank(
    ledger: LedgerId,
    amount: SignedAmount,
    how: InstrumentKind,
    number: &str,
    on: Date,
) -> LedgerLine {
    let mut l = plain(ledger, amount);
    l.bank = Some(BankAllocation {
        instrument: how,
        instrument_number: Some(number.to_owned()),
        instrument_date: Some(on),
        bank_date: None,
        favouring: None,
    });
    l
}

fn stock_line(
    item: StockItemId,
    unit: UnitId,
    quantity: i64,
    unit_rate: i64,
    godown: GodownId,
    ledger: LedgerId,
) -> InventoryLine {
    let amount = rupees(quantity * unit_rate);
    InventoryLine {
        item,
        qty: qty(quantity),
        unit,
        rate: rate(unit_rate),
        discount_percent: None,
        amount,
        ledger: Some(ledger),
        godowns: vec![GodownAllocation {
            godown,
            batch: None,
            qty: qty(quantity),
            amount,
        }],
    }
}

/// Which way an item voucher moves money and stock.
#[derive(Clone, Copy)]
enum Side {
    Buy,
    Sell,
}

/// A plain item voucher without GST lines.
struct Trade {
    id: u64,
    side: Side,
    number: &'static str,
    date: Date,
    item: StockItemId,
    unit: UnitId,
    quantity: i64,
    unit_rate: i64,
    godown: GodownId,
    bill: &'static str,
}

fn trade(t: &Trade) -> Voucher {
    let amount = t.quantity * t.unit_rate;
    let (type_id, party, ledger, lines) = match t.side {
        Side::Buy => (
            T_PURCHASE,
            LEDGER_COASTAL,
            LEDGER_PURCHASE,
            vec![
                plain(LEDGER_PURCHASE, dr(amount)),
                billed(LEDGER_COASTAL, cr(amount), BillRefKind::New, t.bill),
            ],
        ),
        Side::Sell => (
            T_SALES,
            LEDGER_SHREE_GANESH,
            LEDGER_SALES_LOCAL,
            vec![
                billed(LEDGER_SHREE_GANESH, dr(amount), BillRefKind::New, t.bill),
                plain(LEDGER_SALES_LOCAL, cr(amount)),
            ],
        ),
    };
    let mut v = base(
        t.id,
        type_id,
        t.number,
        t.date,
        Some(party),
        "Fixture trade",
    );
    v.mode = VoucherMode::ItemInvoice;
    v.reference = match t.side {
        Side::Buy => Some(t.bill.to_owned()),
        Side::Sell => None,
    };
    v.ledger_lines = lines;
    v.inventory_lines = vec![stock_line(
        t.item,
        t.unit,
        t.quantity,
        t.unit_rate,
        t.godown,
        ledger,
    )];
    v
}

// ---------------------------------------------------------------- the 20 vouchers

fn vouchers() -> Result<Vec<Voucher>, FixtureError> {
    let mut all = stock_vouchers();
    all.extend(gst_sales()?);
    all.extend(cash_and_bank_vouchers());
    all.extend(special_vouchers());
    all.sort_by_key(|v| v.id);
    Ok(all)
}

/// V2, V3, V4, V5, V6 and V10: purchases and sales of stock without GST lines.
///
/// R3 movements (README): purchase 50 @ 13 on 10 Apr, sale 120 on 15 Apr, purchase 40 @ 15 on
/// 20 May, sale 30 on 31 May. Widget B: purchase 10 boxes @ 600, sale 5 boxes @ 900.
fn stock_vouchers() -> Vec<Voucher> {
    let buy = |id: u64, number: &'static str, on: Date, bill: &'static str| Trade {
        id,
        side: Side::Buy,
        number,
        date: on,
        item: ITEM_R3,
        unit: UNIT_PCS,
        quantity: 0,
        unit_rate: 0,
        godown: GODOWN_MAIN,
        bill,
    };
    let sell = |id: u64, number: &'static str, on: Date| Trade {
        side: Side::Sell,
        bill: number,
        ..buy(id, number, on, number)
    };
    let with = |t: Trade, item: StockItemId, unit: UnitId, quantity: i64, unit_rate: i64| Trade {
        item,
        unit,
        quantity,
        unit_rate,
        ..t
    };
    let r3 =
        |t: Trade, quantity: i64, unit_rate: i64| with(t, ITEM_R3, UNIT_PCS, quantity, unit_rate);
    let in_pune = |t: Trade| Trade {
        godown: GODOWN_PUNE,
        ..t
    };
    let widget_b = |t: Trade, boxes: i64, rate_each: i64| {
        in_pune(with(t, ITEM_WIDGET_B, UNIT_BOX, boxes, rate_each))
    };
    [
        r3(
            buy(2, "PUR/25-26/0001", date!(2025 - 04 - 10), "CS/2025/041"),
            50,
            13,
        ),
        r3(sell(3, "INV/25-26/0001", date!(2025 - 04 - 15)), 120, 20),
        r3(
            buy(4, "PUR/25-26/0002", date!(2025 - 05 - 20), "CS/2025/058"),
            40,
            15,
        ),
        r3(sell(5, "INV/25-26/0002", date!(2025 - 05 - 31)), 30, 20),
        widget_b(
            buy(6, "PUR/25-26/0003", date!(2025 - 06 - 05), "CS/2025/071"),
            10,
            600,
        ),
        widget_b(sell(10, "INV/25-26/0005", date!(2025 - 07 - 10)), 5, 900),
    ]
    .iter()
    .map(trade)
    .collect()
}

/// V7 (intra-state) and V8 (inter-state): Widget A, 100 pcs at 100, taxable value 10,000 at 18%.
///
/// V8 is booked to a party registered in state 27 but its place of supply is forced to 29 (ship-to
/// in Karnataka, IGST Act s.10(1)(a): supply involving movement of goods is placed where the
/// movement ends). An engine that derives place of supply from the party master alone would give
/// CGST+SGST here; G-card reviewers must treat the voucher's own `place_of_supply` as authoritative.
fn gst_sales() -> Result<Vec<Voucher>, FixtureError> {
    // CGST Act s.9 / IGST Act s.5 at 18%: 10,000 x 18% = 1,800, split 900 + 900 within the state.
    let intra = gst_sale(
        7,
        "INV/25-26/0003",
        date!(2025 - 06 - 15),
        LEDGER_SALES_LOCAL,
        &[(LEDGER_CGST, 900), (LEDGER_SGST, 900)],
        StateCode::new(27)?,
    );
    let inter = gst_sale(
        8,
        "INV/25-26/0004",
        date!(2025 - 06 - 20),
        LEDGER_SALES_INTERSTATE,
        &[(LEDGER_IGST, 1_800)],
        StateCode::new(29)?,
    );
    Ok(vec![intra, inter])
}

fn gst_sale(
    id: u64,
    number: &str,
    on: Date,
    sales: LedgerId,
    taxes: &[(LedgerId, i64)],
    place_of_supply: StateCode,
) -> Voucher {
    let taxable = 10_000;
    let tax_total: i64 = taxes.iter().map(|(_, t)| t).sum();
    let mut lines = vec![
        billed(
            LEDGER_SHREE_GANESH,
            dr(taxable + tax_total),
            BillRefKind::New,
            number,
        ),
        plain(sales, cr(taxable)),
    ];
    for (ledger, tax) in taxes {
        lines.push(plain(*ledger, cr(*tax)));
    }
    let mut v = base(
        id,
        T_SALES,
        number,
        on,
        Some(LEDGER_SHREE_GANESH),
        "Fixture GST sale",
    );
    v.mode = VoucherMode::ItemInvoice;
    v.place_of_supply = Some(place_of_supply);
    v.ledger_lines = lines;
    v.inventory_lines = vec![stock_line(
        ITEM_WIDGET_A,
        UNIT_PCS,
        100,
        100,
        GODOWN_MAIN,
        sales,
    )];
    v
}

/// V1, V9, V11, V12, V13, V19 and V20: opening capital, receipts, payments and a contra.
fn cash_and_bank_vouchers() -> Vec<Voucher> {
    let mut opening = base(
        1,
        T_JOURNAL,
        "JV/25-26/0001",
        date!(2025 - 04 - 01),
        None,
        "Opening capital introduced in cash",
    );
    opening.ledger_lines = vec![
        plain(LEDGER_CASH, dr(100_000)),
        plain(LEDGER_CAPITAL, cr(100_000)),
    ];

    let mut cash_receipt = base(
        13,
        T_RECEIPT,
        "RCT/25-26/0002",
        date!(2025 - 08 - 08),
        Some(LEDGER_SHREE_GANESH),
        "Cash received on account",
    );
    cash_receipt.ledger_lines = vec![
        plain(LEDGER_CASH, dr(2_832)),
        billed(
            LEDGER_SHREE_GANESH,
            cr(2_832),
            BillRefKind::OnAccount,
            "On account",
        ),
    ];

    vec![
        opening,
        receipt_against_v7(),
        rent_by_cheque(),
        cash_deposit(),
        cash_receipt,
        supplier_payment(),
        year_end_rent(),
    ]
}

/// V9: the customer pays the whole of the bill that V7 opened.
fn receipt_against_v7() -> Voucher {
    let on = date!(2025 - 06 - 25);
    let mut v = base(
        9,
        T_RECEIPT,
        "RCT/25-26/0001",
        on,
        Some(LEDGER_SHREE_GANESH),
        "Against INV/25-26/0003",
    );
    v.ledger_lines = vec![
        on_bank(
            LEDGER_HDFC,
            dr(11_800),
            InstrumentKind::Neft,
            "NEFT0001",
            on,
        ),
        billed(
            LEDGER_SHREE_GANESH,
            cr(11_800),
            BillRefKind::Against,
            "INV/25-26/0003",
        ),
    ];
    v
}

/// V11: a payment by cheque.
fn rent_by_cheque() -> Voucher {
    let on = date!(2025 - 07 - 31);
    let mut v = base(
        11,
        T_PAYMENT,
        "PAY/25-26/0001",
        on,
        None,
        "July rent by cheque",
    );
    v.ledger_lines = vec![
        plain(LEDGER_RENT, dr(12_000)),
        on_bank(
            LEDGER_HDFC,
            cr(12_000),
            InstrumentKind::Cheque,
            "000101",
            on,
        ),
    ];
    v
}

/// V12: cash deposited in the bank.
fn cash_deposit() -> Voucher {
    let on = date!(2025 - 08 - 04);
    let mut v = base(
        12,
        T_CONTRA,
        "CON/25-26/0001",
        on,
        None,
        "Cash deposited in the bank",
    );
    v.ledger_lines = vec![
        on_bank(LEDGER_HDFC, dr(5_000), InstrumentKind::Other, "DEP0001", on),
        plain(LEDGER_CASH, cr(5_000)),
    ];
    v
}

/// V19: pays the supplier bill opened by V2 in full.
fn supplier_payment() -> Voucher {
    let on = date!(2025 - 10 - 15);
    let mut v = base(
        19,
        T_PAYMENT,
        "PAY/25-26/0003",
        on,
        Some(LEDGER_COASTAL),
        "Against CS/2025/041",
    );
    v.ledger_lines = vec![
        billed(LEDGER_COASTAL, dr(650), BillRefKind::Against, "CS/2025/041"),
        on_bank(LEDGER_HDFC, cr(650), InstrumentKind::Neft, "NEFT0002", on),
    ];
    v
}

/// V20: the last voucher of the year.
fn year_end_rent() -> Voucher {
    let on = date!(2026 - 03 - 31);
    let mut v = base(
        20,
        T_PAYMENT,
        "PAY/25-26/0004",
        on,
        None,
        "March rent by transfer",
    );
    v.ledger_lines = vec![
        plain(LEDGER_RENT, dr(12_000)),
        on_bank(
            LEDGER_HDFC,
            cr(12_000),
            InstrumentKind::Neft,
            "NEFT0003",
            on,
        ),
    ];
    v
}

/// V14 credit note, V15 cancelled, V16 optional, V17 and V18 the rate-change pair.
fn special_vouchers() -> Vec<Voucher> {
    let mut credit_note = base(
        14,
        T_CREDIT_NOTE,
        "CN/25-26/0001",
        date!(2025 - 08 - 12),
        Some(LEDGER_SHREE_GANESH),
        "Price difference on INV/25-26/0005",
    );
    credit_note.mode = VoucherMode::AccountingInvoice;
    credit_note.ledger_lines = vec![
        plain(LEDGER_SALES_LOCAL, dr(1_000)),
        billed(
            LEDGER_SHREE_GANESH,
            cr(1_000),
            BillRefKind::Against,
            "INV/25-26/0005",
        ),
    ];

    // A cancelled voucher keeps its number and has no amounts.
    let mut cancelled = base(
        15,
        T_SALES,
        "INV/25-26/0006",
        date!(2025 - 08 - 20),
        None,
        "Cancelled: issued in error",
    );
    cancelled.status = VoucherStatus::Cancelled;

    // An optional voucher stays out of the books. It touches Cash on purpose, so that a ledger
    // total that wrongly includes optional vouchers is caught.
    let mut optional = base(
        16,
        T_PAYMENT,
        "PAY/25-26/0002",
        date!(2025 - 08 - 25),
        None,
        "Optional: possible rent revision",
    );
    optional.status = VoucherStatus::Optional;
    optional.ledger_lines = vec![plain(LEDGER_RENT, dr(1_000)), plain(LEDGER_CASH, cr(1_000))];

    vec![
        credit_note,
        cancelled,
        optional,
        rate_change_sale(17, "INV/25-26/0007", Some(date!(2025 - 09 - 21))),
        rate_change_sale(18, "INV/25-26/0008", None),
    ]
}

/// A sale invoiced on 23 Sep 2025 (a day after the 22 Sep 2025 GST rate change), without tax
/// lines because the rate comes from the rule pack. `supply_date` is the date of supply when it
/// differs from the invoice date.
fn rate_change_sale(id: u64, number: &str, supply_date: Option<Date>) -> Voucher {
    let mut v = base(
        id,
        T_SALES,
        number,
        date!(2025 - 09 - 23),
        Some(LEDGER_SHREE_GANESH),
        "Rate-change fixture: tax lines left to the rule pack",
    );
    v.mode = VoucherMode::AccountingInvoice;
    v.supply_date = supply_date;
    v.ledger_lines = vec![
        billed(LEDGER_SHREE_GANESH, dr(10_000), BillRefKind::New, number),
        plain(LEDGER_SALES_LOCAL, cr(10_000)),
    ];
    v
}

// ---------------------------------------------------------------- derived read model

/// One posting per ledger line of every voucher that has lines, sorted by `(date, voucher)`.
fn postings(vouchers: &[Voucher]) -> Vec<Posting> {
    let mut out: Vec<Posting> = vouchers
        .iter()
        .flat_map(|v| {
            v.ledger_lines.iter().map(move |l| Posting {
                voucher: v.id,
                date: v.date,
                ledger: l.ledger,
                amount: l.amount,
                status: v.status,
                type_id: v.type_id,
            })
        })
        .collect();
    out.sort_by_key(|p| (p.date, p.voucher));
    out
}

#[derive(Clone, Copy)]
enum StockDirection {
    In,
    Out,
}

/// Direction of stock for a voucher kind. Listed in full so that a new kind must be decided here.
fn direction(kind: BaseVoucherKind) -> Option<StockDirection> {
    match kind {
        BaseVoucherKind::Purchase
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::CreditNote => Some(StockDirection::In),
        BaseVoucherKind::Sales
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::DebitNote => Some(StockDirection::Out),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => None,
    }
}

/// Base units per unit of the line, or `None` when the item cannot convert.
fn unit_factor(item: &StockItem, unit: UnitId) -> Option<Decimal> {
    if unit == item.unit {
        Some(Decimal::ONE)
    } else {
        item.alt_unit
            .filter(|alt| alt.unit == unit)
            .map(|alt| alt.base_per_alt)
    }
}

/// Stock moves of the regular vouchers, one per godown allocation, in base units.
fn stock_moves(vouchers: &[Voucher], masters: &Masters) -> Result<Vec<StockMove>, FixtureError> {
    let mut moves = Vec::new();
    for v in vouchers
        .iter()
        .filter(|v| v.status == VoucherStatus::Regular)
    {
        for line in &v.inventory_lines {
            moves.extend(line_moves(v, line, masters)?);
        }
    }
    moves.sort_by_key(|m| (m.date, m.voucher));
    Ok(moves)
}

fn line_moves(
    v: &Voucher,
    line: &InventoryLine,
    masters: &Masters,
) -> Result<Vec<StockMove>, FixtureError> {
    let voucher = v.id.0;
    let item = masters
        .stock_items
        .get(&line.item)
        .ok_or(FixtureError::MissingItem { voucher })?;
    let kind = masters
        .voucher_types
        .get(&v.type_id)
        .ok_or(FixtureError::MissingVoucherType { voucher })?
        .base;
    let way = direction(kind).ok_or(FixtureError::NoStockDirection { voucher })?;
    let factor = unit_factor(item, line.unit).ok_or(FixtureError::Unit { voucher })?;
    let mut out = Vec::new();
    for alloc in &line.godowns {
        let base_qty = Qty(alloc
            .qty
            .0
            .checked_mul(factor)
            .ok_or(FixtureError::Quantity { voucher })?);
        let (qty_in, qty_out) = match way {
            StockDirection::In => (base_qty, Qty::ZERO),
            StockDirection::Out => (Qty::ZERO, base_qty),
        };
        out.push(StockMove {
            voucher: v.id,
            date: v.date,
            item: line.item,
            godown: alloc.godown,
            batch: alloc.batch,
            qty_in,
            qty_out,
            value: alloc.amount,
            status: ledgerly_model::VoucherStatus::Regular,
        });
    }
    Ok(out)
}
