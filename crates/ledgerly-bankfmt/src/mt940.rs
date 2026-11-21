//! MT940 statements. **Card BF4** implements this module.
//!
//! Frozen interface (plan card BF4):
//!
//! ```text
//! pub fn parse_mt940(text: &str) -> Result<RawStatement, BankFmtError>;
//! ```
//!
//! Format notes (SWIFT customer statement message, one statement per file):
//!
//! - An optional SWIFT envelope (`{1:..}{2:..}{4:` first line, `-}` last line) is accepted.
//! - `:25:` becomes `account_hint`; `:60F:` the opening and `:62F:` the closing balance, both with
//!   credit positive (`C` is `+`, `D` is `-`). Only currency `INR` is accepted.
//! - Each `:61:` is one [`RawLine`]. `date` is the entry date (`MMDD`, its year taken as the one
//!   nearest the value date) when the field has one, else the value date. `value_date` is always
//!   the value date. Years are `2000 + YY` and must fall in 2000 to 2050; any other year is an
//!   error. `D` and `RC` (reversal of a credit, a debit entry)
//!   are money out; `C` and `RD` (reversal of a debit, a credit entry) are money in. The
//!   reference is the customer reference, or the bank reference after `//` when the customer
//!   reference is empty or `NONREF`.
//! - The `:86:` that follows a `:61:` gives the description (lines joined with a space); without
//!   one the `:61:` supplementary details are used. `RawLine::raw` holds the field text of the
//!   `:61:` and `:86:` as in the file. `RawLine::row` is the 1-based line number of the `:61:`.
//! - MT940 carries no running balance, so `RawLine::balance` is `None`.
//! - Strict: unknown tags, intermediate pages (`:60M:`/`:62M:`), several statements in one file,
//!   a statement-level `:86:`, and amounts with more than two decimals are errors, never skipped.
//!   Informational tags `:20: :21: :28: :28C: :64: :65:` are accepted and not kept.
//!
//! The parser does not check balances; call [`verify_balances`] (spec 6.5: an import is rejected
//! unless opening + sum of lines = closing).

use crate::{BankFmtError, ParsedStatement, RawLine, RawStatement};
use ledgerly_core::{Date, Month, Paise};

/// Registry id stored in `RawStatement::format_id`: the id of `formats/generic-mt940.toml`.
const FORMAT_ID: &str = "generic-mt940";

/// The years a two-digit `YY` may stand for. `YY` is read as `2000 + YY`, so a year outside this
/// window (for example `99`, which would otherwise read as 2099 for a 1999 statement) is an error.
/// The window is a design choice awaiting owner confirmation.
const YEAR_WINDOW: std::ops::RangeInclusive<i32> = 2000..=2050;

/// One tagged field of the message.
struct Field {
    /// 1-based line number of the tag line.
    line: u32,
    /// The tag without colons, for example `61`.
    tag: String,
    /// The value on the tag line, then the continuation lines.
    lines: Vec<String>,
}

#[derive(Clone, Copy)]
enum Side {
    Debit,
    Credit,
}

fn format_err(reason: impl Into<String>) -> BankFmtError {
    BankFmtError::Format {
        reason: reason.into(),
    }
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX)
}

/// Statement under construction while the fields are read in order.
#[derive(Default)]
struct Builder {
    account: Option<String>,
    opening: Option<Paise>,
    closing: Option<Paise>,
    /// The `:62F:` date, the statement's stated end date.
    end_date: Option<Date>,
    seen_reference: bool,
    lines: Vec<RawLine>,
    /// The `:61:` line still waiting for its optional `:86:`.
    open_line: Option<RawLine>,
    /// Whether `open_line` already received its `:86:`.
    info_seen: bool,
}

impl Builder {
    fn flush(&mut self) {
        if let Some(done) = self.open_line.take() {
            self.lines.push(done);
        }
        self.info_seen = false;
    }

    fn accept(&mut self, field: &Field) -> Result<(), BankFmtError> {
        match field.tag.as_str() {
            "20" => {
                if self.seen_reference {
                    return Err(format_err(
                        "more than one statement (:20:) in the file is not supported",
                    ));
                }
                self.seen_reference = true;
            }
            "25" => {
                if self.account.is_some() {
                    return Err(format_err(format!("line {}: second :25:", field.line)));
                }
                self.account = Some(first_value(field).trim().to_owned());
            }
            "60F" => {
                if self.opening.is_some() {
                    return Err(format_err(format!("line {}: second :60F:", field.line)));
                }
                self.opening = Some(parse_balance(field)?.0);
            }
            "61" => self.accept_61(field)?,
            "86" => self.accept_86(field)?,
            "62F" => {
                if self.closing.is_some() {
                    return Err(format_err(format!("line {}: second :62F:", field.line)));
                }
                self.flush();
                let (closing, end) = parse_balance(field)?;
                self.closing = Some(closing);
                self.end_date = Some(end);
            }
            "21" | "28" | "28C" | "64" | "65" => {}
            other => {
                return Err(format_err(format!(
                    "line {}: tag :{other}: is not supported",
                    field.line
                )));
            }
        }
        Ok(())
    }

