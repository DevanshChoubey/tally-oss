//! Writing Tally-importable XML. **Card X4** implements this module.
//!
//! Frozen interface (plan card X4):
//!
//! ```text
//! pub fn write_masters(m: &[TallyMaster]) -> String;
//! pub fn write_vouchers(v: &[TVoucher]) -> String;
//! ```
//!
//! Added beside the frozen pair (see the interface request filed with this card):
//! [`try_write_masters`] and [`try_write_vouchers`] return the same text or the first
//! [`WriteError`] found, so a caller that must not hand a CA a file that does not read back the
//! same (spec 6.7, "exit path") can refuse instead.
//!
//! # Document shape
//!
//! The shape is the "Import Data" form of research note R8 section 2.3 (d) and (e) (vendor
//! shape, `docs/research/08-data-interop-editions.md`): `ENVELOPE/HEADER/TALLYREQUEST`, then
//! `BODY/IMPORTDATA/REQUESTDESC/REPORTNAME` (`All Masters` or `Vouchers`), then
//! `REQUESTDATA/TALLYMESSAGE` holding every object. No company is named (the frozen signatures
//! take none), so Tally imports into the company that is open. The text has no XML declaration;
//! the caller picks the encoding of the bytes. Each master or voucher is followed by one newline;
//! nothing else is indented, so leaf text is exact.
//!
//! Every document is read back by this crate's own parsers
//! ([`crate::masters::parse_masters_text`], [`crate::vouchers::parse_vouchers_xml`]) to the value
//! that was written (the round-trip property tested in `tests/x4_writer.rs`). Whether Tally
//! itself accepts a document is `pending`: no capture from a Tally release backs the shape.
//!
//! Masters: the element carries the record's attributes verbatim, then `GUID`, `ALTERID` (when
//! known), `NAME`, `PARENT` (when set) and the unmodelled children.
//!
//! Vouchers: the element carries the voucher's attributes verbatim, then `GUID`, `ALTERID`,
//! `DATE` (`YYYYMMDD`), `VOUCHERTYPENAME`, `VOUCHERNUMBER`, `PARTYLEDGERNAME`, `NARRATION`, the
//! ledger lines (`LEDGERENTRIES.LIST`), the inventory lines (`ALLINVENTORYENTRIES.LIST`) and the
//! unmodelled children. A debit line is written with a negative `AMOUNT` and
//! `ISDEEMEDPOSITIVE` = `Yes`, a credit line with a positive `AMOUNT` and `No` (R8 2.3 (e)).
//! Bill, inventory and batch amounts are written with the sign they hold.
//!
//! # Input the writer cannot write faithfully
//!
//! The `try_` functions report the first of these. The frozen infallible functions still return a
//! document, rendered as follows, so nothing is dropped without the `try_` pair being able to say
//! so:
//!
//! - A character XML 1.0 forbids becomes a numeric reference (`&#4;`), the form Tally itself
//!   writes; this crate's readers drop it and warn.
//! - A name that is not a valid XML name has its bad characters replaced by `_`.
//! - A ledger line whose `ISDEEMEDPOSITIVE` contradicts its side, or a zero-amount line without
//!   the flag, is written with the flag the side implies. A negative magnitude is written as its
//!   absolute value.
//! - A `NAME` attribute that differs from the master's name, or a `VCHTYPE` attribute that differs
//!   from the voucher type, is written with the typed value.
//! - Unmodelled children nested deeper than 100 levels are left out.
//! - Every other problem (empty required text, text the reader trims, repeated attributes,
//!   unmodelled children that collide with modelled tags, a unit the reader cannot split, a year
//!   outside 0 to 9999) is written as given and will not read back identically.
//!
//! An optional text field that is `Some("")` is left out, because the readers treat an empty
//! element as absent.

use ledgerly_core::{Date, DrCr, Paise};
use thiserror::Error;

