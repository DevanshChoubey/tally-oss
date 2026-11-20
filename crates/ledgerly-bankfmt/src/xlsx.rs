//! XLSX statements. **Card BF3** implements this module.
//!
//! Frozen interface (plan card BF3):
//!
//! ```text
//! pub fn parse_xlsx(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError>;
//! ```
//!
//! `FormatSpec` is defined by card BF1 in `registry.rs`, which had not landed when BF3 was built,
//! so `parse_xlsx` itself is not provided yet (see the BF3 interface request in the card report).
//! The parser core is [`parse_xlsx_with_layout`], driven by the [`XlsxLayout`] defined here; once
//! `FormatSpec` exists, `parse_xlsx` is a conversion to [`XlsxLayout`] plus this call. This is the
//! same arrangement card BF2 used for CSV.
//!
//! Banks put a letterhead (bank name, address, account holder, period) above the transaction
//! table, so rows cannot be skipped by a fixed count. The header row is found by content: it is the
//! first row in which every one of [`XlsxLayout::header_markers`] matches a cell. Columns are then
//! looked up by their header text, so a bank that moves a column does not silently shift values.
//!
//! Behaviour, all of it strict (no guessing, nothing rounded):
//!
//! - Header matching ignores case and runs of white space; it never matches part of a cell.
//! - A column header that is missing, or that matches two cells, is a format error.
//! - Rows are numbered by 1-based sheet row, letterhead rows included, so a row error points at the
//!   row the user sees in a spreadsheet program.
//! - Rows whose cells are all blank are ignored. A row whose first non-blank cell starts with one
//!   of [`XlsxLayout::end_markers`] ends the table (closing-balance and legend lines).
//! - Dates are real date cells, ISO text, or text in the layout's [`DateFormat`]. A bare number in
//!   a date column is an error, because without a date format it may not be a date. Excel serial
//!   60 (the non-existent 29 Feb 1900) is an error, because the date it names does not exist.
//! - Amounts are numeric cells or text with plain, western (`123,456.78`) or Indian
//!   (`1,23,456.78`) grouping, at most two decimals, an optional leading `-`. Numbers are read from
//!   the cell's shortest decimal text, never through floating-point arithmetic, so a value such as
//!   `0.30000000000000004` is an error rather than a rounded amount.
//! - Exactly one of debit and credit is non-zero on every line.
//! - Opening and closing balances are not read from the sheet; they are `None`.

use crate::csv::{AmountColumns, DateFormat};
use crate::{BankFmtError, RawLine, RawStatement};
use calamine::{Data, Reader, Xlsx};
use ledgerly_core::{Date, Month, Paise};
use std::io::Cursor;

/// How the amount of a line is laid out in the sheet, by header text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XlsxAmounts {
    /// Separate withdrawal and deposit columns; exactly one is filled.
    Split {
        /// Header of the withdrawal (debit) column.
        debit: String,
        /// Header of the deposit (credit) column.
        credit: String,
    },
    /// One signed column: negative is money out (debit), positive is money in (credit).
    Signed {
        /// Header of the signed amount column.
        amount: String,
    },
    /// An unsigned amount column plus a column holding `Dr` or `Cr` (case-insensitive).
    WithIndicator {
        /// Header of the amount column.
        amount: String,
        /// Header of the `Dr`/`Cr` marker column.
        indicator: String,
    },
}

/// Everything the XLSX parser needs to know about one bank's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XlsxLayout {
    /// Format registry id, copied to `RawStatement::format_id`.
    pub format_id: String,
    /// Sheet to read; `None` reads the first sheet.
    pub sheet: Option<String>,
    /// Header texts that identify the header row: the first row in which every marker matches a
    /// cell. An empty list makes the first non-empty row the header.
    pub header_markers: Vec<String>,
    /// Texts that end the transaction table: a row whose first non-blank cell starts with one of
    /// these (compared ignoring case and repeated spaces) is the end of the data.
    pub end_markers: Vec<String>,
    /// Header of the transaction date column.
    pub date_col: String,
    /// Header of the value date column, if the file has one.
    pub value_date_col: Option<String>,
    /// Header of the description column.
    pub description_col: String,
    /// Header of the cheque or reference number column, if any.
    pub reference_col: Option<String>,
    /// Amount layout.
    pub amounts: XlsxAmounts,
    /// Header of the running balance column, if any (credit positive).
    pub balance_col: Option<String>,
    /// Layout of dates held as text, shared by the date and value date columns.
    pub date_format: DateFormat,
    /// Separator between the parts of a text date.
    pub date_separator: char,
}