    fn accept_61(&mut self, field: &Field) -> Result<(), BankFmtError> {
        if self.opening.is_none() || self.closing.is_some() {
            return Err(format_err(format!(
                "line {}: :61: outside the :60F: .. :62F: range",
                field.line
            )));
        }
        self.flush();
        let line = parse_61(field).map_err(|reason| BankFmtError::Row {
            row: field.line,
            reason,
        })?;
        self.open_line = Some(line);
        Ok(())
    }

    fn accept_86(&mut self, field: &Field) -> Result<(), BankFmtError> {
        let Some(line) = self.open_line.as_mut() else {
            return Err(format_err(format!(
                "line {}: :86: without a :61: (statement-level :86: is not supported)",
                field.line
            )));
        };
        if self.info_seen {
            return Err(format_err(format!(
                "line {}: second :86: for one :61:",
                field.line
            )));
        }
        line.description = join_trimmed(&field.lines);
        line.raw.push(format!(":86:{}", field.lines.join("\n")));
        self.info_seen = true;
        Ok(())
    }

    fn finish(mut self) -> Result<ParsedStatement, BankFmtError> {
        self.flush();
        let opening = self
            .opening
            .ok_or_else(|| format_err("missing opening balance :60F:"))?;
        let closing = self
            .closing
            .ok_or_else(|| format_err("missing closing balance :62F:"))?;
        Ok(ParsedStatement {
            raw: RawStatement {
                format_id: FORMAT_ID.to_owned(),
                account_hint: self.account,
                opening: Some(opening),
                closing: Some(closing),
                lines: self.lines,
            },
            end_date: self.end_date,
        })
    }
}

/// Parses an MT940 statement.
///
/// # Errors
///
/// [`BankFmtError::Row`] for a bad `:61:` (with its line number) and [`BankFmtError::Format`] for
/// anything else that is malformed or not supported (see the module docs).
pub fn parse_mt940(text: &str) -> Result<RawStatement, BankFmtError> {
    parse_mt940_with_end(text).map(|parsed| parsed.raw)
}

/// Parses an MT940 statement and keeps the `:62F:` date as the statement's stated end date
/// (card C3). [`parse_mt940`] is this without the end date.
///
/// # Errors
///
/// Exactly those of [`parse_mt940`].
pub fn parse_mt940_with_end(text: &str) -> Result<ParsedStatement, BankFmtError> {
    let mut builder = Builder::default();
    for field in &tokenise(text)? {
        builder.accept(field)?;
    }
    builder.finish()
}

/// Checks that opening + credits - debits of all lines equals the closing balance, and that every
/// running balance stated on a line agrees on the way.
///
/// # Errors
///
/// [`BankFmtError::Format`] when a balance is missing or the totals disagree, and
/// [`BankFmtError::Row`] when a stated running balance is wrong.
pub fn verify_balances(statement: &RawStatement) -> Result<(), BankFmtError> {
    let (Some(opening), Some(closing)) = (statement.opening, statement.closing) else {
        return Err(format_err("opening or closing balance is missing"));
    };
    let mut running = opening;
    for line in &statement.lines {
        running = running
            .checked_add(line.credit)
            .and_then(|r| r.checked_sub(line.debit))
            .map_err(|e| BankFmtError::Row {
                row: line.row,
                reason: e.to_string(),
            })?;
        match line.balance {
            Some(stated) if stated != running => {
                return Err(BankFmtError::Row {
                    row: line.row,
                    reason: format!(
                        "stated balance {} paise but the lines so far give {} paise",
                        stated.get(),
                        running.get()
                    ),
                });
            }
            Some(_) | None => {}
        }
    }
    if running == closing {
        Ok(())
    } else {
        Err(format_err(format!(
            "opening plus lines is {} paise but the closing balance is {} paise",
            running.get(),
            closing.get()
        )))
    }
}

fn first_value(field: &Field) -> &str {
    field.lines.first().map_or("", String::as_str)
}

