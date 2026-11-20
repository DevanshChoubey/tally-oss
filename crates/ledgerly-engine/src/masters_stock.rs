//! Inventory and cost master commands. **Card E2** implements this module.
//!
//! Frozen interface (plan card E2):
//!
//! ```text
//! Same create/alter/delete verbs as E1 for Unit, StockGroup, StockItem, Godown, Batch,
//! CostCategory and CostCentre, for example:
//! pub fn create_unit(u: Unit, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_godown(id: GodownId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
//!
//! Every verb is a pure check of the command against [`Books`]; nothing is written. A create
//! yields one [`Event::MasterCreated`], an alter one [`Event::MasterAltered`] (before and after
//! taken from the books, not from the command), a delete one [`Event::MasterDeleted`] of the
//! stored master. An alter that changes nothing yields no event (`Value(vec![])`). Every command
//! either yields events or an [`EngineError`]; no master is changed behind the user's back.
//!
//! Rules enforced (spec section 6.4; research R3 "Data-model"):
//!
//! - Names are unique (trimmed, case-insensitive) within their kind. Stock item names and aliases
//!   share one namespace. Cost centres are unique within their category, batches within their item.
//! - Parents exist and never form a cycle (stock groups, godowns, cost centres).
//! - A compound unit refers to two existing, different units, never to itself, directly or
//!   through other compound units, with a factor above zero. Compound units may nest.
//! - A unit's kind cannot change, and its decimals cannot shrink, once anything uses it
//!   (R3: "a compound unit is immutable once used").
//! - A master used by vouchers, stock movements or other masters cannot be deleted; the error
//!   says what uses it.
//!
//! Company feature switches are not consulted: `Features::validate` (card M3) owns the
//! dependency rules and the plan does not make these commands depend on it.

use std::collections::BTreeSet;

use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, Date, Decimal, GodownId, Outcome, StockGroupId,
    StockItemId, UnitId,
};
use ledgerly_model::{
    Batch, Books, CostCategory, CostCentre, Godown, MasterSnapshot, StockGroup, StockItem,
    Taxability, Unit, UnitKind, Voucher,
};
use ledgerly_store::Event;

use crate::EngineError;

/// Largest number of decimal places a quantity can carry (the scale limit of `Decimal`).
const MAX_DECIMALS: u8 = 28;

// ----------------------------------------------------------------------------------------------
// Shared helpers
// ----------------------------------------------------------------------------------------------

fn invalid(field: &str, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn missing(what: String) -> EngineError {
    EngineError::NotFound { what }
}

fn exists(what: String) -> EngineError {
    EngineError::Duplicate { what }
}

/// Comparison key for names: trimmed and lower-cased.
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}

fn require_name(field: &str, name: &str) -> Result<(), EngineError> {
    if name.trim().is_empty() {
        Err(invalid(field, "the name is empty; enter one"))
    } else {
        Ok(())
    }
}

fn created(snapshot: MasterSnapshot) -> Result<Outcome<Vec<Event>>, EngineError> {
    Ok(Outcome::Value(vec![Event::MasterCreated(snapshot)]))
}

fn deleted(snapshot: MasterSnapshot) -> Result<Outcome<Vec<Event>>, EngineError> {
    Ok(Outcome::Value(vec![Event::MasterDeleted(snapshot)]))
}

/// One alter event, or none when the master is unchanged.
fn altered(
    changed: bool,
    before: MasterSnapshot,
    after: MasterSnapshot,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if changed {
        Ok(Outcome::Value(vec![Event::MasterAltered { before, after }]))
    } else {
        Ok(Outcome::Value(Vec::new()))
    }
}

/// [`EngineError::InUse`] when any voucher (cancelled ones included) matches `uses`.
fn not_in_vouchers(books: &Books, uses: impl Fn(&Voucher) -> bool) -> Result<(), EngineError> {
    let vouchers = books.vouchers.values().filter(|&v| uses(v)).count();
    if vouchers > 0 {
        Err(EngineError::InUse { vouchers })
    } else {
        Ok(())
    }
}

/// Whether walking up from `start` through `parent_of` meets `target` (`start` included).
/// Stops at an existing cycle instead of looping.
fn climbs_to<T: Copy + Ord>(start: T, target: T, parent_of: impl Fn(T) -> Option<T>) -> bool {
    let mut seen = BTreeSet::new();
    let mut at = Some(start);
    while let Some(current) = at {
        if current == target {
            return true;
        }
        if !seen.insert(current) {
            return false;
        }
        at = parent_of(current);
    }
    false
}

