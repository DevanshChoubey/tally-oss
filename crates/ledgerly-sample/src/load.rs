//! Posting the seed through the API, in the order a real set of books would be built: masters,
//! the invented April to October history, the mock's November vouchers, the bank statement, and
//! last the period lock.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr as _;

use ledgerly_api::{Api, Command, ImsAction};
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Outcome, Paise, Period, Qty,
    Rate, RoundingRule, SignedAmount, StateCode, StockItemId, UnitId, UserId, VoucherId,
    round_rupees_to_paise,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, BillAllocation, BillRefKind,
    BillWiseConfig, Company, EntityKind, Features, GodownAllocation, GstClassification, GstHistory,
    GstRegistration, InstrumentKind, InventoryLine, Ledger, LedgerLine, NegativeStockPolicy,
    PartyDetails, RegistrationType, ReservedGroup, ReservedLedger, StockItem, TaxDetails,
    TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus,
};
use ledgerly_store::Actor;
use time::UtcOffset;
use time::format_description::well_known::Iso8601;

use crate::SampleError;
use crate::seed::{
    EntitySeed, GroupSeed, ItemLineSeed, KindSeed, LedgerSeed, LineSeed, ModeSeed,
    RegistrationSeed, SectionSeed, Seed, TaxSeed, TaxabilitySeed, VoucherSeed,
};

/// The statement files the seed can name, embedded so the loader needs no folder.
const STATEMENTS: [(&str, &str); 1] = [(
    "saptagiri-nov-2026.mt940",
    include_str!("../assets/saptagiri-nov-2026.mt940"),
)];

fn value_error(field: impl Into<String>, reason: impl Into<String>) -> SampleError {
    SampleError::Value {
        field: field.into(),
        reason: reason.into(),
    }
}

fn parse_date(field: &str, text: &str) -> Result<Date, SampleError> {
    Date::parse(text, &Iso8601::DATE).map_err(|e| value_error(field, e.to_string()))
}

fn parse_decimal(field: &str, text: &str) -> Result<Decimal, SampleError> {
    Decimal::from_str(text).map_err(|e| value_error(field, e.to_string()))
}

/// Rupees as text to paise, refusing anything with more than two decimals.
fn parse_paise(field: &str, text: &str) -> Result<Paise, SampleError> {
    let rupees = parse_decimal(field, text)?;
    let paise = round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)
        .map_err(|e| value_error(field, e.to_string()))?;
    if paise.to_rupees() == rupees {
        Ok(paise)
    } else {
        Err(value_error(field, "more than two decimals"))
    }
}

fn state_code(field: &str, code: u8) -> Result<StateCode, SampleError> {
    StateCode::new(code).map_err(|e| value_error(field, e.to_string()))
}

fn group_id(under: ReservedGroup) -> Result<GroupId, SampleError> {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(under))
        .map(|g| g.id)
        .ok_or_else(|| value_error("group", format!("no reserved group {under:?}")))
}

fn reserved(group: GroupSeed) -> ReservedGroup {
    match group {
        GroupSeed::CapitalAccount => ReservedGroup::CapitalAccount,
        GroupSeed::SecuredLoans => ReservedGroup::SecuredLoans,
        GroupSeed::SundryCreditors => ReservedGroup::SundryCreditors,
        GroupSeed::DutiesAndTaxes => ReservedGroup::DutiesAndTaxes,
        GroupSeed::Provisions => ReservedGroup::Provisions,
        GroupSeed::FixedAssets => ReservedGroup::FixedAssets,
        GroupSeed::SundryDebtors => ReservedGroup::SundryDebtors,
        GroupSeed::CashInHand => ReservedGroup::CashInHand,
        GroupSeed::BankAccounts => ReservedGroup::BankAccounts,
        GroupSeed::LoansAndAdvancesAsset => ReservedGroup::LoansAndAdvancesAsset,
        GroupSeed::SalesAccounts => ReservedGroup::SalesAccounts,
        GroupSeed::PurchaseAccounts => ReservedGroup::PurchaseAccounts,
        GroupSeed::DirectExpenses => ReservedGroup::DirectExpenses,
        GroupSeed::IndirectExpenses => ReservedGroup::IndirectExpenses,
        GroupSeed::IndirectIncomes => ReservedGroup::IndirectIncomes,
    }
}

