//! The master lists: rows per kind with search, and copies of single masters.

use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, GodownId, GroupId, LedgerId, StockGroupId, StockItemId,
    UnitId, VoucherTypeId,
};
use ledgerly_model::{MasterSnapshot, Masters};

use super::{MasterKind, MasterRef};

/// One line of a master list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRow {
    /// Which master.
    pub master: MasterRef,
    /// Its name (a unit's symbol).
    pub name: String,
    /// A short description: group, parent, GSTIN, unit and so on. Never a computed balance.
    pub detail: String,
}

struct Entry {
    id: u64,
    name: String,
    detail: String,
    search: Vec<String>,
}

fn with_aliases(name: &str, aliases: &[String]) -> Vec<String> {
    let mut v = vec![name.to_owned()];
    v.extend(aliases.iter().cloned());
    v
}

fn group_entries(m: &Masters) -> Vec<Entry> {
    m.groups
        .values()
        .map(|g| {
            let mut detail = match g.parent.and_then(|p| m.groups.get(&p)) {
                Some(p) => format!("under {}", p.name),
                None => "primary".to_owned(),
            };
            if g.reserved.is_some() {
                detail.push_str(", reserved");
            }
            Entry {
                id: g.id.0,
                name: g.name.clone(),
                detail,
                search: with_aliases(&g.name, &g.aliases),
            }
        })
        .collect()
}

fn ledger_entries(m: &Masters) -> Vec<Entry> {
    m.ledgers
        .values()
        .map(|l| {
            let group = m.groups.get(&l.group).map_or("?", |g| g.name.as_str());
            let mut detail = format!("under {group}");
            if !l.opening.amount.is_zero() {
                let side = match l.opening.side {
                    ledgerly_core::DrCr::Dr => "Dr",
                    ledgerly_core::DrCr::Cr => "Cr",
                };
                detail.push_str(&format!(
                    ", opening {} {side}",
                    ledgerly_core::format::format_inr(l.opening.amount)
                ));
            }
            let mut search = with_aliases(&l.name, &l.aliases);
            if let Some(g) = l.party.as_ref().and_then(|p| p.gstin.as_ref()) {
                detail.push_str(&format!(", GSTIN {g}"));
                search.push(g.as_str().to_owned());
            }
            if l.reserved.is_some() {
                detail.push_str(", reserved");
            }
            Entry {
                id: l.id.0,
                name: l.name.clone(),
                detail,
                search,
            }
        })
        .collect()
}

fn inventory_entries(m: &Masters, kind: MasterKind) -> Vec<Entry> {
    match kind {
        MasterKind::Unit => m
            .units
            .values()
            .map(|u| Entry {
                id: u.id.0,
                name: u.symbol.clone(),
                detail: u.formal_name.clone(),
                search: vec![u.symbol.clone(), u.formal_name.clone()],
            })
            .collect(),
        MasterKind::StockGroup => m
            .stock_groups
            .values()
            .map(|g| Entry {
                id: g.id.0,
                name: g.name.clone(),
                detail: g
                    .parent
                    .and_then(|p| m.stock_groups.get(&p))
                    .map_or_else(|| "primary".to_owned(), |p| format!("under {}", p.name)),
                search: vec![g.name.clone()],
            })
            .collect(),
        MasterKind::StockItem => m
            .stock_items
            .values()
            .map(|i| Entry {
                id: i.id.0,
                name: i.name.clone(),
                detail: format!(
                    "unit {}{}",
                    m.units.get(&i.unit).map_or("?", |u| u.symbol.as_str()),
                    i.hsn
                        .as_ref()
                        .map(|h| format!(", HSN {}", h.as_str()))
                        .unwrap_or_default()
                ),
                search: with_aliases(&i.name, &i.aliases),
            })
            .collect(),
        MasterKind::Godown => m
            .godowns
            .values()
            .map(|g| Entry {
                id: g.id.0,
                name: g.name.clone(),
                detail: g
                    .parent
                    .and_then(|p| m.godowns.get(&p))
                    .map_or_else(|| "primary".to_owned(), |p| format!("under {}", p.name)),
                search: vec![g.name.clone()],
            })
            .collect(),
        MasterKind::Batch => m
            .batches
            .values()
            .map(|b| Entry {
                id: b.id.0,
                name: b.name.clone(),
                detail: m
                    .stock_items
                    .get(&b.item)
                    .map_or_else(String::new, |i| format!("of {}", i.name)),
                search: vec![b.name.clone()],
            })
            .collect(),
        MasterKind::Group
        | MasterKind::Ledger
        | MasterKind::VoucherType
        | MasterKind::CostCategory
        | MasterKind::CostCentre => Vec::new(),
    }
}

