//! Form drafts: the text a user typed for one master, and the check that turns it into a
//! [`MasterSnapshot`].
//!
//! A draft never touches the store. [`MasterDraft::build`] validates against the open books and
//! returns the full master the host should create or alter; fields the form has no control for
//! are copied from the master being altered, never reset. A new master carries
//! [`UNASSIGNED_ID`] until the store assigns the real id on commit.

use std::str::FromStr;

use ledgerly_core::{
    CostCategoryId, CostCentreId, Decimal, DrCr, GodownId, GroupId, Gstin, Hsn, Outcome, Paise,
    Pan, Rate, RoundingRule, SignedAmount, StateCode, StockGroupId, StockItemId, UnitId,
    round_rupees_to_paise,
};
use ledgerly_model::{
    Address, BankDetails, BillWiseConfig, Books, CostCategory, CostCentre, Features, Godown, Group,
    GstClassification, Ledger, MasterSnapshot, Nature, PartyDetails, RegistrationType, StockGroup,
    StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod,
};

use super::check::{check_unique, clean_labels, clean_name, creates_cycle, invalid};
use super::{MasterError, MasterKind};

/// The id a new master carries until the store assigns one on commit.
pub const UNASSIGNED_ID: u64 = 0;

/// Most decimals a unit may allow. Our own limit; no source gives one.
const UNIT_MAX_DECIMALS: u8 = 6;

// ---------------------------------------------------------------------------------------------
// Drafts
// ---------------------------------------------------------------------------------------------

/// A group form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupDraft {
    /// Name.
    pub name: String,
    /// Aliases, separated by commas or lines.
    pub aliases: String,
    /// Parent group; `None` makes a primary group.
    pub parent: Option<GroupId>,
    /// Nature, asked only for a primary group (spec 5: nature only on primaries).
    pub nature: Option<Nature>,
    /// Whether a primary group's ledgers count in gross profit. A sub-group inherits it.
    pub affects_gross_profit: bool,
}

/// Party details of a ledger form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyDraft {
    /// Registration type.
    pub registration: RegistrationType,
    /// GSTIN text.
    pub gstin: String,
    /// PAN text.
    pub pan: String,
    /// State; taken from the GSTIN when left empty.
    pub state: Option<StateCode>,
    /// Address lines, one per line.
    pub address_lines: String,
    /// PIN code.
    pub pincode: String,
    /// Country.
    pub country: String,
}

impl Default for PartyDraft {
    fn default() -> Self {
        PartyDraft {
            registration: RegistrationType::Regular,
            gstin: String::new(),
            pan: String::new(),
            state: None,
            address_lines: String::new(),
            pincode: String::new(),
            country: "India".to_owned(),
        }
    }
}

/// Bank details of a ledger form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BankDraft {
    /// Account number.
    pub account_number: String,
    /// IFSC.
    pub ifsc: String,
    /// Bank name.
    pub bank_name: String,
    /// Branch.
    pub branch: String,
}

/// Tax details (a duty or tax ledger) of a ledger form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxDraft {
    /// Which tax.
    pub kind: TaxLedgerKind,
    /// Rate in percent; empty when the ledger has none.
    pub rate_percent: String,
}

/// GST classification of a ledger form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstDraft {
    /// HSN or SAC; empty for none.
    pub hsn: String,
    /// Taxability.
    pub taxability: Taxability,
    /// Rate in percent; empty when none. Only a taxable ledger may carry one.
    pub rate_percent: String,
}

/// A ledger form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerDraft {
    /// Name.
    pub name: String,
    /// Aliases, separated by commas or lines.
    pub aliases: String,
    /// Group.
    pub group: Option<GroupId>,
    /// Opening balance in rupees, for example `12,34,567.00`; empty is zero.
    pub opening: String,
    /// Side of the opening balance.
    pub opening_side: DrCr,
    /// Bill-wise tracking.
    pub bill_wise: bool,
    /// Default credit days, digits; only for a bill-wise ledger.
    pub credit_days: String,
    /// Cost centres apply.
    pub cost_centres: bool,
    /// Party details, when the ledger is a party.
    pub party: Option<PartyDraft>,
    /// Bank details.
    pub bank: Option<BankDraft>,
    /// Tax details.
    pub tax: Option<TaxDraft>,
    /// GST classification.
    pub gst: Option<GstDraft>,
}

impl Default for LedgerDraft {
    fn default() -> Self {
        LedgerDraft {
            name: String::new(),
            aliases: String::new(),
            group: None,
            opening: String::new(),
            opening_side: DrCr::Dr,
            bill_wise: false,
            credit_days: String::new(),
            cost_centres: false,
            party: None,
            bank: None,
            tax: None,
            gst: None,
        }
    }
}

/// A simple unit form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDraft {
    /// Symbol, for example `Nos`.
    pub symbol: String,
    /// Formal name.
    pub formal_name: String,
    /// Decimal places, digits.
    pub decimals: String,
    /// GST unit quantity code: three capital letters, or empty.
    pub uqc: String,
}

