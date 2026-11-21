//! View models for the masters screen: the ledger and stock item rows, the party detail, and the
//! GSTIN check the screen shows.
//!
//! Everything here reads the masters and returns plain values. Nothing draws, writes or computes
//! a figure. Ledger balances and stock quantities and values are not in these types: the engine
//! does not supply them to this screen yet, so the screen shows them as not available rather than
//! inventing them (rule C4).

use ledgerly_core::{Gstin, LedgerId, StockItemId};
use ledgerly_model::{Ledger, Masters};

use super::{MasterKind, MasterRef, list_rows};

/// The kinds the screen shows as tabs (mock v2: Ledgers, Stock items), in tab order.
pub const PRIMARY_KINDS: [MasterKind; 2] = [MasterKind::Ledger, MasterKind::StockItem];

/// The kinds behind "More master kinds" (decision 8: they live under Settings). Every kind except
/// the two tabs, in the order of [`MasterKind::ALL`].
pub const MORE_KINDS: [MasterKind; 8] = [
    MasterKind::Group,
    MasterKind::VoucherType,
    MasterKind::Unit,
    MasterKind::StockGroup,
    MasterKind::Godown,
    MasterKind::Batch,
    MasterKind::CostCategory,
    MasterKind::CostCentre,
];

/// The tab index of `kind`, or `None` when the kind is behind "More master kinds" and no tab is
/// selected.
#[must_use]
pub fn primary_tab(kind: MasterKind) -> Option<usize> {
    PRIMARY_KINDS.iter().position(|k| *k == kind)
}

/// What the GSTIN check says about a typed GSTIN. The check is [`Gstin::parse`]: length,
/// characters, state code, PAN shape and check character, all on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GstinCheck {
    /// Nothing typed, so there is nothing to check.
    Empty,
    /// Every check passes.
    Valid,
    /// The first problem found, in the words of `ledgerly_core`.
    Invalid(String),
}

/// Checks `text` as a GSTIN. Surrounding spaces are ignored, as [`Gstin::parse`] ignores them.
#[must_use]
pub fn check_gstin(text: &str) -> GstinCheck {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return GstinCheck::Empty;
    }
    match Gstin::parse(trimmed) {
        Ok(_) => GstinCheck::Valid,
        Err(e) => GstinCheck::Invalid(e.to_string()),
    }
}

/// One ledger line in the ledger table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRow {
    /// Which ledger.
    pub master: MasterRef,
    /// Its name.
    pub name: String,
    /// Its aliases, joined with `, `; empty when it has none.
    pub alias: String,
    /// The name of its group, or a plain note when the group is missing from the books.
    pub group: String,
    /// Its GSTIN as stored; empty for a ledger with none.
    pub gstin: String,
}

/// The ledgers the filter keeps, in name order (the order of [`list_rows`]).
#[must_use]
pub fn ledger_rows(masters: &Masters, filter: &str) -> Vec<LedgerRow> {
    list_rows(masters, MasterKind::Ledger, filter)
        .into_iter()
        .filter_map(|row| {
            // `list_rows` reads these same ledgers, so every row has one; a row with no ledger
            // would be a bug in the list, and is left out rather than shown half-filled.
            let ledger = masters.ledgers.get(&LedgerId(row.master.id))?;
            Some(LedgerRow {
                master: row.master,
                name: row.name,
                alias: ledger.aliases.join(", "),
                group: group_name(masters, ledger),
                gstin: gstin_text(ledger),
            })
        })
        .collect()
}

/// One stock item line in the stock item table. Stock quantities and values are not here: the
/// engine does not supply them yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    /// Which stock item.
    pub master: MasterRef,
    /// Its name.
    pub name: String,
    /// Its HSN code as stored; empty when none is stored.
    pub hsn: String,
    /// The GST rate of its latest entry in the GST history, as `18%`; empty when there is none.
    /// The history is not shown date by date yet.
    pub gst_rate: String,
}

