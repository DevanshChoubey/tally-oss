//! Staging and mapping to events (spec 6.7 step 3). **Card B3** implements this module.
//!
//! Frozen interface (plan card B3):
//!
//! ```text
//! pub fn map_to_events(masters: &[TallyMaster], vouchers: &[TVoucher]) -> Result<Vec<Event>, MapError>;
//! ```
//!
//! # What is mapped
//!
//! Events come out in dependency order: reserved groups, custom groups (parents first), ledgers,
//! voucher types, then vouchers in the order given.
//!
//! * **Groups.** A group whose `RESERVEDNAME` child (or, when absent, whose `NAME`) is the name of
//!   one of the 28 reserved groups is that reserved group: it is never created again, it is
//!   altered to carry the Tally identity (and the user's name for it). Any other group needs a
//!   parent that resolves, otherwise [`MapError::UnknownParent`]. Ledgers resolve their parent
//!   against the same table, so a ledger under a reserved group gets that group's id.
//! * **Ledgers.** Name, group, Tally identity, bill-wise and cost-centre flags. An opening balance
//!   other than zero is [`MapError::Unsupported`]: the sign convention of a master's
//!   `OPENINGBALANCE` is documented nowhere in the research (R8 documents voucher lines only), so
//!   the value stays pending instead of being guessed.
//! * **Voucher types.** The base kind comes from the `PARENT` (a custom type) or the name (a
//!   built-in one), matched against the names in research note R1.
//! * **Vouchers.** Accounting vouchers with ledger lines and bill allocations. Inventory lines are
//!   [`MapError::Unsupported`]. Every element the mapping does not model is kept in
//!   `Voucher::extra` (rule A9).
//! * **Other masters** (stock items, units, godowns, cost centres and categories, stock groups)
//!   are not mapped yet: each one raises a [`MapWarning`] saying so. Vouchers that use them are
//!   refused, so no number depends on a master that was skipped.
//!
//! # Ids
//!
//! Ids are allocated in this batch, deterministically, for a company that holds only the seeded
//! reserved groups (ids 1 to 28): custom groups start at 29, every other kind at 1. Applying the
//! events to a company with other masters needs an id base the frozen interface does not carry
//! (see the interface request filed with this card).
//!
//! # Warnings
//!
//! Master records have no place for unknown fields, so a field that is not modelled is reported
//! as a [`MapWarning`] (rule A9: a dropped field is a reported import warning).
//! [`map_with_warnings`] returns them; [`map_to_events`] is the frozen entry point and returns
//! the events only.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use ledgerly_core::{
    Decimal, DrCr, GroupId, LedgerId, MoneyError, Paise, SignedAmount, TallyRef, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Group, Ledger, LedgerLine,
    MasterSnapshot, NumberingConfig, NumberingMethod, RestartPeriod, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use ledgerly_store::Event;
use ledgerly_tally_xml::masters::{MasterRecord, RawElement, TallyMaster};
use ledgerly_tally_xml::vouchers::{RawNode, TBillAllocation, TLedgerEntry, TVoucher};
use thiserror::Error;

/// The name Tally gives the reserved profit and loss ledger (research R1, "Reserved ledgers").
const PROFIT_AND_LOSS_LEDGER: &str = "Profit & Loss A/c";

