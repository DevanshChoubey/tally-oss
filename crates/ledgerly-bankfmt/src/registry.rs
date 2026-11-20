//! Format registry. **Card BF1** implements this module.
//!
//! Frozen interface (plan card BF1):
//!
//! ```text
//! pub struct FormatSpec { .. }   pub struct Registry { .. }
//! pub fn load_registry(dir_contents: &[(String, String)]) -> Result<Registry, BankFmtError>;   // (file name, TOML)
//! ```
//!
//! A bank format is data: one TOML file per format (`formats/*.toml`). Loading is strict. An
//! unknown key, a missing key, a key that does not belong to the declared kind or Dr/Cr
//! convention, two roles on one column, or two files with one id is an error that names the file
//! and the format id. Nothing is defaulted: a CSV file must say its delimiter and how many rows to
//! skip, and every table format must say its date layout and its Dr/Cr convention.
//!
//! ```toml
//! id = "my-bank-csv"            # lower-case letters, digits, '-' and '_'
//! name = "My Bank, CSV"
//! kind = "csv"                  # "csv" | "xlsx" | "mt940"
//! delimiter = ","               # csv only, one ASCII character
//! skip_rows = 1                 # csv: required. xlsx: this or header_cells
//! # header_cells = ["Date", "Narration"]   # xlsx: header row = first row holding all of these
//! # sheet = "Statement"                     # xlsx only, optional: sheet name
//! date_format = "dd/mm/yyyy"    # dd/mm/yyyy, dd/mm/yy, yyyy/mm/dd, dd/Mon/yyyy; the separator
//!                               # may be '/', '-', '.' or ' '
//! # century_base = 2000         # required for dd/mm/yy, forbidden otherwise
//! amounts = "split"             # "split" | "signed" | "indicator" (the Dr/Cr convention)
//!
//! [columns]                     # 0-based column indexes; no two roles may share one
//! date = 0
//! value_date = 1                # optional
//! description = 2
//! reference = 3                 # optional
//! debit = 4                     # split: debit and credit
//! credit = 5
//! # amount = 4                  # signed: amount; indicator: amount and indicator
//! # indicator = 5
//! balance = 6                   # optional
//! ```
//!
//! `kind = "mt940"` takes only `id`, `name` and `kind`: the parser reads the message tags.
//!
//! Only layouts without a claim about a particular bank ship today (`generic-*`). A bank's own
//! layout enters `formats/` only with an observed sample file as provenance (rule A4).

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::BankFmtError;
use crate::csv::{AmountColumns, CsvLayout, DateFormat};

/// What kind of file a format describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    /// Delimited text.
    Csv,
    /// Excel workbook.
    Xlsx,
    /// SWIFT MT940 message text.
    Mt940,
}

/// How the first transaction row of a table is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderRows {
    /// Skip this many leading records (letterhead and header), then read transactions.
    Skip(u32),
    /// The header is the first row that holds every one of these cells (compared ignoring case
    /// and surrounding spaces); transactions start on the row after it.
    FindHeader(Vec<String>),
}

/// Column positions (0-based) of the fields that are not amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    /// Transaction date.
    pub date: usize,
    /// Value date, if the file has one.
    pub value_date: Option<usize>,
    /// Description or narration.
    pub description: usize,
    /// Cheque or reference number, if any.
    pub reference: Option<usize>,
    /// Running balance, if any (credit positive).
    pub balance: Option<usize>,
}

/// The table layout of a CSV or XLSX format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSpec {
    /// Field delimiter. `Some` for CSV, `None` for XLSX.
    pub delimiter: Option<u8>,
    /// How the first transaction row is found.
    pub header: HeaderRows,
    /// Worksheet name for XLSX; `None` means the first sheet. Always `None` for CSV.
    pub sheet: Option<String>,
    /// Date layout of the date and value date columns.
    pub date_format: DateFormat,
    /// Separator between the date parts.
    pub date_separator: char,
    /// Non-amount columns.
    pub columns: Columns,
    /// Dr/Cr convention and the amount columns.
    pub amounts: AmountColumns,
}

/// One bank statement format, as described by a `formats/*.toml` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatSpec {
    /// Registry id, copied to `RawStatement::format_id`.
    pub id: String,
    /// Human name shown in the import picker.
    pub name: String,
    /// File kind.
    pub kind: FileKind,
    /// Table layout; `Some` for CSV and XLSX, `None` for MT940.
    pub table: Option<TableSpec>,
}

