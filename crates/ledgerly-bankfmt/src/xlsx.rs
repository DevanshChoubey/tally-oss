//! XLSX statements. **Card BF3** implements this module.
//!
//! Frozen interface (plan card BF3):
//!
//! ```text
//! pub fn parse_xlsx(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError>;
//! ```
//!
//! There are two ways in, sharing one reader:
//!
//! - [`parse_xlsx`] (and [`parse_xlsx_with_labels`]) read a registry [`FormatSpec`] of
//!   `kind = "xlsx"`. Columns are the 0-based sheet column indexes the format gives (column `A` is
//!   0, counted from the sheet edge, whatever the used range is). The first transaction row is
//!   found by [`HeaderRows`]: `Skip(n)` skips the first `n` sheet rows; `FindHeader(cells)` takes
//!   the first row holding every one of the cells and reads transactions from the row after it.
//! - [`parse_xlsx_with_layout`] (and [`parse_xlsx_with_layout_and_labels`]) read an
//!   [`XlsxLayout`], which names the columns by header text instead of position, so a bank that
//!   moves a column does not silently shift values.
//!
//! Banks put a letterhead (bank name, address, account holder, period) above the transaction
//! table, so rows cannot be skipped by a fixed count when the letterhead length varies. The
//! header row is therefore found by content: the first row in which every one of the header cells
//! matches a cell.
//!
//! Behaviour, all of it strict (no guessing, nothing rounded):
//!
//! - Header matching ignores case and runs of white space; it never matches part of a cell.
//! - In a layout, a column header that is missing, or that matches two cells, is a format error.
//! - Rows are numbered by 1-based sheet row, letterhead rows included, so a row error points at the
//!   row the user sees in a spreadsheet program.
//! - Rows whose cells are all blank are ignored. In a layout, a row whose first non-blank cell
//!   starts with one of [`XlsxLayout::end_markers`] ends the table (closing-balance and legend
//!   lines). A registry format has no end markers yet, so a footer under its table is a row error.
//! - Dates are real date cells, ISO text, or text in the format's [`DateFormat`]. A bare number in
//!   a date column is an error, because without a date format it may not be a date. Excel serial
//!   60 (the non-existent 29 Feb 1900) is an error, because the date it names does not exist.
//! - Amounts are numeric cells or text with plain, western (`123,456.78`) or Indian
//!   (`1,23,456.78`) grouping, at most two decimals, an optional leading `-`. Numbers are read from
//!   the cell's shortest decimal text, never through floating-point arithmetic, so a value such as
//!   `0.30000000000000004` is an error rather than a rounded amount.
//! - Exactly one of debit and credit is non-zero on every line.
//!
//! Opening and closing balances and the account number live in the letterhead or footer, not in
//! the table, and registry formats do not describe them yet. [`BalanceLabels`] supplies them: the
//! caller names the label text of each (for example `Opening Balance`), and the value is the first
//! non-blank cell to the right of the one cell in the sheet that holds that label. With no labels
//! given, `opening`, `closing` and `account_hint` are `None`, and the balance check (spec 6.5) then
//! reports the missing balance rather than guessing one.

use crate::csv::{AmountColumns, DateFormat};
use crate::registry::{FileKind, FormatSpec, HeaderRows, TableSpec};
use crate::{BankFmtError, RawLine, RawStatement};
use calamine::{Data, Range, Reader, Xlsx};
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

/// Everything the XLSX parser needs to know about one bank's layout, with columns named by header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XlsxLayout {
    /// Format registry id, copied to `RawStatement::format_id`.
    pub format_id: String,
    /// Sheet to read; `None` reads the first sheet.
    pub sheet: Option<String>,
    /// Header texts that identify the header row: the first row in which every marker matches a
    /// cell. An empty list makes the first row that is not entirely blank the header.
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

/// Label texts that locate the statement-level values outside the transaction table.
///
/// A label matches a whole cell, ignoring case, repeated white space and a trailing `:`. The value
/// is the first non-blank cell to the right of it in the same row. Each configured label must
/// match exactly one cell outside the header row (none, or more than one, is an error), and its
/// value must be readable (an amount for the balances). Balances are read as written, credit
/// positive, a leading `-` meaning overdrawn; a `Dr`/`Cr` suffix is not understood and is an error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BalanceLabels {
    /// Label of the opening balance.
    pub opening: Option<String>,
    /// Label of the closing balance.
    pub closing: Option<String>,
    /// Label of the account number or name, copied to `RawStatement::account_hint`.
    pub account: Option<String>,
}