/// Why a set of Tally objects could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MapError {
    /// A group or ledger names a parent group that does not exist.
    #[error(
        "{kind} '{name}' is under group '{parent}', which is not in the export. Export the \
         masters again with all groups included."
    )]
    UnknownParent {
        /// `group` or `ledger`.
        kind: &'static str,
        /// Name of the object.
        name: String,
        /// The parent name it refers to.
        parent: String,
    },
    /// Groups name each other as parents in a loop.
    #[error("group '{name}' is part of a parent loop, so it has no path to a primary group")]
    GroupCycle {
        /// A group in the loop.
        name: String,
    },
    /// A ledger has no parent group.
    #[error(
        "ledger '{ledger}' has no parent group, so it cannot be placed in the chart of accounts"
    )]
    LedgerWithoutGroup {
        /// Ledger name.
        ledger: String,
    },
    /// Two objects of one kind share a name, so references by name would be ambiguous.
    #[error("two {kind} masters are named '{name}'; names must be unique to map vouchers")]
    DuplicateName {
        /// Kind of master.
        kind: &'static str,
        /// The repeated name.
        name: String,
    },
    /// Two objects carry the same Tally GUID.
    #[error("GUID '{guid}' appears on more than one object in the import")]
    DuplicateGuid {
        /// The repeated GUID.
        guid: String,
    },
    /// A voucher has no GUID, so it cannot be matched on a re-import.
    #[error("{voucher} has no GUID; staging is keyed by GUID")]
    MissingGuid {
        /// Voucher description.
        voucher: String,
    },
    /// A group's `RESERVEDNAME` is not one of the 28 reserved groups.
    #[error("group '{group}' says it is the reserved group '{reserved_name}', which is not known")]
    UnknownReservedName {
        /// Group name.
        group: String,
        /// The reserved name it claims.
        reserved_name: String,
    },
    /// A reserved group sits under a different parent than the reserved one.
    #[error("reserved group '{group}' has been moved under another parent in Tally; not mapped")]
    ReservedGroupMoved {
        /// Group name in the export.
        group: String,
    },
    /// A voucher names a ledger that is not in the masters.
    #[error("{voucher} uses ledger '{ledger}', which is not in the masters")]
    UnknownLedger {
        /// Voucher description.
        voucher: String,
        /// Ledger name.
        ledger: String,
    },
    /// A voucher names a voucher type that is not in the masters.
    #[error("{voucher} has voucher type '{voucher_type}', which is not in the masters")]
    UnknownVoucherType {
        /// Voucher description.
        voucher: String,
        /// Voucher type name.
        voucher_type: String,
    },
    /// Bill allocations do not add up to their ledger line under the sign convention.
    #[error(
        "{voucher}: bill allocations on '{ledger}' total {allocated} paise as written, but the \
         line is {line} paise"
    )]
    BillSumMismatch {
        /// Voucher description.
        voucher: String,
        /// Ledger name.
        ledger: String,
        /// Sum of the allocations as written.
        allocated: i64,
        /// The line in the same convention.
        line: i64,
    },
    /// A field holds a value that is not understood.
    #[error("{object}: <{tag}> value '{value}' {reason}")]
    BadValue {
        /// Object description.
        object: String,
        /// Tag name.
        tag: String,
        /// The text found.
        value: String,
        /// What is wrong.
        reason: String,
    },
    /// Something that is not handled yet (listed as pending, never guessed).
    #[error("not supported: {what}: {reason}")]
    Unsupported {
        /// What was met.
        what: String,
        /// Why it is pending.
        reason: String,
    },
    /// Money arithmetic failed.
    #[error("amount error in {context}: {source}")]
    Money {
        /// Where.
        context: String,
        /// Cause.
        source: MoneyError,
    },
    /// More ids were needed than a `u64` holds.
    #[error("too many {what} to number")]
    IdOverflow {
        /// What was being numbered.
        what: &'static str,
    },
}

/// A non-fatal finding: something in the export was not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapWarning {
    /// The object it is about, such as `ledger 'Sharma Hardware'`.
    pub object: String,
    /// What was not imported and why.
    pub message: String,
}

/// Events and warnings from one mapping run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// Events in dependency order.
    pub events: Vec<Event>,
    /// Everything that was not imported, in the order it was met.
    pub warnings: Vec<MapWarning>,
}

/// Maps staged Tally masters and vouchers to events, in dependency order.
///
/// # Errors
/// [`MapError`] for the first object that cannot be mapped; nothing is returned in that case.
pub fn map_to_events(
    masters: &[TallyMaster],
    vouchers: &[TVoucher],
) -> Result<Vec<Event>, MapError> {
    map_with_warnings(masters, vouchers).map(|mapped| mapped.events)
}

/// Like [`map_to_events`], and also returns the warnings for fields and masters not imported.
///
/// # Errors
/// [`MapError`] for the first object that cannot be mapped.
pub fn map_with_warnings(
    masters: &[TallyMaster],
    vouchers: &[TVoucher],
) -> Result<Mapped, MapError> {
    let mut state = State::new()?;
    state.map_groups(masters)?;
    state.map_ledgers(masters)?;
    state.map_voucher_types(masters)?;
    state.warn_unmapped_masters(masters);
    for voucher in vouchers {
        state.map_voucher(voucher)?;
    }
    Ok(Mapped {
        events: state.events,
        warnings: state.warnings,
    })
}