// ----------------------------------------------------------------------------------------------
// Units
// ----------------------------------------------------------------------------------------------

/// Whether the compound-unit graph leads from `start` to `target` (`start` included).
fn compound_reaches(books: &Books, start: UnitId, target: UnitId) -> bool {
    let mut stack = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(current) = stack.pop() {
        if current == target {
            return true;
        }
        if !seen.insert(current) {
            continue;
        }
        if let Some(unit) = books.masters.units.get(&current) {
            match unit.kind {
                UnitKind::Simple => {}
                UnitKind::Compound { first, second, .. } => {
                    stack.push(first);
                    stack.push(second);
                }
            }
        }
    }
    false
}

fn check_compound(
    id: UnitId,
    first: UnitId,
    factor: Decimal,
    second: UnitId,
    books: &Books,
) -> Result<(), EngineError> {
    if first == id || second == id {
        return Err(invalid(
            "compound unit",
            "a compound unit cannot be made of itself; pick two other units",
        ));
    }
    if first == second {
        return Err(invalid(
            "compound unit",
            "both sides are the same unit; pick two different units",
        ));
    }
    if factor <= Decimal::ZERO {
        return Err(invalid(
            "compound unit factor",
            format!("{factor} is not above zero; enter how many of the smaller unit make one"),
        ));
    }
    for part in [first, second] {
        if !books.masters.units.contains_key(&part) {
            return Err(missing(format!(
                "unit {} used in the compound unit",
                part.0
            )));
        }
        if compound_reaches(books, part, id) {
            return Err(invalid(
                "compound unit",
                "it would be made of itself through other compound units",
            ));
        }
    }
    Ok(())
}

fn check_unit(u: &Unit, books: &Books) -> Result<(), EngineError> {
    require_name("unit symbol", &u.symbol)?;
    if u.decimals > MAX_DECIMALS {
        return Err(invalid(
            "unit decimals",
            format!(
                "{} places is more than the {MAX_DECIMALS} supported",
                u.decimals
            ),
        ));
    }
    if u.uqc.as_deref().is_some_and(|code| code.trim().is_empty()) {
        return Err(invalid(
            "unit GST code",
            "the GST unit quantity code is empty; enter it or leave it unset",
        ));
    }
    let symbol = key(&u.symbol);
    if books
        .masters
        .units
        .values()
        .any(|other| other.id != u.id && key(&other.symbol) == symbol)
    {
        return Err(exists(format!("unit '{}'", u.symbol.trim())));
    }
    match u.kind {
        UnitKind::Simple => Ok(()),
        UnitKind::Compound {
            first,
            factor,
            second,
        } => check_compound(u.id, first, factor, second, books),
    }
}

/// Names the first master (item or compound unit) that uses unit `id`.
fn unit_master_user(books: &Books, id: UnitId) -> Option<String> {
    let item = books
        .masters
        .stock_items
        .values()
        .find(|i| i.unit == id || i.alt_unit.is_some_and(|a| a.unit == id));
    if let Some(item) = item {
        return Some(format!("stock item '{}'", item.name));
    }
    books
        .masters
        .units
        .values()
        .find(|u| match u.kind {
            UnitKind::Simple => false,
            UnitKind::Compound { first, second, .. } => first == id || second == id,
        })
        .map(|u| format!("compound unit '{}'", u.symbol))
}

fn unit_in_vouchers(books: &Books, id: UnitId) -> bool {
    books
        .vouchers
        .values()
        .any(|v| v.inventory_lines.iter().any(|l| l.unit == id))
}

/// Create a unit of measure.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or symbol; [`EngineError::Invalid`] for an empty
/// symbol, an unsupported number of decimals, or a compound unit that is made of itself (directly
/// or through other compound units), of one unit twice, or has a factor of zero or less;
/// [`EngineError::NotFound`] when a compound part does not exist.
pub fn create_unit(u: Unit, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.units.contains_key(&u.id) {
        return Err(exists(format!("unit id {}", u.id.0)));
    }
    check_unit(&u, books)?;
    created(MasterSnapshot::Unit(u))
}

