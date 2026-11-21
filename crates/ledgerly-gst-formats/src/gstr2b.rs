//! GSTR-2B reader. **Card F3** implements this module.
//!
//! Frozen interface (plan card F3):
//!
//! ```text
//! pub struct Gstr2b { .. }   // supplier invoices, credit notes; unknown fields in `extra`
//! pub fn parse_gstr2b(json: &str) -> Result<Gstr2b, FormatError>;
//! ```
//!
//! # Shape read
//!
//! The portal publishes no formal schema for the downloaded GSTR-2B JSON (research R4 section 4).
//! This reader accepts the shape recalled from the portal download:
//!
//! ```text
//! { "data": { "gstin", "rtnprd": "MMYYYY", "version", "gendt": "dd-mm-yyyy", "chksum",
//!             "docdata": { "b2b":  [ { "ctin", "trdnm", "supprd", "supfildt",
//!                                      "inv": [ { "inum", "typ", "dt", "val", "pos", "rev",
//!                                                 "itcavl", "rsn", "diffprcnt", "irn",
//!                                                 "items": [ { "num", "rt", "txval", "igst",
//!                                                              "cgst", "sgst", "cess" } ] } ] } ],
//!                          "cdnr": [ { "ctin", .., "nt": [ { "ntnum", "typ": "C" | "D", "dt",
//!                                                            "val", .. "items": [..] } ] } ] } } }
//! ```
//!
//! That shape is **pending observed**: it has not yet been compared with a file downloaded from
//! the portal (rule A4/A5). Every field the reader does not know is kept, untouched, in an
//! `extra` map on the object that held it (rule A9), so a different real shape loses nothing and
//! is visible. Sections other than `b2b` and `cdnr` (amendments, ISD, import of goods) are kept
//! in [`Gstr2b::docdata_extra`] and are not interpreted.
//!
//! Amounts are rupees in the file and become [`Paise`]. A number with more than two decimal
//! places is an error, never rounded. Numbers are read through the decimal text `serde_json`
//! gives for them, so no floating point arithmetic is done. This crate does not enable the
//! `arbitrary_precision` feature of `serde_json`, so that text is the shortest text that
//! round-trips the `f64` the parser produced. That is exact only for numbers of up to 15
//! significant digits, so `parse_root` refuses any number with more digits before it is
//! parsed (see the `json_digits` module), with a typed error rather than a rounded value.

use std::str::FromStr;

use ledgerly_core::{MoneyError, Outcome, Paise, RoundingRule, round_rupees_to_paise};
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use time::{Date, Month};

use crate::FormatError;
use crate::json_digits::reject_long_numbers;

/// A GST return period, `MMYYYY` in the portal files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReturnPeriod {
    /// Calendar year.
    pub year: i32,
    /// Calendar month.
    pub month: Month,
}

/// What kind of inward document a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InwardDocKind {
    /// A tax invoice (section `b2b`).
    Invoice,
    /// A credit note (section `cdnr`, type `C`).
    CreditNote,
    /// A debit note (section `cdnr`, type `D`).
    DebitNote,
}

/// Whether the portal says input tax credit is available on a document (`itcavl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItcAvailability {
    /// `Y`.
    Available,
    /// `N`.
    Unavailable,
    /// Any other code, kept as written.
    Other(String),
}

/// Taxable value and the four tax heads of one item line or one document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxAmounts {
    /// Taxable value (`txval`).
    pub taxable: Paise,
    /// Integrated tax (`igst`).
    pub igst: Paise,
    /// Central tax (`cgst`).
    pub cgst: Paise,
    /// State or union territory tax (`sgst`).
    pub sgst: Paise,
    /// Cess (`cess`).
    pub cess: Paise,
}

impl TaxAmounts {
    /// All amounts zero.
    pub const ZERO: TaxAmounts = TaxAmounts {
        taxable: Paise::ZERO,
        igst: Paise::ZERO,
        cgst: Paise::ZERO,
        sgst: Paise::ZERO,
        cess: Paise::ZERO,
    };