fn entity(kind: EntitySeed) -> EntityKind {
    match kind {
        EntitySeed::Company => EntityKind::Company,
        EntitySeed::Llp => EntityKind::Llp,
        EntitySeed::Partnership => EntityKind::Partnership,
        EntitySeed::Proprietorship => EntityKind::Proprietorship,
        EntitySeed::Other => EntityKind::Other,
    }
}

fn tax_kind(kind: TaxSeed) -> TaxLedgerKind {
    match kind {
        TaxSeed::Cgst => TaxLedgerKind::Cgst,
        TaxSeed::Sgst => TaxLedgerKind::Sgst,
        TaxSeed::Igst => TaxLedgerKind::Igst,
        TaxSeed::Tds => TaxLedgerKind::Tds,
    }
}

fn voucher_kind(kind: KindSeed) -> BaseVoucherKind {
    match kind {
        KindSeed::Sales => BaseVoucherKind::Sales,
        KindSeed::Purchase => BaseVoucherKind::Purchase,
        KindSeed::Payment => BaseVoucherKind::Payment,
        KindSeed::Receipt => BaseVoucherKind::Receipt,
        KindSeed::Journal => BaseVoucherKind::Journal,
        KindSeed::Contra => BaseVoucherKind::Contra,
    }
}

/// Runs one command; anything but a value is an error naming `what`.
fn run(api: &mut Api, what: &str, command: Command) -> Result<(), SampleError> {
    match api.execute(command) {
        Ok(Outcome::Value(_)) => Ok(()),
        Ok(Outcome::Unsupported(u)) => Err(SampleError::Unsupported {
            what: format!("{what} ({})", u.what),
            reason: u.reason,
        }),
        Err(source) => Err(SampleError::Api {
            what: what.to_owned(),
            source: Box::new(source),
        }),
    }
}

/// What the loader needs to know about a ledger after it is created.
struct LedgerInfo {
    id: LedgerId,
    credit_days: Option<u32>,
    bill_wise: bool,
    /// The party's state, for a sale's place of supply.
    state: Option<StateCode>,
}

/// The ledgers by seed key.
struct Keys {
    ledgers: BTreeMap<String, LedgerInfo>,
    items: BTreeMap<String, (StockItemId, UnitId)>,
    cgst: LedgerId,
    sgst: LedgerId,
    igst: LedgerId,
    round_off: LedgerId,
    company_state: StateCode,
}

impl Keys {
    fn ledger(&self, field: &str, key: &str) -> Result<&LedgerInfo, SampleError> {
        self.ledgers
            .get(key)
            .ok_or_else(|| value_error(field, format!("no ledger with key `{key}`")))
    }
}

/// The dates and names the app shows for the sample company.
pub(crate) struct Built {
    pub(crate) api: Api,
}

/// Builds the sample company in a new file at `path`.
pub(crate) fn build(
    seed: &Seed,
    path: &Path,
    clock: Clock,
    negative_stock: NegativeStockPolicy,
    with_2b: bool,
) -> Result<Built, SampleError> {
    let mut company = company(seed)?;
    company.negative_stock = negative_stock;
    let mut api = Api::create(path, &company, clock).map_err(|source| SampleError::Api {
        what: "creating the company file".to_owned(),
        source: Box::new(source),
    })?;
    let keys = masters(seed, &mut api)?;
    number_series(&mut api)?;
    vouchers(seed, &mut api, &keys, clock)?;
    statements(seed, &mut api, &keys)?;
    let until = parse_date("lock_until", &seed.lock_until)?;
    run(&mut api, "locking the books", Command::LockPeriod(until))?;
    if with_2b {
        gstr2b_files(seed, &mut api)?;
    }
    Ok(Built { api })
}

/// The number series of the sample company's voucher types: a prefix and four digits, as its
/// vouchers' numbers read (SI/0417, PY/0318; the credit and debit notes' CN/ and DN/ are the sample's own, the mock shows none). A type not listed keeps the seeded bare counter.
const SERIES: [(BaseVoucherKind, &str); 8] = [
    (BaseVoucherKind::Sales, "SI/"),
    (BaseVoucherKind::Purchase, "PB/"),
    (BaseVoucherKind::Payment, "PY/"),
    (BaseVoucherKind::Receipt, "RC/"),
    (BaseVoucherKind::Journal, "JV/"),
    (BaseVoucherKind::Contra, "CT/"),
    (BaseVoucherKind::CreditNote, "CN/"),
    (BaseVoucherKind::DebitNote, "DN/"),
];

