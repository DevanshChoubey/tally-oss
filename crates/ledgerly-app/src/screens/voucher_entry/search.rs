//! The ledger type-ahead: ranking ledgers for a typed query ([`search_ledgers`]).

use std::collections::BTreeMap;

use ledgerly_core::{LedgerId, SignedAmount};
use ledgerly_model::{Ledger, Masters};

/// How many type-ahead items are listed at most.
const PICKER_LIMIT: usize = 50;

/// A ledger offered by the type-ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerHit {
    /// The ledger.
    pub id: LedgerId,
    /// Its name.
    pub name: String,
    /// The alias that matched, when the name itself did not match as well.
    pub via_alias: Option<String>,
    /// GSTIN, for parties that have one.
    pub gstin: Option<String>,
    /// Current balance as supplied by the host; `None` when not supplied.
    pub balance: Option<SignedAmount>,
    /// Whether the ledger is among the recently used.
    pub recent: bool,
}

/// One line of the type-ahead list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerItem {
    /// An existing ledger.
    Ledger(LedgerHit),
    /// "Create a ledger with the typed name".
    Create {
        /// The typed name.
        name: String,
    },
}

/// How well the lowercase `candidate` matches the lowercase `query`: 0 exact, 1 prefix, 2 a word
/// starts with it, 3 contains it, 4 its letters appear in order. `None` for no match.
fn match_rank(candidate: &str, query: &str) -> Option<u8> {
    if candidate == query {
        Some(0)
    } else if candidate.starts_with(query) {
        Some(1)
    } else if candidate.split_whitespace().any(|w| w.starts_with(query)) {
        Some(2)
    } else if candidate.contains(query) {
        Some(3)
    } else {
        let mut rest = candidate.chars();
        query.chars().all(|q| rest.any(|x| x == q)).then_some(4)
    }
}

/// The best rank of a ledger's name (given lowercase as `name_lower`) or aliases for the
/// lowercase `q`, and the alias that gave it when it beat the name. An empty query matches
/// everything with rank 5.
fn best_match(ledger: &Ledger, name_lower: &str, q: &str) -> Option<(u8, Option<String>)> {
    if q.is_empty() {
        return Some((5, None));
    }
    let mut best = match_rank(name_lower, q).map(|r| (r, None));
    for alias in &ledger.aliases {
        if let Some(r) = match_rank(&alias.to_lowercase(), q)
            && best.as_ref().is_none_or(|(b, _)| r < *b)
        {
            best = Some((r, Some(alias.clone())));
        }
    }
    best
}

/// Ledgers matching `query` (name or alias; exact, prefix, word, substring, then in-order
/// letters), at most 50.
///
/// Order: an exact name or alias match first; then ledgers in `recents` (most recent first);
/// then the rest by match quality and name. An empty query lists every ledger, recents first.
/// When the query is not exactly an existing name, a [`PickerItem::Create`] item ends the list.
#[must_use]
pub fn search_ledgers(
    masters: &Masters,
    query: &str,
    recents: &[LedgerId],
    balances: &BTreeMap<LedgerId, SignedAmount>,
) -> Vec<PickerItem> {
    type SortKey = (u8, usize, u8, String, LedgerId);
    let typed = query.trim();
    let q = typed.to_lowercase();
    let mut exists = false;
    let mut scored: Vec<(SortKey, &Ledger, Option<String>)> = Vec::new();
    for ledger in masters.ledgers.values() {
        let name_lower = ledger.name.to_lowercase();
        exists = exists || name_lower == q;
        let Some((rank, via_alias)) = best_match(ledger, &name_lower, &q) else {
            continue;
        };
        let recent_pos = recents.iter().position(|r| *r == ledger.id);
        let key = (
            u8::from(rank != 0),
            recent_pos.unwrap_or(usize::MAX),
            rank,
            name_lower,
            ledger.id,
        );
        scored.push((key, ledger, via_alias));
    }
    // Only the best PICKER_LIMIT are listed: partition them out before sorting, so a company with
    // tens of thousands of ledgers does not pay a full sort on every frame. Keys end in the ledger
    // id, so they are distinct and the order is total.
    if scored.len() > PICKER_LIMIT {
        let _partitioned = scored.select_nth_unstable_by(PICKER_LIMIT, |a, b| a.0.cmp(&b.0));
        scored.truncate(PICKER_LIMIT);
    }
    scored.sort_by_cached_key(|entry| entry.0.clone());
    let mut items: Vec<PickerItem> = scored
        .into_iter()
        .map(|(key, ledger, via_alias)| {
            PickerItem::Ledger(LedgerHit {
                id: ledger.id,
                name: ledger.name.clone(),
                via_alias,
                gstin: ledger
                    .party
                    .as_ref()
                    .and_then(|p| p.gstin.as_ref())
                    .map(|g| g.as_str().to_owned()),
                balance: balances.get(&ledger.id).copied(),
                recent: key.1 != usize::MAX,
            })
        })
        .collect();
    if !q.is_empty() && !exists {
        items.push(PickerItem::Create {
            name: typed.to_owned(),
        });
    }
    items
}