impl Default for UnitDraft {
    fn default() -> Self {
        UnitDraft {
            symbol: String::new(),
            formal_name: String::new(),
            decimals: "0".to_owned(),
            uqc: String::new(),
        }
    }
}

/// A stock group, godown or cost category form: a name and an optional parent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeDraft {
    /// Name.
    pub name: String,
    /// Parent id (a `StockGroupId` or `GodownId` as its raw number).
    pub parent: Option<u64>,
}

/// A cost centre form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostCentreDraft {
    /// Name.
    pub name: String,
    /// Category.
    pub category: Option<CostCategoryId>,
    /// Parent centre.
    pub parent: Option<CostCentreId>,
}

/// A stock item form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockItemDraft {
    /// Name.
    pub name: String,
    /// Aliases, separated by commas or lines.
    pub aliases: String,
    /// Stock group.
    pub group: Option<StockGroupId>,
    /// Unit.
    pub unit: Option<UnitId>,
    /// Valuation method.
    pub valuation: ValuationMethod,
    /// HSN or SAC; empty for none.
    pub hsn: String,
    /// Batch tracking.
    pub batches: bool,
}

impl Default for StockItemDraft {
    fn default() -> Self {
        StockItemDraft {
            name: String::new(),
            aliases: String::new(),
            group: None,
            unit: None,
            valuation: ValuationMethod::AverageCost,
            hsn: String::new(),
            batches: false,
        }
    }
}

/// One form, of any master kind this screen edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterDraft {
    /// A group.
    Group(GroupDraft),
    /// A ledger.
    Ledger(Box<LedgerDraft>),
    /// A simple unit.
    Unit(UnitDraft),
    /// A stock group.
    StockGroup(TreeDraft),
    /// A stock item.
    StockItem(StockItemDraft),
    /// A godown.
    Godown(TreeDraft),
    /// A cost category (the parent is unused).
    CostCategory(TreeDraft),
    /// A cost centre.
    CostCentre(CostCentreDraft),
}

