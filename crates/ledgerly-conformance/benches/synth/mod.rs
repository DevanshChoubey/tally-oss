//! Synthetic books for the performance benchmarks (card H3, spec section 8).
//!
//! [`synth_books`] builds a [`Books`] of any size with no randomness, so two runs of the same
//! [`Shape`] give equal books and the figures in every report have a closed form that a test can
//! derive by hand. Voucher number `i` (0-based) is built like this:
//!
//! - kind: `i % 4` is Sales, Purchase, Receipt, Payment;
//! - amount in paise: `((i % 9973) + 100) * 100 + (i % 100)`;
//! - date: 1 April 2025 plus `i % 365` days, so every voucher falls in financial year 2025-26;
//! - party: debtor `i % debtors` for Sales and Receipt, creditor `i % creditors` for Purchase and
//!   Payment;
//! - lines: Sales debits the debtor and credits Sales; Purchase debits Purchases and credits the
//!   creditor; Receipt debits Bank and credits the debtor; Payment debits the creditor and credits
//!   Bank;
//! - number: the type's prefix and the counter `i / 4 + 1`, zero-padded to 7 digits.
//!
//! The module is shared by the benchmark and by `tests/h3_synth.rs`, so it uses `pub(crate)`
//! items and allows items that one of the two does not use.
#![allow(
    dead_code,
    reason = "shared by the benchmark and the generator tests, each uses a subset"
)]
#![allow(
    clippy::missing_errors_doc,
    reason = "benchmark support code, errors are named by SynthError"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, FinancialYear, GroupId, LedgerId, MoneyError, Paise, PeriodError,
    SignedAmount, StateCode, StateCodeError, VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, LedgerLine,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, Posting, ReservedGroup,
    ReservedLedger, RestartPeriod, Scenario, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_reports::{ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use thiserror::Error;
use time::Duration;

/// Days in the synthetic financial year (1 April 2025 to 31 March 2026).
const DAYS: u64 = 365;
/// Distinct amount steps; see the module doc.
const AMOUNT_STEPS: u64 = 9973;
/// Number of voucher kinds generated.
const KINDS: u64 = 4;
/// Zero-padded width of voucher counters.
const NUMBER_WIDTH: u8 = 7;

/// Sales ledger id.
pub(crate) const LEDGER_SALES: LedgerId = LedgerId(1);
/// Purchases ledger id.
pub(crate) const LEDGER_PURCHASES: LedgerId = LedgerId(2);
/// Bank ledger id.
pub(crate) const LEDGER_BANK: LedgerId = LedgerId(3);
/// Cash ledger id (reserved, no postings).
pub(crate) const LEDGER_CASH: LedgerId = LedgerId(4);
/// Profit and loss ledger id (reserved, no postings).
pub(crate) const LEDGER_PROFIT_LOSS: LedgerId = LedgerId(5);
/// Id of the first debtor; debtors are numbered upward from here.
pub(crate) const DEBTOR_BASE: u64 = 1000;

/// Sales voucher type id.
pub(crate) const TYPE_SALES: VoucherTypeId = VoucherTypeId(1);
/// Purchase voucher type id.
pub(crate) const TYPE_PURCHASE: VoucherTypeId = VoucherTypeId(2);
/// Receipt voucher type id.
pub(crate) const TYPE_RECEIPT: VoucherTypeId = VoucherTypeId(3);
/// Payment voucher type id.
pub(crate) const TYPE_PAYMENT: VoucherTypeId = VoucherTypeId(4);
/// Journal voucher type id (unnumbered, used only for the post benchmark).
pub(crate) const TYPE_JOURNAL: VoucherTypeId = VoucherTypeId(5);

/// Why the generator failed. The generator never panics, so a bad shape or an arithmetic overflow
/// comes back as a value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum SynthError {
    /// A count in the shape is zero.
    #[error("{0} must be at least 1")]
    Shape(&'static str),
    /// A date fell outside the calendar.
    #[error("date out of range")]
    Date,
    /// A count did not fit its integer type.
    #[error("count out of range")]
    Count,
    /// A reserved group is missing from the seed.
    #[error("reserved group missing from the seed")]
    MissingGroup,
    /// Money arithmetic failed.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// A period could not be built.
    #[error(transparent)]
    Period(#[from] PeriodError),
    /// A state code was rejected.
    #[error(transparent)]
    State(#[from] StateCodeError),
}

/// The size of a synthetic company.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    /// Number of vouchers.
    pub(crate) vouchers: u64,
    /// Number of debtor ledgers, at least 1.
    pub(crate) debtors: u64,
    /// Number of creditor ledgers, at least 1.
    pub(crate) creditors: u64,
}

impl Shape {
    /// The spec section 8 company: one million vouchers over a few thousand parties.
    pub(crate) const fn spec_target() -> Self {
        Shape {
            vouchers: 1_000_000,
            debtors: 1_000,
            creditors: 1_000,
        }
    }

    /// A small company for smoke runs: `vouchers` vouchers over 10 debtors and 10 creditors.
    pub(crate) const fn small(vouchers: u64) -> Self {
        Shape {
            vouchers,
            debtors: 10,
            creditors: 10,
        }
    }
}

/// The four generated voucher kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sales,
    Purchase,
    Receipt,
    Payment,
}

impl Kind {
    fn of(index: u64) -> Self {
        match index % KINDS {
            0 => Kind::Sales,
            1 => Kind::Purchase,
            2 => Kind::Receipt,
            _ => Kind::Payment,
        }
    }

    fn type_id(self) -> VoucherTypeId {
        match self {
            Kind::Sales => TYPE_SALES,
            Kind::Purchase => TYPE_PURCHASE,
            Kind::Receipt => TYPE_RECEIPT,
            Kind::Payment => TYPE_PAYMENT,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Kind::Sales => "S/",
            Kind::Purchase => "P/",
            Kind::Receipt => "RC/",
            Kind::Payment => "PM/",
        }
    }
}

fn check_shape(shape: Shape) -> Result<(), SynthError> {
    if shape.vouchers == 0 {
        return Err(SynthError::Shape("vouchers"));
    }
    if shape.debtors == 0 {
        return Err(SynthError::Shape("debtors"));
    }
    if shape.creditors == 0 {
        return Err(SynthError::Shape("creditors"));
    }
    Ok(())
}

/// First day of the synthetic financial year.
pub(crate) fn fy_start() -> Result<Date, SynthError> {
    Ok(FinancialYear::april(2025)?.start)
}

/// Last day of the synthetic financial year.
pub(crate) fn fy_end() -> Result<Date, SynthError> {
    Ok(FinancialYear::april(2025)?.end()?)
}

/// Amount of voucher `index` in paise (see the module doc).
pub(crate) fn amount_paise(index: u64) -> Result<Paise, SynthError> {
    let rupees = (index % AMOUNT_STEPS).saturating_add(100);
    let paise = rupees.saturating_mul(100).saturating_add(index % 100);
    i64::try_from(paise)
        .map(Paise::new)
        .map_err(|_| SynthError::Count)
}

/// The 365 dates of the year, in order.
fn year_dates() -> Result<Vec<Date>, SynthError> {
    let start = fy_start()?;
    (0..DAYS)
        .map(|d| {
            let days = i64::try_from(d).map_err(|_| SynthError::Count)?;
            start
                .checked_add(Duration::days(days))
                .ok_or(SynthError::Date)
        })
        .collect()
}

fn debtor_id(shape: Shape, index: u64) -> LedgerId {
    LedgerId(DEBTOR_BASE.saturating_add(index % shape.debtors))
}

fn creditor_id(shape: Shape, index: u64) -> LedgerId {
    let base = DEBTOR_BASE.saturating_add(shape.debtors);
    LedgerId(base.saturating_add(index % shape.creditors))
}

fn company() -> Result<Company, SynthError> {
    let start = fy_start()?;
    Ok(Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Synthetic Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27)?,
        pan: None,
        registrations: Vec::new(),
        fy_start: start,
        books_begin: start,
        entity_kind: EntityKind::Proprietorship,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    })
}

