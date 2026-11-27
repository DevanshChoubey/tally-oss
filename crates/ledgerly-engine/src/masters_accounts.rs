//! Group and ledger commands. **Card E1** implements this module.
//!
//! Frozen interface (plan card E1):
//!
//! ```text
//! pub fn create_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_group(id: GroupId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn create_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_ledger(id: LedgerId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
//!
//! Every command is a pure check of the command against [`Books`]; the events it returns are what
//! the store commits. Rules enforced (spec section 5, invariant 5, and research R1):
//!
//! - Reserved groups and reserved ledgers cannot be deleted; renaming them is harmless.
//! - A group can never become its own ancestor.
//! - A ledger used by any voucher cannot be deleted.
//! - Nature is set only on primary groups; sub-groups inherit it.
//!
//! Left `Unsupported` because no source settles them: changing the parent, nature or
//! gross-profit flag of a reserved group, and moving a reserved ledger to another group.
//! Name uniqueness is checked on the trimmed, exactly equal name within the same kind of master
//! (no case folding: no source says whether names are case-insensitive).

use ledgerly_core::{GroupId, LedgerId, Outcome};
use ledgerly_model::{Books, Group, Ledger, MasterSnapshot, ReservedLedger, Voucher};
use ledgerly_store::Event;

use crate::EngineError;

/// Builds an [`EngineError::Invalid`].
fn invalid(field: &str, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

/// Checks the parts of a group that do not depend on whether it is new or altered.
fn check_group_shape(g: &Group, books: &Books) -> Result<(), EngineError> {
    let name = g.name.trim();
    if name.is_empty() {
        return Err(invalid("group name", "the name is empty; type a name"));
    }
    match g.parent {
        Some(parent) => {
            if !books.masters.groups.contains_key(&parent) {
                return Err(EngineError::NotFound {
                    what: format!("parent group {}", parent.0),
                });
            }
            if g.nature.is_some() {
                return Err(invalid(
                    "group nature",
                    "only a primary group has a nature; a sub-group inherits it from its parent",
                ));
            }
        }
        None => {
            if g.reserved.is_none() && g.nature.is_none() {
                return Err(invalid(
                    "group nature",
                    "a primary group needs a nature (asset, liability, income or expense)",
                ));
            }
        }
    }
    let clash = books
        .masters
        .groups
        .values()
        .any(|other| other.id != g.id && other.name.trim() == name);
    if clash {
        return Err(EngineError::Duplicate {
            what: format!("group '{name}'"),
        });
    }
    Ok(())
}

/// `true` when placing `id` under `new_parent` would make `id` its own ancestor.
///
/// Walks the parent chain from `new_parent`. A chain longer than the number of groups must
/// already contain a loop, which is reported as a cycle too rather than followed forever.
fn would_cycle(id: GroupId, new_parent: GroupId, books: &Books) -> bool {
    let limit = books.masters.groups.len();
    let mut current = Some(new_parent);
    let mut steps = 0_usize;
    while let Some(c) = current {
        if c == id {
            return true;
        }
        steps = steps.saturating_add(1);
        if steps > limit {
            return true;
        }
        current = books.masters.groups.get(&c).and_then(|g| g.parent);
    }
    false
}

/// Create a group.
///
/// Reserved groups are seeded with the company and cannot be created by a command.
///
/// # Errors
/// [`EngineError::Duplicate`] when the id or name is taken; [`EngineError::NotFound`] when the
/// parent does not exist; [`EngineError::Invalid`] for an empty name, a reserved marker, a
/// nature on a sub-group, or a primary group without a nature.
pub fn create_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.groups.contains_key(&g.id) {
        return Err(EngineError::Duplicate {
            what: format!("group id {}", g.id.0),
        });
    }
    if g.reserved.is_some() {
        return Err(invalid(
            "group",
            "reserved groups are created with the company and cannot be added",
        ));
    }
    check_group_shape(&g, books)?;
    Ok(Outcome::Value(vec![Event::MasterCreated(
        MasterSnapshot::Group(g),
    )]))
}