/// Gives the sample's voucher types their prefixes and the four-digit width.
fn number_series(api: &mut Api) -> Result<(), SampleError> {
    for (kind, prefix) in SERIES {
        let mut vt = default_voucher_types()
            .into_iter()
            .find(|t| t.base == kind)
            .ok_or_else(|| value_error("voucher type", format!("no default type for {kind:?}")))?;
        prefix.clone_into(&mut vt.numbering.prefix);
        vt.numbering.width = 4;
        run(
            api,
            "numbering a voucher type",
            Command::AlterVoucherType(vt),
        )?;
    }
    Ok(())
}

fn company(seed: &Seed) -> Result<Company, SampleError> {
    let c = &seed.company;
    let state = state_code("company state", c.state)?;
    let gstin = Gstin::parse(&c.gstin).map_err(|e| value_error("company gstin", e.to_string()))?;
    let fy_start = parse_date("company fy_start", &c.fy_start)?;
    Ok(Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: c.name.clone(),
        mailing_name: None,
        address: Address {
            lines: c.address.clone(),
            pincode: None,
            state: Some(state),
            country: String::new(),
        },
        state,
        pan: None,
        registrations: vec![GstRegistration {
            gstin,
            state,
            effective_from: fy_start,
            composition: false,
        }],
        fy_start,
        books_begin: parse_date("company books_begin", &c.books_begin)?,
        entity_kind: entity(c.entity),
        features: Features {
            gst: true,
            inventory: true,
            bill_wise: true,
            bank_reconciliation: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    })
}

fn opening(l: &LedgerSeed) -> Result<SignedAmount, SampleError> {
    let field = format!("ledger {} opening", l.key);
    match (&l.opening_dr, &l.opening_cr) {
        (Some(dr), None) => Ok(SignedAmount::dr(parse_paise(&field, dr)?)),
        (None, Some(cr)) => Ok(SignedAmount::cr(parse_paise(&field, cr)?)),
        (None, None) => Ok(SignedAmount::dr(Paise::ZERO)),
        (Some(_), Some(_)) => Err(value_error(field, "both opening_dr and opening_cr")),
    }
}

fn ledger(l: &LedgerSeed, id: LedgerId) -> Result<Ledger, SampleError> {
    let field = format!("ledger {}", l.key);
    let party = match (l.registration, l.state) {
        (Some(registration), Some(state)) => {
            let gstin = match &l.gstin {
                Some(g) => Some(Gstin::parse(g).map_err(|e| value_error(&field, e.to_string()))?),
                None => None,
            };
            Some(PartyDetails {
                registration: match registration {
                    RegistrationSeed::Regular => RegistrationType::Regular,
                    RegistrationSeed::Unregistered => RegistrationType::Unregistered,
                },
                gstin,
                pan: None,
                state: Some(state_code(&field, state)?),
                address: Address {
                    lines: l.city.iter().cloned().collect(),
                    pincode: None,
                    state: Some(state_code(&field, state)?),
                    country: String::new(),
                },
            })
        }
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(value_error(field, "registration and state go together"));
        }
    };
    let gst = match &l.gst {
        Some(g) => Some(GstClassification {
            hsn: None,
            taxability: match g.taxability {
                TaxabilitySeed::Taxable => Taxability::Taxable,
            },
            rate_percent: Some(Rate(parse_decimal(&field, &g.rate)?)),
        }),
        None => None,
    };
    Ok(Ledger {
        ineligible_itc: l.eligible_itc.map(|eligible| !eligible),
        id,
        name: l.name.clone(),
        aliases: l.aliases.clone(),
        group: group_id(reserved(l.group))?,
        opening: opening(l)?,
        bill_wise: l.bill_wise.then_some(BillWiseConfig {
            default_credit_days: l.credit_days,
        }),
        cost_centres_applicable: false,
        party,
        bank: l.bank.as_ref().map(|b| BankDetails {
            account_number: b.account.clone(),
            ifsc: b.ifsc.clone(),
            bank_name: b.name.clone(),
            branch: b.branch.clone(),
        }),
        tax: l.tax.map(|kind| TaxDetails {
            kind: tax_kind(kind),
            rate_percent: None,
        }),
        gst,
        reserved: l
            .invoice_rounding
            .then_some(ReservedLedger::InvoiceRounding),
        tally: None,
    })
}