fn group_id(groups: &[Group], which: ReservedGroup) -> Result<GroupId, SynthError> {
    groups
        .iter()
        .find(|g| g.reserved == Some(which))
        .map(|g| g.id)
        .ok_or(SynthError::MissingGroup)
}

fn ledger(id: LedgerId, name: String, group: GroupId, reserved: Option<ReservedLedger>) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name,
        aliases: Vec::new(),
        group,
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved,
        tally: None,
    }
}

fn ledgers(shape: Shape, groups: &[Group]) -> Result<BTreeMap<LedgerId, Ledger>, SynthError> {
    let mut out = BTreeMap::new();
    let mut add = |l: Ledger| {
        out.insert(l.id, l);
    };
    let sales = group_id(groups, ReservedGroup::SalesAccounts)?;
    let purchases = group_id(groups, ReservedGroup::PurchaseAccounts)?;
    let bank = group_id(groups, ReservedGroup::BankAccounts)?;
    let cash = group_id(groups, ReservedGroup::CashInHand)?;
    let reserves = group_id(groups, ReservedGroup::ReservesAndSurplus)?;
    let debtors = group_id(groups, ReservedGroup::SundryDebtors)?;
    let creditors = group_id(groups, ReservedGroup::SundryCreditors)?;
    add(ledger(LEDGER_SALES, "Sales".to_owned(), sales, None));
    add(ledger(
        LEDGER_PURCHASES,
        "Purchases".to_owned(),
        purchases,
        None,
    ));
    add(ledger(LEDGER_BANK, "Bank".to_owned(), bank, None));
    add(ledger(
        LEDGER_CASH,
        "Cash".to_owned(),
        cash,
        Some(ReservedLedger::Cash),
    ));
    add(ledger(
        LEDGER_PROFIT_LOSS,
        "Profit & Loss A/c".to_owned(),
        reserves,
        Some(ReservedLedger::ProfitAndLoss),
    ));
    for j in 0..shape.debtors {
        let id = LedgerId(DEBTOR_BASE.saturating_add(j));
        add(ledger(id, format!("Debtor {j}"), debtors, None));
    }
    for j in 0..shape.creditors {
        let id = creditor_id(shape, j);
        add(ledger(id, format!("Creditor {j}"), creditors, None));
    }
    Ok(out)
}

