//! CSV statements. **Card BF2** implements this module.
//!
//! Frozen interface (plan card BF2):
//!
//! ```text
//! pub fn parse_csv(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError>;
//! ```
//!
//! `FormatSpec` is defined by card BF1 in `registry.rs`, which had not landed when BF2 was built,
//! so `parse_csv` itself is not provided yet (see `docs/interface-requests.md`, BF2). The parser
//! core is [`parse_csv_with_layout`], driven by the [`CsvLayout`] defined here; once `FormatSpec`
//! exists, `parse_csv` is a conversion to [`CsvLayout`] plus this call.
//!
//! Behaviour, all of it strict (no guessing, nothing rounded):
//!
//! - The first `skip_rows` records (letterhead, header) are skipped without being decoded.
//! - Rows are numbered by 1-based record position in the file, skipped records included.
//! - Records whose cells are all blank are ignored.
//! - Amounts accept plain, western (`123,456.78`) and Indian (`1,23,456.78`) grouping, at most two
//!   decimals, an optional leading `-`. Anything else is a row error.
//! - Exactly one of debit and credit is non-zero on every line.
//! - The opening and closing balance come from the balance column, when the first and last line
//!   state one (see `balances_from_column`); otherwise they stay unset.

use crate::{BankFmtError, RawLine, RawStatement};
use ledgerly_core::{Date, Month, Paise};

/// How the amount of a line is laid out in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmountColumns {
    /// Separate withdrawal and deposit columns; exactly one is filled.
    Split {
        /// Column index of the withdrawal (debit) amount.
        debit: usize,
        /// Column index of the deposit (credit) amount.
        credit: usize,
    },
    /// One signed column: negative is money out (debit), positive is money in (credit).
    Signed {
        /// Column index of the signed amount.
        amount: usize,
    },
    /// An unsigned amount column plus a column holding `Dr` or `Cr` (case-insensitive).
    WithIndicator {
        /// Column index of the amount.
        amount: usize,
        /// Column index of the `Dr`/`Cr` marker.
        indicator: usize,
    },
}

/// Date text layout of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// `dd<sep>mm<sep>yyyy`.
    DdMmYyyy,
    /// `dd<sep>mm<sep>yy`; the year is `century_base + yy` (for example base 2000).
    DdMmYy {
        /// Added to the two-digit year.
        century_base: i32,
    },
    /// `yyyy<sep>mm<sep>dd`.
    YyyyMmDd,
    /// `dd<sep>Mon<sep>yyyy` with an English three-letter month (`Apr`), case-insensitive.
    DdMonYyyy,
}

/// Everything the CSV parser needs to know about one bank's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvLayout {
    /// Format registry id, copied to `RawStatement::format_id`.
    pub format_id: String,
    /// Field delimiter byte.
    pub delimiter: u8,
    /// Records to skip before the first transaction (letterhead and header rows).
    pub skip_rows: u32,
    /// Column index of the transaction date.
    pub date_col: usize,
    /// Column index of the value date, if the file has one.
    pub value_date_col: Option<usize>,
    /// Column index of the description.
    pub description_col: usize,
    /// Column index of the cheque or reference number, if any.
    pub reference_col: Option<usize>,
    /// Amount layout.
    pub amounts: AmountColumns,
    /// Column index of the running balance, if any (credit positive).
    pub balance_col: Option<usize>,
    /// Date layout, shared by the date and value date columns.
    pub date_format: DateFormat,
    /// Separator between the date parts.
    pub date_separator: char,
}

const MONTHS: [(&str, Month); 12] = [
    ("jan", Month::January),
    ("feb", Month::February),
    ("mar", Month::March),
    ("apr", Month::April),
    ("may", Month::May),
    ("jun", Month::June),
    ("jul", Month::July),
    ("aug", Month::August),
    ("sep", Month::September),
    ("oct", Month::October),
    ("nov", Month::November),
    ("dec", Month::December),
];

/// Parse a CSV bank statement using an explicit layout.
///
/// # Errors
/// [`BankFmtError::Row`] naming the record number for any cell that cannot be read exactly;
/// [`BankFmtError::Format`] when the file is not readable as CSV.
pub fn parse_csv_with_layout(
    bytes: &[u8],
    layout: &CsvLayout,
) -> Result<RawStatement, BankFmtError> {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(layout.delimiter)
        .from_reader(body);
    let mut lines = Vec::new();
    for (idx, record) in reader.byte_records().enumerate() {
        let row = u32::try_from(idx)
            .ok()
            .and_then(|i| i.checked_add(1))
            .ok_or_else(|| BankFmtError::Format {
                reason: "more than u32::MAX records".to_owned(),
            })?;
        let record = record.map_err(|e| BankFmtError::Format {
            reason: format!("record {row}: {e}"),
        })?;
        if row <= layout.skip_rows {
            continue;
        }
        let mut cells = Vec::with_capacity(record.len());
        for cell in &record {
            let text =
                std::str::from_utf8(cell).map_err(|_| row_err(row, "cell is not valid UTF-8"))?;
            cells.push(text.to_owned());
        }
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        lines.push(parse_line(row, cells, layout)?);
    }
    let (opening, closing) = balances_from_column(&lines)?;
    Ok(RawStatement {
        format_id: layout.format_id.clone(),
        account_hint: None,
        opening,
        closing,
        lines,
    })
}