/// Column positions resolved from the header row.
struct Columns {
    date: usize,
    value_date: Option<usize>,
    description: usize,
    reference: Option<usize>,
    amounts: AmountColumns,
    balance: Option<usize>,
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

/// Parse an XLSX bank statement using an explicit layout.
///
/// # Errors
/// [`BankFmtError::Format`] when the file is not a readable workbook, the sheet or the header row
/// cannot be found, or a header is missing or ambiguous; [`BankFmtError::Row`] naming the sheet
/// row for any cell that cannot be read exactly.
pub fn parse_xlsx_with_layout(
    bytes: &[u8],
    layout: &XlsxLayout,
) -> Result<RawStatement, BankFmtError> {
    let mut book = calamine::open_workbook_from_rs::<Xlsx<_>, _>(Cursor::new(bytes))
        .map_err(|e| format_err(format!("not a readable XLSX file: {e}")))?;
    let sheet_name = match &layout.sheet {
        Some(name) => name.clone(),
        None => book
            .sheet_names()
            .first()
            .cloned()
            .ok_or_else(|| format_err("the workbook has no sheets"))?,
    };
    let range = book
        .worksheet_range(&sheet_name)
        .map_err(|e| format_err(format!("sheet '{sheet_name}' cannot be read: {e}")))?;
    let first_row = range
        .start()
        .map(|(row, _col)| row)
        .ok_or_else(|| format_err(format!("sheet '{sheet_name}' is empty")))?;
    let rows: Vec<&[Data]> = range.rows().collect();

    let header_idx = rows
        .iter()
        .position(|r| has_all_markers(r, &layout.header_markers))
        .ok_or_else(|| {
            format_err(format!(
                "sheet '{sheet_name}': no header row containing {:?}",
                layout.header_markers
            ))
        })?;
    let header = rows
        .get(header_idx)
        .copied()
        .ok_or_else(|| format_err("header row vanished"))?;
    let cols = resolve_columns(header, layout)?;

    let mut lines = Vec::new();
    for (idx, row) in rows.iter().enumerate().skip(header_idx.saturating_add(1)) {
        let row_no = sheet_row(first_row, idx)?;
        if row.iter().all(is_blank) {
            continue;
        }
        if is_end_row(row, &layout.end_markers) {
            break;
        }
        lines.push(parse_line(row_no, row, &cols, layout)?);
    }
    Ok(RawStatement {
        format_id: layout.format_id.clone(),
        account_hint: None,
        opening: None,
        closing: None,
        lines,
    })
}

fn format_err(reason: impl Into<String>) -> BankFmtError {
    BankFmtError::Format {
        reason: reason.into(),
    }
}

fn row_err(row: u32, reason: impl Into<String>) -> BankFmtError {
    BankFmtError::Row {
        row,
        reason: reason.into(),
    }
}

/// 1-based sheet row of the `idx`-th row of the used range, which starts at 0-based `first_row`.
fn sheet_row(first_row: u32, idx: usize) -> Result<u32, BankFmtError> {
    u32::try_from(idx)
        .ok()
        .and_then(|i| first_row.checked_add(i))
        .and_then(|r| r.checked_add(1))
        .ok_or_else(|| format_err("sheet has more than u32::MAX rows"))
}

/// Lower-case, with white space collapsed to single spaces and the ends trimmed.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_blank(cell: &Data) -> bool {
    match cell {
        Data::Empty => true,
        Data::String(s) => s.trim().is_empty(),
        Data::Int(_)
        | Data::Float(_)
        | Data::Bool(_)
        | Data::DateTime(_)
        | Data::DateTimeIso(_)
        | Data::DurationIso(_)
        | Data::Error(_) => false,
    }
}

fn has_all_markers(row: &[Data], markers: &[String]) -> bool {
    markers.iter().all(|marker| {
        let want = normalize(marker);
        row.iter().any(|c| normalize(&c.to_string()) == want)
    })
}

fn is_end_row(row: &[Data], end_markers: &[String]) -> bool {
    let Some(first) = row.iter().find(|c| !is_blank(c)) else {
        return false;
    };
    let text = normalize(&first.to_string());
    end_markers
        .iter()
        .any(|m| !normalize(m).is_empty() && text.starts_with(&normalize(m)))
}