fn join_trimmed(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Recognises `:TAG:rest`; tags are 2-3 upper-case alphanumerics starting with a digit, or `NS`.
fn tag_line(line: &str) -> Option<(&str, &str)> {
    let body = line.strip_prefix(':')?;
    let (tag, rest) = body.split_once(':')?;
    let shape_ok = (2..=3).contains(&tag.len())
        && tag
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase());
    let start_ok = tag.starts_with(|c: char| c.is_ascii_digit()) || tag == "NS";
    (shape_ok && start_ok).then_some((tag, rest))
}

fn tokenise(text: &str) -> Result<Vec<Field>, BankFmtError> {
    let mut fields: Vec<Field> = Vec::new();
    let mut ended = false;
    for (index, raw_line) in text.lines().enumerate() {
        let line_no = line_number(index);
        let mut line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if ended {
            return Err(format_err(format!(
                "line {line_no}: text after the end of the statement; several statements in one file are not supported"
            )));
        }
        if fields.is_empty() && line.starts_with('{') {
            match line.split_once("{4:") {
                Some((_, rest)) => line = rest.trim(),
                None => continue,
            }
            if line.is_empty() {
                continue;
            }
        }
        if line == "-}" || line == "-" {
            ended = true;
            continue;
        }
        if let Some((tag, rest)) = tag_line(line) {
            fields.push(Field {
                line: line_no,
                tag: tag.to_owned(),
                lines: vec![rest.to_owned()],
            });
        } else if let Some(last) = fields.last_mut() {
            last.lines.push(line.to_owned());
        } else {
            return Err(format_err(format!(
                "line {line_no}: text before the first tag"
            )));
        }
    }
    Ok(fields)
}

/// Splits `n` leading ASCII digits off `s`.
fn take_digits(s: &str, n: usize) -> Option<(&str, &str)> {
    let (head, tail) = s.split_at_checked(n)?;
    head.bytes()
        .all(|b| b.is_ascii_digit())
        .then_some((head, tail))
}

fn two_digits(s: &str) -> Option<u8> {
    s.parse::<u8>().ok()
}

fn parse_yymmdd(text: &str) -> Result<Date, String> {
    let bad = || format!("'{text}' is not a valid YYMMDD date");
    let (yy, rest) = text.split_at_checked(2).ok_or_else(bad)?;
    let (mm, dd) = rest.split_at_checked(2).ok_or_else(bad)?;
    let year = 2000_i32.saturating_add(i32::from(two_digits(yy).ok_or_else(bad)?));
    if !YEAR_WINDOW.contains(&year) {
        return Err(format!(
            "'{text}' has year {year}, outside {} to {}; two-digit years are read as 2000 to 2050 only",
            YEAR_WINDOW.start(),
            YEAR_WINDOW.end()
        ));
    }
    let month = Month::try_from(two_digits(mm).ok_or_else(bad)?).map_err(|_| bad())?;
    Date::from_calendar_date(year, month, two_digits(dd).ok_or_else(bad)?).map_err(|_| bad())
}

/// Year of an `MMDD` entry date: the candidate year (value year -1, 0, +1) nearest the value date.
fn resolve_entry_date(value: Date, mmdd: &str) -> Result<Date, String> {
    let bad = || format!("'{mmdd}' is not a valid MMDD entry date");
    let (mm, dd) = mmdd.split_at_checked(2).ok_or_else(bad)?;
    let month = Month::try_from(two_digits(mm).ok_or_else(bad)?).map_err(|_| bad())?;
    let day = two_digits(dd).ok_or_else(bad)?;
    (value.year().saturating_sub(1)..=value.year().saturating_add(1))
        .filter_map(|year| Date::from_calendar_date(year, month, day).ok())
        .min_by_key(|candidate| {
            i64::from(candidate.to_julian_day())
                .saturating_sub(i64::from(value.to_julian_day()))
                .abs()
        })
        .ok_or_else(bad)
}

/// SWIFT amount: digits, a decimal comma, at most two decimals, at most 15 characters.
fn parse_swift_amount(text: &str) -> Result<Paise, String> {
    let bad =
        || format!("'{text}' is not a valid amount (digits, a decimal comma, at most 2 decimals)");
    if text.len() > 15 {
        return Err(bad());
    }
    let (int_part, frac) = text.split_once(',').ok_or_else(bad)?;
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if int_part.is_empty() || !digits(int_part) || frac.len() > 2 || !digits(frac) {
        return Err(bad());
    }
    let rupees: i64 = int_part.parse().map_err(|_| bad())?;
    let frac_paise: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().map_err(|_| bad())?.saturating_mul(10),
        _ => frac.parse::<i64>().map_err(|_| bad())?,
    };
    rupees
        .checked_mul(100)
        .and_then(|p| p.checked_add(frac_paise))
        .map(Paise::new)
        .ok_or_else(|| format!("amount '{text}' is too large"))
}

