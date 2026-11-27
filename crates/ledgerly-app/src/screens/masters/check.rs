//! Checks shared by the master forms: names, parent trees and what blocks a delete.
//!
//! Everything here reads the books and answers yes or a typed [`MasterError`]; nothing writes.

use std::collections::BTreeSet;

use ledgerly_core::{CostCategoryId, CostCentreId, GodownId, GroupId, LedgerId};
use ledgerly_core::{StockGroupId, StockItemId, UnitId};
use ledgerly_model::{Books, CostAllocation, Masters, UnitKind, Voucher};

use super::{MasterError, MasterKind, MasterRef};

/// Longest name, alias or symbol, in characters.
pub(crate) const NAME_MAX_CHARS: usize = 200;

/// A field problem with a field name and a reason.
pub(crate) fn invalid(field: &str, reason: impl Into<String>) -> MasterError {
    MasterError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

/// Trims a name and checks it is usable: not empty, not too long, no control characters.
pub(crate) fn clean_name(field: &str, text: &str) -> Result<String, MasterError> {
    let name = text.trim();
    if name.is_empty() {
        return Err(MasterError::NameEmpty {
            field: field.to_owned(),
        });
    }
    if name.chars().count() > NAME_MAX_CHARS {
        return Err(invalid(
            field,
            format!("longer than {NAME_MAX_CHARS} characters"),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid(field, "contains a control character"));
    }
    Ok(name.to_owned())
}

/// Splits a comma or line separated list, trims each item and drops empty ones.
pub(crate) fn split_list(text: &str) -> Vec<String> {
    text.split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The name followed by the aliases, each checked as a name.
pub(crate) fn clean_labels(name: &str, aliases: &str) -> Result<Vec<String>, MasterError> {
    let mut labels = vec![clean_name("Name", name)?];
    for alias in split_list(aliases) {
        labels.push(clean_name("Alias", &alias)?);
    }
    Ok(labels)
}

/// Every label (name, alias or symbol) used by masters of `kind`, with the owner's id.
fn labels_of(masters: &Masters, kind: MasterKind) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    match kind {
        MasterKind::Group => {
            for g in masters.groups.values() {
                out.push((g.id.0, g.name.clone()));
                out.extend(g.aliases.iter().map(|a| (g.id.0, a.clone())));
            }
        }
        MasterKind::Ledger => {
            for l in masters.ledgers.values() {
                out.push((l.id.0, l.name.clone()));
                out.extend(l.aliases.iter().map(|a| (l.id.0, a.clone())));
            }
        }
        MasterKind::StockItem => {
            for i in masters.stock_items.values() {
                out.push((i.id.0, i.name.clone()));
                out.extend(i.aliases.iter().map(|a| (i.id.0, a.clone())));
            }
        }
        MasterKind::VoucherType => {
            out.extend(
                masters
                    .voucher_types
                    .values()
                    .map(|t| (t.id.0, t.name.clone())),
            );
        }
        MasterKind::Unit => {
            out.extend(masters.units.values().map(|u| (u.id.0, u.symbol.clone())));
        }
        MasterKind::StockGroup => {
            out.extend(
                masters
                    .stock_groups
                    .values()
                    .map(|g| (g.id.0, g.name.clone())),
            );
        }
        MasterKind::Godown => {
            out.extend(masters.godowns.values().map(|g| (g.id.0, g.name.clone())));
        }
        MasterKind::CostCategory => {
            out.extend(
                masters
                    .cost_categories
                    .values()
                    .map(|c| (c.id.0, c.name.clone())),
            );
        }
        MasterKind::CostCentre => {
            out.extend(
                masters
                    .cost_centres
                    .values()
                    .map(|c| (c.id.0, c.name.clone())),
            );
        }
        MasterKind::Batch => {}
    }
    out
}

/// Fails when a label repeats within `labels` or is already used by another master of `kind`
/// (the master being altered, `editing`, does not count). Comparison ignores case, so the
/// type-ahead never has two entries that look the same.
pub(crate) fn check_unique(
    masters: &Masters,
    kind: MasterKind,
    editing: Option<u64>,
    labels: &[String],
) -> Result<(), MasterError> {
    let mut seen = BTreeSet::new();
    for label in labels {
        if !seen.insert(label.to_lowercase()) {
            return Err(MasterError::NameTaken {
                name: label.clone(),
            });
        }
    }
    let taken: BTreeSet<String> = labels_of(masters, kind)
        .into_iter()
        .filter(|(id, _)| Some(*id) != editing)
        .map(|(_, label)| label.to_lowercase())
        .collect();
    for label in labels {
        if taken.contains(&label.to_lowercase()) {
            return Err(MasterError::NameTaken {
                name: label.clone(),
            });
        }
    }
    Ok(())
}

/// `true` when putting `editing` under `new_parent` would make a loop (or the chain above
/// `new_parent` is longer than `limit`, which can only be a loop).
pub(crate) fn creates_cycle(
    editing: u64,
    new_parent: u64,
    parent_of: impl Fn(u64) -> Option<u64>,
    limit: usize,
) -> bool {
    let mut current = Some(new_parent);
    for _ in 0..=limit {
        match current {
            None => return false,
            Some(c) if c == editing => return true,
            Some(c) => current = parent_of(c),
        }
    }
    true
}

fn in_use(name: &str, by: &str) -> MasterError {
    MasterError::InUse {
        name: name.to_owned(),
        by: by.to_owned(),
    }
}

fn missing(kind: MasterKind) -> MasterError {
    MasterError::NotFound { kind: kind.label() }
}

fn voucher_uses_ledger(v: &Voucher, id: LedgerId) -> bool {
    v.party == Some(id)
        || v.ledger_lines.iter().any(|l| l.ledger == id)
        || v.inventory_lines.iter().any(|l| l.ledger == Some(id))
        || v.gst
            .as_ref()
            .is_some_and(|g| g.lines.iter().any(|l| l.ledger == id))
}

fn group_blockers(masters: &Masters, id: GroupId) -> Result<(), MasterError> {
    let g = masters
        .groups
        .get(&id)
        .ok_or_else(|| missing(MasterKind::Group))?;
    if g.reserved.is_some() {
        return Err(MasterError::Reserved {
            name: g.name.clone(),
            what: "deleted".to_owned(),
        });
    }
    if masters.groups.values().any(|c| c.parent == Some(id)) {
        return Err(in_use(&g.name, "sub-groups"));
    }
    if masters.ledgers.values().any(|l| l.group == Some(id)) {
        return Err(in_use(&g.name, "ledgers"));
    }
    Ok(())
}

fn ledger_blockers(books: &Books, id: LedgerId) -> Result<(), MasterError> {
    let l = books
        .masters
        .ledgers
        .get(&id)
        .ok_or_else(|| missing(MasterKind::Ledger))?;
    if l.reserved.is_some() {
        return Err(MasterError::Reserved {
            name: l.name.clone(),
            what: "deleted".to_owned(),
        });
    }
    if books.vouchers.values().any(|v| voucher_uses_ledger(v, id))
        || books.postings.iter().any(|p| p.ledger == id)
    {
        return Err(in_use(&l.name, "vouchers"));
    }
    if !l.opening.amount.is_zero() {
        return Err(in_use(&l.name, "an opening balance"));
    }
    Ok(())
}

fn unit_blockers(books: &Books, id: UnitId) -> Result<(), MasterError> {
    let m = &books.masters;
    let u = m.units.get(&id).ok_or_else(|| missing(MasterKind::Unit))?;
    let by_item = m
        .stock_items
        .values()
        .any(|i| i.unit == id || i.alt_unit.is_some_and(|a| a.unit == id));
    let by_unit = m.units.values().any(|o| match o.kind {
        UnitKind::Simple => false,
        UnitKind::Compound { first, second, .. } => first == id || second == id,
    });
    let by_voucher = books
        .vouchers
        .values()
        .any(|v| v.inventory_lines.iter().any(|l| l.unit == id));
    if by_item || by_unit || by_voucher {
        return Err(in_use(&u.symbol, "stock items, units or vouchers"));
    }
    Ok(())
}

fn stock_group_blockers(masters: &Masters, id: StockGroupId) -> Result<(), MasterError> {
    let g = masters
        .stock_groups
        .get(&id)
        .ok_or_else(|| missing(MasterKind::StockGroup))?;
    if masters.stock_groups.values().any(|c| c.parent == Some(id))
        || masters.stock_items.values().any(|i| i.group == Some(id))
    {
        return Err(in_use(&g.name, "stock items or sub-groups"));
    }
    Ok(())
}

fn stock_item_blockers(books: &Books, id: StockItemId) -> Result<(), MasterError> {
    let m = &books.masters;
    let item = m
        .stock_items
        .get(&id)
        .ok_or_else(|| missing(MasterKind::StockItem))?;
    let used = books
        .vouchers
        .values()
        .any(|v| v.inventory_lines.iter().any(|l| l.item == id))
        || books.stock_moves.iter().any(|s| s.item == id)
        || !item.opening.is_empty()
        || m.batches.values().any(|b| b.item == id);
    if used {
        return Err(in_use(
            &item.name,
            "vouchers, stock movements, batches or opening stock",
        ));
    }
    Ok(())
}

fn godown_blockers(books: &Books, id: GodownId) -> Result<(), MasterError> {
    let m = &books.masters;
    let g = m
        .godowns
        .get(&id)
        .ok_or_else(|| missing(MasterKind::Godown))?;
    let used = m.godowns.values().any(|c| c.parent == Some(id))
        || m.stock_items
            .values()
            .any(|i| i.opening.iter().any(|o| o.godown == id))
        || books.stock_moves.iter().any(|s| s.godown == id)
        || books.vouchers.values().any(|v| {
            v.inventory_lines
                .iter()
                .any(|l| l.godowns.iter().any(|a| a.godown == id))
        });
    if used {
        return Err(in_use(&g.name, "sub-godowns, stock or vouchers"));
    }
    Ok(())
}

fn any_cost_allocation(books: &Books, test: impl Fn(&CostAllocation) -> bool) -> bool {
    books
        .vouchers
        .values()
        .any(|v| v.ledger_lines.iter().any(|l| l.cost.iter().any(&test)))
}

fn cost_category_blockers(books: &Books, id: CostCategoryId) -> Result<(), MasterError> {
    let c = books
        .masters
        .cost_categories
        .get(&id)
        .ok_or_else(|| missing(MasterKind::CostCategory))?;
    if books
        .masters
        .cost_centres
        .values()
        .any(|x| x.category == id)
        || any_cost_allocation(books, |a| a.category == id)
    {
        return Err(in_use(&c.name, "cost centres or vouchers"));
    }
    Ok(())
}

fn cost_centre_blockers(books: &Books, id: CostCentreId) -> Result<(), MasterError> {
    let c = books
        .masters
        .cost_centres
        .get(&id)
        .ok_or_else(|| missing(MasterKind::CostCentre))?;
    if books
        .masters
        .cost_centres
        .values()
        .any(|x| x.parent == Some(id))
        || any_cost_allocation(books, |a| a.centre == id)
    {
        return Err(in_use(&c.name, "sub-centres or vouchers"));
    }
    Ok(())
}

/// Why `target` may not be deleted, or `Ok` when nothing in the books refers to it.
///
/// The store checks again when it commits; this is the early answer the screen shows before it
/// asks the user to confirm (spec 5 invariant 5: reserved groups and ledgers cannot be deleted).
pub(crate) fn delete_blockers(books: &Books, target: MasterRef) -> Result<(), MasterError> {
    let m = &books.masters;
    match target.kind {
        MasterKind::Group => group_blockers(m, GroupId(target.id)),
        MasterKind::Ledger => ledger_blockers(books, LedgerId(target.id)),
        MasterKind::Unit => unit_blockers(books, UnitId(target.id)),
        MasterKind::StockGroup => stock_group_blockers(m, StockGroupId(target.id)),
        MasterKind::StockItem => stock_item_blockers(books, StockItemId(target.id)),
        MasterKind::Godown => godown_blockers(books, GodownId(target.id)),
        MasterKind::CostCategory => cost_category_blockers(books, CostCategoryId(target.id)),
        MasterKind::CostCentre => cost_centre_blockers(books, CostCentreId(target.id)),
        MasterKind::VoucherType => Err(MasterError::Unsupported {
            what: "deleting a voucher type".to_owned(),
            reason: "voucher types are not edited on this screen yet (numbering rules are pending)"
                .to_owned(),
        }),
        MasterKind::Batch => Err(MasterError::Unsupported {
            what: "deleting a batch".to_owned(),
            reason: "batches are created and closed from stock entry, not from this screen"
                .to_owned(),
        }),
    }
}