    /// Field-wise checked sum.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when any head does not fit in paise.
    pub fn checked_add(self, other: TaxAmounts) -> Result<TaxAmounts, MoneyError> {
        Ok(TaxAmounts {
            taxable: self.taxable.checked_add(other.taxable)?,
            igst: self.igst.checked_add(other.igst)?,
            cgst: self.cgst.checked_add(other.cgst)?,
            sgst: self.sgst.checked_add(other.sgst)?,
            cess: self.cess.checked_add(other.cess)?,
        })
    }

    /// The four tax heads added together (not the taxable value).
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when the sum does not fit in paise.
    pub fn checked_total_tax(self) -> Result<Paise, MoneyError> {
        self.igst
            .checked_add(self.cgst)?
            .checked_add(self.sgst)?
            .checked_add(self.cess)
    }
}

/// One item line of an inward document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InwardItem {
    /// Line number (`num`), when present.
    pub num: Option<u64>,
    /// GST rate in percent (`rt`), when present.
    pub rate: Option<Decimal>,
    /// Taxable value and tax heads.
    pub amounts: TaxAmounts,
    /// Fields of the item the reader does not know, as written.
    pub extra: Map<String, Value>,
}

/// One inward document: a supplier invoice, credit note or debit note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InwardDoc {
    /// Invoice, credit note or debit note.
    pub kind: InwardDocKind,
    /// Document number as the supplier wrote it (`inum` or `ntnum`).
    pub number: String,
    /// Document date (`dt`).
    pub date: Date,
    /// Document value including tax (`val`).
    pub value: Paise,
    /// Invoice type code (`typ` on an invoice, for example `R`); `None` for notes and when absent.
    pub invoice_type: Option<String>,
    /// Place of supply state code as written (`pos`).
    pub place_of_supply: Option<String>,
    /// Reverse charge flag (`rev`): `Y` is true, `N` is false.
    pub reverse_charge: Option<bool>,
    /// ITC availability (`itcavl`).
    pub itc_available: Option<ItcAvailability>,
    /// Reason the ITC is not available (`rsn`).
    pub reason: Option<String>,
    /// Differential percentage (`diffprcnt`), for example `0.65`.
    pub diff_percent: Option<Decimal>,
    /// e-Invoice reference number (`irn`).
    pub irn: Option<String>,
    /// Item lines (`items`). Empty when the file has none.
    pub items: Vec<InwardItem>,
    /// Fields of the document the reader does not know, as written.
    pub extra: Map<String, Value>,
}

impl InwardDoc {
    /// Sum of the item lines.
    ///
    /// `Unsupported` when the document has no item lines: the file then carries no tax split and
    /// a zero would be a guess.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when a sum does not fit in paise.
    pub fn tax_totals(&self) -> Result<Outcome<TaxAmounts>, MoneyError> {
        if self.items.is_empty() {
            return Ok(Outcome::unsupported(
                "GSTR-2B document tax split",
                format!(
                    "document {} has no item lines, so its tax heads are not in the file",
                    self.number
                ),
            ));
        }
        let mut total = TaxAmounts::ZERO;
        for item in &self.items {
            total = total.checked_add(item.amounts)?;
        }
        Ok(Outcome::Value(total))
    }
}

/// The documents of one supplier within one section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplierGroup<D> {
    /// Supplier GSTIN as written (`ctin`); not validated, the file may carry any text.
    pub ctin: String,
    /// Supplier trade name (`trdnm`).
    pub trade_name: Option<String>,
    /// Period in which the supplier filed (`supprd`), as written.
    pub supplier_period: Option<String>,
    /// Date the supplier filed (`supfildt`), as written.
    pub supplier_filing_date: Option<String>,
    /// The supplier's documents, in file order.
    pub docs: Vec<D>,
    /// Fields of the supplier row the reader does not know, as written.
    pub extra: Map<String, Value>,
}