/// Column positions: absolute 0-based sheet columns.
struct Columns {
    date: usize,
    value_date: Option<usize>,
    description: usize,
    reference: Option<usize>,
    amounts: AmountColumns,
    balance: Option<usize>,
}

/// How dates held as text are read.
#[derive(Clone, Copy)]
struct Dates {
    format: DateFormat,
    separator: char,
}

/// Everything the shared reader needs once the header and columns are settled.
struct Job<'a> {
    format_id: &'a str,
    /// Index (into the used range) of the first row that may hold a transaction.
    data_start: usize,
    /// Index of the header row, when one was found by content (labels are not looked for in it).
    header_row: Option<usize>,
    cols: Columns,
    end_markers: &'a [String],
    dates: Dates,
    labels: &'a BalanceLabels,
}

/// The used range of a sheet.
struct View<'a> {
    name: &'a str,
    rows: Vec<&'a [Data]>,
    /// 0-based sheet row of the first row of the used range.
    first_row: u32,
    /// 0-based sheet column of the first column of the used range.
    first_col: usize,
}

/// One row of the used range, with the sheet row number and the column offset of the range.
#[derive(Clone, Copy)]
struct Row<'a> {
    no: u32,
    cells: &'a [Data],
    col0: usize,
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

/// Parse an XLSX bank statement described by a registry format. Opening and closing balances and
/// the account hint are not read (use [`parse_xlsx_with_labels`] to read them).
///
/// # Errors
/// [`BankFmtError::Format`] when the format is not an XLSX format, the file is not a readable
/// workbook, or the sheet or header row cannot be found; [`BankFmtError::Row`] naming the sheet
/// row for any cell that cannot be read exactly.
pub fn parse_xlsx(bytes: &[u8], spec: &FormatSpec) -> Result<RawStatement, BankFmtError> {
    parse_xlsx_with_labels(bytes, spec, &BalanceLabels::default())
}

/// [`parse_xlsx`], also reading the opening balance, closing balance and account hint from the
/// cells named by `labels`.
///
/// # Errors
/// As [`parse_xlsx`]; also [`BankFmtError::Format`] for a label that is empty, matches no cell or
/// more than one cell, and [`BankFmtError::Row`] for a label with no value or an unreadable one.
pub fn parse_xlsx_with_labels(
    bytes: &[u8],
    spec: &FormatSpec,
    labels: &BalanceLabels,
) -> Result<RawStatement, BankFmtError> {
    let table = xlsx_table(spec)?;
    let (name, range) = open_range(bytes, table.sheet.as_deref())?;
    let view = View::new(&name, &range)?;
    let (data_start, header_row) = match &table.header {
        HeaderRows::Skip(n) => {
            // The first `n` sheet rows are skipped; the used range may start below sheet row 1.
            let skip = usize::try_from(n.saturating_sub(view.first_row)).unwrap_or(usize::MAX);
            (skip, None)
        }
        HeaderRows::FindHeader(cells) => {
            let idx = view.find_header(cells)?;
            (idx.saturating_add(1), Some(idx))
        }
    };
    let job = Job {
        format_id: &spec.id,
        data_start,
        header_row,
        cols: Columns {
            date: table.columns.date,
            value_date: table.columns.value_date,
            description: table.columns.description,
            reference: table.columns.reference,
            amounts: table.amounts.clone(),
            balance: table.columns.balance,
        },
        end_markers: &[],
        dates: Dates {
            format: table.date_format,
            separator: table.date_separator,
        },
        labels,
    };
    read_statement(&view, &job)
}

/// Parse an XLSX bank statement using an explicit layout. Opening and closing balances and the
/// account hint are not read (use [`parse_xlsx_with_layout_and_labels`] to read them).
///
/// # Errors
/// [`BankFmtError::Format`] when the file is not a readable workbook, the sheet or the header row
/// cannot be found, or a header is missing or ambiguous; [`BankFmtError::Row`] naming the sheet
/// row for any cell that cannot be read exactly.
pub fn parse_xlsx_with_layout(
    bytes: &[u8],
    layout: &XlsxLayout,
) -> Result<RawStatement, BankFmtError> {
    parse_xlsx_with_layout_and_labels(bytes, layout, &BalanceLabels::default())
}