impl FormatSpec {
    /// The layout the CSV parser reads (`parse_csv_with_layout`).
    ///
    /// # Errors
    /// [`BankFmtError::Format`] when this is not a CSV format. A spec built by [`load_registry`]
    /// never fails this check for a `kind = "csv"` format; the fields are public, so it is
    /// verified again here.
    pub fn csv_layout(&self) -> Result<CsvLayout, BankFmtError> {
        let fail = |reason: &str| BankFmtError::Format {
            reason: format!("format '{}': {reason}", self.id),
        };
        if self.kind != FileKind::Csv {
            return Err(fail("is not a CSV format"));
        }
        let table = self
            .table
            .as_ref()
            .ok_or_else(|| fail("has no table layout"))?;
        let delimiter = table.delimiter.ok_or_else(|| fail("has no delimiter"))?;
        let HeaderRows::Skip(skip_rows) = &table.header else {
            return Err(fail(
                "finds its header by cell text, which CSV does not support",
            ));
        };
        Ok(CsvLayout {
            format_id: self.id.clone(),
            delimiter,
            skip_rows: *skip_rows,
            date_col: table.columns.date,
            value_date_col: table.columns.value_date,
            description_col: table.columns.description,
            reference_col: table.columns.reference,
            amounts: table.amounts.clone(),
            balance_col: table.columns.balance,
            date_format: table.date_format,
            date_separator: table.date_separator,
        })
    }
}

/// A set of loaded formats, ordered by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    specs: BTreeMap<String, FormatSpec>,
}

impl Registry {
    /// The format with this id.
    pub fn get(&self, id: &str) -> Option<&FormatSpec> {
        self.specs.get(id)
    }

    /// All formats, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &FormatSpec> {
        self.specs.values()
    }

    /// Number of formats.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// True when no format is loaded.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// Load a registry from `(file name, TOML text)` pairs, for example the contents of `formats/`.
///
/// Every file name must end in `.toml`; filter other files (such as `README.md`) out before the
/// call. Files are read in the order given and the first error is returned.
///
/// # Errors
/// [`BankFmtError::Registry`] naming the format id (or the file name when the file does not parse
/// far enough to have one) for: a name not ending in `.toml`, TOML syntax errors, unknown or
/// missing keys, keys that do not fit the kind or Dr/Cr convention, bad values, two roles on one
/// column, and two files with the same id.
pub fn load_registry(dir_contents: &[(String, String)]) -> Result<Registry, BankFmtError> {
    let mut specs: BTreeMap<String, FormatSpec> = BTreeMap::new();
    let mut origin: BTreeMap<String, &str> = BTreeMap::new();
    for (file, text) in dir_contents {
        let spec = load_one(file, text)?;
        if let Some(first) = origin.get(&spec.id) {
            return Err(BankFmtError::Registry {
                id: spec.id,
                reason: format!("defined in both '{first}' and '{file}'"),
            });
        }
        origin.insert(spec.id.clone(), file.as_str());
        specs.insert(spec.id.clone(), spec);
    }
    Ok(Registry { specs })
}

/// The formats shipped in `formats/`, compiled into the library.
///
/// # Errors
/// [`BankFmtError::Registry`] if a shipped file is invalid (a test guards against it).
pub fn builtin_registry() -> Result<Registry, BankFmtError> {
    let files: [(&str, &str); 5] = [
        (
            "generic-csv-indicator.toml",
            include_str!("../formats/generic-csv-indicator.toml"),
        ),
        (
            "generic-csv-signed.toml",
            include_str!("../formats/generic-csv-signed.toml"),
        ),
        (
            "generic-csv-split.toml",
            include_str!("../formats/generic-csv-split.toml"),
        ),
        (
            "generic-mt940.toml",
            include_str!("../formats/generic-mt940.toml"),
        ),
        (
            "generic-xlsx-split.toml",
            include_str!("../formats/generic-xlsx-split.toml"),
        ),
    ];
    let owned: Vec<(String, String)> = files
        .iter()
        .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
        .collect();
    load_registry(&owned)
}

/// The Dr/Cr convention of a table format.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Convention {
    Split,
    Signed,
    Indicator,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColumns {
    date: usize,
    value_date: Option<usize>,
    description: usize,
    reference: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
    amount: Option<usize>,
    indicator: Option<usize>,
    balance: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpec {
    id: String,
    name: String,
    kind: FileKind,
    delimiter: Option<String>,
    skip_rows: Option<u32>,
    header_cells: Option<Vec<String>>,
    sheet: Option<String>,
    date_format: Option<String>,
    century_base: Option<i32>,
    amounts: Option<Convention>,
    columns: Option<RawColumns>,
}

fn load_one(file: &str, text: &str) -> Result<FormatSpec, BankFmtError> {
    let file_err = |reason: String| BankFmtError::Registry {
        id: file.to_owned(),
        reason,
    };
    if !file.ends_with(".toml") {
        return Err(file_err("file name does not end in .toml".to_owned()));
    }
    let raw: RawSpec = toml::from_str(text).map_err(|e| file_err(format!("{file}: {e}")))?;
    let id = raw.id.clone();
    build(&raw).map_err(|reason| BankFmtError::Registry {
        id,
        reason: format!("{file}: {reason}"),
    })
}

fn build(raw: &RawSpec) -> Result<FormatSpec, String> {
    if raw.id.is_empty()
        || !raw
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(
            "id must be non-empty and use only lower-case letters, digits, '-' and '_'".to_owned(),
        );
    }
    if raw.name.trim().is_empty() {
        return Err("name must not be empty".to_owned());
    }
    let table = match raw.kind {
        FileKind::Mt940 => {
            reject_table_keys(raw)?;
            None
        }
        FileKind::Csv | FileKind::Xlsx => Some(build_table(raw)?),
    };
    Ok(FormatSpec {
        id: raw.id.clone(),
        name: raw.name.clone(),
        kind: raw.kind,
        table,
    })
}