use crate::masters::{MasterRecord, RawElement, TallyMaster};
use crate::vouchers::{
    RawNode, TBatchAllocation, TBillAllocation, TInventoryEntry, TLedgerEntry, TQty, TRate,
    TVoucher,
};

/// Deepest nesting of unmodelled children that is written; deeper ones are left out. The readers
/// refuse documents nested beyond 128 levels, and the envelope and object use a few of them.
const MAX_EXTRA_DEPTH: usize = 100;

/// Tags a master reads into typed fields; an unmodelled child with one of these would be misread.
const MASTER_FIELDS: &[&str] = &["GUID", "ALTERID", "NAME", "PARENT"];
/// Tags a voucher reads into typed fields or lines.
const VOUCHER_FIELDS: &[&str] = &[
    "GUID",
    "ALTERID",
    "VOUCHERTYPENAME",
    "DATE",
    "VOUCHERNUMBER",
    "PARTYLEDGERNAME",
    "NARRATION",
    "ALLLEDGERENTRIES.LIST",
    "LEDGERENTRIES.LIST",
    "INVENTORYENTRIES.LIST",
    "ALLINVENTORYENTRIES.LIST",
];
/// Tags a ledger line reads.
const LEDGER_LINE_FIELDS: &[&str] = &[
    "LEDGERNAME",
    "ISDEEMEDPOSITIVE",
    "AMOUNT",
    "BILLALLOCATIONS.LIST",
];
/// Tags a bill allocation reads.
const BILL_FIELDS: &[&str] = &["NAME", "BILLTYPE", "AMOUNT"];
/// Tags an inventory line reads.
const INVENTORY_FIELDS: &[&str] = &[
    "STOCKITEMNAME",
    "ISDEEMEDPOSITIVE",
    "RATE",
    "BILLEDQTY",
    "ACTUALQTY",
    "AMOUNT",
    "BATCHALLOCATIONS.LIST",
];
/// Tags a batch allocation reads.
const BATCH_FIELDS: &[&str] = &[
    "GODOWNNAME",
    "BATCHNAME",
    "AMOUNT",
    "BILLEDQTY",
    "ACTUALQTY",
];

/// Why a document cannot be written so that it reads back identically.
///
/// Every variant names the object (`context`) and the tag, so the user can find and fix the value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
    /// A value holds a character XML 1.0 cannot carry. Remove it from the value.
    #[error("{context}: <{tag}> holds U+{code_point:04X}, which XML 1.0 cannot carry; remove it")]
    InvalidCharacter {
        /// The object being written.
        context: String,
        /// The tag or attribute holding the character.
        tag: String,
        /// The character's code point.
        code_point: u32,
    },
    /// A tag or attribute name is not a valid XML name.
    #[error("{context}: '{name}' is not a valid XML name; rename it")]
    InvalidName {
        /// The object being written.
        context: String,
        /// The name as given.
        name: String,
    },
    /// A required value is empty.
    #[error("{context}: <{tag}> is required but empty; fill it in")]
    Empty {
        /// The object being written.
        context: String,
        /// The tag.
        tag: String,
    },
    /// A value has leading or trailing whitespace that the reader would trim.
    #[error("{context}: <{tag}> has surrounding whitespace, which is lost on reading; trim it")]
    SurroundingWhitespace {
        /// The object being written.
        context: String,
        /// The tag.
        tag: String,
    },
    /// Two fields contradict each other, or a field has a form the reader cannot take back.
    #[error("{context}: {reason}")]
    Inconsistent {
        /// The object being written.
        context: String,
        /// What is wrong and what to change.
        reason: String,
    },
}

/// Writes masters as an "All Masters" import document.
///
/// **Lossy: not for export.** Input that cannot be written faithfully is rendered without an error
/// (see the module doc), so the text may not read back to the same masters. Every export and CA
/// hand-off must use [`try_write_masters`], which refuses such input. This function stays only
/// because its signature is frozen; an interface request asks for it to become fallible.
pub fn write_masters(m: &[TallyMaster]) -> String {
    build_masters(m).0
}