fn unsupported(what: &str, reason: &str) -> MasterError {
    MasterError::Unsupported {
        what: what.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Fails when `kind` needs a feature the company has switched off (spec 5: feature flags).
///
/// # Errors
/// [`MasterError::FeatureOff`] naming the feature.
pub fn creation_allowed(kind: MasterKind, features: &Features) -> Result<(), MasterError> {
    let (on, feature) = match kind {
        MasterKind::Group | MasterKind::Ledger | MasterKind::VoucherType | MasterKind::Batch => {
            (true, "")
        }
        MasterKind::Unit | MasterKind::StockGroup | MasterKind::StockItem => {
            (features.inventory, "Inventory")
        }
        MasterKind::Godown => (features.godowns, "Godowns"),
        MasterKind::CostCategory | MasterKind::CostCentre => {
            (features.cost_centres, "Cost centres")
        }
    };
    if on {
        Ok(())
    } else {
        Err(MasterError::FeatureOff { feature })
    }
}

impl MasterDraft {
    /// An empty form for a new master.
    #[must_use]
    pub fn blank(kind: MasterKind) -> Outcome<MasterDraft> {
        match kind {
            MasterKind::Group => Outcome::Value(MasterDraft::Group(GroupDraft::default())),
            MasterKind::Ledger => Outcome::Value(MasterDraft::Ledger(Box::default())),
            MasterKind::Unit => Outcome::Value(MasterDraft::Unit(UnitDraft::default())),
            MasterKind::StockGroup => Outcome::Value(MasterDraft::StockGroup(TreeDraft::default())),
            MasterKind::StockItem => {
                Outcome::Value(MasterDraft::StockItem(StockItemDraft::default()))
            }
            MasterKind::Godown => Outcome::Value(MasterDraft::Godown(TreeDraft::default())),
            MasterKind::CostCategory => {
                Outcome::Value(MasterDraft::CostCategory(TreeDraft::default()))
            }
            MasterKind::CostCentre => {
                Outcome::Value(MasterDraft::CostCentre(CostCentreDraft::default()))
            }
            MasterKind::VoucherType => Outcome::unsupported(
                "voucher type form",
                "numbering rules and series have no verified behaviour yet, so voucher types \
                 are listed but not edited here",
            ),
            MasterKind::Batch => Outcome::unsupported(
                "batch form",
                "batches are created from stock entry, not from this screen",
            ),
        }
    }

    /// A form filled with an existing master, for altering it.
    #[must_use]
    pub fn from_existing(master: &MasterSnapshot) -> Outcome<MasterDraft> {
        match master {
            MasterSnapshot::Group(g) => Outcome::Value(MasterDraft::Group(GroupDraft {
                name: g.name.clone(),
                aliases: g.aliases.join(", "),
                parent: g.parent,
                nature: g.nature,
                affects_gross_profit: g.affects_gross_profit,
            })),
            MasterSnapshot::Ledger(l) => {
                Outcome::Value(MasterDraft::Ledger(Box::new(ledger_draft(l))))
            }
            MasterSnapshot::Unit(u) => match u.kind {
                UnitKind::Simple => Outcome::Value(MasterDraft::Unit(UnitDraft {
                    symbol: u.symbol.clone(),
                    formal_name: u.formal_name.clone(),
                    decimals: u.decimals.to_string(),
                    uqc: u.uqc.clone().unwrap_or_default(),
                })),
                UnitKind::Compound { .. } => Outcome::unsupported(
                    "compound unit form",
                    "compound units (a box of twelve) are listed but not edited here yet",
                ),
            },
            MasterSnapshot::StockGroup(g) => Outcome::Value(MasterDraft::StockGroup(TreeDraft {
                name: g.name.clone(),
                parent: g.parent.map(|p| p.0),
            })),
            MasterSnapshot::StockItem(i) => {
                Outcome::Value(MasterDraft::StockItem(StockItemDraft {
                    name: i.name.clone(),
                    aliases: i.aliases.join(", "),
                    group: i.group,
                    unit: Some(i.unit),
                    valuation: i.valuation,
                    hsn: i
                        .hsn
                        .as_ref()
                        .map(|h| h.as_str().to_owned())
                        .unwrap_or_default(),
                    batches: i.batches_enabled,
                }))
            }
            MasterSnapshot::Godown(g) => Outcome::Value(MasterDraft::Godown(TreeDraft {
                name: g.name.clone(),
                parent: g.parent.map(|p| p.0),
            })),
            MasterSnapshot::CostCategory(c) => {
                Outcome::Value(MasterDraft::CostCategory(TreeDraft {
                    name: c.name.clone(),
                    parent: None,
                }))
            }
            MasterSnapshot::CostCentre(c) => {
                Outcome::Value(MasterDraft::CostCentre(CostCentreDraft {
                    name: c.name.clone(),
                    category: Some(c.category),
                    parent: c.parent,
                }))
            }
            MasterSnapshot::VoucherType(_) => MasterDraft::blank(MasterKind::VoucherType),
            MasterSnapshot::Batch(_) => MasterDraft::blank(MasterKind::Batch),
        }
    }

    /// Which kind of master this form edits.
    #[must_use]
    pub fn kind(&self) -> MasterKind {
        match self {
            MasterDraft::Group(_) => MasterKind::Group,
            MasterDraft::Ledger(_) => MasterKind::Ledger,
            MasterDraft::Unit(_) => MasterKind::Unit,
            MasterDraft::StockGroup(_) => MasterKind::StockGroup,
            MasterDraft::StockItem(_) => MasterKind::StockItem,
            MasterDraft::Godown(_) => MasterKind::Godown,
            MasterDraft::CostCategory(_) => MasterKind::CostCategory,
            MasterDraft::CostCentre(_) => MasterKind::CostCentre,
        }
    }

    /// Validates the form against `books` and returns the master to create (`editing` is
    /// `None`) or the altered master (`editing` is its id).
    ///
    /// # Errors
    /// A [`MasterError`] naming the first field that is wrong, or what the company has switched
    /// off, or [`MasterError::Unsupported`] for a combination with no verified behaviour.
    pub fn build(
        &self,
        books: &Books,
        editing: Option<u64>,
    ) -> Result<MasterSnapshot, MasterError> {
        if editing.is_none() {
            creation_allowed(self.kind(), &books.company.features)?;
        }
        match self {
            MasterDraft::Group(d) => build_group(d, books, editing.map(GroupId)),
            MasterDraft::Ledger(d) => build_ledger(d, books, editing),
            MasterDraft::Unit(d) => build_unit(d, books, editing.map(UnitId)),
            MasterDraft::StockGroup(d) => build_stock_group(d, books, editing.map(StockGroupId)),
            MasterDraft::StockItem(d) => build_stock_item(d, books, editing.map(StockItemId)),
            MasterDraft::Godown(d) => build_godown(d, books, editing.map(GodownId)),
            MasterDraft::CostCategory(d) => {
                build_cost_category(d, books, editing.map(CostCategoryId))
            }
            MasterDraft::CostCentre(d) => build_cost_centre(d, books, editing.map(CostCentreId)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------------------------

/// `true` when `int_part` is plain digits or Indian grouping (`12,34,567`): a first group of
/// one or two digits, then groups of two, then a last group of three.
fn indian_grouping_ok(int_part: &str) -> bool {
    let parts: Vec<&str> = int_part.split(',').collect();
    let Some((first, tail)) = parts.split_first() else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if tail.is_empty() {
        return digits(first);
    }
    let Some((last, middle)) = tail.split_last() else {
        return false;
    };
    digits(first)
        && (1..=2).contains(&first.len())
        && middle.iter().all(|m| digits(m) && m.len() == 2)
        && digits(last)
        && last.len() == 3
}

/// Parses an amount of rupees such as `1234.50` or `12,34,567.00` into paise.
///
/// No rounding happens: more than two decimals is an error, and commas must follow Indian
/// grouping so `1,00` is never read as a hundred.
///
/// # Errors
/// [`MasterError::Invalid`] naming `field`, or [`MasterError::Money`] on overflow.
pub fn parse_rupees(field: &str, text: &str) -> Result<Paise, MasterError> {
    let text = text.trim();
    let (int_part, frac_part) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text, None),
    };
    let frac_ok = frac_part
        .is_none_or(|f| f.len() <= 2 && !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()));
    if !indian_grouping_ok(int_part) || !frac_ok {
        return Err(invalid(
            field,
            "type rupees with at most two decimals, for example 12,34,567.00 (use the Dr/Cr \
             selector instead of a minus sign)",
        ));
    }
    let cleaned: String = text.chars().filter(|c| *c != ',').collect();
    let value = Decimal::from_str(&cleaned).map_err(|_| invalid(field, "not a number"))?;
    Ok(round_rupees_to_paise(value, RoundingRule::HalfUpToPaise)?)
}

/// Parses a percentage `0..=100`; empty is `None`.
fn parse_percent(field: &str, text: &str) -> Result<Option<Rate>, MasterError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let plain = text.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let value = Decimal::from_str(text).ok().filter(|_| plain);
    match value {
        Some(v) if (Decimal::ZERO..=Decimal::ONE_HUNDRED).contains(&v) => Ok(Some(Rate(v))),
        Some(_) | None => Err(invalid(field, "type a percentage from 0 to 100")),
    }
}

fn parse_digits_u32(field: &str, text: &str) -> Result<Option<u32>, MasterError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(field, "type a whole number"));
    }
    text.parse::<u32>()
        .map(Some)
        .map_err(|_| invalid(field, "the number is too large"))
}

