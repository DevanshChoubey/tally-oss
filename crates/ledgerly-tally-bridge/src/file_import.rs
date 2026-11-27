//! Import from a file in the other program's XML import form into a company that is already open
//! (S22).
//!
//! The help page on importing (<https://help.tallysolutions.com/import-data-from-xml-or-json/>,
//! read 2026-11-27) imports masters or transactions from a file into the open company. For
//! masters it asks how to treat a master that already exists, with the three choices on
//! <https://help.tallysolutions.com/import-data-faq/> (read 2026-11-27): Combine Opening Balance,
//! Ignore Duplicates and Modify with New Data ([`MasterBehaviour`]). Transactions need their
//! masters in the company already; anything that cannot be imported is listed as an exception.
//!
//! This module turns one master or voucher into the change the facade should make, resolved by
//! name against the books as they are at that moment. The facade runs each change through the
//! engine (so the period lock and the voucher checks apply) and counts the outcome.
//!
//! # Derived defaults
//!
//! The sample import file on <https://help.tallysolutions.com/sample-xml/> (read 2026-11-27)
//! marks only `NAME` and `PARENT` as mandatory for a ledger, and its sales voucher carries no
//! `GUID` and none of the `ISCANCELLED`, `ISOPTIONAL`, `ISPOSTDATED` or `ISINVOICE` flags. So, in a
//! file (unlike the live pull, which asks for every field):
//! - an absent `OPENINGBALANCE` on a new ledger is no opening balance (zero);
//! - an absent status flag means the voucher is not cancelled, optional or post-dated;
//! - an absent `ISINVOICE` means the voucher is in voucher mode, as the sample's "As Voucher";
//! - an absent `GUID` means the object carries no identity from the other program.
//!
//! A master's `OPENINGBALANCE` is negative for a debit and positive for a credit (the same
//! derived convention as [`crate::import`]).
//!
//! The FAQ says a voucher type with automatic numbering numbers the imported vouchers itself; the
//! number from the file is then kept in the voucher's `extra` under `VOUCHERNUMBER`.

use std::str::FromStr;

use ledgerly_core::{
    Decimal, DrCr, GroupId, LedgerId, MoneyError, Paise, RoundingRule, SignedAmount, TallyRef,
    VoucherId, round_rupees_to_paise,
};
use ledgerly_model::{
    BillAllocation, BillWiseConfig, Books, Group, Ledger, LedgerLine, Masters, NumberingMethod,
    Voucher, VoucherMode, VoucherStatus,
};
pub use ledgerly_tally_xml::masters::{ImportMaster, ImportMasterKind};
use ledgerly_tally_xml::masters::{RawElement, decode_xml_bytes, parse_import_masters_bytes};
pub use ledgerly_tally_xml::vouchers::TVoucher;
use ledgerly_tally_xml::vouchers::{TLedgerEntry, parse_vouchers_xml};

use crate::mapping::{
    MapError, bill_kind, element_flag, extra_text, insert_unique, node_flag, normalize_parent,
    voucher_extra, voucher_label,
};

/// How a master that already exists in the company is treated (the import screen's "Behaviour
/// of import if masters already exists").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterBehaviour {
    /// Add the file's opening balance to the existing ledger's opening balance.
    CombineOpeningBalance,
    /// Keep the existing master as it is.
    IgnoreDuplicates,
    /// Replace the existing master's group and opening balance with the file's.
    ModifyWithNewData,
}

/// The change one master asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterStep {
    /// Create this group (its id is 0: the facade numbers it).
    CreateGroup(Group),
    /// Alter the existing group to this.
    AlterGroup(Group),
    /// Create this ledger (its id is 0: the facade numbers it).
    CreateLedger(Ledger),
    /// Alter the existing ledger to this; `combined` when only the opening balance was added to.
    AlterLedger {
        /// The ledger after the change.
        ledger: Ledger,
        /// Whether this is Combine Opening Balance.
        combined: bool,
    },
    /// Nothing to change, and why.
    Ignore(String),
}

/// Something in the file that was not imported, and why (the import's exceptions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exception {
    /// The object, such as `ledger 'Kumar Textiles'`.
    pub object: String,
    /// Why it was not imported.
    pub reason: String,
}

/// A master's step and the fields of it that were not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterPlan {
    /// The change to make.
    pub step: MasterStep,
    /// Fields in the file that are not imported, as messages.
    pub dropped: Vec<String>,
}