/// Writes vouchers as a "Vouchers" import document.
///
/// **Lossy: not for export.** A negative ledger magnitude is written as its absolute value, and a
/// character XML forbids is written as a numeric reference that is not well-formed XML 1.0. Every
/// export and CA hand-off must use [`try_write_vouchers`], which refuses such input. This function
/// stays only because its signature is frozen; an interface request asks for it to become fallible.
pub fn write_vouchers(v: &[TVoucher]) -> String {
    build_vouchers(v).0
}

/// Like [`write_masters`], but refuses input that would not read back identically.
///
/// # Errors
///
/// The first [`WriteError`] in document order.
pub fn try_write_masters(m: &[TallyMaster]) -> Result<String, WriteError> {
    let (text, issue) = build_masters(m);
    match issue {
        Some(error) => Err(error),
        None => Ok(text),
    }
}

/// Like [`write_vouchers`], but refuses input that would not read back identically.
///
/// # Errors
///
/// The first [`WriteError`] in document order.
pub fn try_write_vouchers(v: &[TVoucher]) -> Result<String, WriteError> {
    let (text, issue) = build_vouchers(v);
    match issue {
        Some(error) => Err(error),
        None => Ok(text),
    }
}

fn build_masters(masters: &[TallyMaster]) -> (String, Option<WriteError>) {
    // Masters keep leaf text exactly as written, so nothing is trimmed on reading.
    let mut e = Emitter::new(false);
    e.open_document("All Masters");
    for master in masters {
        let (tag, record) = master_parts(master);
        write_master_record(&mut e, tag, record);
        e.raw("\n");
    }
    e.close_document();
    (e.out, e.issue)
}

fn build_vouchers(vouchers: &[TVoucher]) -> (String, Option<WriteError>) {
    // The voucher reader trims every text node.
    let mut e = Emitter::new(true);
    e.open_document("Vouchers");
    for voucher in vouchers {
        write_voucher(&mut e, voucher);
        e.raw("\n");
    }
    e.close_document();
    (e.out, e.issue)
}

// ---------------------------------------------------------------------------------------------
// Characters and names
// ---------------------------------------------------------------------------------------------

/// The characters XML 1.0 allows in a document (production `Char`).
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == ':'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-')
}

fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(is_name_start) && chars.all(is_name_char)
}

/// `name` with every character a name cannot hold replaced by `_`; never empty.
fn sanitized_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let ok = if i == 0 {
                is_name_start(c)
            } else {
                is_name_char(c)
            };
            if ok { c } else { '_' }
        })
        .collect();
    if cleaned.is_empty() {
        String::from("_")
    } else {
        cleaned
    }
}

/// Appends `value` escaped for element text, or for a double-quoted attribute when `attribute`.
/// A character XML forbids becomes a numeric reference; the first such character is returned.
fn escape_into(out: &mut String, value: &str, attribute: bool) -> Option<char> {
    let mut forbidden = None;
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            '\t' if attribute => out.push_str("&#9;"),
            '\n' if attribute => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ if is_xml_char(c) => out.push(c),
            _ => {
                out.push_str("&#");
                out.push_str(&u32::from(c).to_string());
                out.push(';');
                if forbidden.is_none() {
                    forbidden = Some(c);
                }
            }
        }
    }
    forbidden
}

// ---------------------------------------------------------------------------------------------
// Number formats
// ---------------------------------------------------------------------------------------------

/// Rupees with two decimals from a sign and a magnitude in paise; zero never gets a sign.
fn format_paise(negative: bool, magnitude: u64) -> String {
    let sign = if negative && magnitude != 0 { "-" } else { "" };
    let rupees = magnitude / 100;
    let paise = magnitude % 100;
    format!("{sign}{rupees}.{paise:02}")
}

/// A signed amount exactly as it is held.
fn format_signed(amount: Paise) -> String {
    format_paise(amount.is_negative(), amount.get().unsigned_abs())
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "Yes" } else { "No" }
}

