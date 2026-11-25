//! The sentence that says why a stock figure is provisional.
//!
//! Every provisional line (stock summary, balance sheet, profit and loss) carries the same
//! sentence. It is built from the books: the item's name, the unit's symbol, the numbers of the
//! vouchers and the quantities the stock engine's running balance gives
//! ([`ledgerly_stock::negative::negative_episode`]). Nothing here is computed by a screen.

use ledgerly_core::{Date, Decimal, Month, StockItemId, VoucherId};
use ledgerly_model::{Books, StockItem};
use ledgerly_stock::negative::negative_episode;

/// A date as the app writes it: `19 Nov 2026`.
pub(crate) fn short_date(d: Date) -> String {
    format!("{} {} {}", d.day(), month_abbrev(d.month()), d.year())
}

fn month_abbrev(m: Month) -> &'static str {
    match m {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

/// A quantity without a sign or trailing zeros: `-30.50` is `30.5`.
fn plain_qty(q: Decimal) -> String {
    q.abs().normalize().to_string()
}

/// The voucher's number, when it has one.
fn number_of(books: &Books, v: VoucherId) -> Option<&str> {
    books.vouchers.get(&v).and_then(|v| v.number.as_deref())
}

/// "a purchase" or "an advance receipt": the voucher's type name in lower case.
fn kind_of(books: &Books, v: VoucherId) -> String {
    let name = books
        .vouchers
        .get(&v)
        .and_then(|v| books.masters.voucher_types.get(&v.type_id))
        .map_or_else(|| "voucher".to_owned(), |t| t.name.to_lowercase());
    let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}

/// The reason line for a stock item that went below zero on `date`, as seen from `as_of`:
/// `Steel pickaxe 2.5 kg first went below zero on 19 Nov 2026 (−30 nos after SI/0416); a purchase
/// on 20 Nov brought it back to 70`.
///
/// The item is named by its name (its number when the masters do not hold it). The bracket and
/// the last clause are left out when the books hold no movement to name, so the sentence never
/// says more than the engine knows. The recovery's year is written only when it differs from the
/// first date's.
pub(crate) fn negative_stock_reason(
    books: &Books,
    item: StockItemId,
    date: Date,
    as_of: Date,
) -> String {
    let master: Option<&StockItem> = books.masters.stock_items.get(&item);
    let who = master.map_or_else(|| format!("item {}", item.0), |i| i.name.clone());
    let mut text = format!("{who} first went below zero on {}", short_date(date));
    let episode = master.and_then(|i| {
        negative_episode(i, &books.stock_moves, as_of)
            .ok()
            .flatten()
    });
    let Some(e) = episode.filter(|e| e.date == date) else {
        return text;
    };
    let unit = master
        .and_then(|i| books.masters.units.get(&i.unit))
        .map(|u| u.symbol.as_str());
    let mut inside = format!("\u{2212}{}", plain_qty(e.qty_after));
    if let Some(u) = unit {
        inside.push(' ');
        inside.push_str(u);
    }
    if let Some(n) = number_of(books, e.voucher) {
        inside.push_str(" after ");
        inside.push_str(n);
    }
    text.push_str(" (");
    text.push_str(&inside);
    text.push(')');
    if let Some(r) = e.recovery {
        let when = if r.date.year() == date.year() {
            format!("{} {}", r.date.day(), month_abbrev(r.date.month()))
        } else {
            short_date(r.date)
        };
        text.push_str("; ");
        text.push_str(&kind_of(books, r.voucher));
        text.push_str(" on ");
        text.push_str(&when);
        text.push_str(" brought it back to ");
        text.push_str(&plain_qty(r.qty_after));
    }
    text
}
