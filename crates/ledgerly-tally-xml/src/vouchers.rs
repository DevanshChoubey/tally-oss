//! Voucher parsing. **Card X3** implements this module.
//!
//! Frozen interface (plan card X3):
//!
//! ```text
//! pub struct TVoucher { .. }   // guid, alter_id, type name, date, number, ledger entries, bills, inventory, batches, extra
//! pub fn parse_vouchers(env: &Envelope) -> Result<(Vec<TVoucher>, Vec<XmlWarning>), TallyXmlError>;
//! ```
//!
//! # Status
//!
//! The frozen `parse_vouchers(&Envelope)` entry point cannot be written yet: `Envelope` is card
//! X1's type and `envelope.rs` is still the Wave 0 contract text (see the card X3 interface
//! request). Everything else is built here against the decoded document text:
//! [`parse_vouchers_xml`] takes the text that `envelope::decode` returns and yields the same
//! result. `parse_vouchers` becomes a one-line wrapper once X1 lands.
//!
//! # Sign convention (derived, research note R8 section 2.3(e))
//!
//! A ledger line with `ISDEEMEDPOSITIVE` = `Yes` carries a negative `AMOUNT` and is a debit. A
//! line with `No` carries a positive `AMOUNT` and is a credit. Here a negative amount maps to
//! [`DrCr::Dr`] and a positive one to [`DrCr::Cr`]. A flag that contradicts the sign is an error,
//! never a guess. R8 documents the convention for ledger lines only, so bill, inventory and batch
//! amounts keep the sign exactly as written.
//!
//! Anything the parser does not model is kept in `extra` as a [`RawNode`] (rule A9).

use std::str::FromStr;

use ledgerly_core::{Date, Decimal, DrCr, Month, Paise, Qty, Rate};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{TallyXmlError, XmlWarning};

/// Deepest element nesting accepted; deeper input is refused instead of recursed into.
const MAX_DEPTH: usize = 128;

/// An element the parser does not model, kept whole so nothing is silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNode {
    /// Tag name as written.
    pub name: String,
    /// Attributes in document order, values unescaped.
    pub attrs: Vec<(String, String)>,
    /// Text content, trimmed.
    pub text: String,
    /// Child elements in document order.
    pub children: Vec<RawNode>,
}

/// A quantity with its unit, for example `10 Nos`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TQty {
    /// The number.
    pub qty: Qty,
    /// The unit text as written (may be empty when the file gives none).
    pub unit: String,
}

/// A per-unit rate, for example `1000.00/Nos`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TRate {
    /// Rupees per unit.
    pub rate: Rate,
    /// The unit text as written (may be empty).
    pub unit: String,
}

/// One `BILLALLOCATIONS.LIST` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TBillAllocation {
    /// Bill reference name.
    pub name: String,
    /// `BILLTYPE` text (New Ref, Agst Ref, ...), if present.
    pub bill_type: Option<String>,
    /// Amount with its sign exactly as written (not interpreted; see the module doc).
    pub amount: Paise,
    /// Unmodelled children.
    pub extra: Vec<RawNode>,
}

/// One ledger line of a voucher (`ALLLEDGERENTRIES.LIST` or `LEDGERENTRIES.LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TLedgerEntry {
    /// Ledger name.
    pub ledger_name: String,
    /// Debit or credit, from the sign of the amount.
    pub side: DrCr,
    /// Magnitude, never negative.
    pub amount: Paise,
    /// The `ISDEEMEDPOSITIVE` flag as written, if present.
    pub is_deemed_positive: Option<bool>,
    /// Bill-wise allocations on this line.
    pub bill_allocations: Vec<TBillAllocation>,
    /// Unmodelled children.
    pub extra: Vec<RawNode>,
}

/// One `BATCHALLOCATIONS.LIST` entry of an inventory line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TBatchAllocation {
    /// Godown name.
    pub godown: Option<String>,
    /// Batch name.
    pub batch: Option<String>,
    /// Amount with its sign as written.
    pub amount: Option<Paise>,
    /// Billed quantity.
    pub billed_qty: Option<TQty>,
    /// Actual quantity.
    pub actual_qty: Option<TQty>,
    /// Unmodelled children.
    pub extra: Vec<RawNode>,
}