/// What a file holds, with the reader's warnings as `offset N: message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContents<T> {
    /// The objects, in document order.
    pub items: Vec<T>,
    /// Non-fatal problems the reader met.
    pub warnings: Vec<String>,
}

/// Reads the masters of an import file.
///
/// # Errors
/// The reader's error as text when the file is not a readable XML document of masters.
pub fn read_masters(bytes: &[u8]) -> Result<FileContents<ImportMaster>, String> {
    let parsed = parse_import_masters_bytes(bytes).map_err(|e| e.to_string())?;
    Ok(FileContents {
        items: parsed.masters,
        warnings: parsed
            .warnings
            .iter()
            .map(|w| format!("offset {}: {}", w.offset, w.message))
            .collect(),
    })
}

/// Reads the vouchers of an import file.
///
/// # Errors
/// The reader's error as text when the file is not a readable XML document of vouchers.
pub fn read_vouchers(bytes: &[u8]) -> Result<FileContents<TVoucher>, String> {
    let text = decode_xml_bytes(bytes).map_err(|e| e.to_string())?;
    let (items, warnings) = parse_vouchers_xml(&text).map_err(|e| e.to_string())?;
    Ok(FileContents {
        items,
        warnings: warnings
            .iter()
            .map(|w| format!("offset {}: {}", w.offset, w.message))
            .collect(),
    })
}

/// `ledger 'Name'`, `group 'Name'` and so on.
#[must_use]
pub fn master_label(m: &ImportMaster) -> String {
    format!("{} '{}'", kind_word(m.kind), m.name)
}

/// `Sales voucher SO-12 dated 2026-10-05`.
#[must_use]
pub fn voucher_object(v: &TVoucher) -> String {
    voucher_label(v)
}

fn kind_word(kind: ImportMasterKind) -> &'static str {
    match kind {
        ImportMasterKind::Group => "group",
        ImportMasterKind::Ledger => "ledger",
        ImportMasterKind::StockItem => "stock item",
        ImportMasterKind::Unit => "unit",
        ImportMasterKind::Godown => "godown",
        ImportMasterKind::VoucherType => "voucher type",
        ImportMasterKind::CostCentre => "cost centre",
        ImportMasterKind::CostCategory => "cost category",
        ImportMasterKind::StockGroup => "stock group",
    }
}

fn exception(object: &str, reason: impl Into<String>) -> Exception {
    Exception {
        object: object.to_owned(),
        reason: reason.into(),
    }
}

fn from_map(object: &str, error: &MapError) -> Exception {
    exception(object, error.to_string())
}

fn tally_ref(guid: Option<&str>, alter_id: Option<u64>) -> Option<TallyRef> {
    guid.map(|g| TallyRef {
        guid: g.to_owned(),
        alter_id,
    })
}

fn group_named(masters: &Masters, name: &str) -> Option<GroupId> {
    masters
        .groups
        .values()
        .find(|g| g.name == name)
        .map(|g| g.id)
}

fn ledger_named(masters: &Masters, name: &str) -> Option<LedgerId> {
    masters
        .ledgers
        .values()
        .find(|l| l.name == name)
        .map(|l| l.id)
}

/// The change master `m` asks for in `masters` under `behaviour`.
///
/// # Errors
/// An [`Exception`] when the master cannot be imported: an unknown parent group, a kind of master
/// that is not supported yet, a value that cannot be read.
pub fn master_step(
    m: &ImportMaster,
    masters: &Masters,
    behaviour: MasterBehaviour,
) -> Result<MasterPlan, Exception> {
    let object = master_label(m);
    match m.kind {
        ImportMasterKind::Group => group_step(m, masters, behaviour, &object),
        ImportMasterKind::Ledger => ledger_step(m, masters, behaviour, &object),
        ImportMasterKind::VoucherType => {
            if masters.voucher_types.values().any(|t| t.name == m.name) {
                Ok(MasterPlan {
                    step: MasterStep::Ignore(
                        "the company already has a voucher type of this name".to_owned(),
                    ),
                    dropped: Vec::new(),
                })
            } else {
                Err(exception(
                    &object,
                    "new voucher types are not imported yet; the company keeps its own types \
                     (pending)",
                ))
            }
        }
        ImportMasterKind::StockItem
        | ImportMasterKind::Unit
        | ImportMasterKind::Godown
        | ImportMasterKind::CostCentre
        | ImportMasterKind::CostCategory
        | ImportMasterKind::StockGroup => Err(exception(
            &object,
            "this kind of master is not imported from a file yet (pending)",
        )),
    }
}

