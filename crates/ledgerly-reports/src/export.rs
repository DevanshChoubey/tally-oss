//! Report exports. **Card RP8** implements this module.
//!
//! Frozen interface (plan card RP8):
//!
//! ```text
//! pub fn to_csv(report: &Report) -> Result<String, ReportError>;
//! pub fn to_xlsx(report: &Report, path: &Path) -> Result<(), ReportError>;   // atomic write (rule D3)
//! pub fn to_json(report: &Report) -> Result<String, ReportError>;
//! ```
//!
//! Added for the app's Export (see `docs/interface-requests.md`, "Export of the report on
//! screen"): `to_csv_file`, the CSV text written atomically like `to_xlsx`.
//!
//! # Layout (CSV and XLSX)
//!
//! One header line (`Particulars` followed by the column labels), one line per report row (the
//! label is indented two spaces per depth level), a `Total` line when the report has totals, then
//! one `Note: ...` line per note. A row with more cells than the report has columns is an error;
//! a row with fewer is padded on the right with empty cells (headings carry no figures).
//!
//! # Cell conventions
//!
//! Money is never rounded on the way out, and the only float is the one the XLSX format forces:
//!
//! - [`Cell::Amount`] is written as a signed rupee amount with two decimals, **debit positive,
//!   credit negative** (`500.00` is a debit of 500 rupees, `-500.00` a credit). In XLSX the cell
//!   is a number whose format shows `500.00 Dr` or `500.00 Cr`, so the sign convention is visible
//!   and the value stays numeric.
//! - [`Cell::Signed`] is a plain signed rupee amount with two decimals (negative is a loss).
//! - [`Cell::Qty`] is the exact decimal text of the quantity.
//! - [`Cell::Date`] is ISO text (`2025-04-01`), [`Cell::Text`] is text, [`Cell::Empty`] is empty.
//! - [`Cell::Provisional`] is written as **text** (`500.00 Dr (provisional: reason)`), never as a
//!   bare number, so a figure that is not final cannot be summed by mistake.
//! - [`Cell::Unsupported`] is written as text (`UNSUPPORTED what: reason`), never blank or zero.
//!
//! An XLSX number cell is an IEEE double (a `rust_xlsxwriter` / file format limit, the recorded
//! exception to rule A3). The cell is written from the decimal text of the paise
//! (`Decimal::new(paise, 2)`) parsed to `f64`, no arithmetic is done on floats, and the number is
//! written **only if it round-trips exactly**: the double's shortest decimal text must equal the
//! original decimal. A figure that does not (an amount above 2^53 paise, a quantity with more than
//! about 15 significant digits) is a [`ReportError::Export`], never a silently rounded cell. CSV
//! carries every digit and has no such limit. CSV text cells that start with `=`, `+`, `-`,
//! `@`, tab or carriage return get a leading `'` so a spreadsheet does not run them as formulas.
//!
//! JSON is the serde form of [`Report`] (pretty printed), so it round-trips exactly, including
//! the side of every amount.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use ledgerly_core::{Decimal, DrCr, SignedAmount};
use rust_xlsxwriter::{Format, Workbook, Worksheet};

use crate::{Cell, Report, ReportError, ReportRow};

/// How a number is displayed in a spreadsheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumStyle {
    /// Debit positive, credit negative; shown with a `Dr` / `Cr` suffix in XLSX.
    Money,
    /// Two decimals, plain.
    TwoDp,
    /// Whatever the decimal text says (quantities).
    General,
}

/// One exported cell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Blank,
    Text(String),
    Number { text: String, style: NumStyle },
}

/// Exports a report as CSV text (layout and conventions in the module docs).
///
/// # Errors
/// [`ReportError::Invalid`] when a row has more cells than the report has columns,
/// [`ReportError::Money`] on overflow, [`ReportError::Export`] when the CSV writer fails.
pub fn to_csv(report: &Report) -> Result<String, ReportError> {
    let lines = grid(report)?;
    let mut writer = csv::WriterBuilder::new()
        .flexible(true)
        .from_writer(Vec::new());
    for line in &lines {
        let record: Vec<String> = line.iter().map(csv_field).collect();
        writer
            .write_record(&record)
            .map_err(|e| memory_error("csv", &e.to_string()))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| memory_error("csv", &e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| memory_error("csv", &e.to_string()))
}

/// A report's text as the CSV export writes it, for a printed copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportText {
    /// The column headings: "Particulars", then each column's label.
    pub headings: Vec<String>,
    /// The rows in display order, the total row last when the report has one. Each row has one
    /// cell per heading; an empty cell is `""`.
    pub rows: Vec<Vec<String>>,
    /// The notes the report carries.
    pub notes: Vec<String>,
}