/// Alter a group (rename, re-parent, change nature or the gross-profit flag).
///
/// Returns no events when nothing changed. Renaming a reserved group is allowed; changing its
/// parent, nature or gross-profit flag returns `Unsupported` (no source says what is allowed).
///
/// # Errors
/// [`EngineError::NotFound`] for an unknown group or parent; [`EngineError::GroupCycle`] when the
/// new parent is the group itself or one of its descendants; [`EngineError::Duplicate`] for a
/// name clash; [`EngineError::Invalid`] for an empty name, a changed reserved marker, a nature on
/// a sub-group, or a primary group without a nature.
pub fn alter_group(g: Group, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let Some(before) = books.masters.groups.get(&g.id) else {
        return Err(EngineError::NotFound {
            what: format!("group id {}", g.id.0),
        });
    };
    if before.reserved != g.reserved {
        return Err(invalid(
            "group",
            "a group cannot become, or stop being, a reserved group",
        ));
    }
    if before.reserved.is_some()
        && (before.parent != g.parent
            || before.nature != g.nature
            || before.affects_gross_profit != g.affects_gross_profit)
    {
        return Ok(Outcome::unsupported(
            "alter a reserved group's parent, nature or gross-profit flag",
            "no source says which of these may change on a reserved group; only the name, aliases and import identity can be altered",
        ));
    }
    check_group_shape(&g, books)?;
    if let Some(parent) = g.parent
        && would_cycle(g.id, parent, books)
    {
        return Err(EngineError::GroupCycle);
    }
    if *before == g {
        return Ok(Outcome::Value(Vec::new()));
    }
    Ok(Outcome::Value(vec![Event::MasterAltered {
        before: MasterSnapshot::Group(before.clone()),
        after: MasterSnapshot::Group(g),
    }]))
}

/// Delete a group (spec section 5, invariant 5).
///
/// # Errors
/// [`EngineError::NotFound`] for an unknown group; [`EngineError::ReservedGroup`] for a reserved
/// one; [`EngineError::InUseByMaster`] when it still has sub-groups or ledgers.
pub fn delete_group(id: GroupId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let Some(group) = books.masters.groups.get(&id) else {
        return Err(EngineError::NotFound {
            what: format!("group id {}", id.0),
        });
    };
    if let Some(reserved) = group.reserved {
        return Err(EngineError::ReservedGroup(reserved));
    }
    if let Some(child) = books.masters.groups.values().find(|g| g.parent == Some(id)) {
        return Err(EngineError::InUseByMaster {
            what: format!("group '{}'", group.name),
            by: format!("sub-group '{}'", child.name),
        });
    }
    if let Some(ledger) = books.masters.ledgers.values().find(|l| l.group == Some(id)) {
        return Err(EngineError::InUseByMaster {
            what: format!("group '{}'", group.name),
            by: format!("ledger '{}'", ledger.name),
        });
    }
    Ok(Outcome::Value(vec![Event::MasterDeleted(
        MasterSnapshot::Group(group.clone()),
    )]))
}

/// Checks the parts of a ledger that do not depend on whether it is new or altered.
fn check_ledger_shape(l: &Ledger, books: &Books) -> Result<(), EngineError> {
    let name = l.name.trim();
    if name.is_empty() {
        return Err(invalid("ledger name", "the name is empty; type a name"));
    }
    match l.group {
        // Primary holds only the predefined Profit & Loss A/c ("grouped under the Primary
        // group", <https://help.tallysolutions.com/ledgers-and-groups-in-tallyprime/>, read
        // 2026-11-27); every other ledger is created under a group.
        None if l.reserved == Some(ReservedLedger::ProfitAndLoss) => {}
        None => {
            return Err(invalid(
                "ledger group",
                "only the predefined Profit & Loss A/c sits under Primary; choose the group this ledger belongs under",
            ));
        }
        Some(group) if !books.masters.groups.contains_key(&group) => {
            return Err(EngineError::NotFound {
                what: format!("group {} of ledger '{name}'", group.0),
            });
        }
        Some(_) => {}
    }
    if l.opening.amount.is_negative() {
        return Err(invalid(
            "opening balance",
            "the amount is negative; enter a positive amount and choose Dr or Cr",
        ));
    }
    let clash = books
        .masters
        .ledgers
        .values()
        .any(|other| other.id != l.id && other.name.trim() == name);
    if clash {
        return Err(EngineError::Duplicate {
            what: format!("ledger '{name}'"),
        });
    }
    Ok(())
}