fn find_column(header: &[Data], name: &str) -> Result<usize, BankFmtError> {
    let want = normalize(name);
    let mut found: Option<usize> = None;
    for (idx, cell) in header.iter().enumerate() {
        if normalize(&cell.to_string()) == want {
            if found.is_some() {
                return Err(format_err(format!(
                    "column header '{name}' appears more than once in the header row"
                )));
            }
            found = Some(idx);
        }
    }
    found.ok_or_else(|| {
        format_err(format!(
            "column header '{name}' not found in the header row"
        ))
    })
}

fn find_optional(header: &[Data], name: Option<&String>) -> Result<Option<usize>, BankFmtError> {
    name.map(|n| find_column(header, n)).transpose()
}

fn resolve_columns(header: &[Data], layout: &XlsxLayout) -> Result<Columns, BankFmtError> {
    let amounts = match &layout.amounts {
        XlsxAmounts::Split { debit, credit } => AmountColumns::Split {
            debit: find_column(header, debit)?,
            credit: find_column(header, credit)?,
        },
        XlsxAmounts::Signed { amount } => AmountColumns::Signed {
            amount: find_column(header, amount)?,
        },
        XlsxAmounts::WithIndicator { amount, indicator } => AmountColumns::WithIndicator {
            amount: find_column(header, amount)?,
            indicator: find_column(header, indicator)?,
        },
    };
    Ok(Columns {
        date: find_column(header, &layout.date_col)?,
        value_date: find_optional(header, layout.value_date_col.as_ref())?,
        description: find_column(header, &layout.description_col)?,
        reference: find_optional(header, layout.reference_col.as_ref())?,
        amounts,
        balance: find_optional(header, layout.balance_col.as_ref())?,
    })
}

/// The cell at `col`, or `Data::Empty` when the row is shorter (trailing blank cells are absent).
fn cell_at(row: &[Data], col: usize) -> &Data {
    static EMPTY: Data = Data::Empty;
    row.get(col).unwrap_or(&EMPTY)
}

fn text_at(row: &[Data], col: usize) -> String {
    cell_at(row, col).to_string().trim().to_owned()
}

fn parse_line(
    row_no: u32,
    row: &[Data],
    cols: &Columns,
    layout: &XlsxLayout,
) -> Result<RawLine, BankFmtError> {
    let date = parse_date_cell(cell_at(row, cols.date), layout)
        .map_err(|r| row_err(row_no, format!("date: {r}")))?
        .ok_or_else(|| row_err(row_no, "date is empty"))?;
    let value_date = match cols.value_date {
        None => None,
        Some(col) => parse_date_cell(cell_at(row, col), layout)
            .map_err(|r| row_err(row_no, format!("value date: {r}")))?,
    };
    let reference = cols
        .reference
        .map(|col| text_at(row, col))
        .filter(|t| !t.is_empty());
    let (debit, credit) = parse_amounts(row_no, row, &cols.amounts)?;
    let balance = match cols.balance {
        None => None,
        Some(col) => amount_cell(row_no, row, col, "balance")?,
    };
    Ok(RawLine {
        row: row_no,
        date,
        value_date,
        description: text_at(row, cols.description),
        reference,
        debit,
        credit,
        balance,
        raw: row.iter().map(ToString::to_string).collect(),
    })
}