/// A parsed GSTR-2B statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr2b {
    /// Recipient GSTIN the statement is for (`gstin`), as written.
    pub gstin: String,
    /// Return period (`rtnprd`).
    pub return_period: ReturnPeriod,
    /// Format version (`version`).
    pub version: Option<String>,
    /// Date the statement was generated (`gendt`).
    pub generated_on: Option<Date>,
    /// Checksum text (`chksum`), kept, not verified.
    pub checksum: Option<String>,
    /// Supplier invoices (`b2b`).
    pub b2b: Vec<SupplierGroup<InwardDoc>>,
    /// Supplier credit and debit notes (`cdnr`).
    pub cdnr: Vec<SupplierGroup<InwardDoc>>,
    /// Unknown fields of the `data` object (for example `itcsumm`), as written.
    pub extra: Map<String, Value>,
    /// Sections of `docdata` other than `b2b` and `cdnr`, as written.
    pub docdata_extra: Map<String, Value>,
    /// Unknown fields next to `data`, as written.
    pub root_extra: Map<String, Value>,
}

/// Parses a downloaded GSTR-2B JSON file.
///
/// # Errors
/// [`FormatError::Json`] with the JSON path of the first problem: `$` and a line and column for
/// text that is not JSON, or a path such as `$.data.docdata.b2b[0].inv[1].val` for a missing,
/// mistyped or unrepresentable field.
pub fn parse_gstr2b(json: &str) -> Result<Gstr2b, FormatError> {
    let root_value = parse_root(json)?;
    let mut root = Obj::root(&root_value)?;
    let mut data = root.req_obj("data")?;
    let gstin = data.req_str("gstin")?;
    let return_period = data.req_period("rtnprd")?;
    let version = data.opt_str("version")?;
    let generated_on = data.opt_date("gendt")?;
    let checksum = data.opt_str_any(&["chksum", "checksum"])?;
    let mut docdata = data.req_obj("docdata")?;
    let b2b = parse_section(&mut docdata, "b2b", DocForm::Invoice, |rest, mut doc| {
        doc.extra = rest.finish();
        Ok(doc)
    })?;
    let cdnr = parse_section(&mut docdata, "cdnr", DocForm::Note, |rest, mut doc| {
        doc.extra = rest.finish();
        Ok(doc)
    })?;
    let docdata_extra = docdata.finish();
    let extra = data.finish();
    let root_extra = root.finish();
    Ok(Gstr2b {
        gstin,
        return_period,
        version,
        generated_on,
        checksum,
        b2b,
        cdnr,
        extra,
        docdata_extra,
        root_extra,
    })
}

// ---------------------------------------------------------------------------------------------
// Shared reading machinery (also used by `ims`).
// ---------------------------------------------------------------------------------------------

/// Which of the two document layouts a section uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocForm {
    /// `b2b`: `inum`, `dt`, list key `inv`.
    Invoice,
    /// `cdnr`: `ntnum`, `dt`, `typ` C or D, list key `nt`.
    Note,
}

fn json_err(path: impl Into<String>, reason: impl Into<String>) -> FormatError {
    FormatError::Json {
        path: path.into(),
        reason: reason.into(),
    }
}

/// Parses text into a JSON value; a syntax error is reported at `$` with line and column.
///
/// Also refuses any number with more than 15 significant digits: `serde_json` would hold it as an
/// `f64` and lose digits without an error (rule A3).
pub(crate) fn parse_root(json: &str) -> Result<Value, FormatError> {
    let value = serde_json::from_str::<Value>(json).map_err(|e| {
        json_err(
            "$",
            format!("{e} (line {}, column {})", e.line(), e.column()),
        )
    })?;
    reject_long_numbers(json)?;
    Ok(value)
}

/// A JSON object being read: remembers its path and which keys were consumed, so that
/// [`Obj::finish`] returns exactly the keys nobody read (rule A9).
pub(crate) struct Obj<'a> {
    map: &'a Map<String, Value>,
    path: String,
    used: Vec<&'static str>,
}

impl<'a> Obj<'a> {
    pub(crate) fn root(value: &'a Value) -> Result<Self, FormatError> {
        Self::new(value, "$".to_owned())
    }

    fn new(value: &'a Value, path: String) -> Result<Self, FormatError> {
        match value.as_object() {
            Some(map) => Ok(Obj {
                map,
                path,
                used: Vec::new(),
            }),
            None => Err(json_err(path, "expected a JSON object")),
        }
    }

    pub(crate) fn key_path(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }

    /// The value under `key`; JSON `null` counts as absent. Marks the key as read.
    fn take(&mut self, key: &'static str) -> Option<&'a Value> {
        let map: &'a Map<String, Value> = self.map;
        let value = map.get(key)?;
        self.used.push(key);
        if value.is_null() { None } else { Some(value) }
    }

    /// The first of `keys` that is present and not null, with the key used.
    fn take_any(&mut self, keys: &[&'static str]) -> Option<(&'static str, &'a Value)> {
        for &key in keys {
            if let Some(value) = self.take(key) {
                return Some((key, value));
            }
        }
        None
    }

    fn req(&mut self, key: &'static str) -> Result<&'a Value, FormatError> {
        match self.take(key) {
            Some(value) => Ok(value),
            None => Err(json_err(self.key_path(key), "required field is missing")),
        }
    }

    fn string_of(&self, key: &str, value: &Value) -> Result<String, FormatError> {
        match value.as_str() {
            Some(s) => Ok(s.to_owned()),
            None => Err(json_err(self.key_path(key), "expected a string")),
        }
    }

    pub(crate) fn req_str(&mut self, key: &'static str) -> Result<String, FormatError> {
        let value = self.req(key)?;
        self.string_of(key, value)
    }

    pub(crate) fn opt_str(&mut self, key: &'static str) -> Result<Option<String>, FormatError> {
        match self.take(key) {
            Some(value) => self.string_of(key, value).map(Some),
            None => Ok(None),
        }
    }

    fn opt_str_any(&mut self, keys: &[&'static str]) -> Result<Option<String>, FormatError> {
        match self.take_any(keys) {
            Some((key, value)) => self.string_of(key, value).map(Some),
            None => Ok(None),
        }
    }

    fn decimal_of(&self, key: &str, value: &Value) -> Result<Decimal, FormatError> {
        let Value::Number(number) = value else {
            return Err(json_err(self.key_path(key), "expected a number"));
        };
        let text = number.to_string();
        Decimal::from_str(&text)
            .or_else(|_| Decimal::from_scientific(&text))
            .map_err(|e| {
                json_err(
                    self.key_path(key),
                    format!("{text} is not a representable decimal: {e}"),
                )
            })
    }

    fn money_of(&self, key: &str, value: &Value) -> Result<Paise, FormatError> {
        let rupees = self.decimal_of(key, value)?;
        if rupees.round_dp(2) != rupees {
            return Err(json_err(
                self.key_path(key),
                format!("{rupees} has more than two decimal places; amounts are not rounded"),
            ));
        }
        round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise).map_err(|e| {
            json_err(
                self.key_path(key),
                format!("{rupees} rupees does not fit in paise: {e}"),
            )
        })
    }

    pub(crate) fn req_money(&mut self, key: &'static str) -> Result<Paise, FormatError> {
        let value = self.req(key)?;
        self.money_of(key, value)
    }

    fn opt_decimal(&mut self, key: &'static str) -> Result<Option<Decimal>, FormatError> {
        match self.take(key) {
            Some(value) => self.decimal_of(key, value).map(Some),
            None => Ok(None),
        }
    }

    fn opt_u64(&mut self, key: &'static str) -> Result<Option<u64>, FormatError> {
        match self.take(key) {
            Some(value) => match value.as_u64() {
                Some(n) => Ok(Some(n)),
                None => Err(json_err(
                    self.key_path(key),
                    "expected a non-negative whole number",
                )),
            },
            None => Ok(None),
        }
    }

    fn date_of(&self, key: &str, value: &Value) -> Result<Date, FormatError> {
        let text = self.string_of(key, value)?;
        parse_portal_date(&text).ok_or_else(|| {
            json_err(
                self.key_path(key),
                format!("'{text}' is not a date written dd-mm-yyyy"),
            )
        })
    }

    pub(crate) fn opt_date(&mut self, key: &'static str) -> Result<Option<Date>, FormatError> {
        match self.take(key) {
            Some(value) => self.date_of(key, value).map(Some),
            None => Ok(None),
        }
    }

    fn req_date_any(&mut self, keys: &[&'static str]) -> Result<Date, FormatError> {
        match self.take_any(keys) {
            Some((key, value)) => self.date_of(key, value),
            None => Err(json_err(
                self.key_path(keys.first().copied().unwrap_or("dt")),
                "required field is missing",
            )),
        }
    }

    fn req_str_any(&mut self, keys: &[&'static str]) -> Result<String, FormatError> {
        match self.take_any(keys) {
            Some((key, value)) => self.string_of(key, value),
            None => Err(json_err(
                self.key_path(keys.first().copied().unwrap_or("number")),
                "required field is missing",
            )),
        }
    }

    fn period_of(&self, key: &str, value: &Value) -> Result<ReturnPeriod, FormatError> {
        let text = self.string_of(key, value)?;
        parse_return_period(&text).ok_or_else(|| {
            json_err(
                self.key_path(key),
                format!("'{text}' is not a return period written MMYYYY"),
            )
        })
    }

    pub(crate) fn req_period(&mut self, key: &'static str) -> Result<ReturnPeriod, FormatError> {
        let value = self.req(key)?;
        self.period_of(key, value)
    }

    pub(crate) fn opt_period(
        &mut self,
        key: &'static str,
    ) -> Result<Option<ReturnPeriod>, FormatError> {
        match self.take(key) {
            Some(value) => self.period_of(key, value).map(Some),
            None => Ok(None),
        }
    }

    /// `Y` or `N` as a bool.
    fn opt_flag(&mut self, key: &'static str) -> Result<Option<bool>, FormatError> {
        let Some(value) = self.take(key) else {
            return Ok(None);
        };
        match self.string_of(key, value)?.as_str() {
            "Y" => Ok(Some(true)),
            "N" => Ok(Some(false)),
            other => Err(json_err(
                self.key_path(key),
                format!("'{other}' is not Y or N"),
            )),
        }
    }

    fn array_of(&self, key: &str, value: &'a Value) -> Result<&'a [Value], FormatError> {
        match value.as_array() {
            Some(items) => Ok(items.as_slice()),
            None => Err(json_err(self.key_path(key), "expected an array")),
        }
    }

    fn opt_array(&mut self, key: &'static str) -> Result<&'a [Value], FormatError> {
        match self.take(key) {
            Some(value) => self.array_of(key, value),
            None => Ok(&[]),
        }
    }

    fn req_array(&mut self, key: &'static str) -> Result<&'a [Value], FormatError> {
        let value = self.req(key)?;
        self.array_of(key, value)
    }

    pub(crate) fn req_obj(&mut self, key: &'static str) -> Result<Obj<'a>, FormatError> {
        let value = self.req(key)?;
        Obj::new(value, self.key_path(key))
    }

    fn element(&self, key: &str, index: usize, value: &'a Value) -> Result<Obj<'a>, FormatError> {
        Obj::new(value, format!("{}[{index}]", self.key_path(key)))
    }

    /// The keys that were never read, with their values, in file order.
    pub(crate) fn finish(self) -> Map<String, Value> {
        self.map
            .iter()
            .filter(|entry| !self.used.contains(&entry.0.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// `dd-mm-yyyy` to a date; `None` for anything else, including impossible dates.
fn parse_portal_date(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let day = parts.next()?;
    let month = parts.next()?;
    let year = parts.next()?;
    if parts.next().is_some() || day.len() != 2 || month.len() != 2 || year.len() != 4 {
        return None;
    }
    let all_digits = [day, month, year]
        .iter()
        .all(|part| part.bytes().all(|b| b.is_ascii_digit()));
    if !all_digits {
        return None;
    }
    let month = Month::try_from(month.parse::<u8>().ok()?).ok()?;
    Date::from_calendar_date(year.parse::<i32>().ok()?, month, day.parse::<u8>().ok()?).ok()
}

/// `MMYYYY` to a period; `None` for anything else.
fn parse_return_period(text: &str) -> Option<ReturnPeriod> {
    if text.len() != 6 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let month = Month::try_from(text.get(0..2)?.parse::<u8>().ok()?).ok()?;
    let year = text.get(2..6)?.parse::<i32>().ok()?;
    Some(ReturnPeriod { year, month })
}

fn parse_item(obj: &mut Obj<'_>) -> Result<InwardItem, FormatError> {
    let num = obj.opt_u64("num")?;
    let rate = obj.opt_decimal("rt")?;
    let amounts = TaxAmounts {
        taxable: obj.req_money("txval")?,
        igst: obj.req_money("igst")?,
        cgst: obj.req_money("cgst")?,
        sgst: obj.req_money("sgst")?,
        cess: obj.req_money("cess")?,
    };
    Ok(InwardItem {
        num,
        rate,
        amounts,
        extra: Map::new(),
    })
}

fn parse_items(doc: &mut Obj<'_>) -> Result<Vec<InwardItem>, FormatError> {
    let mut items = Vec::new();
    for (i, value) in doc.opt_array("items")?.iter().enumerate() {
        let mut obj = doc.element("items", i, value)?;
        let mut item = parse_item(&mut obj)?;
        item.extra = obj.finish();
        items.push(item);
    }
    Ok(items)
}

fn parse_itc_availability(code: String) -> ItcAvailability {
    match code.as_str() {
        "Y" => ItcAvailability::Available,
        "N" => ItcAvailability::Unavailable,
        _ => ItcAvailability::Other(code),
    }
}

/// Reads the document fields both files share; the caller takes the rest with
/// [`Obj::finish`]. `extra` of the result is empty until the caller sets it.
fn parse_doc(obj: &mut Obj<'_>, form: DocForm) -> Result<InwardDoc, FormatError> {
    let (kind, number, date, invoice_type) = match form {
        DocForm::Invoice => (
            InwardDocKind::Invoice,
            obj.req_str_any(&["inum"])?,
            obj.req_date_any(&["dt", "idt"])?,
            obj.opt_str("typ")?,
        ),
        DocForm::Note => {
            let number = obj.req_str_any(&["ntnum"])?;
            let date = obj.req_date_any(&["dt", "ntdt"])?;
            let code = obj.req_str("typ")?;
            let kind = match code.as_str() {
                "C" => InwardDocKind::CreditNote,
                "D" => InwardDocKind::DebitNote,
                other => {
                    return Err(json_err(
                        obj.key_path("typ"),
                        format!("'{other}' is not C (credit note) or D (debit note)"),
                    ));
                }
            };
            (kind, number, date, None)
        }
    };
    let value = obj.req_money("val")?;
    Ok(InwardDoc {
        kind,
        number,
        date,
        value,
        invoice_type,
        place_of_supply: obj.opt_str("pos")?,
        reverse_charge: obj.opt_flag("rev")?,
        itc_available: obj.opt_str("itcavl")?.map(parse_itc_availability),
        reason: obj.opt_str("rsn")?,
        diff_percent: obj.opt_decimal("diffprcnt")?,
        irn: obj.opt_str("irn")?,
        items: parse_items(obj)?,
        extra: Map::new(),
    })
}

/// Reads one section of supplier groups (`b2b` or `cdnr`) from `docdata`. A section that is not
/// in the file is empty. `finish_doc` receives the document object after the shared fields were
/// read, to take its own fields and the unknown rest.
pub(crate) fn parse_section<D>(
    docdata: &mut Obj<'_>,
    section: &'static str,
    form: DocForm,
    finish_doc: impl Fn(Obj<'_>, InwardDoc) -> Result<D, FormatError>,
) -> Result<Vec<SupplierGroup<D>>, FormatError> {
    let list_key = match form {
        DocForm::Invoice => "inv",
        DocForm::Note => "nt",
    };
    let mut groups = Vec::new();
    for (i, value) in docdata.opt_array(section)?.iter().enumerate() {
        let mut supplier = docdata.element(section, i, value)?;
        let ctin = supplier.req_str("ctin")?;
        let trade_name = supplier.opt_str("trdnm")?;
        let supplier_period = supplier.opt_str("supprd")?;
        let supplier_filing_date = supplier.opt_str("supfildt")?;
        let mut docs = Vec::new();
        for (j, doc_value) in supplier.req_array(list_key)?.iter().enumerate() {
            let mut doc_obj = supplier.element(list_key, j, doc_value)?;
            let doc = parse_doc(&mut doc_obj, form)?;
            docs.push(finish_doc(doc_obj, doc)?);
        }
        groups.push(SupplierGroup {
            ctin,
            trade_name,
            supplier_period,
            supplier_filing_date,
            docs,
            extra: supplier.finish(),
        });
    }
    Ok(groups)
}