/// Alter a unit of measure.
///
/// # Errors
/// As [`create_unit`], plus [`EngineError::NotFound`] when the unit does not exist and
/// [`EngineError::Invalid`] when its kind changes or its decimals shrink while an item, a
/// compound unit or a voucher line uses it.
pub fn alter_unit(u: Unit, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .units
        .get(&u.id)
        .ok_or_else(|| missing(format!("unit id {}", u.id.0)))?;
    check_unit(&u, books)?;
    let risky = before.kind != u.kind || u.decimals < before.decimals;
    if risky && (unit_master_user(books, u.id).is_some() || unit_in_vouchers(books, u.id)) {
        return Err(invalid(
            "unit",
            format!(
                "'{}' is in use, so its kind and decimals cannot be reduced or changed; \
                 create a new unit instead",
                before.symbol
            ),
        ));
    }
    altered(
        *before != u,
        MasterSnapshot::Unit(before.clone()),
        MasterSnapshot::Unit(u),
    )
}

/// Delete a unit of measure.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when voucher lines use
/// it; [`EngineError::Invalid`] naming the stock item or compound unit that uses it.
pub fn delete_unit(id: UnitId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let unit = books
        .masters
        .units
        .get(&id)
        .ok_or_else(|| missing(format!("unit id {}", id.0)))?;
    not_in_vouchers(books, |v| v.inventory_lines.iter().any(|l| l.unit == id))?;
    if let Some(user) = unit_master_user(books, id) {
        return Err(invalid(
            "unit",
            format!("'{}' is used by {user}; change that first", unit.symbol),
        ));
    }
    deleted(MasterSnapshot::Unit(unit.clone()))
}

// ----------------------------------------------------------------------------------------------
// Stock groups
// ----------------------------------------------------------------------------------------------

fn check_stock_group(g: &StockGroup, books: &Books) -> Result<(), EngineError> {
    require_name("stock group name", &g.name)?;
    let name = key(&g.name);
    if books
        .masters
        .stock_groups
        .values()
        .any(|o| o.id != g.id && key(&o.name) == name)
    {
        return Err(exists(format!("stock group '{}'", g.name.trim())));
    }
    if let Some(parent) = g.parent {
        if parent == g.id {
            return Err(EngineError::GroupCycle);
        }
        if !books.masters.stock_groups.contains_key(&parent) {
            return Err(missing(format!("parent stock group {}", parent.0)));
        }
        let up = |id: StockGroupId| books.masters.stock_groups.get(&id).and_then(|p| p.parent);
        if climbs_to(parent, g.id, up) {
            return Err(EngineError::GroupCycle);
        }
    }
    Ok(())
}

/// Create a stock group.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or name; [`EngineError::Invalid`] for an empty name;
/// [`EngineError::NotFound`] for a missing parent; [`EngineError::GroupCycle`] when the group is
/// its own parent.
pub fn create_stock_group(
    g: StockGroup,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.stock_groups.contains_key(&g.id) {
        return Err(exists(format!("stock group id {}", g.id.0)));
    }
    check_stock_group(&g, books)?;
    created(MasterSnapshot::StockGroup(g))
}

/// Alter a stock group (rename or move it).
///
/// # Errors
/// As [`create_stock_group`], plus [`EngineError::NotFound`] when it does not exist and
/// [`EngineError::GroupCycle`] when the new parent is the group itself or one of its sub-groups.
pub fn alter_stock_group(g: StockGroup, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .stock_groups
        .get(&g.id)
        .ok_or_else(|| missing(format!("stock group id {}", g.id.0)))?;
    check_stock_group(&g, books)?;
    altered(
        *before != g,
        MasterSnapshot::StockGroup(before.clone()),
        MasterSnapshot::StockGroup(g),
    )
}

/// Delete a stock group.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::Invalid`] while it has
/// sub-groups or stock items.
pub fn delete_stock_group(
    id: StockGroupId,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let group = books
        .masters
        .stock_groups
        .get(&id)
        .ok_or_else(|| missing(format!("stock group id {}", id.0)))?;
    if let Some(child) = books
        .masters
        .stock_groups
        .values()
        .find(|c| c.parent == Some(id))
    {
        return Err(invalid(
            "stock group",
            format!(
                "'{}' still has the sub-group '{}'; move or delete it first",
                group.name, child.name
            ),
        ));
    }
    if let Some(item) = books
        .masters
        .stock_items
        .values()
        .find(|i| i.group == Some(id))
    {
        return Err(invalid(
            "stock group",
            format!(
                "'{}' still holds the stock item '{}'; move or delete it first",
                group.name, item.name
            ),
        ));
    }
    deleted(MasterSnapshot::StockGroup(group.clone()))
}

