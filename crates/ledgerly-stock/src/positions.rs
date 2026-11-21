//! Stock positions by item, godown and batch. **Card T2** implements this module.
//!
//! Frozen interface (plan card T2):
//!
//! ```text
//! pub struct Position { pub item: StockItemId, pub godown: GodownId, pub batch: Option<BatchId>, pub qty: Qty }
//! pub fn positions(books: &Books, as_of: Date) -> Result<Vec<Position>, StockError>;
//! ```
//!
//! # Definition
//!
//! The position of an `(item, godown, batch)` on a date is the item's opening stock for that
//! godown and batch (`StockItem::opening`, taken at `Company::books_begin`) plus every movement in
//! `Books::stock_moves` dated on or before the date: `qty_in - qty_out`. Quantities are in the
//! item's base unit, as `StockMove` already is.
//!
//! * `Books::stock_moves` is used as given. Which vouchers produce moves (regular ones, not
//!   cancelled, optional or post-dated ones) is decided where the moves are built, not here.
//! * The result is sorted by `(item, godown, batch)`, with the no-batch position before the
//!   batches of the same godown, so the same books always give the same output.
//! * A position whose quantity is exactly zero is left out (nothing is held there). A negative
//!   position is kept as it is: negative stock is reported, never clamped to zero.
//! * A date before `books_begin` is an error: opening stock is only defined from that date, and
//!   no voucher can be dated earlier, so any figure would be a guess.
//! * Moves dated after `as_of` are dropped before they are validated, so a malformed later move is
//!   not reported by an earlier date's positions.
//! * Anything that does not tie to the masters is an error, not a number: an item, godown or
//!   batch that does not exist, a batch that belongs to another item, a negative quantity.
//! * Expiry is recorded on the batch master (`Batch::expiry`) and is read with [`batch_expiry`].
//!   This module does not decide whether a batch is "expired" on a given day: the rule for the
//!   expiry date itself (usable or not) is a policy no source here states, so it is not guessed.

use std::collections::BTreeMap;

use ledgerly_core::{BatchId, Date, Decimal, GodownId, MoneyError, Qty, StockItemId};
use ledgerly_model::Books;
use serde::{Deserialize, Serialize};

use crate::StockError;

/// Quantity held of one item in one godown and batch on a date, in base units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Item.
    pub item: StockItemId,
    /// Godown.
    pub godown: GodownId,
    /// Batch, or `None` for stock held without a batch.
    pub batch: Option<BatchId>,
    /// Quantity on hand in base units; negative when more was issued than received.
    pub qty: Qty,
}

type Key = (StockItemId, GodownId, Option<BatchId>);

fn invalid(field: &str, reason: String) -> StockError {
    StockError::Invalid {
        field: field.to_owned(),
        reason,
    }
}

fn add(totals: &mut BTreeMap<Key, Decimal>, key: Key, qty: Decimal) -> Result<(), StockError> {
    let slot = totals.entry(key).or_insert(Decimal::ZERO);
    *slot = slot
        .checked_add(qty)
        .ok_or(StockError::Money(MoneyError::Overflow))?;
    Ok(())
}

/// Checks that a godown and batch named by an opening line or a movement exist and fit the item.
fn check_location(books: &Books, key: Key, field: &str) -> Result<(), StockError> {
    let (item, godown, batch) = key;
    if !books.masters.godowns.contains_key(&godown) {
        return Err(invalid(
            field,
            format!("item {item:?} refers to godown {godown:?}, which does not exist"),
        ));
    }
    if let Some(batch_id) = batch {
        let Some(master) = books.masters.batches.get(&batch_id) else {
            return Err(invalid(
                field,
                format!("item {item:?} refers to batch {batch_id:?}, which does not exist"),
            ));
        };
        if master.item != item {
            return Err(invalid(
                field,
                format!(
                    "batch {batch_id:?} belongs to item {:?}, not to item {item:?}",
                    master.item
                ),
            ));
        }
    }
    Ok(())
}

fn add_openings(books: &Books, totals: &mut BTreeMap<Key, Decimal>) -> Result<(), StockError> {
    for item in books.masters.stock_items.values() {
        for o in &item.opening {
            if o.qty.0 < Decimal::ZERO {
                return Err(invalid(
                    "opening",
                    format!("item {:?} has a negative opening quantity", item.id),
                ));
            }
            let key = (item.id, o.godown, o.batch);
            check_location(books, key, "opening")?;
            add(totals, key, o.qty.0)?;
        }
    }
    Ok(())
}

fn add_moves(
    books: &Books,
    as_of: Date,
    totals: &mut BTreeMap<Key, Decimal>,
) -> Result<(), StockError> {
    for m in books.stock_moves.iter().filter(|m| m.date <= as_of) {
        if !books.masters.stock_items.contains_key(&m.item) {
            return Err(invalid(
                "moves",
                format!(
                    "voucher {:?} moves item {:?}, which does not exist",
                    m.voucher, m.item
                ),
            ));
        }
        if m.qty_in.0 < Decimal::ZERO || m.qty_out.0 < Decimal::ZERO {
            return Err(invalid(
                "moves",
                format!(
                    "voucher {:?} has a negative movement quantity for item {:?}",
                    m.voucher, m.item
                ),
            ));
        }
        let key = (m.item, m.godown, m.batch);
        check_location(books, key, "moves")?;
        let net = m
            .qty_in
            .0
            .checked_sub(m.qty_out.0)
            .ok_or(StockError::Money(MoneyError::Overflow))?;
        add(totals, key, net)?;
    }
    Ok(())
}

/// Quantity held per `(item, godown, batch)` on `as_of`, from opening stock and movements.
///
/// See the module documentation for the definition, ordering and what is left out.
///
/// # Errors
/// [`StockError::Invalid`] if `as_of` is before the books begin, or if an opening line or a
/// movement names an item, godown or batch that does not exist, a batch of another item, or a
/// negative quantity; [`StockError::Money`] on arithmetic overflow.
pub fn positions(books: &Books, as_of: Date) -> Result<Vec<Position>, StockError> {
    if as_of < books.company.books_begin {
        return Err(invalid(
            "as_of",
            format!(
                "{as_of} is before the books begin on {}; opening stock is not defined earlier",
                books.company.books_begin
            ),
        ));
    }
    let mut totals: BTreeMap<Key, Decimal> = BTreeMap::new();
    add_openings(books, &mut totals)?;
    add_moves(books, as_of, &mut totals)?;
    Ok(totals
        .into_iter()
        .filter(|(_, qty)| !qty.is_zero())
        .map(|((item, godown, batch), qty)| Position {
            item,
            godown,
            batch,
            qty: Qty(qty),
        })
        .collect())
}

/// Expiry date recorded on a batch, or `None` when the batch has no expiry.
///
/// # Errors
/// [`StockError::Invalid`] if the batch does not exist.
pub fn batch_expiry(books: &Books, batch: BatchId) -> Result<Option<Date>, StockError> {
    books
        .masters
        .batches
        .get(&batch)
        .map(|b| b.expiry)
        .ok_or_else(|| invalid("batch", format!("batch {batch:?} does not exist")))
}