/// `:60F:` / `:62F:` value: `C|D`, `YYMMDD`, currency, amount. Returns the signed amount and the
/// date.
fn parse_balance(field: &Field) -> Result<(Paise, Date), BankFmtError> {
    let wrap =
        |reason: String| format_err(format!("line {}: :{}: {reason}", field.line, field.tag));
    let value = first_value(field).trim();
    let (mark, rest) = value
        .split_at_checked(1)
        .ok_or_else(|| wrap("empty balance".to_owned()))?;
    let (date, rest) =
        take_digits(rest, 6).ok_or_else(|| wrap("date must be 6 digits YYMMDD".to_owned()))?;
    let balance_date = parse_yymmdd(date).map_err(wrap)?;
    let (currency, amount_text) = rest
        .split_at_checked(3)
        .ok_or_else(|| wrap("missing currency".to_owned()))?;
    if currency != "INR" {
        return Err(wrap(format!(
            "currency '{currency}' is not supported, only INR"
        )));
    }
    let amount = parse_swift_amount(amount_text).map_err(wrap)?;
    match mark {
        "C" => Ok((amount, balance_date)),
        "D" => amount
            .checked_neg()
            .map(|negated| (negated, balance_date))
            .map_err(|e| wrap(e.to_string())),
        other => Err(wrap(format!("mark '{other}' is not C or D"))),
    }
}

/// Parses the value of one `:61:` field (first line plus supplementary details).
fn parse_61(field: &Field) -> Result<RawLine, String> {
    let first = first_value(field);
    let (value_txt, rest) =
        take_digits(first, 6).ok_or("value date must be 6 digits YYMMDD".to_owned())?;
    let value_date = parse_yymmdd(value_txt)?;
    let (entry_date, rest) = match take_digits(rest, 4) {
        Some((mmdd, after)) => (Some(resolve_entry_date(value_date, mmdd)?), after),
        None => (None, rest),
    };
    let (side, rest) = if let Some(after) = rest.strip_prefix("RD") {
        (Side::Credit, after)
    } else if let Some(after) = rest.strip_prefix("RC") {
        (Side::Debit, after)
    } else if let Some(after) = rest.strip_prefix('D') {
        (Side::Debit, after)
    } else if let Some(after) = rest.strip_prefix('C') {
        (Side::Credit, after)
    } else {
        return Err("debit/credit mark must be D, C, RD or RC".to_owned());
    };
    // Optional funds code: one letter before the amount.
    let rest = match rest.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => rest.get(c.len_utf8()..).unwrap_or(rest),
        Some(_) | None => rest,
    };
    let amount_end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == ','))
        .unwrap_or(rest.len());
    let (amount_txt, rest) = rest
        .split_at_checked(amount_end)
        .ok_or("amount is not followed by a transaction type".to_owned())?;
    let amount = parse_swift_amount(amount_txt)?;
    let (ttype, rest) = rest
        .split_at_checked(4)
        .ok_or("missing transaction type (N, S or F plus 3 characters)".to_owned())?;
    let ttype_ok = ttype.is_ascii()
        && ttype.starts_with(['N', 'S', 'F'])
        && ttype.bytes().skip(1).all(|b| b.is_ascii_alphanumeric());
    if !ttype_ok {
        return Err(format!(
            "'{ttype}' is not a transaction type (N, S or F plus 3 characters)"
        ));
    }
    let (customer, bank) = match rest.split_once("//") {
        Some((c, b)) => (c.trim(), b.trim()),
        None => (rest.trim(), ""),
    };
    let reference = if !customer.is_empty() && customer != "NONREF" {
        Some(customer.to_owned())
    } else if !bank.is_empty() {
        Some(bank.to_owned())
    } else {
        None
    };
    let supplementary = join_trimmed(field.lines.get(1..).unwrap_or(&[]));
    let (debit, credit) = match side {
        Side::Debit => (amount, Paise::ZERO),
        Side::Credit => (Paise::ZERO, amount),
    };
    Ok(RawLine {
        row: field.line,
        date: entry_date.unwrap_or(value_date),
        value_date: Some(value_date),
        description: supplementary,
        reference,
        debit,
        credit,
        balance: None,
        raw: vec![format!(":61:{}", field.lines.join("\n"))],
    })
}