// ---------------------------------------------------------------------------------------------
// Emitter
// ---------------------------------------------------------------------------------------------

/// Builds the document text and remembers the first problem met.
struct Emitter {
    out: String,
    issue: Option<WriteError>,
    /// Names the object being written, for error messages.
    context: String,
    /// Whether the reader of this document trims text (vouchers) or keeps it exactly (masters).
    trims: bool,
}

impl Emitter {
    fn new(trims: bool) -> Self {
        Self {
            out: String::new(),
            issue: None,
            context: String::from("document"),
            trims,
        }
    }

    fn raw(&mut self, text: &str) {
        self.out.push_str(text);
    }

    fn open_document(&mut self, report: &str) {
        self.raw(
            "<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY>\
             <IMPORTDATA><REQUESTDESC><REPORTNAME>",
        );
        self.raw(report);
        self.raw(
            "</REPORTNAME></REQUESTDESC><REQUESTDATA>\
             <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n",
        );
    }

    fn close_document(&mut self) {
        self.raw("</TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>\n");
    }

    fn set_context(&mut self, context: String) {
        self.context = context;
    }

    fn note(&mut self, error: WriteError) {
        if self.issue.is_none() {
            self.issue = Some(error);
        }
    }

    fn inconsistent(&mut self, reason: impl Into<String>) {
        let error = WriteError::Inconsistent {
            context: self.context.clone(),
            reason: reason.into(),
        };
        self.note(error);
    }

    fn invalid_char(&mut self, tag: &str, c: char) {
        let error = WriteError::InvalidCharacter {
            context: self.context.clone(),
            tag: tag.to_owned(),
            code_point: u32::from(c),
        };
        self.note(error);
    }

    /// `name` if it is a valid XML name, otherwise its sanitized form (and a noted problem).
    fn name(&mut self, name: &str) -> String {
        if is_valid_name(name) {
            return name.to_owned();
        }
        let error = WriteError::InvalidName {
            context: self.context.clone(),
            name: name.to_owned(),
        };
        self.note(error);
        sanitized_name(name)
    }

    fn open(&mut self, tag: &str, attributes: &[(String, String)]) {
        let tag = self.name(tag);
        self.out.push('<');
        self.out.push_str(&tag);
        let mut seen: Vec<&str> = Vec::new();
        for (key, value) in attributes {
            if seen.contains(&key.as_str()) {
                self.inconsistent(format!("attribute '{key}' is repeated on <{tag}>"));
            }
            seen.push(key.as_str());
            let key = self.name(key);
            self.out.push(' ');
            self.out.push_str(&key);
            self.out.push_str("=\"");
            let forbidden = escape_into(&mut self.out, value, true);
            self.out.push('"');
            if let Some(c) = forbidden {
                self.invalid_char(&key, c);
            }
        }
        self.out.push('>');
    }

    fn close(&mut self, tag: &str) {
        let tag = self.name(tag);
        self.out.push_str("</");
        self.out.push_str(&tag);
        self.out.push('>');
    }

    /// Element text, escaped; notes text the reader would trim when this document's reader trims.
    fn text(&mut self, tag: &str, value: &str) {
        if self.trims && value.trim() != value {
            let error = WriteError::SurroundingWhitespace {
                context: self.context.clone(),
                tag: tag.to_owned(),
            };
            self.note(error);
        }
        if let Some(c) = escape_into(&mut self.out, value, false) {
            self.invalid_char(tag, c);
        }
    }

    fn leaf(&mut self, tag: &str, value: &str) {
        self.open(tag, &[]);
        self.text(tag, value);
        self.close(tag);
    }

    /// A leaf whose text must not be empty (after trimming, when the reader trims).
    fn required_leaf(&mut self, tag: &str, value: &str) {
        let content = if self.trims { value.trim() } else { value };
        if content.is_empty() {
            let error = WriteError::Empty {
                context: self.context.clone(),
                tag: tag.to_owned(),
            };
            self.note(error);
        }
        self.leaf(tag, value);
    }