fn voucher_type(
    id: VoucherTypeId,
    name: &str,
    base: BaseVoucherKind,
    method: NumberingMethod,
    prefix: &str,
) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method,
            prevent_renumbering: false,
            prefix: prefix.to_owned(),
            suffix: String::new(),
            width: NUMBER_WIDTH,
            restart: RestartPeriod::Never,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn voucher_types() -> BTreeMap<VoucherTypeId, VoucherType> {
    let auto = NumberingMethod::Automatic;
    [
        voucher_type(TYPE_SALES, "Sales", BaseVoucherKind::Sales, auto, "S/"),
        voucher_type(
            TYPE_PURCHASE,
            "Purchase",
            BaseVoucherKind::Purchase,
            auto,
            "P/",
        ),
        voucher_type(
            TYPE_RECEIPT,
            "Receipt",
            BaseVoucherKind::Receipt,
            auto,
            "RC/",
        ),
        voucher_type(
            TYPE_PAYMENT,
            "Payment",
            BaseVoucherKind::Payment,
            auto,
            "PM/",
        ),
        voucher_type(
            TYPE_JOURNAL,
            "Journal",
            BaseVoucherKind::Journal,
            NumberingMethod::None,
            "",
        ),
    ]
    .into_iter()
    .map(|t| (t.id, t))
    .collect()
}