/// [`parse_xlsx_with_layout`], also reading the opening balance, closing balance and account hint
/// from the cells named by `labels`.
///
/// # Errors
/// As [`parse_xlsx_with_layout`]; also [`BankFmtError::Format`] for a label that is empty, matches
/// no cell or more than one cell, and [`BankFmtError::Row`] for a label with no value or an
/// unreadable one.
pub fn parse_xlsx_with_layout_and_labels(
    bytes: &[u8],
    layout: &XlsxLayout,
    labels: &BalanceLabels,
) -> Result<RawStatement, BankFmtError> {
    let (name, range) = open_range(bytes, layout.sheet.as_deref())?;
    let view = View::new(&name, &range)?;
    let header_idx = view.find_header(&layout.header_markers)?;
    let header = view
        .rows
        .get(header_idx)
        .copied()
        .ok_or_else(|| format_err("header row vanished"))?;
    let cols = resolve_columns(header, layout, view.first_col)?;
    let job = Job {
        format_id: &layout.format_id,
        data_start: header_idx.saturating_add(1),
        header_row: Some(header_idx),
        cols,
        end_markers: &layout.end_markers,
        dates: Dates {
            format: layout.date_format,
            separator: layout.date_separator,
        },
        labels,
    };
    read_statement(&view, &job)
}

/// The table description of an XLSX format, checked to be one.
fn xlsx_table(spec: &FormatSpec) -> Result<&TableSpec, BankFmtError> {
    if spec.kind != FileKind::Xlsx {
        return Err(format_err(format!(
            "format '{}' is not an XLSX format",
            spec.id
        )));
    }
    let table = spec
        .table
        .as_ref()
        .ok_or_else(|| format_err(format!("format '{}' has no table layout", spec.id)))?;
    if table.delimiter.is_some() {
        return Err(format_err(format!(
            "format '{}' names a delimiter, which an XLSX format cannot have",
            spec.id
        )));
    }
    Ok(table)
}

fn open_range(bytes: &[u8], sheet: Option<&str>) -> Result<(String, Range<Data>), BankFmtError> {
    let mut book = calamine::open_workbook_from_rs::<Xlsx<_>, _>(Cursor::new(bytes))
        .map_err(|e| format_err(format!("not a readable XLSX file: {e}")))?;
    let name = match sheet {
        Some(name) => name.to_owned(),
        None => book
            .sheet_names()
            .first()
            .cloned()
            .ok_or_else(|| format_err("the workbook has no sheets"))?,
    };
    let range = book
        .worksheet_range(&name)
        .map_err(|e| format_err(format!("sheet '{name}' cannot be read: {e}")))?;
    Ok((name, range))
}

impl<'a> View<'a> {
    fn new(name: &'a str, range: &'a Range<Data>) -> Result<Self, BankFmtError> {
        let (first_row, first_col) = range
            .start()
            .ok_or_else(|| format_err(format!("sheet '{name}' is empty")))?;
        let first_col = usize::try_from(first_col)
            .map_err(|_| format_err("sheet column offset does not fit in memory"))?;
        Ok(View {
            name,
            rows: range.rows().collect(),
            first_row,
            first_col,
        })
    }

    /// Index of the first row holding every marker; with no markers, the first row that is not
    /// entirely blank.
    fn find_header(&self, markers: &[String]) -> Result<usize, BankFmtError> {
        if markers.is_empty() {
            return self
                .rows
                .iter()
                .position(|r| !r.iter().all(is_blank))
                .ok_or_else(|| {
                    format_err(format!("sheet '{}': there is no non-blank row", self.name))
                });
        }
        self.rows
            .iter()
            .position(|r| has_all_markers(r, markers))
            .ok_or_else(|| {
                format_err(format!(
                    "sheet '{}': no header row containing {markers:?}",
                    self.name
                ))
            })
    }

    fn row_no(&self, idx: usize) -> Result<u32, BankFmtError> {
        sheet_row(self.first_row, idx)
    }
}