    /// A leaf that is left out when absent or empty.
    fn optional_leaf(&mut self, tag: &str, value: Option<&str>) {
        if let Some(text) = value.filter(|v| !v.is_empty()) {
            self.leaf(tag, text);
        }
    }

    fn amount_leaf(&mut self, tag: &str, amount: Paise) {
        self.leaf(tag, &format_signed(amount));
    }

    fn qty_leaf(&mut self, tag: &str, quantity: &TQty) {
        let unit = quantity.unit.as_str();
        if unit.contains('=') {
            self.inconsistent(format!(
                "<{tag}> unit '{unit}' contains '=', which the reader takes for a compound quantity"
            ));
        }
        if unit.trim() != unit {
            self.inconsistent(format!("<{tag}> unit '{unit}' has surrounding whitespace"));
        }
        let number = quantity.qty.0;
        let text = if unit.is_empty() {
            number.to_string()
        } else {
            format!("{number} {unit}")
        };
        self.leaf(tag, &text);
    }

    fn rate_leaf(&mut self, tag: &str, rate: &TRate) {
        let unit = rate.unit.as_str();
        if unit.contains('/') {
            self.inconsistent(format!(
                "<{tag}> unit '{unit}' contains '/', which the reader takes for the unit separator"
            ));
        }
        if unit.trim() != unit {
            self.inconsistent(format!("<{tag}> unit '{unit}' has surrounding whitespace"));
        }
        let number = rate.rate.0;
        let text = if unit.is_empty() {
            number.to_string()
        } else {
            format!("{number}/{unit}")
        };
        self.leaf(tag, &text);
    }

