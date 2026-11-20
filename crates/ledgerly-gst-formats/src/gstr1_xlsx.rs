//! GSTR-1 offline tool Excel writer. **Card F2** implements this module.
//!
//! Frozen interface (plan card F2):
//!
//! ```text
//! pub fn gstr1_to_xlsx(r: &Gstr1, path: &Path) -> Result<(), FormatError>;
//! ```
//!
//! # Status: template pending
//!
//! The official GSTR-1 offline tool template is not in `templates/` yet (the owner supplies it;
//! see `templates/README.md`), and the sheet names, header rows and column order are defined by that
//! file alone. Clean-room rule: they are never recalled from memory or copied from another
//! vendor's output. Until the template exists, [`gstr1_to_xlsx`] returns an error that says so
//! instead of writing a workbook that might not import.
//!
//! What is built and tested now is everything that does not depend on the template:
//!
//! - [`write_sheets`]: writes sheets (name, header row, data rows) to an `.xlsx` file. Money is a
//!   [`Paise`] and reaches the sheet as a number with exactly two decimals, via decimal text and
//!   never a float computation. An amount that a spreadsheet double cannot hold exactly is an error.
//! - [`read_layout`]: reads sheet names and header rows from a template workbook.
//! - [`check_against_template`]: compares a written workbook's sheet names and header rows with the
//!   template's, which is the test the card names once the file arrives.
use std::collections::HashSet;
use std::path::Path;

use calamine::{Data, DataType, Reader, Xlsx, open_workbook};
use ledgerly_core::Paise;
use rust_xlsxwriter::{Format, Workbook};

use crate::FormatError;
use crate::gstr1::Gstr1;

/// Largest absolute amount, in paise, that a spreadsheet double holds exactly (15 digits).
const MAX_EXACT_PAISE: u64 = 999_999_999_999_999;

/// Name, header row and headers of one sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSpec {
    /// Sheet (tab) name.
    pub name: String,
    /// Zero-based row that holds the headers. Data rows follow it.
    pub header_row: u32,
    /// Header texts, left to right from column A.
    pub headers: Vec<String>,
}

/// One cell of a data row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// Text. Always stored as a string, never as a formula.
    Text(String),
    /// A whole number such as a count.
    Integer(u32),
    /// An amount in rupees, shown with two decimals.
    Money(Paise),
    /// No value.
    Blank,
}

/// A sheet with its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetData {
    /// Name, header row and headers.
    pub spec: SheetSpec,
    /// Data rows, written below the header row.
    pub rows: Vec<Vec<Cell>>,
}

/// Writes a GSTR-1 workbook in the official offline tool layout.
///
/// # Errors
/// Always an error until the official template is supplied: the sheet layout is defined by that
/// file and is not guessed (rule A1: a value, `Unsupported`, or an error).
pub fn gstr1_to_xlsx(r: &Gstr1, path: &Path) -> Result<(), FormatError> {
    let _ = (r, path);
    Err(FormatError::Field {
        field: "template".to_owned(),
        reason: "Unsupported: the official GSTR-1 offline tool template is not in \
                 gst-formats/templates/ yet, so sheet names and columns are unknown (card F2)"
            .to_owned(),
    })
}

fn sheet_err(context: &str, e: impl std::fmt::Display) -> FormatError {
    FormatError::Spreadsheet(format!("{context}: {e}"))
}

fn money_number(p: Paise) -> Result<f64, FormatError> {
    let abs = p.get().unsigned_abs();
    if abs > MAX_EXACT_PAISE {
        return Err(FormatError::Field {
            field: "amount".to_owned(),
            reason: format!(
                "{} paise is too large to store exactly in a spreadsheet number",
                p.get()
            ),
        });
    }
    let sign = if p.is_negative() { "-" } else { "" };
    let text = format!("{sign}{}.{:02}", abs / 100, abs % 100);
    text.parse::<f64>()
        .map_err(|e| sheet_err("amount text did not parse as a number", e))
}

fn col_index(i: usize) -> Result<u16, FormatError> {
    u16::try_from(i).map_err(|_| FormatError::Field {
        field: "column".to_owned(),
        reason: format!("column {i} is beyond the spreadsheet limit"),
    })
}

fn row_index(base: u32, i: usize) -> Result<u32, FormatError> {
    u32::try_from(i)
        .ok()
        .and_then(|n| base.checked_add(1)?.checked_add(n))
        .ok_or_else(|| FormatError::Field {
            field: "row".to_owned(),
            reason: format!("row {i} is beyond the spreadsheet limit"),
        })
}