impl Row<'_> {
    /// The cell at absolute sheet column `col`; blank when it lies outside the used range.
    fn cell(&self, col: usize) -> &Data {
        static EMPTY: Data = Data::Empty;
        col.checked_sub(self.col0)
            .and_then(|c| self.cells.get(c))
            .unwrap_or(&EMPTY)
    }

    fn text(&self, col: usize) -> String {
        self.cell(col).to_string().trim().to_owned()
    }
}

/// Read the transaction lines and, when labels are given, the statement-level values.
fn read_statement(view: &View<'_>, job: &Job<'_>) -> Result<RawStatement, BankFmtError> {
    let mut lines = Vec::new();
    for (idx, cells) in view.rows.iter().enumerate().skip(job.data_start) {
        if cells.iter().all(is_blank) {
            continue;
        }
        if is_end_row(cells, job.end_markers) {
            break;
        }
        let row = Row {
            no: view.row_no(idx)?,
            cells: *cells,
            col0: view.first_col,
        };
        lines.push(parse_line(&row, &job.cols, job.dates)?);
    }
    let account_hint = match &job.labels.account {
        None => None,
        Some(label) => {
            let (_, cell) = label_value(view, job.header_row, label)?;
            Some(cell.to_string().trim().to_owned())
        }
    };
    let opening = balance_value(view, job.header_row, job.labels.opening.as_ref(), "opening")?;
    let closing = balance_value(view, job.header_row, job.labels.closing.as_ref(), "closing")?;
    Ok(RawStatement {
        format_id: job.format_id.to_owned(),
        account_hint,
        opening,
        closing,
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

/// [`normalize`] for a label: a trailing `:` is not part of the name.
fn label_key(text: &str) -> String {
    normalize(text.trim().trim_end_matches(':'))
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

/// The cell holding `label` (outside the header row) and the first non-blank cell to its right.
fn label_value<'a>(
    view: &View<'a>,
    header_row: Option<usize>,
    label: &str,
) -> Result<(u32, &'a Data), BankFmtError> {
    let want = label_key(label);
    if want.is_empty() {
        return Err(format_err("a statement label must not be empty"));
    }
    let mut found: Vec<(usize, usize)> = Vec::new();
    for (idx, cells) in view.rows.iter().enumerate() {
        if header_row == Some(idx) {
            continue;
        }
        for (col, cell) in cells.iter().enumerate() {
            if label_key(&cell.to_string()) == want {
                found.push((idx, col));
            }
        }
    }
    let (idx, col) = match found.as_slice() {
        [] => {
            return Err(format_err(format!(
                "sheet '{}': label '{label}' not found outside the header row",
                view.name
            )));
        }
        [one] => *one,
        [_, _, ..] => {
            return Err(format_err(format!(
                "sheet '{}': label '{label}' appears {} times outside the header row",
                view.name,
                found.len()
            )));
        }
    };
    let no = view.row_no(idx)?;
    let cells = view
        .rows
        .get(idx)
        .copied()
        .ok_or_else(|| format_err("label row vanished"))?;
    let value = cells
        .iter()
        .skip(col.saturating_add(1))
        .find(|c| !is_blank(c))
        .ok_or_else(|| row_err(no, format!("label '{label}' has no value to its right")))?;
    Ok((no, value))
}

fn balance_value(
    view: &View<'_>,
    header_row: Option<usize>,
    label: Option<&String>,
    what: &str,
) -> Result<Option<Paise>, BankFmtError> {
    let Some(label) = label else {
        return Ok(None);
    };
    let (no, cell) = label_value(view, header_row, label)?;
    let value = amount_from(cell, &format!("{what} balance"))
        .map_err(|r| row_err(no, r))?
        .ok_or_else(|| row_err(no, format!("{what} balance is empty")))?;
    Ok(Some(value))
}

/// Index of `name` among the header cells, as an absolute sheet column.
fn find_column(header: &[Data], name: &str, col0: usize) -> Result<usize, BankFmtError> {
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
    let idx = found.ok_or_else(|| {
        format_err(format!(
            "column header '{name}' not found in the header row"
        ))
    })?;
    idx.checked_add(col0)
        .ok_or_else(|| format_err("column index overflows"))
}

fn find_optional(
    header: &[Data],
    name: Option<&String>,
    col0: usize,
) -> Result<Option<usize>, BankFmtError> {
    name.map(|n| find_column(header, n, col0)).transpose()
}

fn resolve_columns(
    header: &[Data],
    layout: &XlsxLayout,
    col0: usize,
) -> Result<Columns, BankFmtError> {
    let amounts = match &layout.amounts {
        XlsxAmounts::Split { debit, credit } => AmountColumns::Split {
            debit: find_column(header, debit, col0)?,
            credit: find_column(header, credit, col0)?,
        },
        XlsxAmounts::Signed { amount } => AmountColumns::Signed {
            amount: find_column(header, amount, col0)?,
        },
        XlsxAmounts::WithIndicator { amount, indicator } => AmountColumns::WithIndicator {
            amount: find_column(header, amount, col0)?,
            indicator: find_column(header, indicator, col0)?,
        },
    };
    Ok(Columns {
        date: find_column(header, &layout.date_col, col0)?,
        value_date: find_optional(header, layout.value_date_col.as_ref(), col0)?,
        description: find_column(header, &layout.description_col, col0)?,
        reference: find_optional(header, layout.reference_col.as_ref(), col0)?,
        amounts,
        balance: find_optional(header, layout.balance_col.as_ref(), col0)?,
    })
}

fn parse_line(row: &Row<'_>, cols: &Columns, dates: Dates) -> Result<RawLine, BankFmtError> {
    let date = parse_date_cell(row.cell(cols.date), dates)
        .map_err(|r| row_err(row.no, format!("date: {r}")))?
        .ok_or_else(|| row_err(row.no, "date is empty"))?;
    let value_date = match cols.value_date {
        None => None,
        Some(col) => parse_date_cell(row.cell(col), dates)
            .map_err(|r| row_err(row.no, format!("value date: {r}")))?,
    };
    let reference = cols
        .reference
        .map(|col| row.text(col))
        .filter(|t| !t.is_empty());
    let (debit, credit) = parse_amounts(row, &cols.amounts)?;
    let balance = match cols.balance {
        None => None,
        Some(col) => amount_cell(row, col, "balance")?,
    };
    // `raw` is aligned with sheet columns: cells left of the used range are empty strings.
    let raw = std::iter::repeat_n(String::new(), row.col0)
        .chain(row.cells.iter().map(ToString::to_string))
        .collect();
    Ok(RawLine {
        row: row.no,
        date,
        value_date,
        description: row.text(cols.description),
        reference,
        debit,
        credit,
        balance,
        raw,
    })
}

/// The amount in a cell, `None` when the cell is blank; the error is the reason text.
fn amount_from(cell: &Data, what: &str) -> Result<Option<Paise>, String> {
    let text = match cell {
        Data::Empty => return Ok(None),
        Data::String(s) => s.trim().to_owned(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::Bool(_)
        | Data::DateTime(_)
        | Data::DateTimeIso(_)
        | Data::DurationIso(_)
        | Data::Error(_) => {
            return Err(format!("{what}: the cell is not a number or text"));
        }
    };
    if text.is_empty() {
        return Ok(None);
    }
    parse_amount(&text)
        .map(Some)
        .map_err(|r| format!("{what} '{text}': {r}"))
}

/// The amount in the cell at `col` of a transaction row, `None` when the cell is blank.
fn amount_cell(row: &Row<'_>, col: usize, what: &str) -> Result<Option<Paise>, BankFmtError> {
    amount_from(row.cell(col), what).map_err(|r| row_err(row.no, r))
}

fn parse_amounts(row: &Row<'_>, amounts: &AmountColumns) -> Result<(Paise, Paise), BankFmtError> {
    let row_no = row.no;
    match amounts {
        AmountColumns::Split { debit, credit } => {
            let dr = amount_cell(row, *debit, "debit")?;
            let cr = amount_cell(row, *credit, "credit")?;
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
            let value = amount_cell(row, *amount, "amount")?
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
            let value = amount_cell(row, *amount, "amount")?
                .ok_or_else(|| row_err(row_no, "amount is empty"))?;
            if value.is_negative() {
                return Err(row_err(row_no, "negative amount with a Dr/Cr indicator"));
            }
            let marker = row.text(*indicator);
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
fn parse_date_cell(cell: &Data, dates: Dates) -> Result<Option<Date>, String> {
    match cell {
        Data::Empty => Ok(None),
        Data::String(s) => {
            let text = s.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                parse_parts(text, dates.separator, dates.format)
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