/// The fields of `m` other than `consumed` that are not imported.
fn dropped(m: &ImportMaster, consumed: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for element in &m.extra {
        if !consumed.contains(&element.name.as_str()) {
            out.push(format!("field <{}> is not imported", element.name));
        }
    }
    for (key, _) in &m.attributes {
        if key != "NAME" && key != "ACTION" {
            out.push(format!("attribute {key} is not imported"));
        }
    }
    out
}

fn parent_id(m: &ImportMaster, masters: &Masters, object: &str) -> Result<GroupId, Exception> {
    let Some(parent) = normalize_parent(m.parent.as_deref()) else {
        return Err(exception(
            object,
            "it has no parent group, so it cannot be placed in the chart of accounts here (a new \
             primary group needs its nature, which is not read from a file yet)",
        ));
    };
    group_named(masters, &parent).ok_or_else(|| {
        exception(
            object,
            format!("its parent group '{parent}' is not in the company or the file"),
        )
    })
}

fn group_step(
    m: &ImportMaster,
    masters: &Masters,
    behaviour: MasterBehaviour,
    object: &str,
) -> Result<MasterPlan, Exception> {
    let existing = masters.groups.values().find(|g| g.name == m.name);
    let Some(existing) = existing else {
        let parent = parent_id(m, masters, object)?;
        return Ok(MasterPlan {
            step: MasterStep::CreateGroup(Group {
                ineligible_itc: None,
                id: GroupId(0),
                name: m.name.clone(),
                aliases: Vec::new(),
                parent: Some(parent),
                reserved: None,
                nature: None,
                affects_gross_profit: false,
                tally: tally_ref(m.guid.as_deref(), m.alter_id),
            }),
            dropped: dropped(m, &[]),
        });
    };
    let ignore = |why: &str| {
        Ok(MasterPlan {
            step: MasterStep::Ignore(why.to_owned()),
            dropped: Vec::new(),
        })
    };
    match behaviour {
        MasterBehaviour::IgnoreDuplicates => ignore("a group of this name exists; it is kept"),
        MasterBehaviour::CombineOpeningBalance => {
            ignore("a group of this name exists; a group has no opening balance to combine")
        }
        MasterBehaviour::ModifyWithNewData => {
            let parent = match normalize_parent(m.parent.as_deref()) {
                None => existing.parent,
                Some(_) => Some(parent_id(m, masters, object)?),
            };
            if parent == existing.parent {
                return ignore("a group of this name exists and the file changes nothing in it");
            }
            if existing.reserved.is_some() {
                return Err(exception(
                    object,
                    "it is a reserved group, which cannot be moved under another group",
                ));
            }
            let mut after = existing.clone();
            after.parent = parent;
            Ok(MasterPlan {
                step: MasterStep::AlterGroup(after),
                dropped: dropped(m, &[]),
            })
        }
    }
}

/// The file's `OPENINGBALANCE`: `None` when absent or empty, negative for a debit.
fn opening(object: &str, extra: &[RawElement]) -> Result<Option<SignedAmount>, Exception> {
    let Some(text) = extra_text(extra, "OPENINGBALANCE") else {
        return Ok(None);
    };
    let value = Decimal::from_str(text).map_err(|_| {
        exception(
            object,
            format!("its opening balance '{text}' is not a plain decimal amount"),
        )
    })?;
    if value.normalize().scale() > 2 {
        return Err(exception(
            object,
            format!("its opening balance '{text}' has more than two decimal places"),
        ));
    }
    let amount = round_rupees_to_paise(value.abs(), RoundingRule::Truncate)
        .map_err(|_| exception(object, format!("its opening balance '{text}' is too large")))?;
    let side = if value.is_sign_negative() {
        DrCr::Dr
    } else {
        DrCr::Cr
    };
    Ok(Some(SignedAmount { amount, side }))
}

const LEDGER_FIELDS: [&str; 3] = ["OPENINGBALANCE", "ISBILLWISEON", "ISCOSTCENTRESON"];