/// One inventory line (`INVENTORYENTRIES.LIST` or `ALLINVENTORYENTRIES.LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TInventoryEntry {
    /// Stock item name.
    pub stock_item: String,
    /// The `ISDEEMEDPOSITIVE` flag as written, if present.
    pub is_deemed_positive: Option<bool>,
    /// Rate.
    pub rate: Option<TRate>,
    /// Billed quantity.
    pub billed_qty: Option<TQty>,
    /// Actual quantity.
    pub actual_qty: Option<TQty>,
    /// Line amount with its sign as written.
    pub amount: Option<Paise>,
    /// Batch allocations.
    pub batches: Vec<TBatchAllocation>,
    /// Unmodelled children (for example `ACCOUNTINGALLOCATIONS.LIST`).
    pub extra: Vec<RawNode>,
}

/// A voucher as exported by Tally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TVoucher {
    /// Tally GUID, when the file carries one.
    pub guid: Option<String>,
    /// Tally `ALTERID`, when present.
    pub alter_id: Option<u64>,
    /// Voucher type name.
    pub voucher_type: String,
    /// Voucher date.
    pub date: Date,
    /// Voucher number.
    pub number: Option<String>,
    /// Party ledger name.
    pub party: Option<String>,
    /// Narration.
    pub narration: Option<String>,
    /// Attributes of the `VOUCHER` element (`VCHTYPE`, `ACTION`, `REMOTEID`, ...).
    pub attrs: Vec<(String, String)>,
    /// Ledger lines in document order.
    pub ledger_entries: Vec<TLedgerEntry>,
    /// Inventory lines in document order.
    pub inventory_entries: Vec<TInventoryEntry>,
    /// Unmodelled children.
    pub extra: Vec<RawNode>,
}

fn value_err(tag: &str, value: &str, reason: &str) -> TallyXmlError {
    TallyXmlError::Value {
        tag: tag.to_owned(),
        value: value.to_owned(),
        reason: reason.to_owned(),
    }
}

fn missing(tag: &str, path: &str) -> TallyXmlError {
    TallyXmlError::Missing {
        tag: tag.to_owned(),
        path: path.to_owned(),
    }
}

fn offset_of(reader: &Reader<&[u8]>) -> usize {
    usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX)
}

fn xml_err(reader: &Reader<&[u8]>, reason: &impl ToString) -> TallyXmlError {
    TallyXmlError::Xml {
        offset: offset_of(reader),
        reason: reason.to_string(),
    }
}

