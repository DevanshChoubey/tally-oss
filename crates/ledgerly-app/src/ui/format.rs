//! Number and date text for the screens: Indian digit grouping for amounts and the one date
//! format of the mock (`21 Nov 2026`).
//!
//! The grouping logic is copied from `screens/voucher_entry.rs` (`group_indian` and
//! `format_amount`). Those private copies stay where they are for now. `ledgerly_core::format`
//! also has `format_inr`; once the screens move to the kit, the two should be merged.
//!
//! These functions only write a value that the engine has already computed. They never round,
//! convert or add.

use ledgerly_core::{Date, Month, Paise};
use time::Weekday;

use crate::i18n::Tr;

/// An amount in paise as rupees and paise with Indian grouping: `1,00,000.00`, `12,34,567.89`,
/// `-50.25`, `0.00`. The sign is a leading `-` for negative amounts.
#[must_use]
pub fn amount(p: Paise) -> String {
    let abs = p.get().unsigned_abs();
    let sign = if p.is_negative() { "-" } else { "" };
    format!("{sign}{}.{:02}", group_indian(abs / 100), abs % 100)
}

/// Digits of `rupees` grouped the Indian way: the last three digits, then pairs of two.
/// `1234567` becomes `12,34,567`.
fn group_indian(rupees: u64) -> String {
    let mut out: Vec<char> = Vec::new();
    for (i, digit) in rupees.to_string().chars().rev().enumerate() {
        if i == 3 || (i > 3 && i.saturating_sub(3) % 2 == 0) {
            out.push(',');
        }
        out.push(digit);
    }
    out.iter().rev().collect()
}

/// A date as the mock shows it: day without a leading zero, the month abbreviated, the year.
/// `2026-11-21` becomes `21 Nov 2026`, and `2026-11-05` becomes `5 Nov 2026`.
#[must_use]
pub fn date(d: Date) -> String {
    format!("{} {} {}", d.day(), month_abbrev(d.month()), d.year())
}

/// `d` with its weekday in full, as the mock's Home line writes it: `Saturday 21 Nov 2026`.
#[must_use]
pub fn long_date(d: Date) -> String {
    let day = match d.weekday() {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    };
    format!("{day} {}", date(d))
}

/// `d` in the words of `tr`: the day, the month's word from the translation file, the year.
/// The digits stay Latin.
#[must_use]
pub fn date_in(d: Date, tr: &Tr) -> String {
    format!("{} {} {}", d.day(), tr.t(month_key(d.month())), d.year())
}

/// `d` with its weekday in the words of `tr`, as [`long_date`] writes it in English.
#[must_use]
pub fn long_date_in(d: Date, tr: &Tr) -> String {
    format!("{} {}", tr.t(weekday_key(d.weekday())), date_in(d, tr))
}

/// The translation key of the weekday `w`.
fn weekday_key(w: Weekday) -> &'static str {
    match w {
        Weekday::Monday => "format.weekday.monday",
        Weekday::Tuesday => "format.weekday.tuesday",
        Weekday::Wednesday => "format.weekday.wednesday",
        Weekday::Thursday => "format.weekday.thursday",
        Weekday::Friday => "format.weekday.friday",
        Weekday::Saturday => "format.weekday.saturday",
        Weekday::Sunday => "format.weekday.sunday",
    }
}

/// The translation key of the month `m`.
fn month_key(m: Month) -> &'static str {
    match m {
        Month::January => "format.month.jan",
        Month::February => "format.month.feb",
        Month::March => "format.month.mar",
        Month::April => "format.month.apr",
        Month::May => "format.month.may",
        Month::June => "format.month.jun",
        Month::July => "format.month.jul",
        Month::August => "format.month.aug",
        Month::September => "format.month.sep",
        Month::October => "format.month.oct",
        Month::November => "format.month.nov",
        Month::December => "format.month.dec",
    }
}

/// The three-letter English abbreviation of `m`.
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