// ----------------------------------------------------------------------------------------------
// Stock items
// ----------------------------------------------------------------------------------------------

/// Names and aliases of an item must be unique across all items' names and aliases.
fn check_item_names(it: &StockItem, books: &Books) -> Result<(), EngineError> {
    let mut taken = BTreeSet::new();
    for other in books.masters.stock_items.values().filter(|o| o.id != it.id) {
        taken.insert(key(&other.name));
        taken.extend(other.aliases.iter().map(|a| key(a)));
    }
    let mut mine = BTreeSet::new();
    for name in std::iter::once(&it.name).chain(it.aliases.iter()) {
        let k = key(name);
        if k.is_empty() {
            return Err(invalid(
                "stock item name or alias",
                "a name or alias is empty; enter it or remove it",
            ));
        }
        if taken.contains(&k) || !mine.insert(k) {
            return Err(exists(format!(
                "stock item name or alias '{}'",
                name.trim()
            )));
        }
    }
    Ok(())
}

fn check_item_units(it: &StockItem, books: &Books) -> Result<(), EngineError> {
    if let Some(group) = it.group {
        if !books.masters.stock_groups.contains_key(&group) {
            return Err(missing(format!("stock group {} of the item", group.0)));
        }
    }
    if !books.masters.units.contains_key(&it.unit) {
        return Err(missing(format!("base unit {} of the item", it.unit.0)));
    }
    if let Some(alt) = it.alt_unit {
        if !books.masters.units.contains_key(&alt.unit) {
            return Err(missing(format!(
                "alternate unit {} of the item",
                alt.unit.0
            )));
        }
        if alt.unit == it.unit {
            return Err(invalid(
                "alternate unit",
                "it is the same as the base unit; pick another or remove it",
            ));
        }
        if alt.base_per_alt <= Decimal::ZERO {
            return Err(invalid(
                "alternate unit conversion",
                format!(
                    "{} base units per alternate unit is not above zero",
                    alt.base_per_alt
                ),
            ));
        }
    }
    Ok(())
}

fn check_item_gst(it: &StockItem) -> Result<(), EngineError> {
    let hundred = Decimal::from(100);
    let mut previous: Option<Date> = None;
    for entry in &it.gst_history {
        if previous.is_some_and(|p| entry.effective_from <= p) {
            return Err(invalid(
                "GST history",
                format!(
                    "{} is not after the previous entry; list entries in date order, one per date",
                    entry.effective_from
                ),
            ));
        }
        previous = Some(entry.effective_from);
        match (entry.taxability, entry.rate_percent) {
            (Taxability::Taxable, Some(rate)) => {
                if rate.0 < Decimal::ZERO || rate.0 > hundred {
                    return Err(invalid(
                        "GST history rate",
                        format!("{}% is outside 0 to 100", rate.0),
                    ));
                }
            }
            (Taxability::Taxable, None) => {
                return Err(invalid(
                    "GST history rate",
                    format!(
                        "the entry from {} is taxable but has no rate",
                        entry.effective_from
                    ),
                ));
            }
            (Taxability::Exempt | Taxability::NilRated | Taxability::NonGst, Some(_)) => {
                return Err(invalid(
                    "GST history rate",
                    format!(
                        "the entry from {} is not taxable, so it takes no rate",
                        entry.effective_from
                    ),
                ));
            }
            (Taxability::Exempt | Taxability::NilRated | Taxability::NonGst, None) => {}
        }
    }
    let mut previous: Option<Date> = None;
    for cost in &it.standard_costs {
        if previous.is_some_and(|p| cost.from <= p) {
            return Err(invalid(
                "standard cost",
                format!(
                    "{} is not after the previous entry; one per date, in order",
                    cost.from
                ),
            ));
        }
        previous = Some(cost.from);
        if cost.rate.0 < Decimal::ZERO {
            return Err(invalid("standard cost", "a rate below zero is not allowed"));
        }
    }
    Ok(())
}