#[expect(clippy::too_many_lines, reason = "one flat pass over each master kind")]
fn masters(seed: &Seed, api: &mut Api) -> Result<Keys, SampleError> {
    let mut ledgers = BTreeMap::new();
    for (n, l) in (1_u64..).zip(&seed.ledger) {
        let id = LedgerId(n);
        run(
            api,
            &format!("ledger {}", l.key),
            Command::CreateLedger(ledger(l, id)?),
        )?;
        let previous = ledgers.insert(
            l.key.clone(),
            LedgerInfo {
                id,
                credit_days: l.credit_days,
                bill_wise: l.bill_wise,
                state: match l.state {
                    Some(code) => Some(state_code(&format!("ledger {}", l.key), code)?),
                    None => None,
                },
            },
        );
        if previous.is_some() {
            return Err(value_error(format!("ledger {}", l.key), "key used twice"));
        }
    }
    let mut units = BTreeMap::new();
    for (n, u) in (1_u64..).zip(&seed.unit) {
        let id = UnitId(n);
        run(
            api,
            &format!("unit {}", u.symbol),
            Command::CreateUnit(Unit {
                id,
                symbol: u.symbol.clone(),
                formal_name: u.name.clone(),
                decimals: 0,
                kind: UnitKind::Simple,
                uqc: Some(u.uqc.clone()),
                tally: None,
            }),
        )?;
        units.insert(u.symbol.clone(), id);
    }
    let from = parse_date("company fy_start", &seed.company.fy_start)?;
    let mut items = BTreeMap::new();
    for (n, it) in (1_u64..).zip(&seed.item) {
        let field = format!("item {}", it.key);
        let unit = *units
            .get(&it.unit)
            .ok_or_else(|| value_error(&field, format!("no unit `{}`", it.unit)))?;
        // The usual selling rate is checked here; the model keeps none on the item.
        parse_decimal(&field, &it.rate)?;
        let hsn = match &it.hsn {
            Some(h) => Some(Hsn::parse(h).map_err(|e| value_error(&field, e.to_string()))?),
            None => None,
        };
        let id = StockItemId(n);
        run(
            api,
            &field,
            Command::CreateStockItem(StockItem {
                ineligible_itc: None,
                id,
                name: it.name.clone(),
                aliases: Vec::new(),
                group: None,
                unit,
                alt_unit: None,
                hsn,
                gst_history: vec![GstHistory {
                    effective_from: from,
                    taxability: Taxability::Taxable,
                    rate_percent: Some(Rate(parse_decimal(&field, &it.gst_rate)?)),
                }],
                valuation: ValuationMethod::AverageCost,
                batches_enabled: false,
                opening: Vec::new(),
                standard_costs: Vec::new(),
                tally: None,
            }),
        )?;
        items.insert(it.key.clone(), (id, unit));
    }
    let by_tax = |kind: TaxSeed| -> Result<LedgerId, SampleError> {
        let found = seed
            .ledger
            .iter()
            .find(|l| l.tax.is_some_and(|t| tax_kind(t) == tax_kind(kind)));
        let key = found
            .map(|l| l.key.as_str())
            .ok_or_else(|| value_error("tax ledger", format!("none of kind {kind:?}")))?;
        ledgers
            .get(key)
            .map(|i| i.id)
            .ok_or_else(|| value_error("tax ledger", key))
    };
    let (cgst, sgst, igst) = (
        by_tax(TaxSeed::Cgst)?,
        by_tax(TaxSeed::Sgst)?,
        by_tax(TaxSeed::Igst)?,
    );
    let round_off = ledgers
        .get(&seed.company.round_off_ledger)
        .map(|i| i.id)
        .ok_or_else(|| value_error("company round_off_ledger", "names no ledger"))?;
    Ok(Keys {
        ledgers,
        items,
        cgst,
        sgst,
        igst,
        round_off,
        company_state: state_code("company state", seed.company.state)?,
    })
}

fn type_of(kind: BaseVoucherKind) -> Result<ledgerly_core::VoucherTypeId, SampleError> {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .map(|t| t.id)
        .ok_or_else(|| value_error("voucher type", format!("no default type for {kind:?}")))
}

fn blank_voucher(id: u64, v: &VoucherSeed, date: Date) -> Result<Voucher, SampleError> {
    let kind = voucher_kind(v.kind);
    Ok(Voucher {
        id: VoucherId(id),
        type_id: type_of(kind)?,
        number: Some(v.number.clone()),
        series: None,
        date,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: v.reference.clone(),
        reference_date: v.reference.as_ref().map(|_| date),
        narration: v.narration.clone(),
        ledger_lines: Vec::new(),
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    })
}