/// The engine's text for each cell of `report`, as [`to_csv`] writes it. Nothing is formatted
/// or computed here: the figures are the ones in the report.
///
/// # Errors
/// As [`to_csv`]: [`ReportError::Invalid`] when a row has more cells than the report has columns,
/// and [`ReportError::Money`] on overflow.
pub fn report_text(report: &Report) -> Result<ReportText, ReportError> {
    let width = report.columns.len();
    let mut headings = vec!["Particulars".to_owned()];
    headings.extend(report.columns.iter().map(|column| column.label.clone()));
    let mut rows = Vec::with_capacity(report.rows.len().saturating_add(1));
    for row in &report.rows {
        rows.push(value_text(&row_line(row, width)?));
    }
    if !report.totals.is_empty() {
        rows.push(value_text(&line("Total", &report.totals, width)?));
    }
    Ok(ReportText {
        headings,
        rows,
        notes: report.notes.clone(),
    })
}

fn value_text(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .map(|value| match value {
            Value::Blank => String::new(),
            Value::Text(text) | Value::Number { text, .. } => text.clone(),
        })
        .collect()
}

/// Writes [`to_csv`]'s text to `path` atomically, as [`to_xlsx`] does (rule D3): on failure
/// `path` is untouched.
///
/// # Errors
/// The errors of [`to_csv`], and [`ReportError::Export`] when the file cannot be written.
pub fn to_csv_file(report: &Report, path: &Path) -> Result<(), ReportError> {
    let text = to_csv(report)?;
    tracing::info!(path = %path.display(), "exporting report to csv");
    write_atomic(path, text.as_bytes())
}

/// Exports a report as an XLSX workbook with one sheet, written atomically: the bytes go to a
/// temporary file in the same directory, are flushed, and the file is then renamed over `path`
/// (rule D3). On failure `path` is untouched.
///
/// # Errors
/// [`ReportError::Invalid`] for a row wider than the report, [`ReportError::Money`] on overflow,
/// [`ReportError::Export`] when the workbook cannot be built or the file cannot be written.
pub fn to_xlsx(report: &Report, path: &Path) -> Result<(), ReportError> {
    let lines = grid(report)?;
    tracing::info!(path = %path.display(), rows = lines.len(), "exporting report to xlsx");
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet
        .set_name(sheet_name(&report.title))
        .map_err(|e| export_error(path, &e.to_string()))?;
    fill_sheet(sheet, &lines).map_err(|reason| export_error(path, &reason))?;
    let bytes = workbook
        .save_to_buffer()
        .map_err(|e| export_error(path, &e.to_string()))?;
    write_atomic(path, &bytes)
}

/// Exports a report as pretty-printed JSON (the serde form of [`Report`]).
///
/// # Errors
/// [`ReportError::Export`] when serialisation fails.
pub fn to_json(report: &Report) -> Result<String, ReportError> {
    serde_json::to_string_pretty(report).map_err(|e| memory_error("json", &e.to_string()))
}

fn memory_error(format: &str, reason: &str) -> ReportError {
    ReportError::Export {
        path: format!("(in-memory {format})"),
        reason: reason.to_owned(),
    }
}

fn export_error(path: &Path, reason: &str) -> ReportError {
    ReportError::Export {
        path: path.display().to_string(),
        reason: reason.to_owned(),
    }
}

/// Builds the table shared by CSV and XLSX: header, rows, totals, notes.
fn grid(report: &Report) -> Result<Vec<Vec<Value>>, ReportError> {
    let width = report.columns.len();
    let mut lines = Vec::with_capacity(report.rows.len().saturating_add(3));
    let mut header = vec![Value::Text("Particulars".to_owned())];
    header.extend(
        report
            .columns
            .iter()
            .map(|column| Value::Text(column.label.clone())),
    );
    lines.push(header);
    for row in &report.rows {
        lines.push(row_line(row, width)?);
    }
    if !report.totals.is_empty() {
        lines.push(line("Total", &report.totals, width)?);
    }
    for note in &report.notes {
        lines.push(vec![Value::Text(format!("Note: {note}"))]);
    }
    Ok(lines)
}

fn row_line(row: &ReportRow, width: usize) -> Result<Vec<Value>, ReportError> {
    let indent = " ".repeat(usize::from(row.depth).saturating_mul(2));
    line(&format!("{indent}{}", row.label), &row.cells, width)
}

fn line(label: &str, cells: &[Cell], width: usize) -> Result<Vec<Value>, ReportError> {
    if cells.len() > width {
        return Err(ReportError::Invalid {
            field: format!("row '{}'", label.trim()),
            reason: format!(
                "has {} cells but the report has {width} columns, so the figures would not line up",
                cells.len()
            ),
        });
    }
    let mut out = Vec::with_capacity(width.saturating_add(1));
    out.push(Value::Text(label.to_owned()));
    for cell in cells {
        out.push(cell_value(cell)?);
    }
    while out.len() < width.saturating_add(1) {
        out.push(Value::Blank);
    }
    Ok(out)
}