fn check_item_opening(it: &StockItem, books: &Books) -> Result<(), EngineError> {
    let places = books
        .masters
        .units
        .get(&it.unit)
        .map_or(0, |u| u32::from(u.decimals));
    let mut seen = BTreeSet::new();
    for line in &it.opening {
        if !books.masters.godowns.contains_key(&line.godown) {
            return Err(missing(format!(
                "godown {} of the opening stock",
                line.godown.0
            )));
        }
        if let Some(batch) = line.batch {
            let belongs = books
                .masters
                .batches
                .get(&batch)
                .is_some_and(|b| b.item == it.id);
            if !belongs {
                return Err(missing(format!(
                    "batch {} of this item for the opening stock",
                    batch.0
                )));
            }
            if !it.batches_enabled {
                return Err(invalid(
                    "opening stock",
                    "a batch is named but the item does not track batches",
                ));
            }
        }
        if !seen.insert((line.godown, line.batch)) {
            return Err(invalid(
                "opening stock",
                "a godown and batch appear twice; combine them into one line",
            ));
        }
        if line.qty.0 < Decimal::ZERO || line.rate.0 < Decimal::ZERO {
            return Err(invalid(
                "opening stock",
                "quantity and rate cannot be below zero",
            ));
        }
        if line.qty.0.normalize().scale() > places {
            return Err(invalid(
                "opening stock",
                format!(
                    "quantity {} has more decimal places than the base unit allows ({places})",
                    line.qty.0
                ),
            ));
        }
    }
    Ok(())
}

fn check_item(it: &StockItem, books: &Books) -> Result<(), EngineError> {
    require_name("stock item name", &it.name)?;
    check_item_names(it, books)?;
    check_item_units(it, books)?;
    check_item_gst(it)?;
    check_item_opening(it, books)
}

fn item_in_vouchers(books: &Books, id: StockItemId) -> bool {
    books
        .vouchers
        .values()
        .any(|v| v.inventory_lines.iter().any(|l| l.item == id))
}

/// Create a stock item.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id, name or alias; [`EngineError::NotFound`] for a
/// missing group, unit, godown or batch; [`EngineError::Invalid`] for an empty name, an alternate
/// unit equal to the base unit or with a factor of zero or less, GST history out of date order or
/// with a missing or misplaced rate, standard costs out of date order, or opening stock that is
/// negative, repeated per godown and batch, finer than the base unit's decimals, or names a batch
/// of an item that does not track batches.
pub fn create_stock_item(it: StockItem, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.stock_items.contains_key(&it.id) {
        return Err(exists(format!("stock item id {}", it.id.0)));
    }
    check_item(&it, books)?;
    created(MasterSnapshot::StockItem(it))
}

/// Alter a stock item.
///
/// # Errors
/// As [`create_stock_item`], plus [`EngineError::NotFound`] when it does not exist and
/// [`EngineError::Invalid`] when a used item changes its base or alternate unit, or stops
/// tracking batches while batches of it exist.
pub fn alter_stock_item(it: StockItem, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .stock_items
        .get(&it.id)
        .ok_or_else(|| missing(format!("stock item id {}", it.id.0)))?;
    check_item(&it, books)?;
    let used = item_in_vouchers(books, it.id) || books.stock_moves.iter().any(|m| m.item == it.id);
    if used && (before.unit != it.unit || before.alt_unit != it.alt_unit) {
        return Err(invalid(
            "stock item units",
            format!(
                "'{}' has stock entries, so its units cannot change; create a new item instead",
                before.name
            ),
        ));
    }
    if before.batches_enabled
        && !it.batches_enabled
        && books.masters.batches.values().any(|b| b.item == it.id)
    {
        return Err(invalid(
            "stock item batches",
            format!(
                "'{}' still has batches; delete them before turning batches off",
                before.name
            ),
        ));
    }
    altered(
        *before != it,
        MasterSnapshot::StockItem(before.clone()),
        MasterSnapshot::StockItem(it),
    )
}

/// Delete a stock item.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when vouchers use it;
/// [`EngineError::Invalid`] when stock movements or batches of it exist.
pub fn delete_stock_item(
    id: StockItemId,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let item = books
        .masters
        .stock_items
        .get(&id)
        .ok_or_else(|| missing(format!("stock item id {}", id.0)))?;
    not_in_vouchers(books, |v| v.inventory_lines.iter().any(|l| l.item == id))?;
    if books.stock_moves.iter().any(|m| m.item == id) {
        return Err(invalid(
            "stock item",
            format!("'{}' has stock movements; they must go first", item.name),
        ));
    }
    if let Some(batch) = books.masters.batches.values().find(|b| b.item == id) {
        return Err(invalid(
            "stock item",
            format!(
                "'{}' still has the batch '{}'; delete it first",
                item.name, batch.name
            ),
        ));
    }
    deleted(MasterSnapshot::StockItem(item.clone()))
}