fn ledger_step(
    m: &ImportMaster,
    masters: &Masters,
    behaviour: MasterBehaviour,
    object: &str,
) -> Result<MasterPlan, Exception> {
    let file_opening = opening(object, &m.extra)?;
    let bill_wise =
        element_flag(object, &m.extra, "ISBILLWISEON").map_err(|e| from_map(object, &e))?;
    let cost_centres =
        element_flag(object, &m.extra, "ISCOSTCENTRESON").map_err(|e| from_map(object, &e))?;
    let existing = masters.ledgers.values().find(|l| l.name == m.name);
    let Some(existing) = existing else {
        let group = parent_id(m, masters, object)?;
        return Ok(MasterPlan {
            step: MasterStep::CreateLedger(Ledger {
                ineligible_itc: None,
                id: LedgerId(0),
                name: m.name.clone(),
                aliases: Vec::new(),
                group,
                opening: file_opening.unwrap_or_else(|| SignedAmount::dr(Paise::ZERO)),
                bill_wise: bill_wise.unwrap_or(false).then_some(BillWiseConfig {
                    default_credit_days: None,
                }),
                cost_centres_applicable: cost_centres.unwrap_or(false),
                party: None,
                bank: None,
                tax: None,
                gst: None,
                reserved: None,
                tally: tally_ref(m.guid.as_deref(), m.alter_id),
            }),
            dropped: dropped(m, &LEDGER_FIELDS),
        });
    };
    existing_ledger_step(
        m,
        masters,
        behaviour,
        object,
        existing,
        LedgerFile {
            opening: file_opening,
            bill_wise,
            cost_centres,
        },
    )
}

/// What the file says about a ledger.
#[derive(Clone, Copy)]
struct LedgerFile {
    opening: Option<SignedAmount>,
    bill_wise: Option<bool>,
    cost_centres: Option<bool>,
}

fn existing_ledger_step(
    m: &ImportMaster,
    masters: &Masters,
    behaviour: MasterBehaviour,
    object: &str,
    existing: &Ledger,
    file: LedgerFile,
) -> Result<MasterPlan, Exception> {
    let LedgerFile {
        opening: file_opening,
        bill_wise,
        cost_centres,
    } = file;
    let ignore = |why: &str| {
        Ok(MasterPlan {
            step: MasterStep::Ignore(why.to_owned()),
            dropped: Vec::new(),
        })
    };
    match behaviour {
        MasterBehaviour::IgnoreDuplicates => ignore("a ledger of this name exists; it is kept"),
        MasterBehaviour::CombineOpeningBalance => {
            let Some(add) = file_opening else {
                return ignore(
                    "a ledger of this name exists and the file has no opening balance to combine",
                );
            };
            if add.amount.is_zero() {
                return ignore(
                    "a ledger of this name exists and the file's opening balance is zero",
                );
            }
            let money = |e: MoneyError| exception(object, format!("the combined opening: {e}"));
            let sum = existing
                .opening
                .to_signed()
                .map_err(money)?
                .checked_add(add.to_signed().map_err(money)?)
                .map_err(money)?;
            let mut after = existing.clone();
            after.opening = SignedAmount::from_signed(sum).map_err(money)?;
            Ok(MasterPlan {
                step: MasterStep::AlterLedger {
                    ledger: after,
                    combined: true,
                },
                dropped: Vec::new(),
            })
        }
        MasterBehaviour::ModifyWithNewData => {
            let mut after = existing.clone();
            if normalize_parent(m.parent.as_deref()).is_some() {
                after.group = parent_id(m, masters, object)?;
            }
            if let Some(o) = file_opening {
                after.opening = o;
            }
            if let Some(on) = bill_wise {
                after.bill_wise = on.then(|| {
                    existing.bill_wise.unwrap_or(BillWiseConfig {
                        default_credit_days: None,
                    })
                });
            }
            if let Some(on) = cost_centres {
                after.cost_centres_applicable = on;
            }
            if after == *existing {
                return ignore("a ledger of this name exists and the file changes nothing in it");
            }
            Ok(MasterPlan {
                step: MasterStep::AlterLedger {
                    ledger: after,
                    combined: false,
                },
                dropped: dropped(m, &LEDGER_FIELDS),
            })
        }
    }
}