/// A counter that refuses to wrap.
struct Counter(u64);

impl Counter {
    fn take(&mut self, what: &'static str) -> Result<u64, MapError> {
        let current = self.0;
        self.0 = current
            .checked_add(1)
            .ok_or(MapError::IdOverflow { what })?;
        Ok(current)
    }
}

struct State {
    events: Vec<Event>,
    warnings: Vec<MapWarning>,
    guids: BTreeSet<String>,
    groups: BTreeMap<String, GroupId>,
    ledgers: BTreeMap<String, LedgerId>,
    voucher_types: BTreeMap<String, VoucherTypeId>,
    next_group: Counter,
    next_ledger: Counter,
    next_voucher_type: Counter,
    next_voucher: Counter,
}

impl State {
    fn new() -> Result<Self, MapError> {
        let seeds = reserved_groups();
        let first_free = seeds
            .iter()
            .map(|g| g.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(MapError::IdOverflow { what: "groups" })?;
        Ok(State {
            events: Vec::new(),
            warnings: Vec::new(),
            guids: BTreeSet::new(),
            groups: seeds.iter().map(|g| (g.name.clone(), g.id)).collect(),
            ledgers: BTreeMap::new(),
            voucher_types: BTreeMap::new(),
            next_group: Counter(first_free),
            next_ledger: Counter(1),
            next_voucher_type: Counter(1),
            next_voucher: Counter(1),
        })
    }

    fn warn(&mut self, object: &str, message: impl Into<String>) {
        self.warnings.push(MapWarning {
            object: object.to_owned(),
            message: message.into(),
        });
    }

    fn register_guid(&mut self, guid: &str) -> Result<(), MapError> {
        if self.guids.insert(guid.to_owned()) {
            Ok(())
        } else {
            Err(MapError::DuplicateGuid {
                guid: guid.to_owned(),
            })
        }
    }

    /// Warns about every child element and attribute of `rec` that is not in `consumed`.
    fn warn_dropped(&mut self, object: &str, rec: &MasterRecord, consumed: &[&str]) {
        for element in &rec.extra {
            if !consumed.contains(&element.name.as_str()) {
                self.warn(
                    object,
                    format!(
                        "field <{}> is not imported (kept only in the raw capture)",
                        element.name
                    ),
                );
            }
        }
        for (key, _) in &rec.attributes {
            if key != "NAME" && key != "ACTION" {
                self.warn(
                    object,
                    format!("attribute {key} is not imported (kept only in the raw capture)"),
                );
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Groups
    // ---------------------------------------------------------------------------------------

    fn map_groups(&mut self, masters: &[TallyMaster]) -> Result<(), MapError> {
        let seeds = reserved_groups();
        let mut reserved: Vec<(&MasterRecord, &Group)> = Vec::new();
        let mut custom: Vec<&MasterRecord> = Vec::new();
        for master in masters {
            let TallyMaster::Group(rec) = master else {
                continue;
            };
            let explicit = extra_text(&rec.extra, "RESERVEDNAME");
            let key = explicit.unwrap_or(rec.name.as_str());
            match (seeds.iter().find(|g| g.name == key), explicit) {
                (Some(seed), _) => reserved.push((rec, seed)),
                (None, Some(claimed)) => {
                    return Err(MapError::UnknownReservedName {
                        group: rec.name.clone(),
                        reserved_name: claimed.to_owned(),
                    });
                }
                (None, None) => custom.push(rec),
            }
        }
        self.map_reserved_groups(&reserved)?;
        self.map_custom_groups(custom)
    }

    fn map_reserved_groups(
        &mut self,
        reserved: &[(&MasterRecord, &Group)],
    ) -> Result<(), MapError> {
        let mut seen: BTreeSet<GroupId> = BTreeSet::new();
        for &(rec, seed) in reserved {
            if !seen.insert(seed.id) {
                return Err(MapError::DuplicateName {
                    kind: "group",
                    name: rec.name.clone(),
                });
            }
            self.register_guid(&rec.tally_ref.guid)?;
            self.groups.remove(&seed.name);
            if self.groups.insert(rec.name.clone(), seed.id).is_some() {
                return Err(MapError::DuplicateName {
                    kind: "group",
                    name: rec.name.clone(),
                });
            }
        }
        for &(rec, seed) in reserved {
            let actual = match normalize_parent(rec.parent.as_deref()) {
                None => None,
                Some(parent) => {
                    let found = self.groups.get(&parent).copied();
                    Some(found.ok_or_else(|| MapError::UnknownParent {
                        kind: "group",
                        name: rec.name.clone(),
                        parent,
                    })?)
                }
            };
            if actual != seed.parent {
                return Err(MapError::ReservedGroupMoved {
                    group: rec.name.clone(),
                });
            }
            let mut after = seed.clone();
            after.name.clone_from(&rec.name);
            after.tally = Some(rec.tally_ref.clone());
            self.warn_dropped(&format!("group '{}'", rec.name), rec, &["RESERVEDNAME"]);
            self.events.push(Event::MasterAltered {
                before: MasterSnapshot::Group(seed.clone()),
                after: MasterSnapshot::Group(after),
            });
        }
        Ok(())
    }

    fn map_custom_groups(&mut self, mut pending: Vec<&MasterRecord>) -> Result<(), MapError> {
        while !pending.is_empty() {
            let before = pending.len();
            let mut deferred: Vec<&MasterRecord> = Vec::new();
            for rec in std::mem::take(&mut pending) {
                let Some(parent) = normalize_parent(rec.parent.as_deref()) else {
                    return Err(MapError::Unsupported {
                        what: format!("custom primary group '{}'", rec.name),
                        reason: "the nature of a new primary group is not read from the export \
                                 yet (pending)"
                            .to_owned(),
                    });
                };
                match self.groups.get(&parent).copied() {
                    Some(parent_id) => self.create_group(rec, parent_id)?,
                    None => deferred.push(rec),
                }
            }
            if deferred.len() == before {
                return Err(stuck_group_error(&deferred));
            }
            pending = deferred;
        }
        Ok(())
    }

    fn create_group(&mut self, rec: &MasterRecord, parent: GroupId) -> Result<(), MapError> {
        if self.groups.contains_key(&rec.name) {
            return Err(MapError::DuplicateName {
                kind: "group",
                name: rec.name.clone(),
            });
        }
        self.register_guid(&rec.tally_ref.guid)?;
        let id = GroupId(self.next_group.take("groups")?);
        self.groups.insert(rec.name.clone(), id);
        self.warn_dropped(&format!("group '{}'", rec.name), rec, &[]);
        self.events
            .push(Event::MasterCreated(MasterSnapshot::Group(Group {
                id,
                name: rec.name.clone(),
                aliases: Vec::new(),
                parent: Some(parent),
                reserved: None,
                nature: None,
                affects_gross_profit: false,
                tally: Some(rec.tally_ref.clone()),
            })));
        Ok(())
    }

    // ---------------------------------------------------------------------------------------
    // Ledgers
    // ---------------------------------------------------------------------------------------

    fn map_ledgers(&mut self, masters: &[TallyMaster]) -> Result<(), MapError> {
        for master in masters {
            if let TallyMaster::Ledger(rec) = master {
                self.map_ledger(rec)?;
            }
        }
        Ok(())
    }

    fn map_ledger(&mut self, rec: &MasterRecord) -> Result<(), MapError> {
        let object = format!("ledger '{}'", rec.name);
        let Some(parent) = normalize_parent(rec.parent.as_deref()) else {
            if rec.name == PROFIT_AND_LOSS_LEDGER {
                self.warn(
                    &object,
                    "the reserved profit and loss ledger is not imported; the books have their own",
                );
                return Ok(());
            }
            return Err(MapError::LedgerWithoutGroup {
                ledger: rec.name.clone(),
            });
        };
        let found = self.groups.get(&parent).copied();
        let group = found.ok_or_else(|| MapError::UnknownParent {
            kind: "ledger",
            name: rec.name.clone(),
            parent,
        })?;
        if self.ledgers.contains_key(&rec.name) {
            return Err(MapError::DuplicateName {
                kind: "ledger",
                name: rec.name.clone(),
            });
        }
        check_opening_is_zero(&object, &rec.extra)?;
        let bill_wise = element_flag(&object, &rec.extra, "ISBILLWISEON")?
            .unwrap_or(false)
            .then_some(BillWiseConfig {
                default_credit_days: None,
            });
        let cost_centres_applicable =
            element_flag(&object, &rec.extra, "ISCOSTCENTRESON")?.unwrap_or(false);
        self.register_guid(&rec.tally_ref.guid)?;
        let id = LedgerId(self.next_ledger.take("ledgers")?);
        self.ledgers.insert(rec.name.clone(), id);
        self.warn_dropped(
            &object,
            rec,
            &["OPENINGBALANCE", "ISBILLWISEON", "ISCOSTCENTRESON"],
        );
        self.events
            .push(Event::MasterCreated(MasterSnapshot::Ledger(Ledger {
                id,
                name: rec.name.clone(),
                aliases: Vec::new(),
                group,
                opening: SignedAmount::dr(Paise::ZERO),
                bill_wise,
                cost_centres_applicable,
                party: None,
                bank: None,
                tax: None,
                gst: None,
                reserved: None,
                tally: Some(rec.tally_ref.clone()),
            })));
        Ok(())
    }

    // ---------------------------------------------------------------------------------------
    // Voucher types and skipped masters
    // ---------------------------------------------------------------------------------------

    fn map_voucher_types(&mut self, masters: &[TallyMaster]) -> Result<(), MapError> {
        for master in masters {
            if let TallyMaster::VoucherType(rec) = master {
                self.map_voucher_type(rec)?;
            }
        }
        Ok(())
    }

    fn map_voucher_type(&mut self, rec: &MasterRecord) -> Result<(), MapError> {
        let object = format!("voucher type '{}'", rec.name);
        let base_name = normalize_parent(rec.parent.as_deref()).unwrap_or_else(|| rec.name.clone());
        let base = base_kind(&base_name).ok_or_else(|| MapError::Unsupported {
            what: format!("{object} based on '{base_name}'"),
            reason: "this base voucher type is not one of the 18 kinds in v1 (pending)".to_owned(),
        })?;
        if self.voucher_types.contains_key(&rec.name) {
            return Err(MapError::DuplicateName {
                kind: "voucher type",
                name: rec.name.clone(),
            });
        }
        self.register_guid(&rec.tally_ref.guid)?;
        let id = VoucherTypeId(self.next_voucher_type.take("voucher types")?);
        self.voucher_types.insert(rec.name.clone(), id);
        self.warn_dropped(&object, rec, &[]);
        self.warn(
            &object,
            "numbering settings are not imported; numbers from Tally are kept as typed",
        );
        self.events
            .push(Event::MasterCreated(MasterSnapshot::VoucherType(
                VoucherType {
                    id,
                    name: rec.name.clone(),
                    base,
                    abbreviation: rec.name.clone(),
                    numbering: NumberingConfig {
                        method: NumberingMethod::Manual,
                        prevent_renumbering: true,
                        prefix: String::new(),
                        suffix: String::new(),
                        width: 0,
                        restart: RestartPeriod::Never,
                        series: Vec::new(),
                    },
                    active: true,
                    tally: Some(rec.tally_ref.clone()),
                },
            )));
        Ok(())
    }

    fn warn_unmapped_masters(&mut self, masters: &[TallyMaster]) {
        for master in masters {
            let (kind, name) = match master {
                TallyMaster::StockItem(r) => ("stock item", &r.name),
                TallyMaster::Unit(r) => ("unit", &r.name),
                TallyMaster::Godown(r) => ("godown", &r.name),
                TallyMaster::CostCentre(r) => ("cost centre", &r.name),
                TallyMaster::CostCategory(r) => ("cost category", &r.name),
                TallyMaster::StockGroup(r) => ("stock group", &r.name),
                TallyMaster::Group(_) | TallyMaster::Ledger(_) | TallyMaster::VoucherType(_) => {
                    continue;
                }
            };
            self.warn(
                &format!("{kind} '{name}'"),
                "not imported: this kind of master is not supported yet; vouchers that use it \
                 are refused",
            );
        }
    }

    // ---------------------------------------------------------------------------------------
    // Vouchers
    // ---------------------------------------------------------------------------------------

    fn map_voucher(&mut self, v: &TVoucher) -> Result<(), MapError> {
        let label = voucher_label(v);
        let Some(guid) = v.guid.as_deref() else {
            return Err(MapError::MissingGuid { voucher: label });
        };
        if !v.inventory_entries.is_empty() {
            return Err(MapError::Unsupported {
                what: format!("{label} has inventory lines"),
                reason: "stock items are not mapped yet (pending)".to_owned(),
            });
        }
        let type_id = self.voucher_types.get(&v.voucher_type).copied();
        let type_id = type_id.ok_or_else(|| MapError::UnknownVoucherType {
            voucher: label.clone(),
            voucher_type: v.voucher_type.clone(),
        })?;
        let party = match v.party.as_deref() {
            Some(name) => Some(self.ledger_id(&label, name)?),
            None => None,
        };
        let mut ledger_lines = Vec::new();
        for entry in &v.ledger_entries {
            ledger_lines.push(self.map_line(&label, entry)?);
        }
        let status = status_of(&label, &v.extra)?;
        let invoice = node_flag(&label, &v.extra, "ISINVOICE")?.unwrap_or(false);
        self.register_guid(guid)?;
        let id = VoucherId(self.next_voucher.take("vouchers")?);
        self.events.push(Event::VoucherPosted(Voucher {
            id,
            type_id,
            number: v.number.clone(),
            series: None,
            date: v.date,
            supply_date: None,
            payment_date: None,
            status,
            mode: if invoice {
                VoucherMode::AccountingInvoice
            } else {
                VoucherMode::AsVoucher
            },
            party,
            place_of_supply: None,
            reverse_charge: false,
            reference: None,
            reference_date: None,
            narration: v.narration.clone().unwrap_or_default(),
            ledger_lines,
            inventory_lines: Vec::new(),
            gst: None,
            tally: Some(TallyRef {
                guid: guid.to_owned(),
                alter_id: v.alter_id,
            }),
            extra: voucher_extra(v),
        }));
        Ok(())
    }

    fn ledger_id(&self, label: &str, name: &str) -> Result<LedgerId, MapError> {
        self.ledgers
            .get(name)
            .copied()
            .ok_or_else(|| MapError::UnknownLedger {
                voucher: label.to_owned(),
                ledger: name.to_owned(),
            })
    }

    fn map_line(&mut self, label: &str, entry: &TLedgerEntry) -> Result<LedgerLine, MapError> {
        let ledger = self.ledger_id(label, &entry.ledger_name)?;
        let bills = self.map_bills(label, entry)?;
        for node in &entry.extra {
            self.warn(
                label,
                format!(
                    "<{}> on the line for '{}' is not imported",
                    node.name, entry.ledger_name
                ),
            );
        }
        Ok(LedgerLine {
            ledger,
            amount: SignedAmount {
                amount: entry.amount,
                side: entry.side,
            },
            bills,
            cost: Vec::new(),
            bank: None,
        })
    }

    /// Bill allocations keep the sign as written, so their side is derived from the line: the
    /// allocations must total the line in Tally's convention (debit negative), the same rule the
    /// model enforces (bill allocations sum to the line). Anything else is an error.
    fn map_bills(
        &mut self,
        label: &str,
        entry: &TLedgerEntry,
    ) -> Result<Vec<BillAllocation>, MapError> {
        if entry.bill_allocations.is_empty() {
            return Ok(Vec::new());
        }
        let money = |source: MoneyError| MapError::Money {
            context: format!("{label}, bills on '{}'", entry.ledger_name),
            source,
        };
        let line = match entry.side {
            DrCr::Dr => entry.amount.checked_neg().map_err(money)?,
            DrCr::Cr => entry.amount,
        };
        let mut total = Paise::ZERO;
        for bill in &entry.bill_allocations {
            total = total.checked_add(bill.amount).map_err(money)?;
        }
        if total != line {
            return Err(MapError::BillSumMismatch {
                voucher: label.to_owned(),
                ledger: entry.ledger_name.clone(),
                allocated: total.get(),
                line: line.get(),
            });
        }
        let mut out = Vec::new();
        for bill in &entry.bill_allocations {
            for node in &bill.extra {
                self.warn(
                    label,
                    format!("<{}> on bill '{}' is not imported", node.name, bill.name),
                );
            }
            let kind = bill_kind(label, bill)?;
            let debit_positive = bill.amount.checked_neg().map_err(money)?;
            out.push(BillAllocation {
                kind,
                name: bill.name.clone(),
                due_date: None,
                credit_days: None,
                amount: SignedAmount::from_signed(debit_positive).map_err(money)?,
            });
        }
        Ok(out)
    }
}

fn bill_kind(label: &str, bill: &TBillAllocation) -> Result<BillRefKind, MapError> {
    match bill.bill_type.as_deref() {
        Some("New Ref") => Ok(BillRefKind::New),
        Some("Agst Ref") => Ok(BillRefKind::Against),
        Some("Advance") => Ok(BillRefKind::Advance),
        Some("On Account") => Ok(BillRefKind::OnAccount),
        Some(other) => Err(MapError::Unsupported {
            what: format!("{label}: bill '{}' of type '{other}'", bill.name),
            reason: "bill type is not one of New Ref, Agst Ref, Advance, On Account (pending)"
                .to_owned(),
        }),
        None => Err(MapError::Unsupported {
            what: format!("{label}: bill '{}' without a type", bill.name),
            reason: "the kind of reference cannot be guessed (pending)".to_owned(),
        }),
    }
}

fn stuck_group_error(stuck: &[&MasterRecord]) -> MapError {
    let names: BTreeSet<&str> = stuck.iter().map(|r| r.name.as_str()).collect();
    for rec in stuck {
        let parent = normalize_parent(rec.parent.as_deref()).unwrap_or_default();
        if names.contains(parent.as_str()) {
            continue;
        }
        return MapError::UnknownParent {
            kind: "group",
            name: rec.name.clone(),
            parent,
        };
    }
    MapError::GroupCycle {
        name: stuck.first().map_or_default(|r| r.name.clone()),
    }
}

/// The parent name without control characters (Tally writes the root as a control character
/// followed by `Primary`), or `None` for a primary object.
fn normalize_parent(raw: Option<&str>) -> Option<String> {
    let cleaned: String = raw?.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed == "Primary" {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Text of the first child element called `tag`, trimmed; `None` when absent or empty.
fn extra_text<'a>(extra: &'a [RawElement], tag: &str) -> Option<&'a str> {
    extra
        .iter()
        .find(|e| e.name == tag)
        .map(|e| e.text.trim())
        .filter(|t| !t.is_empty())
}

fn bad_value(object: &str, tag: &str, value: &str, reason: &str) -> MapError {
    MapError::BadValue {
        object: object.to_owned(),
        tag: tag.to_owned(),
        value: value.to_owned(),
        reason: reason.to_owned(),
    }
}

fn parse_yes_no(object: &str, tag: &str, text: &str) -> Result<bool, MapError> {
    match text.trim() {
        "Yes" => Ok(true),
        "No" => Ok(false),
        other => Err(bad_value(object, tag, other, "is not Yes or No")),
    }
}

/// A Yes/No child of a master; `None` when the child is absent.
fn element_flag(object: &str, extra: &[RawElement], tag: &str) -> Result<Option<bool>, MapError> {
    match extra.iter().find(|e| e.name == tag) {
        Some(e) => parse_yes_no(object, tag, &e.text).map(Some),
        None => Ok(None),
    }
}

/// A Yes/No child of a voucher; `None` when the child is absent.
fn node_flag(object: &str, extra: &[RawNode], tag: &str) -> Result<Option<bool>, MapError> {
    match extra.iter().find(|n| n.name == tag) {
        Some(n) => parse_yes_no(object, tag, &n.text).map(Some),
        None => Ok(None),
    }
}

fn check_opening_is_zero(object: &str, extra: &[RawElement]) -> Result<(), MapError> {
    let Some(text) = extra_text(extra, "OPENINGBALANCE") else {
        return Ok(());
    };
    let value = Decimal::from_str(text).map_err(|_| {
        bad_value(
            object,
            "OPENINGBALANCE",
            text,
            "is not a plain decimal amount",
        )
    })?;
    if value.is_zero() {
        Ok(())
    } else {
        Err(MapError::Unsupported {
            what: format!("{object} has an opening balance of {text}"),
            reason: "the debit/credit sign of a master's OPENINGBALANCE is not documented (R8 \
                     covers voucher lines only), so it is pending"
                .to_owned(),
        })
    }
}

/// Base kind from a Tally voucher type name (names from research R1).
fn base_kind(name: &str) -> Option<BaseVoucherKind> {
    Some(match name {
        "Contra" => BaseVoucherKind::Contra,
        "Payment" => BaseVoucherKind::Payment,
        "Receipt" => BaseVoucherKind::Receipt,
        "Journal" => BaseVoucherKind::Journal,
        "Sales" => BaseVoucherKind::Sales,
        "Purchase" => BaseVoucherKind::Purchase,
        "Credit Note" => BaseVoucherKind::CreditNote,
        "Debit Note" => BaseVoucherKind::DebitNote,
        "Memorandum" => BaseVoucherKind::Memorandum,
        "Reversing Journal" => BaseVoucherKind::ReversingJournal,
        "Receipt Note" => BaseVoucherKind::ReceiptNote,
        "Delivery Note" => BaseVoucherKind::DeliveryNote,
        "Rejection In" => BaseVoucherKind::RejectionIn,
        "Rejection Out" => BaseVoucherKind::RejectionOut,
        "Stock Journal" => BaseVoucherKind::StockJournal,
        "Physical Stock" => BaseVoucherKind::PhysicalStock,
        "Sales Order" => BaseVoucherKind::SalesOrder,
        "Purchase Order" => BaseVoucherKind::PurchaseOrder,
        _ => return None,
    })
}

fn voucher_label(v: &TVoucher) -> String {
    format!(
        "{} voucher {} dated {}",
        v.voucher_type,
        v.number.as_deref().unwrap_or("(no number)"),
        v.date
    )
}

/// Status from the `ISCANCELLED`, `ISOPTIONAL` and `ISPOSTDATED` flags. A voucher with none of
/// them set is regular; with more than one set it is refused.
fn status_of(label: &str, extra: &[RawNode]) -> Result<VoucherStatus, MapError> {
    let flags = [
        ("ISCANCELLED", VoucherStatus::Cancelled),
        ("ISOPTIONAL", VoucherStatus::Optional),
        ("ISPOSTDATED", VoucherStatus::PostDated),
    ];
    let mut set: Vec<VoucherStatus> = Vec::new();
    for (tag, status) in flags {
        if node_flag(label, extra, tag)? == Some(true) {
            set.push(status);
        }
    }
    match set.as_slice() {
        [] => Ok(VoucherStatus::Regular),
        [one] => Ok(*one),
        [..] => Err(MapError::Unsupported {
            what: format!("{label} is flagged cancelled, optional or post-dated more than once"),
            reason: "the combination has no single status in the books (pending)".to_owned(),
        }),
    }
}

/// Every unmodelled element and attribute of the voucher, keyed by tag (attributes as `@NAME`).
/// A repeated key gets `#2`, `#3`, ... so nothing is overwritten.
fn voucher_extra(v: &TVoucher) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in &v.attrs {
        insert_unique(&mut out, &format!("@{key}"), value.clone());
    }
    for node in &v.extra {
        insert_unique(&mut out, &node.name, node_value(node));
    }
    out
}

fn insert_unique(map: &mut BTreeMap<String, String>, key: &str, value: String) {
    if !map.contains_key(key) {
        map.insert(key.to_owned(), value);
        return;
    }
    let mut n: u64 = 2;
    loop {
        let candidate = format!("{key}#{n}");
        if let std::collections::btree_map::Entry::Vacant(e) = map.entry(candidate) {
            e.insert(value);
            return;
        }
        n = n.saturating_add(1);
    }
}

/// A leaf's text, or the element written back as XML when it has attributes or children.
fn node_value(node: &RawNode) -> String {
    if node.attrs.is_empty() && node.children.is_empty() {
        return node.text.clone();
    }
    let mut out = String::new();
    write_node(node, &mut out);
    out
}

fn write_node(node: &RawNode, out: &mut String) {
    out.push('<');
    out.push_str(&node.name);
    for (key, value) in &node.attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(&escape(value));
        out.push('"');
    }
    out.push('>');
    out.push_str(&escape(&node.text));
    for child in &node.children {
        write_node(child, out);
    }
    out.push_str("</");
    out.push_str(&node.name);
    out.push('>');
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