// ----------------------------------------------------------------------------------------------
// Godowns
// ----------------------------------------------------------------------------------------------

fn check_godown(g: &Godown, books: &Books) -> Result<(), EngineError> {
    require_name("godown name", &g.name)?;
    let name = key(&g.name);
    if books
        .masters
        .godowns
        .values()
        .any(|o| o.id != g.id && key(&o.name) == name)
    {
        return Err(exists(format!("godown '{}'", g.name.trim())));
    }
    if let Some(parent) = g.parent {
        if parent == g.id {
            return Err(EngineError::GroupCycle);
        }
        if !books.masters.godowns.contains_key(&parent) {
            return Err(missing(format!("parent godown {}", parent.0)));
        }
        let up = |id: GodownId| books.masters.godowns.get(&id).and_then(|p| p.parent);
        if climbs_to(parent, g.id, up) {
            return Err(EngineError::GroupCycle);
        }
    }
    Ok(())
}

/// Create a godown.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or name; [`EngineError::Invalid`] for an empty name;
/// [`EngineError::NotFound`] for a missing parent; [`EngineError::GroupCycle`] when the godown is
/// its own parent.
pub fn create_godown(g: Godown, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.godowns.contains_key(&g.id) {
        return Err(exists(format!("godown id {}", g.id.0)));
    }
    check_godown(&g, books)?;
    created(MasterSnapshot::Godown(g))
}

/// Alter a godown (rename or move it).
///
/// # Errors
/// As [`create_godown`], plus [`EngineError::NotFound`] when it does not exist and
/// [`EngineError::GroupCycle`] when the new parent is the godown itself or one inside it.
pub fn alter_godown(g: Godown, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .godowns
        .get(&g.id)
        .ok_or_else(|| missing(format!("godown id {}", g.id.0)))?;
    check_godown(&g, books)?;
    altered(
        *before != g,
        MasterSnapshot::Godown(before.clone()),
        MasterSnapshot::Godown(g),
    )
}

/// Delete a godown.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when vouchers allocate
/// stock to it; [`EngineError::Invalid`] while it has sub-godowns, opening stock or stock
/// movements.
pub fn delete_godown(id: GodownId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let godown = books
        .masters
        .godowns
        .get(&id)
        .ok_or_else(|| missing(format!("godown id {}", id.0)))?;
    not_in_vouchers(books, |v| {
        v.inventory_lines
            .iter()
            .any(|l| l.godowns.iter().any(|a| a.godown == id))
    })?;
    if let Some(child) = books
        .masters
        .godowns
        .values()
        .find(|c| c.parent == Some(id))
    {
        return Err(invalid(
            "godown",
            format!(
                "'{}' still has the sub-godown '{}'; move or delete it first",
                godown.name, child.name
            ),
        ));
    }
    let opening = books
        .masters
        .stock_items
        .values()
        .find(|i| i.opening.iter().any(|o| o.godown == id));
    if let Some(item) = opening {
        return Err(invalid(
            "godown",
            format!(
                "'{}' holds opening stock of '{}'; remove that stock first",
                godown.name, item.name
            ),
        ));
    }
    if books.stock_moves.iter().any(|m| m.godown == id) {
        return Err(invalid(
            "godown",
            format!(
                "'{}' holds stock movements; they must go first",
                godown.name
            ),
        ));
    }
    deleted(MasterSnapshot::Godown(godown.clone()))
}

// ----------------------------------------------------------------------------------------------
// Batches
// ----------------------------------------------------------------------------------------------

fn check_batch(b: &Batch, books: &Books) -> Result<(), EngineError> {
    require_name("batch name", &b.name)?;
    let item = books
        .masters
        .stock_items
        .get(&b.item)
        .ok_or_else(|| missing(format!("stock item {} of the batch", b.item.0)))?;
    if !item.batches_enabled {
        return Err(invalid(
            "batch",
            format!(
                "'{}' does not track batches; turn batches on for the item first",
                item.name
            ),
        ));
    }
    let name = key(&b.name);
    if books
        .masters
        .batches
        .values()
        .any(|o| o.id != b.id && o.item == b.item && key(&o.name) == name)
    {
        return Err(exists(format!(
            "batch '{}' of '{}'",
            b.name.trim(),
            item.name
        )));
    }
    if let (Some(mfg), Some(expiry)) = (b.mfg, b.expiry) {
        if expiry < mfg {
            return Err(invalid(
                "batch expiry",
                format!("{expiry} is before the manufacturing date {mfg}"),
            ));
        }
    }
    Ok(())
}