/// Voucher `v` as a voucher of `books` with id `id`, ready to post, and the fields of it that
/// were not imported.
///
/// # Errors
/// An [`Exception`] when the voucher cannot be imported: a voucher type or ledger the company
/// does not have, inventory lines, bills that do not add up, a value that cannot be read.
pub fn voucher_step(
    v: &TVoucher,
    books: &Books,
    id: VoucherId,
) -> Result<(Voucher, Vec<String>), Exception> {
    let label = voucher_label(v);
    if !v.inventory_entries.is_empty() {
        return Err(exception(
            &label,
            "it has inventory lines, which are not imported from a file yet (pending)",
        ));
    }
    let masters = &books.masters;
    let vt = masters
        .voucher_types
        .values()
        .find(|t| t.name == v.voucher_type)
        .ok_or_else(|| {
            exception(
                &label,
                format!(
                    "the company has no voucher type '{}'; create it first",
                    v.voucher_type
                ),
            )
        })?;
    let party = v
        .party
        .as_deref()
        .map(|name| known_ledger(masters, &label, name))
        .transpose()?;
    let (ledger_lines, dropped) = lines(&label, v, masters)?;
    let status = status(&label, v)?;
    let invoice = node_flag(&label, &v.extra, "ISINVOICE")
        .map_err(|e| from_map(&label, &e))?
        .unwrap_or(false);
    let mut extra = voucher_extra(v);
    let number = match vt.numbering.method {
        NumberingMethod::Manual => v.number.clone(),
        NumberingMethod::Automatic | NumberingMethod::None => {
            if let Some(n) = &v.number {
                insert_unique(&mut extra, "VOUCHERNUMBER", n.clone());
            }
            None
        }
    };
    Ok((
        Voucher {
            id,
            type_id: vt.id,
            number,
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
            tally: tally_ref(v.guid.as_deref(), v.alter_id),
            extra,
        },
        dropped,
    ))
}

fn known_ledger(masters: &Masters, label: &str, name: &str) -> Result<LedgerId, Exception> {
    ledger_named(masters, name).ok_or_else(|| {
        exception(
            label,
            format!("the company has no ledger '{name}'; import the masters first"),
        )
    })
}

/// The voucher's ledger lines, and the line fields that are not imported.
fn lines(
    label: &str,
    v: &TVoucher,
    masters: &Masters,
) -> Result<(Vec<LedgerLine>, Vec<String>), Exception> {
    let mut dropped = Vec::new();
    let mut out = Vec::new();
    for entry in &v.ledger_entries {
        for node in &entry.extra {
            dropped.push(format!(
                "<{}> on the line for '{}' is not imported",
                node.name, entry.ledger_name
            ));
        }
        out.push(LedgerLine {
            ineligible_itc: None,
            ledger: known_ledger(masters, label, &entry.ledger_name)?,
            amount: SignedAmount {
                amount: entry.amount,
                side: entry.side,
            },
            bills: bills(label, entry)?,
            cost: Vec::new(),
            bank: None,
        });
    }
    Ok((out, dropped))
}

/// The status from the flags present; an absent flag is not set (see the module docs).
fn status(label: &str, v: &TVoucher) -> Result<VoucherStatus, Exception> {
    let flags = [
        ("ISCANCELLED", VoucherStatus::Cancelled),
        ("ISOPTIONAL", VoucherStatus::Optional),
        ("ISPOSTDATED", VoucherStatus::PostDated),
    ];
    let mut set = Vec::new();
    for (tag, status) in flags {
        if node_flag(label, &v.extra, tag)
            .map_err(|e| from_map(label, &e))?
            .unwrap_or(false)
        {
            set.push(status);
        }
    }
    match set.as_slice() {
        [] => Ok(VoucherStatus::Regular),
        [one] => Ok(*one),
        [..] => Err(exception(
            label,
            "it is flagged cancelled, optional or post-dated more than once",
        )),
    }
}

/// Bill allocations, as the live import maps them: they must total the line in the file's
/// convention (debit negative), and each one's side follows from its sign.
fn bills(label: &str, entry: &TLedgerEntry) -> Result<Vec<BillAllocation>, Exception> {
    if entry.bill_allocations.is_empty() {
        return Ok(Vec::new());
    }
    let money = |e: MoneyError| exception(label, format!("bills on '{}': {e}", entry.ledger_name));
    let line = match entry.side {
        DrCr::Dr => entry.amount.checked_neg().map_err(money)?,
        DrCr::Cr => entry.amount,
    };
    let mut total = Paise::ZERO;
    for bill in &entry.bill_allocations {
        total = total.checked_add(bill.amount).map_err(money)?;
    }
    if total != line {
        return Err(from_map(
            label,
            &MapError::BillSumMismatch {
                voucher: label.to_owned(),
                ledger: entry.ledger_name.clone(),
                allocated: total.get(),
                line: line.get(),
            },
        ));
    }
    let mut out = Vec::new();
    for bill in &entry.bill_allocations {
        let kind = bill_kind(label, bill).map_err(|e| from_map(label, &e))?;
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
