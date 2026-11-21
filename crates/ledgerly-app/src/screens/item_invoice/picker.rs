//! Type-ahead for stock items and ledgers, and the labels and row summaries the lists show.

use super::draft::line_amount;
use super::{Field, ItemInvoiceScreen, PICKER_LIMIT, Pick};
use crate::screens::voucher_entry::{LedgerHit, PickerItem, search_ledgers};
use ledgerly_core::format::format_inr;
use ledgerly_core::{DrCr, LedgerId, SignedAmount, StockItemId};
use ledgerly_model::Masters;
use std::collections::BTreeMap;

/// A stock item offered by the type-ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemHit {
    /// The item.
    pub id: StockItemId,
    /// Its name.
    pub name: String,
    /// The alias that matched, when the name itself did not match as well.
    pub via_alias: Option<String>,
    /// Symbol of the item's unit, when the unit exists.
    pub unit: Option<String>,
}

/// 0 exact, 1 prefix, 2 a word starts with it, 3 contains it; `None` for no match.
fn item_rank(candidate: &str, query: &str) -> Option<u8> {
    if candidate == query {
        Some(0)
    } else if candidate.starts_with(query) {
        Some(1)
    } else if candidate.split_whitespace().any(|w| w.starts_with(query)) {
        Some(2)
    } else if candidate.contains(query) {
        Some(3)
    } else {
        None
    }
}

/// Stock items whose name or alias matches `query`, best match first then by name, at most 50.
/// An empty query lists every item.
#[must_use]
pub fn search_items(masters: &Masters, query: &str) -> Vec<ItemHit> {
    let q = query.trim().to_lowercase();
    let mut found: Vec<((u8, String, StockItemId), ItemHit)> = Vec::new();
    for item in masters.stock_items.values() {
        let name_lower = item.name.to_lowercase();
        let mut best: Option<(u8, Option<String>)> = if q.is_empty() {
            Some((4, None))
        } else {
            item_rank(&name_lower, &q).map(|r| (r, None))
        };
        // An empty query matches by name alone: no alias was typed, so none can be the reason.
        for alias in item.aliases.iter().filter(|_| !q.is_empty()) {
            if let Some(r) = item_rank(&alias.to_lowercase(), &q)
                && best.as_ref().is_none_or(|(b, _)| r < *b)
            {
                best = Some((r, Some(alias.clone())));
            }
        }
        let Some((rank, via_alias)) = best else {
            continue;
        };
        let unit = masters.units.get(&item.unit).map(|u| u.symbol.clone());
        found.push((
            (rank, name_lower, item.id),
            ItemHit {
                id: item.id,
                name: item.name.clone(),
                via_alias,
                unit,
            },
        ));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.truncate(PICKER_LIMIT);
    found.into_iter().map(|(_, hit)| hit).collect()
}

/// The ledgers matching `query`, without the "create" entry (this screen creates no masters).
pub(super) fn ledger_hits(
    masters: &Masters,
    query: &str,
    recents: &[LedgerId],
    balances: &BTreeMap<LedgerId, SignedAmount>,
) -> Vec<LedgerHit> {
    search_ledgers(masters, query, recents, balances)
        .into_iter()
        .filter_map(|item| match item {
            PickerItem::Ledger(hit) => Some(hit),
            PickerItem::Create { .. } => None,
        })
        .collect()
}
fn ledger_label(hit: &LedgerHit) -> String {
    let mut parts = vec![hit.name.clone()];
    if let Some(alias) = &hit.via_alias {
        parts.push(format!("(alias {alias})"));
    }
    if let Some(balance) = hit.balance {
        let side = match balance.side {
            DrCr::Dr => "Dr",
            DrCr::Cr => "Cr",
        };
        parts.push(format!("{} {side}", format_inr(balance.amount)));
    }
    if let Some(gstin) = &hit.gstin {
        parts.push(gstin.clone());
    }
    parts.join("   ")
}

fn item_label(hit: &ItemHit) -> String {
    let mut parts = vec![hit.name.clone()];
    if let Some(alias) = &hit.via_alias {
        parts.push(format!("(alias {alias})"));
    }
    if let Some(unit) = &hit.unit {
        parts.push(format!("per {unit}"));
    }
    parts.join("   ")
}

impl ItemInvoiceScreen {
    fn pick_kind(&self) -> Option<Pick> {
        match self.focus {
            Field::Party => self.draft.party.is_none().then_some(Pick::Party),
            Field::Account => self.draft.account.is_none().then_some(Pick::Account),
            Field::Item(i) => self
                .draft
                .rows
                .get(i)
                .filter(|r| r.item.is_none())
                .map(|_| Pick::Item(i)),
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Narration => None,
        }
    }

    /// The lines of the type-ahead list that is open, as shown.
    pub(super) fn pick_labels(&self, masters: &Masters) -> Vec<String> {
        let Some(kind) = self.pick_kind() else {
            return Vec::new();
        };
        match kind {
            Pick::Party => {
                let hits = self.ledger_choices(masters, &self.draft.party_text);
                hits.iter().map(ledger_label).collect()
            }
            Pick::Account => {
                let hits = self.ledger_choices(masters, &self.draft.account_text);
                hits.iter().map(ledger_label).collect()
            }
            Pick::Item(i) => {
                let text = self.draft.rows.get(i).map_or("", |r| r.item_text.as_str());
                search_items(masters, text).iter().map(item_label).collect()
            }
        }
    }

    /// `true` while the field that has the keyboard holds an unfinished search.
    pub(super) fn searching(&self) -> bool {
        let text = match self.pick_kind() {
            Some(Pick::Party) => self.draft.party_text.as_str(),
            Some(Pick::Account) => self.draft.account_text.as_str(),
            Some(Pick::Item(i)) => self.draft.rows.get(i).map_or("", |r| r.item_text.as_str()),
            None => return false,
        };
        !text.trim().is_empty()
    }

    pub(super) fn clear_search(&mut self) {
        match self.pick_kind() {
            Some(Pick::Party) => self.set_party_text(""),
            Some(Pick::Account) => self.set_account_text(""),
            Some(Pick::Item(i)) => self.set_item_text(i, ""),
            None => {}
        }
    }

    pub(super) fn row_summary(&self, masters: &Masters, i: usize) -> String {
        let Some(row) = self.draft.rows.get(i) else {
            return String::new();
        };
        if row.is_blank() {
            return String::new();
        }
        let unit = row
            .item
            .and_then(|id| masters.stock_items.get(&id))
            .and_then(|item| masters.units.get(&item.unit))
            .map_or(String::new(), |u| format!("{}  ", u.symbol));
        match line_amount(masters, row, i.saturating_add(1)) {
            Ok(amount) => format!("{unit}Amount {}", format_inr(amount)),
            Err(e) => format!("{unit}{e}"),
        }
    }
}