fn reject_table_keys(raw: &RawSpec) -> Result<(), String> {
    let present: Vec<&str> = [
        ("delimiter", raw.delimiter.is_some()),
        ("skip_rows", raw.skip_rows.is_some()),
        ("header_cells", raw.header_cells.is_some()),
        ("sheet", raw.sheet.is_some()),
        ("date_format", raw.date_format.is_some()),
        ("century_base", raw.century_base.is_some()),
        ("amounts", raw.amounts.is_some()),
        ("columns", raw.columns.is_some()),
    ]
    .iter()
    .filter(|(_, set)| *set)
    .map(|(key, _)| *key)
    .collect();
    if present.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "kind mt940 takes no table keys, but found: {}",
            present.join(", ")
        ))
    }
}

fn build_table(raw: &RawSpec) -> Result<TableSpec, String> {
    let is_csv = raw.kind == FileKind::Csv;
    let delimiter = build_delimiter(raw, is_csv)?;
    let header = build_header(raw, is_csv)?;
    let sheet = match (&raw.sheet, is_csv) {
        (Some(_), true) => return Err("'sheet' applies to xlsx only".to_owned()),
        (Some(name), false) if name.trim().is_empty() => {
            return Err("'sheet' must not be empty".to_owned());
        }
        (sheet, _) => sheet.clone(),
    };
    let pattern = raw
        .date_format
        .as_deref()
        .ok_or_else(|| "missing 'date_format'".to_owned())?;
    let (date_format, date_separator) = build_date(pattern, raw.century_base)?;
    let columns = raw
        .columns
        .as_ref()
        .ok_or_else(|| "missing [columns] table".to_owned())?;
    let convention = raw
        .amounts
        .ok_or_else(|| "missing 'amounts' (split, signed or indicator)".to_owned())?;
    let amounts = build_amounts(convention, columns)?;
    check_distinct_columns(columns)?;
    Ok(TableSpec {
        delimiter,
        header,
        sheet,
        date_format,
        date_separator,
        columns: Columns {
            date: columns.date,
            value_date: columns.value_date,
            description: columns.description,
            reference: columns.reference,
            balance: columns.balance,
        },
        amounts,
    })
}

fn build_delimiter(raw: &RawSpec, is_csv: bool) -> Result<Option<u8>, String> {
    match (&raw.delimiter, is_csv) {
        (None, true) => Err("missing 'delimiter' (csv must name its delimiter)".to_owned()),
        (Some(_), false) => Err("'delimiter' applies to csv only".to_owned()),
        (None, false) => Ok(None),
        (Some(text), true) => {
            let mut chars = text.chars();
            let byte = match (chars.next(), chars.next()) {
                (Some(c), None) => u8::try_from(c).ok(),
                (None, _) | (Some(_), Some(_)) => None,
            };
            match byte {
                Some(b) if b.is_ascii() && !matches!(b, b'"' | b'\n' | b'\r') => Ok(Some(b)),
                Some(_) | None => Err(format!(
                    "'delimiter' must be one ASCII character other than a quote or a line break, got '{text}'"
                )),
            }
        }
    }
}

fn build_header(raw: &RawSpec, is_csv: bool) -> Result<HeaderRows, String> {
    match (raw.skip_rows, &raw.header_cells, is_csv) {
        (Some(n), None, _) => Ok(HeaderRows::Skip(n)),
        (None, Some(_), true) => Err("'header_cells' applies to xlsx only".to_owned()),
        (None, Some(cells), false) => {
            if cells.is_empty() || cells.iter().any(|c| c.trim().is_empty()) {
                Err("'header_cells' must be a non-empty list of non-empty cell texts".to_owned())
            } else {
                Ok(HeaderRows::FindHeader(cells.clone()))
            }
        }
        (Some(_), Some(_), _) => {
            Err("give either 'skip_rows' or 'header_cells', not both".to_owned())
        }
        (None, None, true) => {
            Err("missing 'skip_rows' (csv must say how many rows to skip)".to_owned())
        }
        (None, None, false) => Err("missing 'skip_rows' or 'header_cells'".to_owned()),
    }
}