/// The amount in a cell, `None` when the cell is blank.
fn amount_cell(
    row_no: u32,
    row: &[Data],
    col: usize,
    what: &str,
) -> Result<Option<Paise>, BankFmtError> {
    let text = match cell_at(row, col) {
        Data::Empty => return Ok(None),
        Data::String(s) => s.trim().to_owned(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::Bool(_)
        | Data::DateTime(_)
        | Data::DateTimeIso(_)
        | Data::DurationIso(_)
        | Data::Error(_) => {
            return Err(row_err(
                row_no,
                format!("{what}: the cell is not a number or text"),
            ));
        }
    };
    if text.is_empty() {
        return Ok(None);
    }
    parse_amount(&text)
        .map(Some)
        .map_err(|r| row_err(row_no, format!("{what} '{text}': {r}")))
}

fn parse_amounts(
    row_no: u32,
    row: &[Data],
    amounts: &AmountColumns,
) -> Result<(Paise, Paise), BankFmtError> {
    match amounts {
        AmountColumns::Split { debit, credit } => {
            let dr = amount_cell(row_no, row, *debit, "debit")?;
            let cr = amount_cell(row_no, row, *credit, "credit")?;
            match (dr, cr) {
                (Some(d), None) if !d.is_negative() => Ok((d, Paise::ZERO)),
                (None, Some(c)) if !c.is_negative() => Ok((Paise::ZERO, c)),
                (Some(_), Some(_)) => Err(row_err(row_no, "both debit and credit are filled")),
                (None, None) => Err(row_err(row_no, "neither debit nor credit is filled")),
                (Some(_), None) | (None, Some(_)) => Err(row_err(
                    row_no,
                    "negative amount in a split debit/credit column",
                )),
            }
        }
        AmountColumns::Signed { amount } => {
            let value = amount_cell(row_no, row, *amount, "amount")?
                .ok_or_else(|| row_err(row_no, "amount is empty"))?;
            if value.is_negative() {
                let magnitude = value
                    .checked_neg()
                    .map_err(|e| row_err(row_no, e.to_string()))?;
                Ok((magnitude, Paise::ZERO))
            } else {
                Ok((Paise::ZERO, value))
            }
        }
        AmountColumns::WithIndicator { amount, indicator } => {
            let value = amount_cell(row_no, row, *amount, "amount")?
                .ok_or_else(|| row_err(row_no, "amount is empty"))?;
            if value.is_negative() {
                return Err(row_err(row_no, "negative amount with a Dr/Cr indicator"));
            }
            let marker = text_at(row, *indicator);
            if marker.eq_ignore_ascii_case("dr") {
                Ok((value, Paise::ZERO))
            } else if marker.eq_ignore_ascii_case("cr") {
                Ok((Paise::ZERO, value))
            } else {
                Err(row_err(
                    row_no,
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
            if f.len() == 1 {
                v.checked_mul(10).ok_or_else(|| "bad decimals".to_owned())?
            } else {
                v
            }
        }
    };
    let mut rupees: i64 = 0;
    for b in int_part.bytes().filter(|b| *b != b',') {
        let digit = i64::from(b.wrapping_sub(b'0'));
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

/// A date cell: `Ok(None)` when blank, an error message when it cannot be read exactly.
fn parse_date_cell(cell: &Data, layout: &XlsxLayout) -> Result<Option<Date>, String> {
    match cell {
        Data::Empty => Ok(None),
        Data::String(s) => {
            let text = s.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                parse_date_text(text, layout)
                    .map(Some)
                    .map_err(|r| format!("'{text}': {r}"))
            }
        }
        Data::DateTime(dt) => {
            if !dt.is_datetime() {
                return Err("the cell holds a duration, not a date".to_owned());
            }
            let (year, month, day, ..) = dt.to_ymd_hms_milli();
            let serial = dt.as_f64();
            let month = Month::try_from(month).map_err(|e| e.to_string())?;
            Date::from_calendar_date(i32::from(year), month, day)
                .map(Some)
                .map_err(|e| format!("serial {serial}: {e}"))
        }
        Data::DateTimeIso(s) => {
            let date_part = s.split('T').next().unwrap_or_default();
            parse_parts(date_part, '-', DateFormat::YyyyMmDd)
                .map(Some)
                .map_err(|r| format!("'{s}': {r}"))
        }
        Data::Int(_) | Data::Float(_) => Err(
            "a bare number in a date column (the cell has no date format, so it may not be a date)"
                .to_owned(),
        ),
        Data::Bool(_) | Data::DurationIso(_) | Data::Error(_) => {
            Err("the cell is not a date".to_owned())
        }
    }
}

fn parse_date_text(text: &str, layout: &XlsxLayout) -> Result<Date, String> {
    parse_parts(text, layout.date_separator, layout.date_format)
}

fn parse_parts(text: &str, separator: char, format: DateFormat) -> Result<Date, String> {
    let parts: Vec<&str> = text.split(separator).collect();
    let [a, b, c] = parts.as_slice() else {
        return Err(format!("expected three parts separated by '{separator}'"));
    };
    let num = |s: &str, len: usize| -> Result<i32, String> {
        if s.len() == len && all_digits(s) {
            s.parse().map_err(|_| "bad number".to_owned())
        } else {
            Err(format!("'{s}' is not {len} digits"))
        }
    };
    let (year, month, day) = match format {
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