/// A typed line as a ledger line: its side from `dr`, its bill and cheque from the seed.
fn typed_line(
    keys: &Keys,
    field: &str,
    l: &LineSeed,
    debit: bool,
    date: Date,
) -> Result<LedgerLine, SampleError> {
    let info = keys.ledger(field, &l.ledger)?;
    let paise = parse_paise(field, &l.amount)?;
    let amount = if debit {
        SignedAmount::dr(paise)
    } else {
        SignedAmount::cr(paise)
    };
    let mut bills = Vec::new();
    if let Some(name) = &l.new_bill {
        bills.push(BillAllocation {
            kind: BillRefKind::New,
            name: name.clone(),
            due_date: None,
            credit_days: info.credit_days,
            amount,
        });
    }
    if let Some(name) = &l.against {
        bills.push(BillAllocation {
            kind: BillRefKind::Against,
            name: name.clone(),
            due_date: None,
            credit_days: None,
            amount,
        });
    }
    let cleared = match l.cleared_days {
        Some(days) => Some(
            date.checked_add(time::Duration::days(i64::from(days)))
                .ok_or_else(|| value_error(field, "the bank date is out of range"))?,
        ),
        None => None,
    };
    let bank = match (&l.chq, cleared) {
        (Some(number), bank_date) => Some(BankAllocation {
            instrument: InstrumentKind::Cheque,
            instrument_number: Some(number.clone()),
            instrument_date: Some(date),
            bank_date,
            favouring: None,
        }),
        // A line the bank cleared with no cheque: the instrument is not recorded.
        (None, Some(bank_date)) => Some(BankAllocation {
            instrument: InstrumentKind::Other,
            instrument_number: None,
            instrument_date: None,
            bank_date: Some(bank_date),
            favouring: None,
        }),
        (None, None) => None,
    };
    Ok(LedgerLine {
        ineligible_itc: None,
        ledger: info.id,
        amount,
        bills,
        cost: Vec::new(),
        bank,
    })
}

fn typed_lines(keys: &Keys, v: &VoucherSeed, date: Date) -> Result<Vec<LedgerLine>, SampleError> {
    let field = format!("voucher {}", v.number);
    let mut lines = Vec::new();
    for l in &v.dr {
        lines.push(typed_line(keys, &field, l, true, date)?);
    }
    for l in &v.cr {
        lines.push(typed_line(keys, &field, l, false, date)?);
    }
    Ok(lines)
}

fn inventory_line(
    keys: &Keys,
    field: &str,
    l: &ItemLineSeed,
    account: LedgerId,
) -> Result<InventoryLine, SampleError> {
    let (item, unit) = *keys
        .items
        .get(&l.item)
        .ok_or_else(|| value_error(field, format!("no item `{}`", l.item)))?;
    let qty = parse_decimal(field, &l.qty)?;
    let rate = parse_decimal(field, &l.rate)?;
    let amount = qty
        .checked_mul(rate)
        .ok_or_else(|| value_error(field, "quantity times rate overflows"))?;
    let amount = parse_paise_exact(field, amount)?;
    Ok(InventoryLine {
        item,
        qty: Qty(qty),
        unit,
        rate: Rate(rate),
        discount_percent: None,
        amount,
        ledger: Some(account),
        godowns: vec![GodownAllocation {
            godown: MAIN_LOCATION,
            batch: None,
            qty: Qty(qty),
            amount,
        }],
    })
}

fn parse_paise_exact(field: &str, rupees: Decimal) -> Result<Paise, SampleError> {
    let paise = round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise)
        .map_err(|e| value_error(field, e.to_string()))?;
    if paise.to_rupees() == rupees {
        Ok(paise)
    } else {
        Err(value_error(field, "more than two decimals"))
    }
}