fn build_date(pattern: &str, century_base: Option<i32>) -> Result<(DateFormat, char), String> {
    let bad = || {
        format!(
            "'date_format' '{pattern}' is not one of dd/mm/yyyy, dd/mm/yy, yyyy/mm/dd, dd/Mon/yyyy \
             (separator '/', '-', '.' or ' ')"
        )
    };
    let separator = pattern
        .chars()
        .find(|c| !c.is_ascii_alphabetic())
        .filter(|c| matches!(*c, '/' | '-' | '.' | ' '))
        .ok_or_else(bad)?;
    let parts: Vec<&str> = pattern.split(separator).collect();
    let format = match parts.as_slice() {
        ["dd", "mm", "yyyy"] => DateFormat::DdMmYyyy,
        ["dd", "mm", "yy"] => DateFormat::DdMmYy { century_base: 0 },
        ["yyyy", "mm", "dd"] => DateFormat::YyyyMmDd,
        ["dd", "Mon", "yyyy"] => DateFormat::DdMonYyyy,
        _ => return Err(bad()),
    };
    let format = match (format, century_base) {
        (DateFormat::DdMmYy { .. }, Some(base)) => {
            if !(1900..=2100).contains(&base) || base.rem_euclid(100) != 0 {
                return Err(format!(
                    "'century_base' must be a multiple of 100 from 1900 to 2100, got {base}"
                ));
            }
            DateFormat::DdMmYy { century_base: base }
        }
        (DateFormat::DdMmYy { .. }, None) => {
            return Err("missing 'century_base' (two-digit years need one, e.g. 2000)".to_owned());
        }
        (DateFormat::DdMmYyyy, None) => DateFormat::DdMmYyyy,
        (DateFormat::YyyyMmDd, None) => DateFormat::YyyyMmDd,
        (DateFormat::DdMonYyyy, None) => DateFormat::DdMonYyyy,
        (DateFormat::DdMmYyyy | DateFormat::YyyyMmDd | DateFormat::DdMonYyyy, Some(_)) => {
            return Err("'century_base' applies only to a two-digit-year date_format".to_owned());
        }
    };
    Ok((format, separator))
}

fn need(value: Option<usize>, key: &str, convention: &str) -> Result<usize, String> {
    value.ok_or_else(|| format!("amounts = \"{convention}\" needs columns.{key}"))
}

fn forbid(value: Option<usize>, key: &str, convention: &str) -> Result<(), String> {
    match value {
        None => Ok(()),
        Some(_) => Err(format!(
            "amounts = \"{convention}\" does not use columns.{key}"
        )),
    }
}

fn build_amounts(convention: Convention, c: &RawColumns) -> Result<AmountColumns, String> {
    match convention {
        Convention::Split => {
            forbid(c.amount, "amount", "split")?;
            forbid(c.indicator, "indicator", "split")?;
            Ok(AmountColumns::Split {
                debit: need(c.debit, "debit", "split")?,
                credit: need(c.credit, "credit", "split")?,
            })
        }
        Convention::Signed => {
            forbid(c.debit, "debit", "signed")?;
            forbid(c.credit, "credit", "signed")?;
            forbid(c.indicator, "indicator", "signed")?;
            Ok(AmountColumns::Signed {
                amount: need(c.amount, "amount", "signed")?,
            })
        }
        Convention::Indicator => {
            forbid(c.debit, "debit", "indicator")?;
            forbid(c.credit, "credit", "indicator")?;
            Ok(AmountColumns::WithIndicator {
                amount: need(c.amount, "amount", "indicator")?,
                indicator: need(c.indicator, "indicator", "indicator")?,
            })
        }
    }
}

fn check_distinct_columns(c: &RawColumns) -> Result<(), String> {
    let roles: [(&str, Option<usize>); 9] = [
        ("date", Some(c.date)),
        ("value_date", c.value_date),
        ("description", Some(c.description)),
        ("reference", c.reference),
        ("debit", c.debit),
        ("credit", c.credit),
        ("amount", c.amount),
        ("indicator", c.indicator),
        ("balance", c.balance),
    ];
    let mut seen: BTreeMap<usize, &str> = BTreeMap::new();
    for (role, index) in roles
        .into_iter()
        .filter_map(|(role, index)| index.map(|i| (role, i)))
    {
        if let Some(prev) = seen.insert(index, role) {
            return Err(format!(
                "columns.{prev} and columns.{role} both use column {index}"
            ));
        }
    }
    Ok(())
}