/// Create a batch of a stock item.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or a batch name already used on the item;
/// [`EngineError::NotFound`] for a missing item; [`EngineError::Invalid`] for an empty name, an
/// item that does not track batches, or an expiry before the manufacturing date.
pub fn create_batch(b: Batch, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.batches.contains_key(&b.id) {
        return Err(exists(format!("batch id {}", b.id.0)));
    }
    check_batch(&b, books)?;
    created(MasterSnapshot::Batch(b))
}

/// Alter a batch (name and dates; a batch never moves to another item).
///
/// # Errors
/// As [`create_batch`], plus [`EngineError::NotFound`] when it does not exist and
/// [`EngineError::Invalid`] when the item changes.
pub fn alter_batch(b: Batch, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .batches
        .get(&b.id)
        .ok_or_else(|| missing(format!("batch id {}", b.id.0)))?;
    if before.item != b.item {
        return Err(invalid(
            "batch item",
            "a batch cannot move to another item; create a new batch instead",
        ));
    }
    check_batch(&b, books)?;
    altered(
        *before != b,
        MasterSnapshot::Batch(before.clone()),
        MasterSnapshot::Batch(b),
    )
}

/// Delete a batch.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when voucher lines
/// allocate stock to it; [`EngineError::Invalid`] when opening stock or stock movements use it.
pub fn delete_batch(id: BatchId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let batch = books
        .masters
        .batches
        .get(&id)
        .ok_or_else(|| missing(format!("batch id {}", id.0)))?;
    not_in_vouchers(books, |v| {
        v.inventory_lines
            .iter()
            .any(|l| l.godowns.iter().any(|a| a.batch == Some(id)))
    })?;
    let opening = books
        .masters
        .stock_items
        .values()
        .any(|i| i.opening.iter().any(|o| o.batch == Some(id)));
    if opening || books.stock_moves.iter().any(|m| m.batch == Some(id)) {
        return Err(invalid(
            "batch",
            format!("'{}' holds stock; remove that stock first", batch.name),
        ));
    }
    deleted(MasterSnapshot::Batch(batch.clone()))
}

// ----------------------------------------------------------------------------------------------
// Cost categories and cost centres
// ----------------------------------------------------------------------------------------------

fn check_cost_category(c: &CostCategory, books: &Books) -> Result<(), EngineError> {
    require_name("cost category name", &c.name)?;
    let name = key(&c.name);
    if books
        .masters
        .cost_categories
        .values()
        .any(|o| o.id != c.id && key(&o.name) == name)
    {
        return Err(exists(format!("cost category '{}'", c.name.trim())));
    }
    Ok(())
}

/// Create a cost category.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or name; [`EngineError::Invalid`] for an empty name.
pub fn create_cost_category(
    c: CostCategory,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.cost_categories.contains_key(&c.id) {
        return Err(exists(format!("cost category id {}", c.id.0)));
    }
    check_cost_category(&c, books)?;
    created(MasterSnapshot::CostCategory(c))
}

/// Alter (rename) a cost category.
///
/// # Errors
/// As [`create_cost_category`], plus [`EngineError::NotFound`] when it does not exist.
pub fn alter_cost_category(
    c: CostCategory,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .cost_categories
        .get(&c.id)
        .ok_or_else(|| missing(format!("cost category id {}", c.id.0)))?;
    check_cost_category(&c, books)?;
    altered(
        *before != c,
        MasterSnapshot::CostCategory(before.clone()),
        MasterSnapshot::CostCategory(c),
    )
}

/// Delete a cost category.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when voucher lines
/// allocate to it; [`EngineError::Invalid`] while it has cost centres.
pub fn delete_cost_category(
    id: CostCategoryId,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let category = books
        .masters
        .cost_categories
        .get(&id)
        .ok_or_else(|| missing(format!("cost category id {}", id.0)))?;
    not_in_vouchers(books, |v| {
        v.ledger_lines
            .iter()
            .any(|l| l.cost.iter().any(|c| c.category == id))
    })?;
    if let Some(centre) = books
        .masters
        .cost_centres
        .values()
        .find(|c| c.category == id)
    {
        return Err(invalid(
            "cost category",
            format!(
                "'{}' still has the cost centre '{}'; move or delete it first",
                category.name, centre.name
            ),
        ));
    }
    deleted(MasterSnapshot::CostCategory(category.clone()))
}