/// The lines the engine's tax gives an invoice: the party for the whole value, the particulars,
/// one line per tax head the engine charged, and the round-off. Every figure is the engine's.
fn taxed_lines(
    api: &Api,
    keys: &Keys,
    draft: &Voucher,
    kind: BaseVoucherKind,
    party: (LedgerId, Option<LedgerLine>),
    particulars: Vec<LedgerLine>,
) -> Result<Vec<LedgerLine>, SampleError> {
    let breakup = match api.voucher_gst(draft) {
        Ok(Outcome::Value(b)) => b,
        Ok(Outcome::Unsupported(u)) => {
            return Err(SampleError::Unsupported {
                what: u.what,
                reason: u.reason,
            });
        }
        Err(source) => {
            return Err(SampleError::Api {
                what: "the tax of an invoice".to_owned(),
                source: Box::new(source),
            });
        }
    };
    let buying = matches!(kind, BaseVoucherKind::Purchase);
    let side = |paise: Paise| {
        if buying {
            SignedAmount::dr(paise)
        } else {
            SignedAmount::cr(paise)
        }
    };
    let opposite = |paise: Paise| {
        if buying {
            SignedAmount::cr(paise)
        } else {
            SignedAmount::dr(paise)
        }
    };
    let plain = |ledger: LedgerId, amount: SignedAmount| LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    let total = breakup
        .total_before_round_off
        .checked_add(breakup.round_off)
        .map_err(|e| value_error("invoice total", e.to_string()))?;
    let mut party_line = party.1.unwrap_or_else(|| plain(party.0, opposite(total)));
    party_line.amount = opposite(total);
    for b in &mut party_line.bills {
        b.amount = party_line.amount;
    }
    let mut lines = vec![party_line];
    lines.extend(particulars);
    let heads = [
        (keys.cgst, breakup.totals.cgst),
        (keys.sgst, breakup.totals.sgst),
        (keys.igst, breakup.totals.igst),
    ];
    for (ledger, tax) in heads {
        if !tax.is_zero() {
            lines.push(plain(ledger, side(tax)));
        }
    }
    // The party line is the taxable value plus tax plus the round-off; a positive round-off makes
    // it bigger than the rest, so the round-off ledger takes the side that closes the difference.
    let round = breakup.round_off;
    if !round.is_zero() {
        let magnitude = round
            .checked_abs()
            .map_err(|e| value_error("round-off", e.to_string()))?;
        let party_bigger = !round.is_negative();
        let on_tax_side = party_bigger;
        let amount = if on_tax_side {
            side(magnitude)
        } else {
            opposite(magnitude)
        };
        lines.push(plain(keys.round_off, amount));
    }
    Ok(lines)
}

/// The bill a party line of an invoice opens, when the party is bill-wise.
fn party_bill(info: &LedgerInfo, name: &str) -> Option<BillAllocation> {
    info.bill_wise.then(|| BillAllocation {
        kind: BillRefKind::New,
        name: name.to_owned(),
        due_date: None,
        credit_days: info.credit_days,
        // Set to the party line's amount once the engine has worked it out.
        amount: SignedAmount::dr(Paise::ZERO),
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per voucher mode, each short"
)]
fn build_voucher(api: &Api, keys: &Keys, id: u64, v: &VoucherSeed) -> Result<Voucher, SampleError> {
    let field = format!("voucher {}", v.number);
    let date = parse_date(&field, &v.date)?;
    let kind = voucher_kind(v.kind);
    let mut out = blank_voucher(id, v, date)?;
    match v.mode {
        ModeSeed::Plain => {
            out.ledger_lines = typed_lines(keys, v, date)?;
        }
        ModeSeed::Invoice => {
            out.mode = VoucherMode::AccountingInvoice;
            out.party = Some(party_of(keys, &field, v)?.id);
            out.place_of_supply = Some(place(keys, v, &field)?);
            out.ledger_lines = typed_lines(keys, v, date)?;
        }
        ModeSeed::Items => {
            let party = party_of(keys, &field, v)?;
            let account = keys
                .ledger(&field, v.account.as_deref().unwrap_or_default())?
                .id;
            out.mode = VoucherMode::ItemInvoice;
            out.party = Some(party.id);
            out.place_of_supply = Some(place(keys, v, &field)?);
            for l in &v.items {
                out.inventory_lines
                    .push(inventory_line(keys, &field, l, account)?);
            }
            let mut total_paise = Paise::ZERO;
            for l in &out.inventory_lines {
                total_paise = total_paise
                    .checked_add(l.amount)
                    .map_err(|e| value_error(&field, e.to_string()))?;
            }
            let particular = LedgerLine {
                ineligible_itc: None,
                ledger: account,
                amount: if matches!(kind, BaseVoucherKind::Purchase) {
                    SignedAmount::dr(total_paise)
                } else {
                    SignedAmount::cr(total_paise)
                },
                bills: Vec::new(),
                cost: Vec::new(),
                bank: None,
            };
            let (target, bill) = if let Some(cash) = &v.paid_to {
                (keys.ledger(&field, cash)?, None)
            } else {
                let name = v.reference.as_deref().unwrap_or(&v.number);
                (party, party_bill(party, name))
            };
            let party_line = bill.map(|b| LedgerLine {
                ineligible_itc: None,
                ledger: target.id,
                amount: b.amount,
                bills: vec![b],
                cost: Vec::new(),
                bank: None,
            });
            let lines = taxed_lines(
                api,
                keys,
                &out,
                kind,
                (target.id, party_line),
                vec![particular],
            )?;
            out.ledger_lines = lines;
        }
        ModeSeed::Taxed => {
            let party = party_of(keys, &field, v)?;
            out.mode = VoucherMode::AccountingInvoice;
            out.party = Some(party.id);
            out.place_of_supply = Some(place(keys, v, &field)?);
            let particulars = typed_lines(keys, v, date)?;
            out.ledger_lines.clone_from(&particulars);
            let name = v.reference.as_deref().unwrap_or(&v.number);
            let party_line = party_bill(party, name).map(|b| LedgerLine {
                ineligible_itc: None,
                ledger: party.id,
                amount: b.amount,
                bills: vec![b],
                cost: Vec::new(),
                bank: None,
            });
            out.ledger_lines =
                taxed_lines(api, keys, &out, kind, (party.id, party_line), particulars)?;
        }
    }
    Ok(out)
}

fn party_of<'k>(
    keys: &'k Keys,
    field: &str,
    v: &VoucherSeed,
) -> Result<&'k LedgerInfo, SampleError> {
    keys.ledger(field, v.party.as_deref().unwrap_or_default())
}