fn node_from_start(
    reader: &Reader<&[u8]>,
    start: &BytesStart<'_>,
) -> Result<RawNode, TallyXmlError> {
    let name = start.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| xml_err(reader, &e))?;
        let key = attr.key.as_ref().to_owned();
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| xml_err(reader, &e))?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(RawNode {
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

/// Checks that a decoded document is an import file: its first element is `ENVELOPE`.
///
/// [`parse_vouchers_xml`] finds vouchers wherever they sit, so it reads text with no element, or
/// another document, as "no vouchers". A file the user hands in to import must be an `ENVELOPE`
/// document (<https://help.tallysolutions.com/sample-xml/>, read 2026-11-27), so this refuses the
/// rest before anything is imported.
///
/// # Errors
/// [`TallyXmlError::Missing`] when the document has no element or its root is not `ENVELOPE`,
/// [`TallyXmlError::Xml`] when it is not well-formed before its first element.
pub fn check_envelope_root(text: &str) -> Result<(), TallyXmlError> {
    let mut reader = Reader::from_str(text);
    loop {
        let event = reader.read_event().map_err(|e| xml_err(&reader, &e))?;
        match event {
            Event::Start(e) | Event::Empty(e) => {
                return if e.name().as_ref() == "ENVELOPE" {
                    Ok(())
                } else {
                    Err(missing_envelope())
                };
            }
            Event::Text(t) => {
                if !t
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .trim()
                    .is_empty()
                {
                    return Err(missing_envelope());
                }
            }
            Event::Eof | Event::End(_) | Event::CData(_) | Event::GeneralRef(_) => {
                return Err(missing_envelope());
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
}

fn missing_envelope() -> TallyXmlError {
    TallyXmlError::Missing {
        tag: "ENVELOPE".to_owned(),
        path: "document root".to_owned(),
    }
}

/// Parses every `VOUCHER` element in a decoded Tally XML document.
///
/// The document is the text returned by `crate::envelope::decode` (card X1). Only one voucher is held as
/// a tree at a time. `VOUCHER` elements are found wherever they sit (`TALLYMESSAGE`, `COLLECTION`).
///
/// # Errors
/// [`TallyXmlError::Xml`] for malformed XML or nesting deeper than 128 levels,
/// [`TallyXmlError::Truncated`] when the document ends inside an element,
/// [`TallyXmlError::Missing`] for a missing date, type, ledger name or amount, and
/// [`TallyXmlError::Value`] for a bad date, an amount that is not exact paise, a flag that
/// contradicts the sign, or a repeated single-valued tag.
pub fn parse_vouchers_xml(text: &str) -> Result<(Vec<TVoucher>, Vec<XmlWarning>), TallyXmlError> {
    let mut reader = Reader::from_str(text);
    let mut vouchers = Vec::new();
    // Elements open outside any voucher, and the tree being built inside one.
    let mut outer_depth = 0usize;
    let mut stack: Vec<RawNode> = Vec::new();
    loop {
        let event = reader.read_event().map_err(|e| xml_err(&reader, &e))?;
        match event {
            Event::Start(e) => {
                open_element(&reader, &e, &mut outer_depth, &mut stack)?;
            }
            Event::Empty(e) => {
                open_element(&reader, &e, &mut outer_depth, &mut stack)?;
                close_element(&mut outer_depth, &mut stack, &mut vouchers)?;
            }
            Event::End(_) => close_element(&mut outer_depth, &mut stack, &mut vouchers)?,
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text
                        .push_str(&t.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
            }
            Event::CData(c) => {
                if let Some(top) = stack.last_mut() {
                    top.text
                        .push_str(&c.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    let resolved = resolve_ref(&r).ok_or_else(|| {
                        xml_err(&reader, &"unknown entity reference inside a voucher")
                    })?;
                    top.text.push_str(&resolved);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if outer_depth > 0 || !stack.is_empty() {
        return Err(TallyXmlError::Truncated {
            offset: offset_of(&reader),
        });
    }
    Ok((vouchers, Vec::new()))
}

fn resolve_ref(r: &quick_xml::events::BytesRef<'_>) -> Option<String> {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return Some(c.to_string());
    }
    let name = r.xml_content(quick_xml::XmlVersion::Implicit1_0);
    quick_xml::escape::resolve_predefined_entity(&name).map(str::to_owned)
}

fn open_element(
    reader: &Reader<&[u8]>,
    start: &BytesStart<'_>,
    outer_depth: &mut usize,
    stack: &mut Vec<RawNode>,
) -> Result<(), TallyXmlError> {
    if outer_depth.saturating_add(stack.len()) >= MAX_DEPTH {
        return Err(xml_err(reader, &"elements nested too deeply"));
    }
    if stack.is_empty() && start.name().as_ref() != "VOUCHER" {
        *outer_depth += 1;
        return Ok(());
    }
    stack.push(node_from_start(reader, start)?);
    Ok(())
}

fn close_element(
    outer_depth: &mut usize,
    stack: &mut Vec<RawNode>,
    out: &mut Vec<TVoucher>,
) -> Result<(), TallyXmlError> {
    let Some(mut node) = stack.pop() else {
        *outer_depth = outer_depth.saturating_sub(1);
        return Ok(());
    };
    node.text = String::from(node.text.trim());
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None => out.push(voucher_from_node(&node)?),
    }
    Ok(())
}

/// Parses a decimal rupee amount into exact paise; refuses anything that would need rounding.
fn parse_paise(tag: &str, raw: &str) -> Result<Paise, TallyXmlError> {
    let text = raw.trim();
    let dec = Decimal::from_str(text)
        .map_err(|_| value_err(tag, raw, "not a plain decimal rupee amount"))?
        .normalize();
    if dec.scale() > 2 {
        return Err(value_err(
            tag,
            raw,
            "more than two decimal places: not exact paise",
        ));
    }
    let paise = dec
        .checked_mul(Decimal::ONE_HUNDRED)
        .ok_or_else(|| value_err(tag, raw, "amount overflows"))?;
    i64::try_from(paise)
        .map(Paise::new)
        .map_err(|_| value_err(tag, raw, "amount overflows"))
}

fn parse_flag(raw: &str) -> Result<bool, TallyXmlError> {
    match raw.trim() {
        "Yes" => Ok(true),
        "No" => Ok(false),
        other => Err(value_err("ISDEEMEDPOSITIVE", other, "expected Yes or No")),
    }
}

fn parse_qty(tag: &str, raw: &str) -> Result<TQty, TallyXmlError> {
    let text = raw.trim();
    let (number, unit) = match text.split_once(char::is_whitespace) {
        Some((n, u)) => (n, u.trim()),
        None => (text, ""),
    };
    if unit.contains('=') {
        return Err(value_err(tag, raw, "compound quantity is not supported"));
    }
    let dec = Decimal::from_str(number).map_err(|_| value_err(tag, raw, "not a quantity"))?;
    Ok(TQty {
        qty: Qty(dec),
        unit: unit.to_owned(),
    })
}

fn parse_rate(tag: &str, raw: &str) -> Result<TRate, TallyXmlError> {
    let text = raw.trim();
    let (number, unit) = match text.rsplit_once('/') {
        Some((n, u)) => (n.trim(), u.trim()),
        None => (text, ""),
    };
    let dec = Decimal::from_str(number).map_err(|_| value_err(tag, raw, "not a rate"))?;
    Ok(TRate {
        rate: Rate(dec),
        unit: unit.to_owned(),
    })
}

fn parse_date(raw: &str) -> Result<Date, TallyXmlError> {
    let bad = || value_err("DATE", raw, "expected a valid YYYYMMDD date");
    let text = raw.trim();
    if text.len() != 8 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let year = text.get(0..4).and_then(|s| s.parse::<i32>().ok());
    let month = text
        .get(4..6)
        .and_then(|s| s.parse::<u8>().ok())
        .and_then(|m| Month::try_from(m).ok());
    let day = text.get(6..8).and_then(|s| s.parse::<u8>().ok());
    match (year, month, day) {
        (Some(y), Some(m), Some(d)) => Date::from_calendar_date(y, m, d).map_err(|_| bad()),
        _ => Err(bad()),
    }
}

/// Stores `node` in `slot`; a second occurrence of a single-valued tag is an error.
fn set_once<'a>(slot: &mut Option<&'a RawNode>, node: &'a RawNode) -> Result<(), TallyXmlError> {
    if slot.is_some() {
        return Err(value_err(
            &node.name,
            &node.text,
            "tag appears more than once",
        ));
    }
    *slot = Some(node);
    Ok(())
}

fn non_empty(node: Option<&RawNode>) -> Option<&str> {
    node.map(|n| n.text.as_str()).filter(|t| !t.is_empty())
}

fn opt_string(node: Option<&RawNode>) -> Option<String> {
    non_empty(node).map(str::to_owned)
}

fn opt_paise(tag: &str, node: Option<&RawNode>) -> Result<Option<Paise>, TallyXmlError> {
    non_empty(node).map(|t| parse_paise(tag, t)).transpose()
}

fn opt_qty(tag: &str, node: Option<&RawNode>) -> Result<Option<TQty>, TallyXmlError> {
    non_empty(node).map(|t| parse_qty(tag, t)).transpose()
}

fn opt_flag(node: Option<&RawNode>) -> Result<Option<bool>, TallyXmlError> {
    non_empty(node).map(parse_flag).transpose()
}

fn voucher_from_node(node: &RawNode) -> Result<TVoucher, TallyXmlError> {
    let mut guid = None;
    let mut alter = None;
    let mut vtype = None;
    let mut date = None;
    let mut number = None;
    let mut party = None;
    let mut narration = None;
    let mut ledger_entries = Vec::new();
    let mut inventory_entries = Vec::new();
    let mut extra = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "GUID" => set_once(&mut guid, child)?,
            "ALTERID" => set_once(&mut alter, child)?,
            "VOUCHERTYPENAME" => set_once(&mut vtype, child)?,
            "DATE" => set_once(&mut date, child)?,
            "VOUCHERNUMBER" => set_once(&mut number, child)?,
            "PARTYLEDGERNAME" => set_once(&mut party, child)?,
            "NARRATION" => set_once(&mut narration, child)?,
            "ALLLEDGERENTRIES.LIST" | "LEDGERENTRIES.LIST" => {
                ledger_entries.push(ledger_entry_from_node(child)?);
            }
            "INVENTORYENTRIES.LIST" | "ALLINVENTORYENTRIES.LIST" => {
                inventory_entries.push(inventory_entry_from_node(child)?);
            }
            _ => extra.push(child.clone()),
        }
    }
    let date = parse_date(non_empty(date).ok_or_else(|| missing("DATE", "VOUCHER"))?)?;
    let voucher_type = voucher_type_of(node, non_empty(vtype))?;
    let alter_id = non_empty(alter)
        .map(|t| {
            t.parse::<u64>()
                .map_err(|_| value_err("ALTERID", t, "not an unsigned integer"))
        })
        .transpose()?;
    Ok(TVoucher {
        guid: opt_string(guid),
        alter_id,
        voucher_type,
        date,
        number: opt_string(number),
        party: opt_string(party),
        narration: opt_string(narration),
        attrs: node.attrs.clone(),
        ledger_entries,
        inventory_entries,
        extra,
    })
}

/// The type from `VOUCHERTYPENAME`, else the `VCHTYPE` attribute; both present must agree.
fn voucher_type_of(node: &RawNode, tag_value: Option<&str>) -> Result<String, TallyXmlError> {
    let attr = node
        .attrs
        .iter()
        .find(|(k, _)| k == "VCHTYPE")
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty());
    match (tag_value, attr) {
        (Some(t), Some(a)) if t != a => Err(value_err(
            "VCHTYPE",
            a,
            "attribute disagrees with <VOUCHERTYPENAME>",
        )),
        (Some(t), _) => Ok(t.to_owned()),
        (None, Some(a)) => Ok(a.to_owned()),
        (None, None) => Err(missing("VOUCHERTYPENAME", "VOUCHER")),
    }
}

/// Maps the as-written amount and flag to a side and a non-negative magnitude.
fn side_of(amount: Paise, raw: &str, flag: Option<bool>) -> Result<(DrCr, Paise), TallyXmlError> {
    let conflict = || {
        value_err(
            "AMOUNT",
            raw,
            "sign contradicts <ISDEEMEDPOSITIVE> (Yes goes with a negative amount)",
        )
    };
    if amount.is_negative() {
        if flag == Some(false) {
            return Err(conflict());
        }
        let magnitude = amount
            .checked_abs()
            .map_err(|_| value_err("AMOUNT", raw, "amount overflows"))?;
        return Ok((DrCr::Dr, magnitude));
    }
    if amount.is_zero() {
        return match flag {
            Some(true) => Ok((DrCr::Dr, amount)),
            Some(false) => Ok((DrCr::Cr, amount)),
            None => Err(value_err(
                "AMOUNT",
                raw,
                "zero amount without <ISDEEMEDPOSITIVE>: side cannot be told",
            )),
        };
    }
    if flag == Some(true) {
        return Err(conflict());
    }
    Ok((DrCr::Cr, amount))
}

fn ledger_entry_from_node(node: &RawNode) -> Result<TLedgerEntry, TallyXmlError> {
    let path = node.name.as_str();
    let mut name = None;
    let mut flag = None;
    let mut amount = None;
    let mut bills = Vec::new();
    let mut extra = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "LEDGERNAME" => set_once(&mut name, child)?,
            "ISDEEMEDPOSITIVE" => set_once(&mut flag, child)?,
            "AMOUNT" => set_once(&mut amount, child)?,
            "BILLALLOCATIONS.LIST" => bills.push(bill_from_node(child)?),
            _ => extra.push(child.clone()),
        }
    }
    let ledger_name = non_empty(name)
        .ok_or_else(|| missing("LEDGERNAME", path))?
        .to_owned();
    let raw = amount
        .map(|n| n.text.as_str())
        .ok_or_else(|| missing("AMOUNT", path))?;
    let is_deemed_positive = opt_flag(flag)?;
    let (side, magnitude) = side_of(parse_paise("AMOUNT", raw)?, raw, is_deemed_positive)?;
    Ok(TLedgerEntry {
        ledger_name,
        side,
        amount: magnitude,
        is_deemed_positive,
        bill_allocations: bills,
        extra,
    })
}

fn bill_from_node(node: &RawNode) -> Result<TBillAllocation, TallyXmlError> {
    let path = node.name.as_str();
    let mut name = None;
    let mut kind = None;
    let mut amount = None;
    let mut extra = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "NAME" => set_once(&mut name, child)?,
            "BILLTYPE" => set_once(&mut kind, child)?,
            "AMOUNT" => set_once(&mut amount, child)?,
            _ => extra.push(child.clone()),
        }
    }
    Ok(TBillAllocation {
        name: non_empty(name)
            .ok_or_else(|| missing("NAME", path))?
            .to_owned(),
        bill_type: opt_string(kind),
        amount: opt_paise("AMOUNT", amount)?.ok_or_else(|| missing("AMOUNT", path))?,
        extra,
    })
}