/// Two-line voucher: debit `dr`, credit `cr`, the same amount.
fn voucher(
    id: u64,
    type_id: VoucherTypeId,
    date: Date,
    party: Option<LedgerId>,
    lines: (LedgerId, LedgerId, Paise),
) -> Voucher {
    let (dr, cr, amount) = lines;
    let line = |ledger, side: SignedAmount| LedgerLine {
        ineligible_itc: None,
        ledger,
        amount: side,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![
            line(dr, SignedAmount::dr(amount)),
            line(cr, SignedAmount::cr(amount)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Voucher number `index` (0-based) as the generator builds it.
fn generated_voucher(shape: Shape, index: u64, dates: &[Date]) -> Result<Voucher, SynthError> {
    let kind = Kind::of(index);
    let amount = amount_paise(index)?;
    let day = usize::try_from(index % DAYS).map_err(|_| SynthError::Count)?;
    let date = *dates.get(day).ok_or(SynthError::Date)?;
    let debtor = debtor_id(shape, index);
    let creditor = creditor_id(shape, index);
    let (dr, cr, party) = match kind {
        Kind::Sales => (debtor, LEDGER_SALES, debtor),
        Kind::Purchase => (LEDGER_PURCHASES, creditor, creditor),
        Kind::Receipt => (LEDGER_BANK, debtor, debtor),
        Kind::Payment => (creditor, LEDGER_BANK, creditor),
    };
    let mut v = voucher(
        index.saturating_add(1),
        kind.type_id(),
        date,
        Some(party),
        (dr, cr, amount),
    );
    let counter = (index / KINDS).saturating_add(1);
    v.number = Some(format!(
        "{}{:0>width$}",
        kind.prefix(),
        counter,
        width = usize::from(NUMBER_WIDTH)
    ));
    Ok(v)
}

/// The ledger postings of a voucher, as the store derives them.
fn postings_of(v: &Voucher) -> impl Iterator<Item = Posting> + '_ {
    v.ledger_lines.iter().map(|l| Posting {
        voucher: v.id,
        date: v.date,
        ledger: l.ledger,
        amount: l.amount,
        status: v.status,
        type_id: v.type_id,
    })
}

/// Builds the synthetic company described in the module doc.
pub(crate) fn synth_books(shape: Shape) -> Result<Books, SynthError> {
    check_shape(shape)?;
    let groups = reserved_groups();
    let masters = Masters {
        ledgers: ledgers(shape, &groups)?,
        voucher_types: voucher_types(),
        groups: groups.into_iter().map(|g| (g.id, g)).collect(),
        ..Masters::default()
    };
    let dates = year_dates()?;
    let mut vouchers = BTreeMap::new();
    let mut postings = Vec::new();
    for index in 0..shape.vouchers {
        let v = generated_voucher(shape, index, &dates)?;
        postings.extend(postings_of(&v));
        vouchers.insert(v.id, v);
    }
    // `Books::postings` is sorted by (date, voucher).
    postings.sort_by_key(|p| (p.date, p.voucher));
    Ok(Books {
        company: company()?,
        masters,
        vouchers,
        postings,
        stock_moves: Vec::new(),
    })
}

/// A valid voucher whose id is not in the books built from `shape`, for the post benchmarks.
///
/// With `numbered` it is a Receipt of the automatically numbered type (so posting it allocates a
/// number by scanning the type's vouchers); otherwise a Journal of an unnumbered type.
pub(crate) fn fresh_voucher(shape: Shape, numbered: bool) -> Result<Voucher, SynthError> {
    check_shape(shape)?;
    let date = year_dates()?.get(100).copied().ok_or(SynthError::Date)?;
    let id = shape.vouchers.saturating_add(1);
    let amount = Paise::new(123_456);
    let debtor = debtor_id(shape, 0);
    Ok(if numbered {
        voucher(
            id,
            TYPE_RECEIPT,
            date,
            Some(debtor),
            (LEDGER_BANK, debtor, amount),
        )
    } else {
        voucher(
            id,
            TYPE_JOURNAL,
            date,
            None,
            (debtor, creditor_id(shape, 0), amount),
        )
    })
}

/// Parameters of a full-year report: period and as-of date are the whole financial year.
pub(crate) fn report_params(kind: ReportKind, detail: Detail) -> Result<ReportParams, SynthError> {
    let year = FinancialYear::april(2025)?;
    Ok(ReportParams {
        kind,
        period: year.period()?,
        as_of: year.end()?,
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    })
}