    /// Notes unmodelled children whose tag the reader would take for a modelled field.
    fn reject_reserved<'a>(&mut self, names: impl Iterator<Item = &'a str>, reserved: &[&str]) {
        for name in names {
            if reserved.contains(&name) {
                self.inconsistent(format!(
                    "an unmodelled <{name}> would be read back as the modelled field of that name"
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Masters
// ---------------------------------------------------------------------------------------------

fn master_parts(master: &TallyMaster) -> (&'static str, &MasterRecord) {
    match master {
        TallyMaster::Group(r) => ("GROUP", r),
        TallyMaster::Ledger(r) => ("LEDGER", r),
        TallyMaster::StockItem(r) => ("STOCKITEM", r),
        TallyMaster::Unit(r) => ("UNIT", r),
        TallyMaster::Godown(r) => ("GODOWN", r),
        TallyMaster::VoucherType(r) => ("VOUCHERTYPE", r),
        TallyMaster::CostCentre(r) => ("COSTCENTRE", r),
        TallyMaster::CostCategory(r) => ("COSTCATEGORY", r),
        TallyMaster::StockGroup(r) => ("STOCKGROUP", r),
    }
}

fn write_master_record(e: &mut Emitter, tag: &str, record: &MasterRecord) {
    e.set_context(format!("{tag} '{}'", record.name));
    let mut attributes = Vec::with_capacity(record.attributes.len());
    for (key, value) in &record.attributes {
        if key == "NAME" && *value != record.name {
            e.inconsistent(format!(
                "NAME attribute '{value}' differs from the name '{}'",
                record.name
            ));
            attributes.push((key.clone(), record.name.clone()));
        } else {
            attributes.push((key.clone(), value.clone()));
        }
    }
    e.open(tag, &attributes);
    e.required_leaf("GUID", &record.tally_ref.guid);
    if let Some(alter_id) = record.tally_ref.alter_id {
        e.leaf("ALTERID", &alter_id.to_string());
    }
    e.required_leaf("NAME", &record.name);
    e.optional_leaf("PARENT", record.parent.as_deref());
    e.reject_reserved(record.extra.iter().map(|x| x.name.as_str()), MASTER_FIELDS);
    for extra in &record.extra {
        write_raw_element(e, extra, 0);
    }
    e.close(tag);
}

fn write_raw_element(e: &mut Emitter, element: &RawElement, depth: usize) {
    if depth >= MAX_EXTRA_DEPTH {
        e.inconsistent(format!(
            "<{}> is nested deeper than {MAX_EXTRA_DEPTH} levels and was left out",
            element.name
        ));
        return;
    }
    e.open(&element.name, &element.attributes);
    if element.children.is_empty() {
        e.text(&element.name, &element.text);
    } else {
        // The reader clears whitespace-only text around children, so there is nothing to keep.
        if !element.text.trim().is_empty() {
            e.text(&element.name, &element.text);
        }
        for child in &element.children {
            write_raw_element(e, child, depth + 1);
        }
    }
    e.close(&element.name);
}

// ---------------------------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------------------------

fn format_date(e: &mut Emitter, date: Date) -> String {
    let year = date.year();
    if !(0..=9999).contains(&year) {
        e.inconsistent(format!(
            "year {year} cannot be written as four digits of YYYYMMDD"
        ));
    }
    let month = u8::from(date.month());
    let day = date.day();
    format!("{year:04}{month:02}{day:02}")
}

fn write_voucher(e: &mut Emitter, v: &TVoucher) {
    let number = v.number.as_deref().unwrap_or("(no number)");
    e.set_context(format!("voucher {} {number}", v.voucher_type));
    let mut attributes = Vec::with_capacity(v.attrs.len());
    for (key, value) in &v.attrs {
        if key == "VCHTYPE" && *value != v.voucher_type {
            e.inconsistent(format!(
                "VCHTYPE attribute '{value}' differs from the voucher type '{}'",
                v.voucher_type
            ));
            attributes.push((key.clone(), v.voucher_type.clone()));
        } else {
            attributes.push((key.clone(), value.clone()));
        }
    }
    e.open("VOUCHER", &attributes);
    e.optional_leaf("GUID", v.guid.as_deref());
    if let Some(alter_id) = v.alter_id {
        e.leaf("ALTERID", &alter_id.to_string());
    }
    let date = format_date(e, v.date);
    e.leaf("DATE", &date);
    e.required_leaf("VOUCHERTYPENAME", &v.voucher_type);
    e.optional_leaf("VOUCHERNUMBER", v.number.as_deref());
    e.optional_leaf("PARTYLEDGERNAME", v.party.as_deref());
    e.optional_leaf("NARRATION", v.narration.as_deref());
    for line in &v.ledger_entries {
        write_ledger_entry(e, line);
    }
    for line in &v.inventory_entries {
        write_inventory_entry(e, line);
    }
    e.reject_reserved(v.extra.iter().map(|x| x.name.as_str()), VOUCHER_FIELDS);
    for extra in &v.extra {
        write_raw_node(e, extra, 0);
    }
    e.close("VOUCHER");
}

fn write_raw_node(e: &mut Emitter, node: &RawNode, depth: usize) {
    if depth >= MAX_EXTRA_DEPTH {
        e.inconsistent(format!(
            "<{}> is nested deeper than {MAX_EXTRA_DEPTH} levels and was left out",
            node.name
        ));
        return;
    }
    e.open(&node.name, &node.attrs);
    if !node.text.is_empty() {
        e.text(&node.name, &node.text);
    }
    for child in &node.children {
        write_raw_node(e, child, depth + 1);
    }
    e.close(&node.name);
}

fn write_ledger_entry(e: &mut Emitter, line: &TLedgerEntry) {
    let name = line.ledger_name.as_str();
    e.open("LEDGERENTRIES.LIST", &[]);
    e.required_leaf("LEDGERNAME", name);
    if line.amount.is_negative() {
        e.inconsistent(format!(
            "ledger line '{name}': the magnitude {} paise is negative; it holds a magnitude and a side",
            line.amount.get()
        ));
    }
    let magnitude = line.amount.get().unsigned_abs();
    let (debit, side_flag) = match line.side {
        DrCr::Dr => (true, "Yes"),
        DrCr::Cr => (false, "No"),
    };
    match line.is_deemed_positive {
        Some(flag) if flag == debit => e.leaf("ISDEEMEDPOSITIVE", yes_no(flag)),
        Some(flag) => {
            e.inconsistent(format!(
                "ledger line '{name}': ISDEEMEDPOSITIVE {} contradicts its side",
                yes_no(flag)
            ));
            e.leaf("ISDEEMEDPOSITIVE", side_flag);
        }
        None if magnitude == 0 => {
            e.inconsistent(format!(
                "ledger line '{name}': a zero amount needs ISDEEMEDPOSITIVE to carry its side"
            ));
            e.leaf("ISDEEMEDPOSITIVE", side_flag);
        }
        None => {}
    }
    e.leaf("AMOUNT", &format_paise(debit, magnitude));
    for bill in &line.bill_allocations {
        write_bill(e, bill);
    }
    e.reject_reserved(
        line.extra.iter().map(|x| x.name.as_str()),
        LEDGER_LINE_FIELDS,
    );
    for extra in &line.extra {
        write_raw_node(e, extra, 0);
    }
    e.close("LEDGERENTRIES.LIST");
}

fn write_bill(e: &mut Emitter, bill: &TBillAllocation) {
    e.open("BILLALLOCATIONS.LIST", &[]);
    e.required_leaf("NAME", &bill.name);
    e.optional_leaf("BILLTYPE", bill.bill_type.as_deref());
    e.amount_leaf("AMOUNT", bill.amount);
    e.reject_reserved(bill.extra.iter().map(|x| x.name.as_str()), BILL_FIELDS);
    for extra in &bill.extra {
        write_raw_node(e, extra, 0);
    }
    e.close("BILLALLOCATIONS.LIST");
}

fn write_inventory_entry(e: &mut Emitter, line: &TInventoryEntry) {
    e.open("ALLINVENTORYENTRIES.LIST", &[]);
    e.required_leaf("STOCKITEMNAME", &line.stock_item);
    if let Some(flag) = line.is_deemed_positive {
        e.leaf("ISDEEMEDPOSITIVE", yes_no(flag));
    }
    if let Some(rate) = &line.rate {
        e.rate_leaf("RATE", rate);
    }
    if let Some(quantity) = &line.billed_qty {
        e.qty_leaf("BILLEDQTY", quantity);
    }
    if let Some(quantity) = &line.actual_qty {
        e.qty_leaf("ACTUALQTY", quantity);
    }
    if let Some(amount) = line.amount {
        e.amount_leaf("AMOUNT", amount);
    }
    for batch in &line.batches {
        write_batch(e, batch);
    }
    e.reject_reserved(line.extra.iter().map(|x| x.name.as_str()), INVENTORY_FIELDS);
    for extra in &line.extra {
        write_raw_node(e, extra, 0);
    }
    e.close("ALLINVENTORYENTRIES.LIST");
}

fn write_batch(e: &mut Emitter, batch: &TBatchAllocation) {
    e.open("BATCHALLOCATIONS.LIST", &[]);
    e.optional_leaf("GODOWNNAME", batch.godown.as_deref());
    e.optional_leaf("BATCHNAME", batch.batch.as_deref());
    if let Some(amount) = batch.amount {
        e.amount_leaf("AMOUNT", amount);
    }
    if let Some(quantity) = &batch.billed_qty {
        e.qty_leaf("BILLEDQTY", quantity);
    }
    if let Some(quantity) = &batch.actual_qty {
        e.qty_leaf("ACTUALQTY", quantity);
    }
    e.reject_reserved(batch.extra.iter().map(|x| x.name.as_str()), BATCH_FIELDS);
    for extra in &batch.extra {
        write_raw_node(e, extra, 0);
    }
    e.close("BATCHALLOCATIONS.LIST");
}