/// The place of supply: the seed's, else the party's state on a sale (the goods go to the buyer),
/// else the company's state (a purchase is delivered to the company).
fn place(keys: &Keys, v: &VoucherSeed, field: &str) -> Result<StateCode, SampleError> {
    if let Some(code) = v.place {
        return state_code(field, code);
    }
    match v.kind {
        KindSeed::Sales => Ok(party_of(keys, field, v)?
            .state
            .unwrap_or(keys.company_state)),
        KindSeed::Purchase
        | KindSeed::Payment
        | KindSeed::Receipt
        | KindSeed::Journal
        | KindSeed::Contra => Ok(keys.company_state),
    }
}

/// The company's two users (the mock's "Ananya K." and "Rohan D."): the first is the default
/// actor of the company file.
const USERS: [(&str, u64, &str); 2] = [("ananya", 1, "Ananya K."), ("rohan", 2, "Rohan D.")];

/// The actor that entered `v`: the user it names, or the assistant acting for that user when it
/// was an accepted draft.
fn actor_of(v: &VoucherSeed) -> Result<Actor, SampleError> {
    let user = match v.by.as_deref() {
        None => UserId(1),
        Some(key) => USERS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, id, _)| UserId(*id))
            .ok_or_else(|| value_error(format!("voucher {}", v.number), "unknown user"))?,
    };
    Ok(if v.ai {
        Actor::Assistant {
            model: "assistant".to_owned(),
            on_behalf_of: user,
        }
    } else {
        Actor::User(user)
    })
}

/// Posts the seed's vouchers, each stamped at noon (UTC+05:30) on its own date, as if entered the
/// day it is dated, so the audit card of a sample voucher reads "Created <voucher date>". The
/// company's own `clock` stamps everything after them.
fn vouchers(seed: &Seed, api: &mut Api, keys: &Keys, clock: Clock) -> Result<(), SampleError> {
    for (_, id, name) in USERS {
        api.set_user_name(UserId(id), name)
            .map_err(|e| value_error("users", e.to_string()))?;
    }
    let mut november_seen = false;
    for (n, v) in (1_u64..).zip(&seed.voucher) {
        match v.section {
            SectionSeed::November => november_seen = true,
            SectionSeed::History if november_seen => {
                return Err(value_error(
                    format!("voucher {}", v.number),
                    "a history voucher comes after a November one",
                ));
            }
            SectionSeed::History => {}
        }
        let built = build_voucher(api, keys, n, v)?;
        api.set_clock(entered_at(&format!("voucher {}", v.number), built.date)?);
        api.set_actor(actor_of(v)?);
        run(
            api,
            &format!("voucher {}", v.number),
            Command::PostVoucher(built),
        )?;
    }
    api.set_actor(Actor::User(UserId(1)));
    api.set_clock(clock);
    Ok(())
}