fn inventory_entry_from_node(node: &RawNode) -> Result<TInventoryEntry, TallyXmlError> {
    let path = node.name.as_str();
    let mut item = None;
    let mut flag = None;
    let mut rate = None;
    let mut billed = None;
    let mut actual = None;
    let mut amount = None;
    let mut batches = Vec::new();
    let mut extra = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "STOCKITEMNAME" => set_once(&mut item, child)?,
            "ISDEEMEDPOSITIVE" => set_once(&mut flag, child)?,
            "RATE" => set_once(&mut rate, child)?,
            "BILLEDQTY" => set_once(&mut billed, child)?,
            "ACTUALQTY" => set_once(&mut actual, child)?,
            "AMOUNT" => set_once(&mut amount, child)?,
            "BATCHALLOCATIONS.LIST" => batches.push(batch_from_node(child)?),
            _ => extra.push(child.clone()),
        }
    }
    Ok(TInventoryEntry {
        stock_item: non_empty(item)
            .ok_or_else(|| missing("STOCKITEMNAME", path))?
            .to_owned(),
        is_deemed_positive: opt_flag(flag)?,
        rate: non_empty(rate).map(|t| parse_rate("RATE", t)).transpose()?,
        billed_qty: opt_qty("BILLEDQTY", billed)?,
        actual_qty: opt_qty("ACTUALQTY", actual)?,
        amount: opt_paise("AMOUNT", amount)?,
        batches,
        extra,
    })
}

fn batch_from_node(node: &RawNode) -> Result<TBatchAllocation, TallyXmlError> {
    let mut godown = None;
    let mut batch = None;
    let mut amount = None;
    let mut billed = None;
    let mut actual = None;
    let mut extra = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "GODOWNNAME" => set_once(&mut godown, child)?,
            "BATCHNAME" => set_once(&mut batch, child)?,
            "AMOUNT" => set_once(&mut amount, child)?,
            "BILLEDQTY" => set_once(&mut billed, child)?,
            "ACTUALQTY" => set_once(&mut actual, child)?,
            _ => extra.push(child.clone()),
        }
    }
    Ok(TBatchAllocation {
        godown: opt_string(godown),
        batch: opt_string(batch),
        amount: opt_paise("AMOUNT", amount)?,
        billed_qty: opt_qty("BILLEDQTY", billed)?,
        actual_qty: opt_qty("ACTUALQTY", actual)?,
        extra,
    })
}
