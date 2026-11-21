//! Parsing the values printed on a bill: amounts, rates and dates. Part of card A3 (`ocr.rs`).
//! Every parser rejects what it cannot read exactly; none rounds or guesses.

use std::str::FromStr;

use ledgerly_core::{Date, Decimal, Paise};
use time::Month;

/// Tokens that may precede an amount and carry no information. A lone `-` is not one: a minus
/// sign is never dropped, so a separate dash token reaches `parse_amount` and is rejected.
fn is_filler(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "@" | ":" | "rs" | "rs." | "inr" | "\u{20b9}"
    )
}

/// A rate token such as `9%`, `@9%` or `(2.5%)`.
fn parse_rate_token(token: &str) -> Option<Decimal> {
    let inner = token
        .trim_matches(|c: char| matches!(c, '@' | '(' | ')'))
        .strip_suffix('%')?;
    if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Decimal::from_str(inner).ok()
}

/// The amount at the end of a tail, and the rate before it when `allow_rate`. Anything else in
/// the tail rejects the line.
pub(super) fn parse_amount_tail(
    tail: &str,
    allow_rate: bool,
) -> Result<(Paise, Option<Decimal>), String> {
    let mut amount: Option<Paise> = None;
    let mut rate: Option<Decimal> = None;
    for token in tail.split_whitespace() {
        if is_filler(token) {
            continue;
        }
        if amount.is_some() {
            return Err(format!("unexpected text '{token}' after the amount"));
        }
        if allow_rate && let Some(r) = parse_rate_token(token) {
            if rate.replace(r).is_some() {
                return Err("two rates before the amount".to_owned());
            }
            continue;
        }
        amount = Some(parse_amount(token)?);
    }
    amount
        .map(|a| (a, rate))
        .ok_or_else(|| "no amount follows the label".to_owned())
}

/// Parses rupees with at most two decimals. No rounding: a third decimal is rejected.
fn parse_amount(raw: &str) -> Result<Paise, String> {
    let bad = || format!("'{raw}' is not an amount in rupees and paise");
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix('\u{20b9}') {
        s = rest;
    } else {
        let lower = s.to_ascii_lowercase();
        for prefix in ["rs.", "rs", "inr"] {
            if lower.starts_with(prefix) {
                s = s.get(prefix.len()..).unwrap_or(s);
                break;
            }
        }
    }
    let (int_part, frac) = match s.split_once('.') {
        Some((i, f)) if !f.is_empty() => (i, f),
        Some(_) => return Err(bad()),
        None => (s, ""),
    };
    let groups: Vec<&str> = int_part.split(',').collect();
    let digits_only = groups
        .iter()
        .all(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit()));
    if !digits_only
        || !grouping_is_valid(&groups)
        || frac.len() > 2
        || !frac.chars().all(|c| c.is_ascii_digit())
    {
        return Err(bad());
    }
    let rupees: i64 = groups.concat().parse().map_err(|_| bad())?;
    // One decimal digit is tenths of a rupee: ".5" is 50 paise.
    let paise_part: i64 = match frac.len() {
        0 => 0,
        1 => frac
            .parse::<i64>()
            .map_err(|_| bad())?
            .checked_mul(10)
            .ok_or_else(bad)?,
        _ => frac.parse().map_err(|_| bad())?,
    };
    rupees
        .checked_mul(100)
        .and_then(|p| p.checked_add(paise_part))
        .map(Paise::new)
        .ok_or_else(|| format!("'{raw}' is too large"))
}

/// Commas only as Indian (`12,34,567`) or Western (`1,234,567`) thousands separators: a first
/// group of 1 to 3 digits, middle groups of 2 or 3, a last group of exactly 3.
fn grouping_is_valid(groups: &[&str]) -> bool {
    let Some((last, rest)) = groups.split_last() else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    last.len() == 3
        && rest.iter().enumerate().all(|(i, g)| {
            if i == 0 {
                (1..=3).contains(&g.len())
            } else {
                g.len() == 2 || g.len() == 3
            }
        })
}

fn month_from_name(name: &str) -> Option<Month> {
    match name.to_ascii_lowercase().as_str() {
        "jan" | "january" => Some(Month::January),
        "feb" | "february" => Some(Month::February),
        "mar" | "march" => Some(Month::March),
        "apr" | "april" => Some(Month::April),
        "may" => Some(Month::May),
        "jun" | "june" => Some(Month::June),
        "jul" | "july" => Some(Month::July),
        "aug" | "august" => Some(Month::August),
        "sep" | "sept" | "september" => Some(Month::September),
        "oct" | "october" => Some(Month::October),
        "nov" | "november" => Some(Month::November),
        "dec" | "december" => Some(Month::December),
        _ => None,
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Parses a day-first date: day, month (number or name) and a four-digit year.
pub(super) fn parse_date(text: &str) -> Result<Date, String> {
    let bad = || format!("'{text}' is not a day-first date such as 15/03/2026");
    let parts: Vec<&str> = text
        .split(|c: char| matches!(c, '/' | '-' | '.') || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    let [day, month, year] = parts.as_slice() else {
        return Err(bad());
    };
    if !all_digits(day) || day.len() > 2 || !all_digits(year) || year.len() != 4 {
        return Err(bad());
    }
    let month = if all_digits(month) && month.len() <= 2 {
        let number: u8 = month.parse().map_err(|_| bad())?;
        Month::try_from(number).map_err(|e| format!("'{text}': {e}"))?
    } else {
        month_from_name(month).ok_or_else(bad)?
    };
    let day: u8 = day.parse().map_err(|_| bad())?;
    let year: i32 = year.parse().map_err(|_| bad())?;
    Date::from_calendar_date(year, month, day).map_err(|e| format!("'{text}': {e}"))
}