/// The clock that stamps a voucher dated `date`: noon, UTC+05:30.
fn entered_at(field: &str, date: Date) -> Result<Clock, SampleError> {
    let offset = UtcOffset::from_hms(5, 30, 0).map_err(|e| value_error(field, e.to_string()))?;
    let at = date
        .with_hms(12, 0, 0)
        .map_err(|e| value_error(field, e.to_string()))?
        .assume_offset(offset);
    Ok(Clock::Fixed(at))
}

fn statements(seed: &Seed, api: &mut Api, keys: &Keys) -> Result<(), SampleError> {
    for s in &seed.statement {
        let field = format!("statement {}", s.file);
        let ledger = keys.ledger(&field, &s.ledger)?.id;
        let (_, text) = STATEMENTS
            .iter()
            .find(|(name, _)| *name == s.file)
            .ok_or_else(|| value_error(&field, "no such embedded file"))?;
        match api.import_bank_statement(text.as_bytes(), &s.format, ledger) {
            Ok(Outcome::Value(_)) => {}
            Ok(Outcome::Unsupported(u)) => {
                return Err(SampleError::Unsupported {
                    what: u.what,
                    reason: u.reason,
                });
            }
            Err(source) => {
                return Err(SampleError::Api {
                    what: field,
                    source: Box::new(source),
                });
            }
        }
    }
    Ok(())
}

/// Wraps an `Api` refusal of a 2B step.
fn api_step<T>(
    what: &str,
    r: Result<Outcome<T>, ledgerly_api::ApiError>,
) -> Result<T, SampleError> {
    match r {
        Ok(Outcome::Value(v)) => Ok(v),
        Ok(Outcome::Unsupported(u)) => Err(SampleError::Unsupported {
            what: u.what,
            reason: u.reason,
        }),
        Err(source) => Err(SampleError::Api {
            what: what.to_owned(),
            source: Box::new(source),
        }),
    }
}

/// Imports the October and November 2B files, records the IMS actions the mock shows (Kaveri's
/// November bill accepted; October's KP/2205 accepted and VL/858 rejected) and marks October's
/// GSTR-3B filed on 20 Nov, which makes October final.
fn gstr2b_files(seed: &Seed, api: &mut Api) -> Result<(), SampleError> {
    let gstin = Gstin::parse(&seed.company.gstin)
        .map_err(|e| value_error("company gstin", e.to_string()))?;
    let period = |from: Date, to: Date| {
        Period::new(from, to).map_err(|e| value_error("2B period", e.to_string()))
    };
    let oct = period(
        parse_date("2B october", "2026-10-01")?,
        parse_date("2B october", "2026-10-31")?,
    )?;
    let nov = period(
        parse_date("2B november", "2026-11-01")?,
        parse_date("2B november", "2026-11-30")?,
    )?;
    let kaveri = seed_gstin(seed, "kaveri")?;
    let vardhaman = seed_gstin(seed, "vardhaman")?;
    api_step(
        "importing the October 2B file",
        api.import_gstr2b(&gstin, oct, "gstr2b-oct-2026.json", crate::GSTR2B_OCTOBER),
    )?;
    api_step(
        "importing the November 2B file",
        api.import_gstr2b(&gstin, nov, "gstr2b-nov-2026.json", crate::GSTR2B_NOVEMBER),
    )?;
    let actions = [
        (oct, kaveri.as_str(), "KP/2205", ImsAction::Accept),
        (oct, vardhaman.as_str(), "VL/858", ImsAction::Reject),
        (nov, kaveri.as_str(), "KP/2231", ImsAction::Accept),
    ];
    for (p, supplier, number, action) in actions {
        api_step(
            "recording an IMS action",
            api.set_ims_action(&gstin, p, supplier, number, action),
        )?;
    }
    api_step(
        "marking October's GSTR-3B filed",
        api.mark_gstr3b_filed(&gstin, oct, parse_date("3B filed on", "2026-11-20")?),
    )
}

/// The GSTIN of the ledger the seed keys `key`.
fn seed_gstin(seed: &Seed, key: &str) -> Result<String, SampleError> {
    seed.ledger
        .iter()
        .find(|l| l.key == key)
        .and_then(|l| l.gstin.clone())
        .ok_or_else(|| value_error(format!("ledger {key}"), "has no GSTIN in the seed"))
}