/// Opening and closing balance read from the balance column (card C3).
///
/// The closing balance is the running balance on the last line. The opening balance is the
/// running balance on the first line taken back by that line's own amounts:
/// `balance - credit + debit`. Both are `None` unless the first and the last line state a
/// balance, so a file without a balance column is still refused by the statement check. The
/// running balances in between are checked later against these two, so a file whose lines are
/// out of order or incomplete is rejected, never repaired.
///
/// # Errors
/// [`BankFmtError::Row`] when the arithmetic overflows.
fn balances_from_column(lines: &[RawLine]) -> Result<(Option<Paise>, Option<Paise>), BankFmtError> {
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return Ok((None, None));
    };
    let (Some(first_balance), Some(closing)) = (first.balance, last.balance) else {
        return Ok((None, None));
    };
    let opening = first_balance
        .checked_sub(first.credit)
        .and_then(|b| b.checked_add(first.debit))
        .map_err(|e| row_err(first.row, e.to_string()))?;
    Ok((Some(opening), Some(closing)))
}

fn row_err(row: u32, reason: impl Into<String>) -> BankFmtError {
    BankFmtError::Row {
        row,
        reason: reason.into(),
    }
}

fn cell(cells: &[String], col: usize, row: u32, what: &str) -> Result<String, BankFmtError> {
    cells.get(col).map(|c| c.trim().to_owned()).ok_or_else(|| {
        row_err(
            row,
            format!(
                "{what}: column {col} is missing (row has {} cells)",
                cells.len()
            ),
        )
    })
}

fn parse_line(row: u32, cells: Vec<String>, layout: &CsvLayout) -> Result<RawLine, BankFmtError> {
    let date_text = cell(&cells, layout.date_col, row, "date")?;
    let date = parse_date(&date_text, layout)
        .map_err(|r| row_err(row, format!("date '{date_text}': {r}")))?;
    let value_date = match layout.value_date_col {
        None => None,
        Some(col) => {
            let text = cell(&cells, col, row, "value date")?;
            if text.is_empty() {
                None
            } else {
                Some(
                    parse_date(&text, layout)
                        .map_err(|r| row_err(row, format!("value date '{text}': {r}")))?,
                )
            }
        }
    };
    let description = cell(&cells, layout.description_col, row, "description")?;
    let reference = match layout.reference_col {
        None => None,
        Some(col) => Some(cell(&cells, col, row, "reference")?).filter(|t| !t.is_empty()),
    };
    let (debit, credit) = parse_amounts(row, &cells, &layout.amounts)?;
    let balance = match layout.balance_col {
        None => None,
        Some(col) => {
            let text = cell(&cells, col, row, "balance")?;
            if text.is_empty() {
                None
            } else {
                Some(
                    parse_amount(&text)
                        .map_err(|r| row_err(row, format!("balance '{text}': {r}")))?,
                )
            }
        }
    };
    Ok(RawLine {
        row,
        date,
        value_date,
        description,
        reference,
        debit,
        credit,
        balance,
        raw: cells,
    })
}

fn amount_cell(
    row: u32,
    cells: &[String],
    col: usize,
    what: &str,
) -> Result<Option<Paise>, BankFmtError> {
    let text = cell(cells, col, row, what)?;
    if text.is_empty() {
        return Ok(None);
    }
    parse_amount(&text)
        .map(Some)
        .map_err(|r| row_err(row, format!("{what} '{text}': {r}")))
}

fn parse_amounts(
    row: u32,
    cells: &[String],
    amounts: &AmountColumns,
) -> Result<(Paise, Paise), BankFmtError> {
    match amounts {
        AmountColumns::Split { debit, credit } => {
            let dr = amount_cell(row, cells, *debit, "debit")?;
            let cr = amount_cell(row, cells, *credit, "credit")?;
            match (dr, cr) {
                (Some(d), None) if !d.is_negative() => Ok((d, Paise::ZERO)),
                (None, Some(c)) if !c.is_negative() => Ok((Paise::ZERO, c)),
                (Some(_), Some(_)) => Err(row_err(row, "both debit and credit are filled")),
                (None, None) => Err(row_err(row, "neither debit nor credit is filled")),
                (Some(_), None) | (None, Some(_)) => Err(row_err(
                    row,
                    "negative amount in a split debit/credit column",
                )),
            }
        }
        AmountColumns::Signed { amount } => {
            let value = amount_cell(row, cells, *amount, "amount")?
                .ok_or_else(|| row_err(row, "amount is empty"))?;
            if value.is_negative() {
                let magnitude = value
                    .checked_neg()
                    .map_err(|e| row_err(row, e.to_string()))?;
                Ok((magnitude, Paise::ZERO))
            } else {
                Ok((Paise::ZERO, value))
            }
        }
        AmountColumns::WithIndicator { amount, indicator } => {
            let value = amount_cell(row, cells, *amount, "amount")?
                .ok_or_else(|| row_err(row, "amount is empty"))?;
            if value.is_negative() {
                return Err(row_err(row, "negative amount with a Dr/Cr indicator"));
            }
            let marker = cell(cells, *indicator, row, "Dr/Cr indicator")?;
            if marker.eq_ignore_ascii_case("dr") {
                Ok((value, Paise::ZERO))
            } else if marker.eq_ignore_ascii_case("cr") {
                Ok((Paise::ZERO, value))
            } else {
                Err(row_err(
                    row,
                    format!("Dr/Cr indicator '{marker}' is neither Dr nor Cr"),
                ))
            }
        }
    }
}