/// Create a ledger.
///
/// A reserved ledger may be created once (company set-up and imports do this).
///
/// # Errors
/// [`EngineError::Duplicate`] when the id, name or reserved role is taken;
/// [`EngineError::NotFound`] when the group does not exist; [`EngineError::Invalid`] for an empty
/// name or a negative opening amount.
pub fn create_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.ledgers.contains_key(&l.id) {
        return Err(EngineError::Duplicate {
            what: format!("ledger id {}", l.id.0),
        });
    }
    if let Some(reserved) = l.reserved
        && books
            .masters
            .ledgers
            .values()
            .any(|other| other.reserved == Some(reserved))
    {
        return Err(EngineError::Duplicate {
            what: format!("reserved ledger {reserved:?}"),
        });
    }
    check_ledger_shape(&l, books)?;
    Ok(Outcome::Value(vec![Event::MasterCreated(
        MasterSnapshot::Ledger(l),
    )]))
}

/// `true` when a voucher has a bill allocation on `ledger`.
fn has_bills_on(v: &Voucher, ledger: LedgerId) -> bool {
    v.ledger_lines
        .iter()
        .any(|line| line.ledger == ledger && !line.bills.is_empty())
}

/// Alter a ledger (rename, change group, opening balance and details).
///
/// Returns no events when nothing changed. Moving a reserved ledger to another group returns
/// `Unsupported` (research R1 does not say where Cash and Profit & Loss may live).
///
/// # Errors
/// [`EngineError::NotFound`] for an unknown ledger or group; [`EngineError::Duplicate`] for a
/// name clash; [`EngineError::Invalid`] for an empty name, a negative opening amount, a changed
/// reserved marker, or turning bill-wise tracking off while vouchers hold bill allocations on it.
pub fn alter_ledger(l: Ledger, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let Some(before) = books.masters.ledgers.get(&l.id) else {
        return Err(EngineError::NotFound {
            what: format!("ledger id {}", l.id.0),
        });
    };
    if before.reserved != l.reserved {
        return Err(invalid(
            "ledger",
            "a ledger cannot become, or stop being, a reserved ledger",
        ));
    }
    if before.reserved.is_some() && before.group != l.group {
        return Ok(Outcome::unsupported(
            "move a reserved ledger to another group",
            "research R1 does not settle where Cash and Profit & Loss may live; only other fields can be altered",
        ));
    }
    check_ledger_shape(&l, books)?;
    if before.bill_wise.is_some()
        && l.bill_wise.is_none()
        && books.vouchers.values().any(|v| has_bills_on(v, l.id))
    {
        return Err(invalid(
            "bill-wise",
            format!(
                "vouchers hold bill allocations on '{}'; they would be orphaned, so keep bill-wise on",
                l.name
            ),
        ));
    }
    if *before == l {
        return Ok(Outcome::Value(Vec::new()));
    }
    Ok(Outcome::Value(vec![Event::MasterAltered {
        before: MasterSnapshot::Ledger(before.clone()),
        after: MasterSnapshot::Ledger(l),
    }]))
}

/// `true` when the voucher refers to `ledger` anywhere (party, lines, inventory lines, GST).
fn voucher_uses_ledger(v: &Voucher, ledger: LedgerId) -> bool {
    v.party == Some(ledger)
        || v.ledger_lines.iter().any(|line| line.ledger == ledger)
        || v.inventory_lines.iter().any(|i| i.ledger == Some(ledger))
        || v.gst
            .as_ref()
            .is_some_and(|g| g.lines.iter().any(|line| line.ledger == ledger))
}

/// Delete a ledger (spec section 5, invariant 5).
///
/// Cancelled vouchers count as users: they keep their lines.
///
/// # Errors
/// [`EngineError::NotFound`] for an unknown ledger; [`EngineError::ReservedLedger`] for a
/// reserved one; [`EngineError::InUse`] with the number of vouchers that refer to it;
/// [`EngineError::Invalid`] when it still has an opening balance (deleting it would silently
/// change the opening trial balance).
pub fn delete_ledger(id: LedgerId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let Some(ledger) = books.masters.ledgers.get(&id) else {
        return Err(EngineError::NotFound {
            what: format!("ledger id {}", id.0),
        });
    };
    if let Some(reserved) = ledger.reserved {
        return Err(EngineError::ReservedLedger(reserved));
    }
    let vouchers = books
        .vouchers
        .values()
        .filter(|v| voucher_uses_ledger(v, id))
        .count();
    if vouchers > 0 {
        return Err(EngineError::InUse { vouchers });
    }
    if !ledger.opening.amount.is_zero() {
        return Err(invalid(
            "ledger",
            format!(
                "'{}' has an opening balance; set it to zero first so the opening trial balance does not change silently",
                ledger.name
            ),
        ));
    }
    Ok(Outcome::Value(vec![Event::MasterDeleted(
        MasterSnapshot::Ledger(ledger.clone()),
    )]))
}