/// Writes `sheets`, in order, to an `.xlsx` file at `path`.
///
/// Headers are written to each sheet's `header_row`, data rows below it.
///
/// # Errors
/// [`FormatError::Field`] for a duplicate sheet name, an amount a spreadsheet cannot hold exactly,
/// or a position beyond the spreadsheet limits; [`FormatError::Spreadsheet`] when the writer or the
/// file system fails (including an invalid sheet name).
pub fn write_sheets(sheets: &[SheetData], path: &Path) -> Result<(), FormatError> {
    let mut seen = HashSet::new();
    for s in sheets {
        if !seen.insert(s.spec.name.to_lowercase()) {
            return Err(FormatError::Field {
                field: "sheet name".to_owned(),
                reason: format!("duplicate sheet name {}", s.spec.name),
            });
        }
    }
    let money_format = Format::new().set_num_format("0.00");
    let mut book = Workbook::new();
    for s in sheets {
        let sheet = book.add_worksheet();
        sheet
            .set_name(s.spec.name.as_str())
            .map_err(|e| sheet_err("sheet name", e))?;
        for (c, h) in s.spec.headers.iter().enumerate() {
            sheet
                .write_string(s.spec.header_row, col_index(c)?, h.as_str())
                .map_err(|e| sheet_err("header", e))?;
        }
        for (r, row) in s.rows.iter().enumerate() {
            let rn = row_index(s.spec.header_row, r)?;
            for (c, cell) in row.iter().enumerate() {
                let cn = col_index(c)?;
                let written = match cell {
                    Cell::Text(t) => sheet.write_string(rn, cn, t.as_str()),
                    Cell::Integer(n) => sheet.write_number(rn, cn, *n),
                    Cell::Money(p) => {
                        sheet.write_number_with_format(rn, cn, money_number(*p)?, &money_format)
                    }
                    Cell::Blank => continue,
                };
                written.map_err(|e| sheet_err("cell", e))?;
            }
        }
    }
    book.save(path).map_err(|e| sheet_err("save", e))
}

/// Reads the sheet names and the header row of every sheet of the workbook at `path`.
///
/// Trailing empty header cells are dropped. A sheet shorter than `header_row` has no headers.
///
/// # Errors
/// [`FormatError::Spreadsheet`] when the file cannot be read.
pub fn read_layout(path: &Path, header_row: u32) -> Result<Vec<SheetSpec>, FormatError> {
    let mut book: Xlsx<_> = open_workbook(path).map_err(|e| sheet_err("open", e))?;
    let mut out = Vec::new();
    for name in book.sheet_names() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| sheet_err("read sheet", e))?;
        let mut headers = Vec::new();
        if let Some((_, end_col)) = range.end() {
            for col in 0..=end_col {
                let text = match range.get_value((header_row, col)) {
                    None | Some(Data::Empty) => String::new(),
                    Some(d) => d.as_string().ok_or_else(|| FormatError::Field {
                        field: name.clone(),
                        reason: format!("header cell {header_row}:{col} is not text or a number"),
                    })?,
                };
                headers.push(text);
            }
        }
        while headers.last().is_some_and(String::is_empty) {
            headers.pop();
        }
        out.push(SheetSpec {
            name,
            header_row,
            headers,
        });
    }
    Ok(out)
}

/// Checks that the workbook at `written` has the same sheet names (in order) and header rows as
/// the workbook at `template`.
///
/// # Errors
/// [`FormatError::Spreadsheet`] when a file cannot be read; [`FormatError::Field`] naming the first
/// sheet or header that differs.
pub fn check_against_template(
    written: &Path,
    template: &Path,
    header_row: u32,
) -> Result<(), FormatError> {
    let want = read_layout(template, header_row)?;
    let got = read_layout(written, header_row)?;
    if want.len() != got.len() {
        return Err(FormatError::Field {
            field: "sheets".to_owned(),
            reason: format!(
                "template has {} sheets, written file has {}",
                want.len(),
                got.len()
            ),
        });
    }
    for (w, g) in want.iter().zip(got.iter()) {
        if w.name != g.name {
            return Err(FormatError::Field {
                field: "sheet name".to_owned(),
                reason: format!("template has {:?}, written file has {:?}", w.name, g.name),
            });
        }
        if w.headers != g.headers {
            return Err(FormatError::Field {
                field: w.name.clone(),
                reason: format!(
                    "header row differs: template {:?}, written {:?}",
                    w.headers, g.headers
                ),
            });
        }
    }
    Ok(())
}