/// The stock items the filter keeps, in name order.
#[must_use]
pub fn item_rows(masters: &Masters, filter: &str) -> Vec<ItemRow> {
    list_rows(masters, MasterKind::StockItem, filter)
        .into_iter()
        .filter_map(|row| {
            // Same reasoning as `ledger_rows`: the row comes from these items.
            let item = masters.stock_items.get(&StockItemId(row.master.id))?;
            let hsn = item
                .hsn
                .as_ref()
                .map_or_else(String::new, |h| h.as_str().to_owned());
            let gst_rate = item
                .gst_history
                .iter()
                .max_by_key(|h| h.effective_from)
                .and_then(|h| h.rate_percent)
                .map_or_else(String::new, |r| format!("{}%", r.0));
            Some(ItemRow {
                master: row.master,
                name: row.name,
                hsn,
                gst_rate,
            })
        })
        .collect()
}

/// The fields the party detail panel shows for one ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerDetail {
    /// Its name.
    pub name: String,
    /// Its aliases, joined with `, `; empty when it has none.
    pub alias: String,
    /// The name of its group.
    pub group: String,
    /// Its GSTIN as stored; empty when it has none.
    pub gstin: String,
    /// The check of [`LedgerDetail::gstin`].
    pub gstin_check: GstinCheck,
    /// The state as `27 Maharashtra`, from the party record or the GSTIN; empty when neither
    /// gives one.
    pub state: String,
    /// The default credit days of a bill-wise ledger; `None` when not set.
    pub credit_days: Option<u32>,
    /// `true` when the ledger tracks bills.
    pub bill_wise: bool,
    /// The address lines and PIN code, joined with `, `; empty when there is none.
    pub address: String,
}

/// The detail of the ledger with raw id `id`, or `None` when no such ledger exists.
#[must_use]
pub fn ledger_detail(masters: &Masters, id: u64) -> Option<LedgerDetail> {
    let ledger = masters.ledgers.get(&LedgerId(id))?;
    let gstin = gstin_text(ledger);
    let gstin_check = check_gstin(&gstin);
    Some(LedgerDetail {
        name: ledger.name.clone(),
        alias: ledger.aliases.join(", "),
        group: group_name(masters, ledger),
        gstin,
        gstin_check,
        state: state_text(ledger),
        credit_days: ledger
            .bill_wise
            .as_ref()
            .and_then(|b| b.default_credit_days),
        bill_wise: ledger.bill_wise.is_some(),
        address: address_text(ledger),
    })
}

/// The name of the group `ledger` sits under.
fn group_name(masters: &Masters, ledger: &Ledger) -> String {
    masters.groups.get(&ledger.group).map_or_else(
        || "Group not found in the books".to_owned(),
        |g| g.name.clone(),
    )
}

/// The GSTIN stored on a ledger's party record, or empty.
fn gstin_text(ledger: &Ledger) -> String {
    ledger
        .party
        .as_ref()
        .and_then(|p| p.gstin.as_ref())
        .map_or_else(String::new, |g| g.as_str().to_owned())
}

/// The state of a party ledger as `27 Maharashtra`. The state on the party record wins; the
/// GSTIN's own state fills in when the record has none.
fn state_text(ledger: &Ledger) -> String {
    let Some(party) = ledger.party.as_ref() else {
        return String::new();
    };
    let from_gstin = party.gstin.as_ref().and_then(Gstin::state);
    match party.state.or(from_gstin) {
        Some(code) => format!("{:02} {}", code.get(), code.name()),
        None => String::new(),
    }
}

/// The address lines of a party ledger and its PIN code, joined with `, `.
fn address_text(ledger: &Ledger) -> String {
    let Some(party) = ledger.party.as_ref() else {
        return String::new();
    };
    let mut parts: Vec<String> = party
        .address
        .lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .cloned()
        .collect();
    if let Some(pin) = party.address.pincode.as_ref() {
        parts.push(pin.clone());
    }
    parts.join(", ")
}