fn other_entries(m: &Masters, kind: MasterKind) -> Vec<Entry> {
    match kind {
        MasterKind::VoucherType => m
            .voucher_types
            .values()
            .map(|t| Entry {
                id: t.id.0,
                name: t.name.clone(),
                detail: format!("{:?}", t.base),
                search: vec![t.name.clone(), t.abbreviation.clone()],
            })
            .collect(),
        MasterKind::CostCategory => m
            .cost_categories
            .values()
            .map(|c| Entry {
                id: c.id.0,
                name: c.name.clone(),
                detail: String::new(),
                search: vec![c.name.clone()],
            })
            .collect(),
        MasterKind::CostCentre => m
            .cost_centres
            .values()
            .map(|c| Entry {
                id: c.id.0,
                name: c.name.clone(),
                detail: m
                    .cost_categories
                    .get(&c.category)
                    .map_or_else(String::new, |k| format!("category {}", k.name)),
                search: vec![c.name.clone()],
            })
            .collect(),
        MasterKind::Group
        | MasterKind::Ledger
        | MasterKind::Unit
        | MasterKind::StockGroup
        | MasterKind::StockItem
        | MasterKind::Godown
        | MasterKind::Batch => Vec::new(),
    }
}

/// The masters of one kind whose name, alias or GSTIN contains `filter` (ignoring case), sorted
/// by name and then id. An empty filter lists them all.
#[must_use]
pub fn list_rows(masters: &Masters, kind: MasterKind, filter: &str) -> Vec<MasterRow> {
    let entries = match kind {
        MasterKind::Group => group_entries(masters),
        MasterKind::Ledger => ledger_entries(masters),
        MasterKind::Unit
        | MasterKind::StockGroup
        | MasterKind::StockItem
        | MasterKind::Godown
        | MasterKind::Batch => inventory_entries(masters, kind),
        MasterKind::VoucherType | MasterKind::CostCategory | MasterKind::CostCentre => {
            other_entries(masters, kind)
        }
    };
    let needle = filter.trim().to_lowercase();
    let mut kept: Vec<Entry> = entries
        .into_iter()
        .filter(|e| {
            needle.is_empty() || e.search.iter().any(|s| s.to_lowercase().contains(&needle))
        })
        .collect();
    kept.sort_by_cached_key(|e| (e.name.to_lowercase(), e.id));
    kept.into_iter()
        .map(|e| MasterRow {
            master: MasterRef { kind, id: e.id },
            name: e.name,
            detail: e.detail,
        })
        .collect()
}

/// A copy of the master `target` names, if it exists.
#[must_use]
pub fn snapshot_of(masters: &Masters, target: MasterRef) -> Option<MasterSnapshot> {
    let id = target.id;
    match target.kind {
        MasterKind::Group => masters
            .groups
            .get(&GroupId(id))
            .cloned()
            .map(MasterSnapshot::Group),
        MasterKind::Ledger => masters
            .ledgers
            .get(&LedgerId(id))
            .cloned()
            .map(MasterSnapshot::Ledger),
        MasterKind::VoucherType => masters
            .voucher_types
            .get(&VoucherTypeId(id))
            .cloned()
            .map(MasterSnapshot::VoucherType),
        MasterKind::Unit => masters
            .units
            .get(&UnitId(id))
            .cloned()
            .map(MasterSnapshot::Unit),
        MasterKind::StockGroup => masters
            .stock_groups
            .get(&StockGroupId(id))
            .cloned()
            .map(MasterSnapshot::StockGroup),
        MasterKind::StockItem => masters
            .stock_items
            .get(&StockItemId(id))
            .cloned()
            .map(MasterSnapshot::StockItem),
        MasterKind::Godown => masters
            .godowns
            .get(&GodownId(id))
            .cloned()
            .map(MasterSnapshot::Godown),
        MasterKind::Batch => masters
            .batches
            .get(&BatchId(id))
            .cloned()
            .map(MasterSnapshot::Batch),
        MasterKind::CostCategory => masters
            .cost_categories
            .get(&CostCategoryId(id))
            .cloned()
            .map(MasterSnapshot::CostCategory),
        MasterKind::CostCentre => masters
            .cost_centres
            .get(&CostCentreId(id))
            .cloned()
            .map(MasterSnapshot::CostCentre),
    }
}

/// The name a snapshot is listed under.
pub(crate) fn snapshot_name(s: &MasterSnapshot) -> &str {
    match s {
        MasterSnapshot::Group(g) => &g.name,
        MasterSnapshot::Ledger(l) => &l.name,
        MasterSnapshot::VoucherType(t) => &t.name,
        MasterSnapshot::Unit(u) => &u.symbol,
        MasterSnapshot::StockGroup(g) => &g.name,
        MasterSnapshot::StockItem(i) => &i.name,
        MasterSnapshot::Godown(g) => &g.name,
        MasterSnapshot::Batch(b) => &b.name,
        MasterSnapshot::CostCategory(c) => &c.name,
        MasterSnapshot::CostCentre(c) => &c.name,
    }
}