fn parse_hsn(text: &str) -> Result<Option<Hsn>, MasterError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Hsn::parse(text)
        .map(Some)
        .map_err(|e| invalid("HSN/SAC", e.to_string()))
}

fn original<'a, T>(
    found: Option<&'a T>,
    editing: bool,
    kind: MasterKind,
) -> Result<Option<&'a T>, MasterError> {
    match (found, editing) {
        (Some(o), _) => Ok(Some(o)),
        (None, false) => Ok(None),
        (None, true) => Err(MasterError::NotFound { kind: kind.label() }),
    }
}

// ---------------------------------------------------------------------------------------------
// Group
// ---------------------------------------------------------------------------------------------

fn build_group(
    d: &GroupDraft,
    books: &Books,
    editing: Option<GroupId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let orig = original(
        editing.and_then(|id| masters.groups.get(&id)),
        editing.is_some(),
        MasterKind::Group,
    )?;
    let labels = clean_labels(&d.name, &d.aliases)?;
    check_unique(masters, MasterKind::Group, editing.map(|g| g.0), &labels)?;
    let parent = match d.parent {
        Some(p) => Some(
            masters
                .groups
                .get(&p)
                .ok_or_else(|| invalid("Under", "the parent group does not exist"))?,
        ),
        None => None,
    };
    if let (Some(id), Some(p)) = (editing, d.parent) {
        let parent_of = |g: u64| {
            masters
                .groups
                .get(&GroupId(g))
                .and_then(|x| x.parent)
                .map(|x| x.0)
        };
        if creates_cycle(id.0, p.0, parent_of, masters.groups.len()) {
            return Err(invalid(
                "Under",
                "a group cannot sit under itself or one of its own sub-groups",
            ));
        }
    }
    let (nature, affects) = group_class(d, parent, orig)?;
    let mut names = labels.into_iter();
    let name = names.next().unwrap_or_default();
    Ok(MasterSnapshot::Group(Group {
        id: editing.unwrap_or(GroupId(UNASSIGNED_ID)),
        name,
        aliases: names.collect(),
        parent: d.parent,
        reserved: orig.and_then(|o| o.reserved),
        nature,
        affects_gross_profit: affects,
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

/// The nature and gross-profit flag a group form results in.
fn group_class(
    d: &GroupDraft,
    parent: Option<&Group>,
    orig: Option<&Group>,
) -> Result<(Option<Nature>, bool), MasterError> {
    if let Some(o) = orig.filter(|o| o.reserved.is_some()) {
        let moved = d.parent != o.parent
            || (o.parent.is_none()
                && (d.nature != o.nature || d.affects_gross_profit != o.affects_gross_profit));
        if moved {
            return Err(MasterError::Reserved {
                name: o.name.clone(),
                what: "moved or reclassified (only its name and aliases can change)".to_owned(),
            });
        }
        return Ok((o.nature, o.affects_gross_profit));
    }
    match parent {
        // Spec 5: nature only on primaries, inherited by sub-groups.
        Some(p) => Ok((None, p.affects_gross_profit)),
        None => match d.nature {
            Some(n) => Ok((Some(n), d.affects_gross_profit)),
            None => Err(invalid("Nature", "a primary group needs a nature")),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Ledger
// ---------------------------------------------------------------------------------------------

fn plain_rupees(amount: Paise) -> String {
    let abs = amount.get().unsigned_abs();
    format!("{}.{:02}", abs / 100, abs % 100)
}

fn ledger_draft(l: &Ledger) -> LedgerDraft {
    LedgerDraft {
        name: l.name.clone(),
        aliases: l.aliases.join(", "),
        group: Some(l.group),
        opening: if l.opening.amount.is_zero() {
            String::new()
        } else {
            plain_rupees(l.opening.amount)
        },
        opening_side: l.opening.side,
        bill_wise: l.bill_wise.is_some(),
        credit_days: l
            .bill_wise
            .and_then(|b| b.default_credit_days)
            .map(|d| d.to_string())
            .unwrap_or_default(),
        cost_centres: l.cost_centres_applicable,
        party: l.party.as_ref().map(|p| PartyDraft {
            registration: p.registration,
            gstin: p
                .gstin
                .as_ref()
                .map(|g| g.as_str().to_owned())
                .unwrap_or_default(),
            pan: p
                .pan
                .as_ref()
                .map(|g| g.as_str().to_owned())
                .unwrap_or_default(),
            state: p.state,
            address_lines: p.address.lines.join("\n"),
            pincode: p.address.pincode.clone().unwrap_or_default(),
            country: p.address.country.clone(),
        }),
        bank: l.bank.as_ref().map(|b| BankDraft {
            account_number: b.account_number.clone(),
            ifsc: b.ifsc.clone(),
            bank_name: b.bank_name.clone(),
            branch: b.branch.clone(),
        }),
        tax: l.tax.map(|t| TaxDraft {
            kind: t.kind,
            rate_percent: t.rate_percent.map(|r| r.0.to_string()).unwrap_or_default(),
        }),
        gst: l.gst.as_ref().map(|g| GstDraft {
            hsn: g
                .hsn
                .as_ref()
                .map(|h| h.as_str().to_owned())
                .unwrap_or_default(),
            taxability: g.taxability,
            rate_percent: g.rate_percent.map(|r| r.0.to_string()).unwrap_or_default(),
        }),
    }
}

/// The opening balance. A zero balance is always stored as Dr so two zero openings compare equal.
fn build_opening(d: &LedgerDraft) -> Result<SignedAmount, MasterError> {
    let text = d.opening.trim();
    let amount = if text.is_empty() {
        Paise::ZERO
    } else {
        parse_rupees("Opening balance", text)?
    };
    Ok(if amount.is_zero() {
        SignedAmount::dr(Paise::ZERO)
    } else if d.opening_side == DrCr::Dr {
        SignedAmount::dr(amount)
    } else {
        SignedAmount::cr(amount)
    })
}

fn build_ledger(
    d: &LedgerDraft,
    books: &Books,
    editing: Option<u64>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let id = editing.map(ledgerly_core::LedgerId);
    let orig = original(
        id.and_then(|i| masters.ledgers.get(&i)),
        id.is_some(),
        MasterKind::Ledger,
    )?;
    let labels = clean_labels(&d.name, &d.aliases)?;
    check_unique(masters, MasterKind::Ledger, editing, &labels)?;
    let group = d.group.ok_or_else(|| invalid("Under", "choose a group"))?;
    if !masters.groups.contains_key(&group) {
        return Err(invalid("Under", "the group does not exist"));
    }
    if let Some(o) = orig.filter(|o| o.reserved.is_some() && o.group != group) {
        return Err(MasterError::Reserved {
            name: o.name.clone(),
            what: "moved to another group".to_owned(),
        });
    }
    let bill_wise = build_bill_wise(d, &books.company.features, orig)?;
    if d.cost_centres
        && !books.company.features.cost_centres
        && !orig.is_some_and(|o| o.cost_centres_applicable)
    {
        return Err(MasterError::FeatureOff {
            feature: "Cost centres",
        });
    }
    let mut names = labels.into_iter();
    let name = names.next().unwrap_or_default();
    Ok(MasterSnapshot::Ledger(Ledger {
        id: id.unwrap_or(ledgerly_core::LedgerId(UNASSIGNED_ID)),
        name,
        aliases: names.collect(),
        group,
        opening: build_opening(d)?,
        bill_wise,
        cost_centres_applicable: d.cost_centres,
        party: d.party.as_ref().map(build_party).transpose()?,
        bank: d.bank.as_ref().map(build_bank).transpose()?,
        tax: d.tax.as_ref().map(build_tax).transpose()?,
        gst: d.gst.as_ref().map(build_gst).transpose()?,
        reserved: orig.and_then(|o| o.reserved),
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

fn build_bill_wise(
    d: &LedgerDraft,
    features: &Features,
    orig: Option<&Ledger>,
) -> Result<Option<BillWiseConfig>, MasterError> {
    let days = parse_digits_u32("Credit days", &d.credit_days)?;
    if !d.bill_wise {
        if days.is_some() {
            return Err(invalid(
                "Credit days",
                "credit days apply only to a bill-wise ledger",
            ));
        }
        return Ok(None);
    }
    if !features.bill_wise && !orig.is_some_and(|o| o.bill_wise.is_some()) {
        return Err(MasterError::FeatureOff {
            feature: "Bill-wise details",
        });
    }
    Ok(Some(BillWiseConfig {
        default_credit_days: days,
    }))
}

fn build_party(p: &PartyDraft) -> Result<PartyDetails, MasterError> {
    let gstin_text = p.gstin.trim();
    let gstin = if gstin_text.is_empty() {
        None
    } else {
        Some(Gstin::parse(gstin_text).map_err(|e| invalid("GSTIN", e.to_string()))?)
    };
    match p.registration {
        RegistrationType::Regular | RegistrationType::Composition | RegistrationType::Sez => {
            if gstin.is_none() {
                return Err(invalid("GSTIN", "a registered party needs a GSTIN"));
            }
        }
        RegistrationType::Unregistered
        | RegistrationType::Consumer
        | RegistrationType::Overseas => {
            if gstin.is_some() {
                return Err(invalid("GSTIN", "this registration type has no GSTIN"));
            }
        }
        RegistrationType::Uin => {
            return Err(unsupported(
                "UIN registrations",
                "a UIN is not shaped like a GSTIN and its format is not sourced yet",
            ));
        }
    }
    let from_gstin = gstin.as_ref().and_then(Gstin::state);
    let state = match (p.state, from_gstin) {
        (Some(s), Some(g)) if s != g => {
            return Err(invalid(
                "State",
                "the state does not match the first two digits of the GSTIN",
            ));
        }
        (Some(s), _) => Some(s),
        (None, g) => g,
    };
    let pan = build_pan(&p.pan, gstin.as_ref())?;
    let country = clean_name("Country", &p.country)?;
    let pincode = p.pincode.trim();
    let pin_ok = pincode.is_empty()
        || p.registration == RegistrationType::Overseas
        || (pincode.len() == 6 && pincode.bytes().all(|b| b.is_ascii_digit()));
    if !pin_ok {
        return Err(invalid("PIN code", "an Indian PIN code has six digits"));
    }
    Ok(PartyDetails {
        registration: p.registration,
        gstin,
        pan,
        state,
        address: Address {
            lines: split_lines(&p.address_lines),
            pincode: (!pincode.is_empty()).then(|| pincode.to_owned()),
            state,
            country,
        },
    })
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn build_pan(text: &str, gstin: Option<&Gstin>) -> Result<Option<Pan>, MasterError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let pan = Pan::parse(text).map_err(|e| invalid("PAN", e.to_string()))?;
    if gstin.is_some_and(|g| g.pan_str() != pan.as_str()) {
        return Err(invalid(
            "PAN",
            "the PAN differs from the one inside the GSTIN",
        ));
    }
    Ok(Some(pan))
}

fn build_bank(b: &BankDraft) -> Result<BankDetails, MasterError> {
    let account_number = clean_name("Account number", &b.account_number)?;
    let bank_name = clean_name("Bank name", &b.bank_name)?;
    let ifsc = b.ifsc.trim();
    let bytes = ifsc.as_bytes();
    // IFSC shape: four capital letters, a zero, six letters or digits.
    let ifsc_ok = bytes.len() == 11
        && bytes.iter().take(4).all(u8::is_ascii_uppercase)
        && bytes.get(4) == Some(&b'0')
        && bytes
            .iter()
            .skip(5)
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if !ifsc_ok {
        return Err(invalid(
            "IFSC",
            "four capital letters, a zero, then six letters or digits",
        ));
    }
    Ok(BankDetails {
        account_number,
        ifsc: ifsc.to_owned(),
        bank_name,
        branch: b.branch.trim().to_owned(),
    })
}

fn build_tax(t: &TaxDraft) -> Result<TaxDetails, MasterError> {
    Ok(TaxDetails {
        kind: t.kind,
        rate_percent: parse_percent("Tax rate", &t.rate_percent)?,
    })
}

fn build_gst(g: &GstDraft) -> Result<GstClassification, MasterError> {
    let rate_percent = parse_percent("GST rate", &g.rate_percent)?;
    if rate_percent.is_some() && g.taxability != Taxability::Taxable {
        return Err(invalid("GST rate", "only a taxable ledger carries a rate"));
    }
    Ok(GstClassification {
        hsn: parse_hsn(&g.hsn)?,
        taxability: g.taxability,
        rate_percent,
    })
}

// ---------------------------------------------------------------------------------------------
// Inventory and cost masters
// ---------------------------------------------------------------------------------------------

fn build_unit(
    d: &UnitDraft,
    books: &Books,
    editing: Option<UnitId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let orig = original(
        editing.and_then(|id| masters.units.get(&id)),
        editing.is_some(),
        MasterKind::Unit,
    )?;
    if orig.is_some_and(|o| matches!(o.kind, UnitKind::Compound { .. })) {
        return Err(unsupported(
            "altering a compound unit",
            "compound units are listed but not edited here yet",
        ));
    }
    let symbol = clean_name("Symbol", &d.symbol)?;
    let formal_name = clean_name("Formal name", &d.formal_name)?;
    check_unique(
        masters,
        MasterKind::Unit,
        editing.map(|u| u.0),
        std::slice::from_ref(&symbol),
    )?;
    let decimals = d
        .decimals
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|n| {
            *n <= UNIT_MAX_DECIMALS && d.decimals.trim().bytes().all(|b| b.is_ascii_digit())
        })
        .ok_or_else(|| {
            invalid(
                "Decimals",
                format!("type a whole number from 0 to {UNIT_MAX_DECIMALS}"),
            )
        })?;
    let uqc = d.uqc.trim();
    let uqc_ok = uqc.is_empty() || (uqc.len() == 3 && uqc.bytes().all(|b| b.is_ascii_uppercase()));
    if !uqc_ok {
        return Err(invalid("UQC", "three capital letters, for example NOS"));
    }
    Ok(MasterSnapshot::Unit(Unit {
        id: editing.unwrap_or(UnitId(UNASSIGNED_ID)),
        symbol,
        formal_name,
        decimals,
        kind: UnitKind::Simple,
        uqc: (!uqc.is_empty()).then(|| uqc.to_owned()),
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

fn build_stock_group(
    d: &TreeDraft,
    books: &Books,
    editing: Option<StockGroupId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let orig = original(
        editing.and_then(|id| masters.stock_groups.get(&id)),
        editing.is_some(),
        MasterKind::StockGroup,
    )?;
    let name = clean_name("Name", &d.name)?;
    check_unique(
        masters,
        MasterKind::StockGroup,
        editing.map(|g| g.0),
        std::slice::from_ref(&name),
    )?;
    if let Some(p) = d.parent {
        if !masters.stock_groups.contains_key(&StockGroupId(p)) {
            return Err(invalid("Under", "the parent stock group does not exist"));
        }
        if let Some(id) = editing {
            let parent_of = |g: u64| {
                masters
                    .stock_groups
                    .get(&StockGroupId(g))
                    .and_then(|x| x.parent)
                    .map(|x| x.0)
            };
            if creates_cycle(id.0, p, parent_of, masters.stock_groups.len()) {
                return Err(invalid(
                    "Under",
                    "a stock group cannot sit under itself or its own sub-groups",
                ));
            }
        }
    }
    Ok(MasterSnapshot::StockGroup(StockGroup {
        id: editing.unwrap_or(StockGroupId(UNASSIGNED_ID)),
        name,
        parent: d.parent.map(StockGroupId),
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

fn build_godown(
    d: &TreeDraft,
    books: &Books,
    editing: Option<GodownId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let orig = original(
        editing.and_then(|id| masters.godowns.get(&id)),
        editing.is_some(),
        MasterKind::Godown,
    )?;
    let name = clean_name("Name", &d.name)?;
    check_unique(
        masters,
        MasterKind::Godown,
        editing.map(|g| g.0),
        std::slice::from_ref(&name),
    )?;
    if let Some(p) = d.parent {
        if !masters.godowns.contains_key(&GodownId(p)) {
            return Err(invalid("Under", "the parent godown does not exist"));
        }
        if let Some(id) = editing {
            let parent_of = |g: u64| {
                masters
                    .godowns
                    .get(&GodownId(g))
                    .and_then(|x| x.parent)
                    .map(|x| x.0)
            };
            if creates_cycle(id.0, p, parent_of, masters.godowns.len()) {
                return Err(invalid(
                    "Under",
                    "a godown cannot sit under itself or its own sub-godowns",
                ));
            }
        }
    }
    Ok(MasterSnapshot::Godown(Godown {
        id: editing.unwrap_or(GodownId(UNASSIGNED_ID)),
        name,
        parent: d.parent.map(GodownId),
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

fn build_cost_category(
    d: &TreeDraft,
    books: &Books,
    editing: Option<CostCategoryId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    original(
        editing.and_then(|id| masters.cost_categories.get(&id)),
        editing.is_some(),
        MasterKind::CostCategory,
    )?;
    let name = clean_name("Name", &d.name)?;
    check_unique(
        masters,
        MasterKind::CostCategory,
        editing.map(|c| c.0),
        std::slice::from_ref(&name),
    )?;
    Ok(MasterSnapshot::CostCategory(CostCategory {
        id: editing.unwrap_or(CostCategoryId(UNASSIGNED_ID)),
        name,
    }))
}

fn build_cost_centre(
    d: &CostCentreDraft,
    books: &Books,
    editing: Option<CostCentreId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    original(
        editing.and_then(|id| masters.cost_centres.get(&id)),
        editing.is_some(),
        MasterKind::CostCentre,
    )?;
    let name = clean_name("Name", &d.name)?;
    check_unique(
        masters,
        MasterKind::CostCentre,
        editing.map(|c| c.0),
        std::slice::from_ref(&name),
    )?;
    let category = d
        .category
        .ok_or_else(|| invalid("Category", "choose a cost category"))?;
    if !masters.cost_categories.contains_key(&category) {
        return Err(invalid("Category", "the cost category does not exist"));
    }
    if let Some(p) = d.parent {
        if !masters.cost_centres.contains_key(&p) {
            return Err(invalid("Under", "the parent cost centre does not exist"));
        }
        if let Some(id) = editing {
            let parent_of = |c: u64| {
                masters
                    .cost_centres
                    .get(&CostCentreId(c))
                    .and_then(|x| x.parent)
                    .map(|x| x.0)
            };
            if creates_cycle(id.0, p.0, parent_of, masters.cost_centres.len()) {
                return Err(invalid(
                    "Under",
                    "a cost centre cannot sit under itself or its own sub-centres",
                ));
            }
        }
    }
    Ok(MasterSnapshot::CostCentre(CostCentre {
        id: editing.unwrap_or(CostCentreId(UNASSIGNED_ID)),
        name,
        category,
        parent: d.parent,
    }))
}

fn build_stock_item(
    d: &StockItemDraft,
    books: &Books,
    editing: Option<StockItemId>,
) -> Result<MasterSnapshot, MasterError> {
    let masters = &books.masters;
    let orig = original(
        editing.and_then(|id| masters.stock_items.get(&id)),
        editing.is_some(),
        MasterKind::StockItem,
    )?;
    let labels = clean_labels(&d.name, &d.aliases)?;
    check_unique(
        masters,
        MasterKind::StockItem,
        editing.map(|i| i.0),
        &labels,
    )?;
    if let Some(g) = d.group
        && !masters.stock_groups.contains_key(&g)
    {
        return Err(invalid("Under", "the stock group does not exist"));
    }
    let unit = d.unit.ok_or_else(|| invalid("Unit", "choose a unit"))?;
    if !masters.units.contains_key(&unit) {
        return Err(invalid("Unit", "the unit does not exist"));
    }
    check_item_rules(d, books, orig, editing, unit)?;
    let mut names = labels.into_iter();
    let name = names.next().unwrap_or_default();
    Ok(MasterSnapshot::StockItem(StockItem {
        id: editing.unwrap_or(StockItemId(UNASSIGNED_ID)),
        name,
        aliases: names.collect(),
        group: d.group,
        unit,
        alt_unit: orig.and_then(|o| o.alt_unit),
        hsn: parse_hsn(&d.hsn)?,
        gst_history: orig.map(|o| o.gst_history.clone()).unwrap_or_default(),
        valuation: d.valuation,
        batches_enabled: d.batches,
        opening: orig.map(|o| o.opening.clone()).unwrap_or_default(),
        standard_costs: orig.map(|o| o.standard_costs.clone()).unwrap_or_default(),
        tally: orig.and_then(|o| o.tally.clone()),
    }))
}

/// Rules that depend on what the item already has: a unit change, batches and standard cost.
fn check_item_rules(
    d: &StockItemDraft,
    books: &Books,
    orig: Option<&StockItem>,
    editing: Option<StockItemId>,
    unit: UnitId,
) -> Result<(), MasterError> {
    if let (Some(o), Some(id)) = (orig, editing)
        && o.unit != unit
    {
        let has_stock = !o.opening.is_empty()
            || books.stock_moves.iter().any(|m| m.item == id)
            || books
                .vouchers
                .values()
                .any(|v| v.inventory_lines.iter().any(|l| l.item == id));
        if has_stock {
            return Err(invalid(
                "Unit",
                "the item already has stock or vouchers, so its unit cannot change",
            ));
        }
    }
    if d.batches && !books.company.features.batches && !orig.is_some_and(|o| o.batches_enabled) {
        return Err(MasterError::FeatureOff { feature: "Batches" });
    }
    if d.valuation == ValuationMethod::StandardCost
        && !orig.is_some_and(|o| !o.standard_costs.is_empty())
    {
        return Err(unsupported(
            "standard cost valuation",
            "standard cost rates are not entered on this screen yet, and valuation has no rate without them",
        ));
    }
    Ok(())
}