fn check_cost_centre(c: &CostCentre, books: &Books) -> Result<(), EngineError> {
    require_name("cost centre name", &c.name)?;
    if !books.masters.cost_categories.contains_key(&c.category) {
        return Err(missing(format!(
            "cost category {} of the centre",
            c.category.0
        )));
    }
    let name = key(&c.name);
    if books
        .masters
        .cost_centres
        .values()
        .any(|o| o.id != c.id && o.category == c.category && key(&o.name) == name)
    {
        return Err(exists(format!(
            "cost centre '{}' in this category",
            c.name.trim()
        )));
    }
    if let Some(parent) = c.parent {
        if parent == c.id {
            return Err(EngineError::GroupCycle);
        }
        let parent_centre = books
            .masters
            .cost_centres
            .get(&parent)
            .ok_or_else(|| missing(format!("parent cost centre {}", parent.0)))?;
        if parent_centre.category != c.category {
            return Err(invalid(
                "cost centre parent",
                format!(
                    "'{}' is in another category; a centre and its parent share one category",
                    parent_centre.name
                ),
            ));
        }
        let up = |id: CostCentreId| books.masters.cost_centres.get(&id).and_then(|p| p.parent);
        if climbs_to(parent, c.id, up) {
            return Err(EngineError::GroupCycle);
        }
    }
    Ok(())
}

fn centre_in_vouchers(books: &Books, id: CostCentreId) -> usize {
    books
        .vouchers
        .values()
        .filter(|v| {
            v.ledger_lines
                .iter()
                .any(|l| l.cost.iter().any(|a| a.centre == id))
        })
        .count()
}

/// Create a cost centre.
///
/// # Errors
/// [`EngineError::Duplicate`] for a taken id or a name already used in the category;
/// [`EngineError::NotFound`] for a missing category or parent; [`EngineError::Invalid`] for an
/// empty name or a parent in another category; [`EngineError::GroupCycle`] when the centre is its
/// own parent.
pub fn create_cost_centre(
    c: CostCentre,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.masters.cost_centres.contains_key(&c.id) {
        return Err(exists(format!("cost centre id {}", c.id.0)));
    }
    check_cost_centre(&c, books)?;
    created(MasterSnapshot::CostCentre(c))
}

/// Alter a cost centre.
///
/// # Errors
/// As [`create_cost_centre`], plus [`EngineError::NotFound`] when it does not exist,
/// [`EngineError::InUse`] when vouchers allocate to a centre whose category changes, and
/// [`EngineError::Invalid`] when a centre with sub-centres changes category.
pub fn alter_cost_centre(c: CostCentre, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .cost_centres
        .get(&c.id)
        .ok_or_else(|| missing(format!("cost centre id {}", c.id.0)))?;
    check_cost_centre(&c, books)?;
    if before.category != c.category {
        let vouchers = centre_in_vouchers(books, c.id);
        if vouchers > 0 {
            return Err(EngineError::InUse { vouchers });
        }
        if books
            .masters
            .cost_centres
            .values()
            .any(|o| o.parent == Some(c.id))
        {
            return Err(invalid(
                "cost centre category",
                format!(
                    "'{}' has sub-centres in its old category; move them first",
                    before.name
                ),
            ));
        }
    }
    altered(
        *before != c,
        MasterSnapshot::CostCentre(before.clone()),
        MasterSnapshot::CostCentre(c),
    )
}

/// Delete a cost centre.
///
/// # Errors
/// [`EngineError::NotFound`] when it does not exist; [`EngineError::InUse`] when voucher lines
/// allocate to it; [`EngineError::Invalid`] while it has sub-centres.
pub fn delete_cost_centre(
    id: CostCentreId,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let centre = books
        .masters
        .cost_centres
        .get(&id)
        .ok_or_else(|| missing(format!("cost centre id {}", id.0)))?;
    let vouchers = centre_in_vouchers(books, id);
    if vouchers > 0 {
        return Err(EngineError::InUse { vouchers });
    }
    if let Some(child) = books
        .masters
        .cost_centres
        .values()
        .find(|o| o.parent == Some(id))
    {
        return Err(invalid(
            "cost centre",
            format!(
                "'{}' still has the sub-centre '{}'; move or delete it first",
                centre.name, child.name
            ),
        ));
    }
    deleted(MasterSnapshot::CostCentre(centre.clone()))
}