fn cell_value(cell: &Cell) -> Result<Value, ReportError> {
    match cell {
        Cell::Amount(amount) => Ok(Value::Number {
            text: amount.to_signed()?.to_rupees().to_string(),
            style: NumStyle::Money,
        }),
        Cell::Signed(paise) => Ok(Value::Number {
            text: paise.to_rupees().to_string(),
            style: NumStyle::TwoDp,
        }),
        Cell::Qty(qty) => Ok(Value::Number {
            text: qty.0.to_string(),
            style: NumStyle::General,
        }),
        Cell::Text(text) => Ok(Value::Text(text.clone())),
        Cell::Date(date) => Ok(Value::Text(date.to_string())),
        Cell::Empty => Ok(Value::Blank),
        Cell::Provisional { amount, reason } => Ok(Value::Text(format!(
            "{} (provisional: {reason})",
            side_text(*amount)
        ))),
        Cell::Unsupported(unsupported) => Ok(Value::Text(format!(
            "UNSUPPORTED {}: {}",
            unsupported.what, unsupported.reason
        ))),
    }
}

/// `500.00 Dr` or `500.00 Cr`.
fn side_text(amount: SignedAmount) -> String {
    let side = match amount.side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    };
    format!("{} {side}", amount.amount.to_rupees())
}

fn csv_field(value: &Value) -> String {
    match value {
        Value::Blank => String::new(),
        Value::Number { text, .. } => text.clone(),
        Value::Text(text) => {
            if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                format!("'{text}")
            } else {
                text.clone()
            }
        }
    }
}

/// A valid worksheet name from a report title: no `[]:*?/\`, no edge apostrophes, at most 31
/// characters, never empty.
fn sheet_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '\'' || c.is_whitespace());
    let short: String = trimmed.chars().take(31).collect();
    let short = short.trim_matches(|c: char| c == '\'' || c.is_whitespace());
    if short.is_empty() {
        "Report".to_owned()
    } else {
        short.to_owned()
    }
}

/// Writes every cell. Errors come back as text for the caller to wrap with the path.
fn fill_sheet(sheet: &mut Worksheet, lines: &[Vec<Value>]) -> Result<(), String> {
    let money = Format::new().set_num_format("0.00\" Dr\";0.00\" Cr\"");
    let two_dp = Format::new().set_num_format("0.00");
    let bold = Format::new().set_bold();
    sheet.set_column_width(0, 40).map_err(|e| e.to_string())?;
    for (r, line) in lines.iter().enumerate() {
        let row = u32::try_from(r).map_err(|e| e.to_string())?;
        for (c, value) in line.iter().enumerate() {
            let col = u16::try_from(c).map_err(|e| e.to_string())?;
            match value {
                Value::Blank => {}
                Value::Text(text) => {
                    let written = if r == 0 {
                        sheet.write_string_with_format(row, col, text.as_str(), &bold)
                    } else {
                        sheet.write_string(row, col, text.as_str())
                    };
                    written.map_err(|e| e.to_string())?;
                }
                Value::Number { text, style } => {
                    let number = exact_f64(text)?;
                    let written = match style {
                        NumStyle::Money => sheet.write_number_with_format(row, col, number, &money),
                        NumStyle::TwoDp => {
                            sheet.write_number_with_format(row, col, number, &two_dp)
                        }
                        NumStyle::General => sheet.write_number(row, col, number),
                    };
                    written.map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

/// Parses decimal text to the `f64` a spreadsheet cell stores, and refuses when the double does
/// not stand for exactly the same decimal (the only float in this crate; see the module docs).
fn exact_f64(text: &str) -> Result<f64, String> {
    let not_exact = || {
        format!(
            "number '{text}' cannot be stored exactly in a spreadsheet cell (more than about 15 \
             significant digits); export it as CSV instead"
        )
    };
    let number: f64 = text
        .parse()
        .map_err(|e| format!("number '{text}' is not a valid number: {e}"))?;
    let original = Decimal::from_str(text)
        .map_err(|e| format!("number '{text}' is not a valid decimal: {e}"))?;
    let back = Decimal::from_str(&number.to_string()).map_err(|_| not_exact())?;
    if back == original {
        Ok(number)
    } else {
        Err(not_exact())
    }
}

/// Distinguishes temp files of exports in the same process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Temp file in the same directory, flush to disk, rename over the target.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ReportError> {
    let name = path
        .file_name()
        .ok_or_else(|| export_error(path, "the path has no file name"))?;
    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    temp_name.push(format!(".{}-{unique}.ledgerly-tmp", std::process::id()));
    let temp = path.with_file_name(temp_name);
    let result = write_then_rename(&temp, path, bytes);
    if let Err(e) = result {
        let _ = fs::remove_file(&temp);
        return Err(export_error(path, &e.to_string()));
    }
    Ok(())
}

fn write_then_rename(temp: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    sync_parent(path);
    Ok(())
}

/// Best effort: flush the directory entry so the rename survives a crash. The file is already in
/// place, so a failure here is not reported as an export failure.
#[cfg(unix)]
fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let dir = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(handle) = fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

/// No portable directory fsync off Unix.
#[cfg(not(unix))]
fn sync_parent(_path: &Path) {}