/// Parse a rupee amount into paise: digits with optional plain, western or Indian comma grouping,
/// at most two decimals, optional leading `-`. Nothing is rounded.
fn parse_amount(text: &str) -> Result<Paise, String> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (int_part, frac_part) = match unsigned.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (unsigned, None),
    };
    if int_part.is_empty() || !valid_grouping(int_part) {
        return Err(
            "not a valid amount (expected digits, optional lakh/million grouping, optional .dd)"
                .to_owned(),
        );
    }
    let paise_part: i64 = match frac_part {
        None => 0,
        Some(f) if f.is_empty() || f.len() > 2 || !f.bytes().all(|b| b.is_ascii_digit()) => {
            return Err("decimals must be one or two digits".to_owned());
        }
        Some(f) => {
            let v: i64 = f.parse().map_err(|_| "bad decimals".to_owned())?;
            if f.len() == 1 { v * 10 } else { v }
        }
    };
    let mut rupees: i64 = 0;
    for b in int_part.bytes().filter(|b| *b != b',') {
        let digit = i64::from(b - b'0');
        rupees = rupees
            .checked_mul(10)
            .and_then(|r| r.checked_add(digit))
            .ok_or_else(|| "amount overflows".to_owned())?;
    }
    let total = rupees
        .checked_mul(100)
        .and_then(|p| p.checked_add(paise_part))
        .ok_or_else(|| "amount overflows".to_owned())?;
    let paise = Paise::new(total);
    if negative {
        paise.checked_neg().map_err(|e| e.to_string())
    } else {
        Ok(paise)
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Plain digits, western grouping (3-3-3) or Indian grouping (2-2-3). Anything else is rejected.
fn valid_grouping(int_part: &str) -> bool {
    let groups: Vec<&str> = int_part.split(',').collect();
    if !groups.iter().all(|g| all_digits(g)) {
        return false;
    }
    let Some((first, rest)) = groups.split_first() else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let western = first.len() <= 3 && rest.iter().all(|g| g.len() == 3);
    let indian = first.len() <= 2
        && rest
            .split_last()
            .is_some_and(|(last, mid)| last.len() == 3 && mid.iter().all(|g| g.len() == 2));
    western || indian
}

fn parse_date(text: &str, layout: &CsvLayout) -> Result<Date, String> {
    let parts: Vec<&str> = text.split(layout.date_separator).collect();
    let [a, b, c] = parts.as_slice() else {
        return Err(format!(
            "expected three parts separated by '{}'",
            layout.date_separator
        ));
    };
    let num = |s: &str, len: usize| -> Result<i32, String> {
        if s.len() == len && all_digits(s) {
            s.parse().map_err(|_| "bad number".to_owned())
        } else {
            Err(format!("'{s}' is not {len} digits"))
        }
    };
    let (year, month, day) = match layout.date_format {
        DateFormat::DdMmYyyy => (num(c, 4)?, month_from_number(num(b, 2)?)?, num(a, 2)?),
        DateFormat::DdMmYy { century_base } => {
            let yy = num(c, 2)?;
            let year = century_base
                .checked_add(yy)
                .ok_or_else(|| "year overflows".to_owned())?;
            (year, month_from_number(num(b, 2)?)?, num(a, 2)?)
        }
        DateFormat::YyyyMmDd => (num(a, 4)?, month_from_number(num(b, 2)?)?, num(c, 2)?),
        DateFormat::DdMonYyyy => (num(c, 4)?, month_from_name(b)?, num(a, 2)?),
    };
    let day = u8::try_from(day).map_err(|_| "day out of range".to_owned())?;
    Date::from_calendar_date(year, month, day).map_err(|e| e.to_string())
}

fn month_from_number(n: i32) -> Result<Month, String> {
    u8::try_from(n)
        .ok()
        .and_then(|m| Month::try_from(m).ok())
        .ok_or_else(|| format!("month {n} out of range"))
}

fn month_from_name(name: &str) -> Result<Month, String> {
    MONTHS
        .iter()
        .find(|(abbr, _)| abbr.eq_ignore_ascii_case(name))
        .map(|(_, m)| *m)
        .ok_or_else(|| format!("'{name}' is not a three-letter English month"))
}
