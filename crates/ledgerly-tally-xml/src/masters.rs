//! Master parsing. **Card X2** implements this module.
//!
//! Frozen interface (plan card X2):
//!
//! ```text
//! pub enum TallyMaster { Group(TGroup), Ledger(TLedger), StockItem(..), Unit(..), Godown(..), VoucherType(..), CostCentre(..), CostCategory(..), StockGroup(..) }
//! pub fn parse_masters(env: &Envelope) -> Result<(Vec<TallyMaster>, Vec<XmlWarning>), TallyXmlError>;
//! ```
//!
//! # Status of `parse_masters`
//!
//! `Envelope` and `RawMessage` are defined by card X1 in `envelope.rs`, which is still only a
//! contract document, so `parse_masters(&Envelope)` cannot be written against it yet. This module
//! therefore parses straight from text or bytes with [`parse_masters_text`] and
//! [`parse_masters_bytes`]; the `Envelope` entry point is a thin wrapper to add once X1 lands
//! (see the interface request filed with this card).
//!
//! # What is read
//!
//! A master is an element whose parent is `TALLYMESSAGE` (import form) or `COLLECTION` (export
//! form) and whose tag is one of the nine kinds in [`TallyMaster`]. `VOUCHER` elements belong to
//! card X3 and are skipped silently. Any other message element is skipped with a warning. Child
//! elements other than `GUID`, `ALTERID`, `NAME` and `PARENT`, and every attribute, are kept
//! verbatim (rule A9). Invalid characters (raw control characters, `&#4;`) are dropped and
//! reported with the byte offset in the decoded text.

use ledgerly_core::TallyRef;
use thiserror::Error;

use crate::{TallyXmlError, XmlWarning};

/// Deepest element nesting accepted, so hostile input cannot exhaust the stack or memory.
const MAX_DEPTH: usize = 128;
/// Longest entity or character reference name that is looked at.
const MAX_ENTITY_LEN: usize = 32;

/// An XML element kept verbatim: used for fields this parser does not model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawElement {
    /// Tag name.
    pub name: String,
    /// Attributes in document order, with entities decoded.
    pub attributes: Vec<(String, String)>,
    /// Text content. Whitespace-only text between child elements is dropped; leaf text is verbatim.
    pub text: String,
    /// Child elements in document order.
    pub children: Vec<RawElement>,
}

/// The fields every master kind shares, plus everything else it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRecord {
    /// GUID and ALTERID, kept so re-imports and syncs match the object.
    pub tally_ref: TallyRef,
    /// The object's name (from the `NAME` child, or the `NAME` attribute when there is no child).
    pub name: String,
    /// The `PARENT` element's text; an empty or absent `PARENT` is `None` (a primary object).
    pub parent: Option<String>,
    /// Attributes of the master element (for example `ACTION`), in document order.
    pub attributes: Vec<(String, String)>,
    /// Child elements that are not modelled fields, in document order.
    pub extra: Vec<RawElement>,
}

/// An account group (`GROUP`).
pub type TGroup = MasterRecord;
/// A ledger (`LEDGER`).
pub type TLedger = MasterRecord;
/// A stock item (`STOCKITEM`).
pub type TStockItem = MasterRecord;
/// A unit of measure (`UNIT`).
pub type TUnit = MasterRecord;
/// A godown, that is a warehouse location (`GODOWN`).
pub type TGodown = MasterRecord;
/// A voucher type (`VOUCHERTYPE`).
pub type TVoucherType = MasterRecord;
/// A cost centre (`COSTCENTRE`).
pub type TCostCentre = MasterRecord;
/// A cost category (`COSTCATEGORY`).
pub type TCostCategory = MasterRecord;
/// A stock group (`STOCKGROUP`).
pub type TStockGroup = MasterRecord;

/// One master read from a Tally document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TallyMaster {
    /// `GROUP`.
    Group(TGroup),
    /// `LEDGER`.
    Ledger(TLedger),
    /// `STOCKITEM`.
    StockItem(TStockItem),
    /// `UNIT`.
    Unit(TUnit),
    /// `GODOWN`.
    Godown(TGodown),
    /// `VOUCHERTYPE`.
    VoucherType(TVoucherType),
    /// `COSTCENTRE`.
    CostCentre(TCostCentre),
    /// `COSTCATEGORY`.
    CostCategory(TCostCategory),
    /// `STOCKGROUP`.
    StockGroup(TStockGroup),
}

/// Everything read from a document: masters in document order and the warnings raised.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MastersParsed {
    /// Masters in document order.
    pub masters: Vec<TallyMaster>,
    /// Non-fatal problems (dropped characters, skipped elements), ordered by byte offset.
    pub warnings: Vec<XmlWarning>,
}

/// A failed parse: the error and everything read before it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{error}")]
pub struct MastersError {
    /// What went wrong, with a byte offset where one exists.
    pub error: TallyXmlError,
    /// The masters and warnings read before the failure. A master being read when it failed is
    /// discarded and reported in `partial.warnings`.
    pub partial: Box<MastersParsed>,
}

/// Decodes `bytes` (UTF-8, UTF-8 with BOM, UTF-16LE or UTF-16BE with BOM) and parses its masters.
///
/// # Errors
///
/// [`TallyXmlError::Decode`] (offset into `bytes`) when the bytes are not valid text, otherwise
/// the errors of [`parse_masters_text`]. The error carries everything parsed before it.
pub fn parse_masters_bytes(bytes: &[u8]) -> Result<MastersParsed, MastersError> {
    match decode_bytes(bytes) {
        Ok(text) => parse_masters_text(&text),
        Err(error) => Err(MastersError {
            error,
            partial: Box::default(),
        }),
    }
}

/// Parses the masters of a decoded Tally XML document. Offsets are byte offsets into `text`.
///
/// # Errors
///
/// [`TallyXmlError::Truncated`] when the document ends early (offset is `text.len()`),
/// [`TallyXmlError::Xml`] for malformed XML, a mismatched end tag or nesting beyond 128 levels,
/// [`TallyXmlError::Missing`] for a root other than `ENVELOPE` or a master without `GUID` or
/// `NAME`, and [`TallyXmlError::Value`] for an invalid `ALTERID`, an empty `GUID` or `NAME`, a
/// repeated typed field, or a `NAME` attribute that disagrees with the `NAME` child.
pub fn parse_masters_text(text: &str) -> Result<MastersParsed, MastersError> {
    let mut parser = Parser::new(text);
    let result = parser.run();
    parser.finish(result)
}

/// The kind of a master read by [`parse_import_masters_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMasterKind {
    /// `GROUP`.
    Group,
    /// `LEDGER`.
    Ledger,
    /// `STOCKITEM`.
    StockItem,
    /// `UNIT`.
    Unit,
    /// `GODOWN`.
    Godown,
    /// `VOUCHERTYPE`.
    VoucherType,
    /// `COSTCENTRE`.
    CostCentre,
    /// `COSTCATEGORY`.
    CostCategory,
    /// `STOCKGROUP`.
    StockGroup,
}

/// A master from a file a user exported for import, where `GUID` may be absent (S22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMaster {
    /// What kind of master it is.
    pub kind: ImportMasterKind,
    /// The `GUID`, when the file carries one.
    pub guid: Option<String>,
    /// The `ALTERID`, when the file carries one.
    pub alter_id: Option<u64>,
    /// The object's name (from the `NAME` child, or the `NAME` attribute).
    pub name: String,
    /// The `PARENT` element's text; an empty or absent `PARENT` is `None`.
    pub parent: Option<String>,
    /// Attributes of the master element, in document order.
    pub attributes: Vec<(String, String)>,
    /// Child elements that are not modelled fields, in document order.
    pub extra: Vec<RawElement>,
}

/// Everything read from an import file: masters in document order and the warnings raised.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportMastersParsed {
    /// Masters in document order.
    pub masters: Vec<ImportMaster>,
    /// Non-fatal problems, ordered by byte offset.
    pub warnings: Vec<XmlWarning>,
}

/// Decodes `bytes` as [`parse_masters_bytes`] does and reads the masters of a file in the import
/// form. Unlike [`parse_masters_text`], a master without `GUID` is accepted: the sample import
/// file on the help page carries only `NAME` and `PARENT`
/// (<https://help.tallysolutions.com/sample-xml/>, read 2026-11-27).
///
/// # Errors
///
/// As [`parse_masters_bytes`], except that an absent `GUID` is not an error (an empty one is).
pub fn parse_import_masters_bytes(bytes: &[u8]) -> Result<ImportMastersParsed, TallyXmlError> {
    let text = decode_bytes(bytes)?;
    let mut parser = Parser::new(&text);
    parser.lenient = true;
    let result = parser.run();
    let imported = std::mem::take(&mut parser.imported);
    match parser.finish(result) {
        Ok(parsed) => Ok(ImportMastersParsed {
            masters: imported,
            warnings: parsed.warnings,
        }),
        Err(failure) => Err(failure.error),
    }
}

/// Decodes a Tally XML file's bytes (UTF-8, UTF-8 with BOM, UTF-16LE or UTF-16BE with BOM) to
/// text, as [`parse_masters_bytes`] does, for parsers that take text.
///
/// # Errors
/// [`TallyXmlError::Decode`] (offset into `bytes`) when the bytes are not valid text.
pub fn decode_xml_bytes(bytes: &[u8]) -> Result<String, TallyXmlError> {
    decode_bytes(bytes)
}

// ---------------------------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------------------------

fn decode_bytes(bytes: &[u8]) -> Result<String, TallyXmlError> {
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        decode_utf16(rest, true, 2)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        decode_utf16(rest, false, 2)
    } else if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        decode_utf8(rest, 3)
    } else {
        decode_utf8(bytes, 0)
    }
}

fn decode_error(offset: usize, reason: &str) -> TallyXmlError {
    TallyXmlError::Decode {
        offset,
        reason: reason.to_owned(),
    }
}

fn decode_utf8(bytes: &[u8], base: usize) -> Result<String, TallyXmlError> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_owned()),
        Err(e) => Err(decode_error(
            base + e.valid_up_to(),
            "invalid UTF-8 sequence",
        )),
    }
}

/// Decodes UTF-16 code units. `base` is the BOM length, so offsets point into the raw input.
fn decode_utf16(payload: &[u8], little_endian: bool, base: usize) -> Result<String, TallyXmlError> {
    let (pairs, remainder) = payload.as_chunks::<2>();
    let remainder_len = remainder.len();
    let units: Vec<u16> = pairs
        .iter()
        .map(|&pair| {
            if little_endian {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            }
        })
        .collect();
    let mut out = String::with_capacity(units.len());
    let mut i = 0usize;
    while let Some(&unit) = units.get(i) {
        let at = base + i * 2;
        if (0xD800..=0xDBFF).contains(&unit) {
            let low = units
                .get(i + 1)
                .copied()
                .filter(|l| (0xDC00..=0xDFFF).contains(l));
            let Some(low) = low else {
                return Err(decode_error(at, "unpaired UTF-16 high surrogate"));
            };
            let code = 0x1_0000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
            let c = char::from_u32(code).ok_or_else(|| decode_error(at, "invalid UTF-16"))?;
            out.push(c);
            i += 2;
        } else {
            let c = char::from_u32(u32::from(unit))
                .ok_or_else(|| decode_error(at, "unpaired UTF-16 low surrogate"))?;
            out.push(c);
            i += 1;
        }
    }
    if remainder_len != 0 {
        return Err(decode_error(
            base + units.len() * 2,
            "UTF-16 input has an odd number of bytes",
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------------------------

fn xml_error(offset: usize, reason: impl Into<String>) -> TallyXmlError {
    TallyXmlError::Xml {
        offset,
        reason: reason.into(),
    }
}

/// The characters XML 1.0 allows in a document (production `Char`).
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-')
}

fn is_valid_name(name: &str) -> bool {
    let starts_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':');
    starts_ok && name.chars().all(is_name_char)
}

/// What a `&...;` reference decodes to.
enum Entity {
    Char(char),
    Invalid,
    Unknown,
}

fn decode_entity(name: &str) -> Entity {
    match name {
        "amp" => return Entity::Char('&'),
        "lt" => return Entity::Char('<'),
        "gt" => return Entity::Char('>'),
        "quot" => return Entity::Char('"'),
        "apos" => return Entity::Char('\''),
        _ => {}
    }
    let Some(number) = name.strip_prefix('#') else {
        return Entity::Unknown;
    };
    let (digits, radix) = number
        .strip_prefix('x')
        .map_or((number, 10), |hex| (hex, 16));
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return Entity::Unknown;
    }
    let Ok(code) = u32::from_str_radix(digits, radix) else {
        return Entity::Invalid;
    };
    char::from_u32(code)
        .filter(|c| is_xml_char(*c))
        .map_or(Entity::Invalid, Entity::Char)
}

/// Splits `&name;` at the start of `tail` into the name and the bytes it occupies.
fn split_entity(tail: &str) -> Option<(&str, usize)> {
    let body = tail.get(1..)?;
    let end = body.find(|c: char| c == ';' || c == '&' || c == '<' || c.is_whitespace())?;
    if end == 0 || end > MAX_ENTITY_LEN || !body.get(end..)?.starts_with(';') {
        return None;
    }
    Some((body.get(..end)?, end + 2))
}

enum Token {
    Start {
        name: String,
        attributes: Vec<(String, String)>,
        self_closing: bool,
        offset: usize,
    },
    End {
        name: String,
        offset: usize,
    },
    Text {
        text: String,
        offset: usize,
    },
    Eof,
}

struct Scanner<'a> {
    text: &'a str,
    pos: usize,
    warnings: Vec<XmlWarning>,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        // A leading byte order mark that survived decoding is not content.
        let pos = if text.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        Self {
            text,
            pos,
            warnings: Vec::new(),
        }
    }

    fn rest(&self) -> &'a str {
        self.text.get(self.pos..).unwrap_or_default()
    }

    fn truncated(&self) -> TallyXmlError {
        TallyXmlError::Truncated {
            offset: self.text.len(),
        }
    }

    fn warn(&mut self, offset: usize, message: String) {
        self.warnings.push(XmlWarning { offset, message });
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn expect_char(&mut self, want: char) -> Result<(), TallyXmlError> {
        match self.peek() {
            Some(c) if c == want => {
                self.bump();
                Ok(())
            }
            Some(c) => Err(xml_error(
                self.pos,
                format!("expected '{want}', found '{c}'"),
            )),
            None => Err(self.truncated()),
        }
    }

    fn next_token(&mut self) -> Result<Token, TallyXmlError> {
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return Ok(Token::Eof);
            }
            if !rest.starts_with('<') {
                return Ok(self.char_data());
            }
            if rest.starts_with("<!--") {
                self.skip_past("-->", 4)?;
                continue;
            }
            if rest.starts_with("<![CDATA[") {
                return self.cdata();
            }
            if rest.starts_with("<?") {
                self.skip_past("?>", 2)?;
                continue;
            }
            if rest.starts_with("<!") {
                return Err(xml_error(
                    self.pos,
                    "DOCTYPE and other declarations are not supported",
                ));
            }
            if rest.starts_with("</") {
                return self.end_tag();
            }
            return self.start_tag();
        }
    }

    /// Moves past the first `end` found at or after `self.pos + skip` (`skip` is ASCII).
    fn skip_past(&mut self, end: &str, skip: usize) -> Result<(), TallyXmlError> {
        let from = self.pos + skip;
        let tail = self.text.get(from..).unwrap_or_default();
        let Some(i) = tail.find(end) else {
            return Err(self.truncated());
        };
        self.pos = from + i + end.len();
        Ok(())
    }

    fn cdata(&mut self) -> Result<Token, TallyXmlError> {
        let offset = self.pos;
        let from = offset + "<![CDATA[".len();
        let tail = self.text.get(from..).unwrap_or_default();
        let Some(len) = tail.find("]]>") else {
            return Err(self.truncated());
        };
        let raw = tail.get(..len).unwrap_or_default();
        let text = self.clean(raw, from, false);
        self.pos = from + len + "]]>".len();
        Ok(Token::Text { text, offset })
    }

    fn char_data(&mut self) -> Token {
        let offset = self.pos;
        let rest = self.rest();
        let len = rest.find('<').unwrap_or(rest.len());
        let raw = rest.get(..len).unwrap_or_default();
        let text = self.clean(raw, offset, true);
        self.pos = offset + len;
        Token::Text { text, offset }
    }

    /// Drops characters XML forbids and decodes references, warning at each repair. `base` is the
    /// offset of `raw` in the document.
    fn clean(&mut self, raw: &str, base: usize, entities: bool) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut i = 0usize;
        while let Some(tail) = raw.get(i..) {
            let Some(c) = tail.chars().next() else {
                break;
            };
            if c == '&' && entities {
                i += self.entity(tail, base + i, &mut out);
                continue;
            }
            if is_xml_char(c) {
                out.push(c);
            } else {
                let message = format!("dropped invalid character U+{:04X}", u32::from(c));
                self.warn(base + i, message);
            }
            i += c.len_utf8();
        }
        out
    }

    /// Handles the `&` at the start of `tail`; returns the bytes consumed.
    fn entity(&mut self, tail: &str, offset: usize, out: &mut String) -> usize {
        let Some((name, used)) = split_entity(tail) else {
            out.push('&');
            self.warn(offset, "bare '&' kept literally".to_owned());
            return 1;
        };
        match decode_entity(name) {
            Entity::Char(c) => out.push(c),
            Entity::Invalid => {
                self.warn(
                    offset,
                    format!("dropped invalid character reference &{name};"),
                );
            }
            Entity::Unknown => {
                out.push_str(tail.get(..used).unwrap_or_default());
                self.warn(offset, format!("unknown entity &{name}; kept literally"));
            }
        }
        used
    }

    fn end_tag(&mut self) -> Result<Token, TallyXmlError> {
        let offset = self.pos;
        let body = self.rest().get(2..).unwrap_or_default();
        let Some(close) = body.find('>') else {
            return Err(self.truncated());
        };
        let name = body.get(..close).unwrap_or_default().trim_end();
        if !is_valid_name(name) {
            return Err(xml_error(offset, format!("invalid end tag name '{name}'")));
        }
        self.pos = offset + 2 + close + 1;
        Ok(Token::End {
            name: name.to_owned(),
            offset,
        })
    }

    fn read_name(&mut self) -> Result<String, TallyXmlError> {
        let rest = self.rest();
        let len = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
        let name = rest.get(..len).unwrap_or_default();
        if !is_valid_name(name) {
            if rest.is_empty() {
                return Err(self.truncated());
            }
            return Err(xml_error(self.pos, "expected an element or attribute name"));
        }
        self.pos += len;
        Ok(name.to_owned())
    }

    fn attribute(&mut self) -> Result<(String, String), TallyXmlError> {
        let name = self.read_name()?;
        self.skip_ws();
        self.expect_char('=')?;
        self.skip_ws();
        let Some(quote) = self.peek().filter(|q| *q == '"' || *q == '\'') else {
            return Err(if self.peek().is_none() {
                self.truncated()
            } else {
                xml_error(self.pos, format!("attribute '{name}' value is not quoted"))
            });
        };
        self.bump();
        let from = self.pos;
        let rest = self.rest();
        let Some(len) = rest.find(quote) else {
            return Err(self.truncated());
        };
        let raw = rest.get(..len).unwrap_or_default();
        let value = self.clean(raw, from, true);
        self.pos = from + len + 1;
        Ok((name, value))
    }

    fn start_tag(&mut self) -> Result<Token, TallyXmlError> {
        let offset = self.pos;
        self.pos += 1;
        let name = self.read_name()?;
        let mut attributes: Vec<(String, String)> = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => return Err(self.truncated()),
                Some('>') => {
                    self.bump();
                    return Ok(Token::Start {
                        name,
                        attributes,
                        self_closing: false,
                        offset,
                    });
                }
                Some('/') => {
                    self.bump();
                    self.expect_char('>')?;
                    return Ok(Token::Start {
                        name,
                        attributes,
                        self_closing: true,
                        offset,
                    });
                }
                Some(_) => {
                    let at = self.pos;
                    let attribute = self.attribute()?;
                    if attributes.iter().any(|(k, _)| *k == attribute.0) {
                        return Err(xml_error(
                            at,
                            format!("attribute '{}' repeated on <{name}>", attribute.0),
                        ));
                    }
                    attributes.push(attribute);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tree building and master extraction
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum MasterKind {
    Group,
    Ledger,
    StockItem,
    Unit,
    Godown,
    VoucherType,
    CostCentre,
    CostCategory,
    StockGroup,
}

impl MasterKind {
    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "GROUP" => Some(Self::Group),
            "LEDGER" => Some(Self::Ledger),
            "STOCKITEM" => Some(Self::StockItem),
            "UNIT" => Some(Self::Unit),
            "GODOWN" => Some(Self::Godown),
            "VOUCHERTYPE" => Some(Self::VoucherType),
            "COSTCENTRE" => Some(Self::CostCentre),
            "COSTCATEGORY" => Some(Self::CostCategory),
            "STOCKGROUP" => Some(Self::StockGroup),
            _ => None,
        }
    }

    fn public(self) -> ImportMasterKind {
        match self {
            Self::Group => ImportMasterKind::Group,
            Self::Ledger => ImportMasterKind::Ledger,
            Self::StockItem => ImportMasterKind::StockItem,
            Self::Unit => ImportMasterKind::Unit,
            Self::Godown => ImportMasterKind::Godown,
            Self::VoucherType => ImportMasterKind::VoucherType,
            Self::CostCentre => ImportMasterKind::CostCentre,
            Self::CostCategory => ImportMasterKind::CostCategory,
            Self::StockGroup => ImportMasterKind::StockGroup,
        }
    }

    fn wrap(self, record: MasterRecord) -> TallyMaster {
        match self {
            Self::Group => TallyMaster::Group(record),
            Self::Ledger => TallyMaster::Ledger(record),
            Self::StockItem => TallyMaster::StockItem(record),
            Self::Unit => TallyMaster::Unit(record),
            Self::Godown => TallyMaster::Godown(record),
            Self::VoucherType => TallyMaster::VoucherType(record),
            Self::CostCentre => TallyMaster::CostCentre(record),
            Self::CostCategory => TallyMaster::CostCategory(record),
            Self::StockGroup => TallyMaster::StockGroup(record),
        }
    }
}

/// The typed children of a master element.
#[derive(Clone, Copy)]
enum Field {
    Guid,
    AlterId,
    Name,
    Parent,
}

fn field_of(tag: &str) -> Option<Field> {
    match tag {
        "GUID" => Some(Field::Guid),
        "ALTERID" => Some(Field::AlterId),
        "NAME" => Some(Field::Name),
        "PARENT" => Some(Field::Parent),
        _ => None,
    }
}

#[derive(Default)]
struct Fields {
    guid: Option<String>,
    alter_id: Option<String>,
    name: Option<String>,
    parent: Option<String>,
    extra: Vec<RawElement>,
}

fn value_error(tag: &str, value: &str, reason: &str) -> TallyXmlError {
    TallyXmlError::Value {
        tag: tag.to_owned(),
        value: value.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Stores a typed child's text in `slot`, refusing repeats and nested content.
fn take_text(
    slot: &mut Option<String>,
    child: RawElement,
    parent: &str,
    offset: usize,
    warnings: &mut Vec<XmlWarning>,
) -> Result<(), TallyXmlError> {
    if slot.is_some() {
        return Err(value_error(
            &child.name,
            &child.text,
            "appears more than once",
        ));
    }
    if !child.children.is_empty() {
        return Err(value_error(
            &child.name,
            &child.text,
            "expected text, found child elements",
        ));
    }
    if !child.attributes.is_empty() {
        warnings.push(XmlWarning {
            offset,
            message: format!("attributes on <{}> in <{parent}> were not kept", child.name),
        });
    }
    *slot = Some(child.text);
    Ok(())
}

fn require(value: Option<String>, tag: &str, path: &str) -> Result<String, TallyXmlError> {
    match value {
        Some(v) if v.is_empty() => Err(value_error(tag, &v, "must not be empty")),
        Some(v) => Ok(v),
        None => Err(TallyXmlError::Missing {
            tag: tag.to_owned(),
            path: path.to_owned(),
        }),
    }
}

fn parse_alter_id(text: &str) -> Result<u64, TallyXmlError> {
    let digits = text.trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(value_error(
            "ALTERID",
            text,
            "expected a non-negative whole number",
        ));
    }
    digits
        .parse::<u64>()
        .map_err(|_| value_error("ALTERID", text, "does not fit in 64 bits"))
}

fn resolve_name(
    child: Option<String>,
    attribute: Option<String>,
    tag: &str,
) -> Result<String, TallyXmlError> {
    let name = match (child, attribute) {
        (Some(c), Some(a)) => {
            if c != a {
                return Err(value_error(
                    "NAME",
                    &c,
                    &format!("differs from the NAME attribute '{a}'"),
                ));
            }
            Some(c)
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    };
    require(name, "NAME", tag)
}

fn build_record(
    element: RawElement,
    offset: usize,
    warnings: &mut Vec<XmlWarning>,
) -> Result<MasterRecord, TallyXmlError> {
    let built = build_fields(element, offset, warnings, true)?;
    let guid = require(built.guid, "GUID", &built.tag)?;
    Ok(MasterRecord {
        tally_ref: TallyRef {
            guid,
            alter_id: built.alter_id,
        },
        name: built.name,
        parent: built.parent,
        attributes: built.attributes,
        extra: built.extra,
    })
}

/// A master's fields, with the `GUID` still optional.
struct Built {
    tag: String,
    guid: Option<String>,
    alter_id: Option<u64>,
    name: String,
    parent: Option<String>,
    attributes: Vec<(String, String)>,
    extra: Vec<RawElement>,
}

/// Reads a master's fields. With `need_guid`, an absent `GUID` is refused before the name is
/// looked at (the strict parser's order); without it only an empty `GUID` is refused.
fn build_fields(
    element: RawElement,
    offset: usize,
    warnings: &mut Vec<XmlWarning>,
    need_guid: bool,
) -> Result<Built, TallyXmlError> {
    let RawElement {
        name: tag,
        attributes,
        children,
        ..
    } = element;
    let mut fields = Fields::default();
    for child in children {
        match field_of(&child.name) {
            Some(Field::Guid) => take_text(&mut fields.guid, child, &tag, offset, warnings)?,
            Some(Field::AlterId) => take_text(&mut fields.alter_id, child, &tag, offset, warnings)?,
            Some(Field::Name) => take_text(&mut fields.name, child, &tag, offset, warnings)?,
            Some(Field::Parent) => take_text(&mut fields.parent, child, &tag, offset, warnings)?,
            None => fields.extra.push(child),
        }
    }
    let guid = match fields.guid {
        Some(g) => Some(require(Some(g), "GUID", &tag)?),
        None if need_guid => {
            return Err(TallyXmlError::Missing {
                tag: "GUID".to_owned(),
                path: tag,
            });
        }
        None => None,
    };
    let attribute_name = attributes
        .iter()
        .find(|(k, _)| k == "NAME")
        .map(|(_, v)| v.clone());
    let name = resolve_name(fields.name, attribute_name, &tag)?;
    let alter_id = fields.alter_id.as_deref().map(parse_alter_id).transpose()?;
    Ok(Built {
        tag,
        guid,
        alter_id,
        name,
        parent: fields.parent.filter(|p| !p.is_empty()),
        attributes,
        extra: fields.extra,
    })
}

/// How a message element (a child of `TALLYMESSAGE` or `COLLECTION`) is treated.
enum Class {
    Master(MasterKind),
    Voucher,
    Other,
}

fn classify(tag: &str) -> Class {
    match MasterKind::from_tag(tag) {
        Some(kind) => Class::Master(kind),
        None if tag == "VOUCHER" => Class::Voucher,
        None => Class::Other,
    }
}

struct Parser<'a> {
    scanner: Scanner<'a>,
    /// Names of the open elements, outermost first.
    path: Vec<String>,
    /// Elements of the master currently being built, outermost first.
    open: Vec<RawElement>,
    open_kind: Option<MasterKind>,
    open_offset: usize,
    root_closed: bool,
    container_seen: bool,
    out: MastersParsed,
    /// Read the import form, where `GUID` is optional ([`parse_import_masters_bytes`]).
    lenient: bool,
    /// Masters read in the import form.
    imported: Vec<ImportMaster>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            scanner: Scanner::new(text),
            path: Vec::new(),
            open: Vec::new(),
            open_kind: None,
            open_offset: 0,
            root_closed: false,
            container_seen: false,
            out: MastersParsed::default(),
            lenient: false,
            imported: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<(), TallyXmlError> {
        loop {
            match self.scanner.next_token()? {
                Token::Start {
                    name,
                    attributes,
                    self_closing,
                    offset,
                } => self.on_start(&name, attributes, self_closing, offset)?,
                Token::End { name, offset } => self.on_end(&name, offset)?,
                Token::Text { text, offset } => self.on_text(&text, offset)?,
                Token::Eof => {
                    return if self.path.is_empty() && self.root_closed {
                        Ok(())
                    } else {
                        Err(self.scanner.truncated())
                    };
                }
            }
        }
    }

    fn warn(&mut self, offset: usize, message: String) {
        self.out.warnings.push(XmlWarning { offset, message });
    }

    fn on_start(
        &mut self,
        name: &str,
        attributes: Vec<(String, String)>,
        self_closing: bool,
        offset: usize,
    ) -> Result<(), TallyXmlError> {
        if self.path.is_empty() {
            if self.root_closed {
                return Err(xml_error(offset, "more than one root element"));
            }
            if name != "ENVELOPE" {
                return Err(TallyXmlError::Missing {
                    tag: "ENVELOPE".to_owned(),
                    path: "document root".to_owned(),
                });
            }
        }
        if self.path.len() >= MAX_DEPTH {
            return Err(xml_error(
                offset,
                format!("elements nested deeper than {MAX_DEPTH} levels"),
            ));
        }
        self.begin(name, attributes, offset);
        self.path.push(name.to_owned());
        if self_closing {
            self.on_end(name, offset)?;
        }
        Ok(())
    }

    /// Decides whether the element that just opened starts, or is part of, a master.
    fn begin(&mut self, name: &str, attributes: Vec<(String, String)>, offset: usize) {
        if name == "TALLYMESSAGE" || name == "COLLECTION" {
            self.container_seen = true;
        }
        let element = RawElement {
            name: name.to_owned(),
            attributes,
            text: String::new(),
            children: Vec::new(),
        };
        if !self.open.is_empty() {
            self.open.push(element);
            return;
        }
        let in_container = self
            .path
            .last()
            .is_some_and(|p| p == "TALLYMESSAGE" || p == "COLLECTION");
        if !in_container {
            return;
        }
        match classify(name) {
            Class::Master(kind) => {
                self.open_kind = Some(kind);
                self.open_offset = offset;
                self.open.push(element);
            }
            Class::Voucher => {}
            Class::Other => self.warn(offset, format!("skipped unsupported element <{name}>")),
        }
    }

    fn on_end(&mut self, name: &str, offset: usize) -> Result<(), TallyXmlError> {
        let Some(open_name) = self.path.pop() else {
            return Err(xml_error(offset, format!("unexpected end tag </{name}>")));
        };
        if open_name != name {
            return Err(xml_error(
                offset,
                format!("end tag </{name}> does not match <{open_name}>"),
            ));
        }
        if self.path.is_empty() {
            self.root_closed = true;
        }
        let Some(mut element) = self.open.pop() else {
            return Ok(());
        };
        if !element.children.is_empty() && element.text.trim().is_empty() {
            element.text.clear();
        }
        if let Some(parent) = self.open.last_mut() {
            parent.children.push(element);
            return Ok(());
        }
        self.finish_master(element)
    }

    fn on_text(&mut self, text: &str, offset: usize) -> Result<(), TallyXmlError> {
        if let Some(element) = self.open.last_mut() {
            element.text.push_str(text);
            return Ok(());
        }
        if self.path.is_empty() && !text.trim().is_empty() {
            return Err(xml_error(offset, "text outside the root element"));
        }
        Ok(())
    }

    fn finish_master(&mut self, element: RawElement) -> Result<(), TallyXmlError> {
        let offset = self.open_offset;
        let Some(kind) = self.open_kind.take() else {
            return Err(xml_error(offset, "master element without a kind"));
        };
        if self.lenient {
            let built = build_fields(element, offset, &mut self.out.warnings, false)?;
            self.imported.push(ImportMaster {
                kind: kind.public(),
                guid: built.guid,
                alter_id: built.alter_id,
                name: built.name,
                parent: built.parent,
                attributes: built.attributes,
                extra: built.extra,
            });
            return Ok(());
        }
        let record = build_record(element, offset, &mut self.out.warnings)?;
        self.out.masters.push(kind.wrap(record));
        Ok(())
    }

    /// Merges warnings and turns the outcome into the public result.
    fn finish(self, result: Result<(), TallyXmlError>) -> Result<MastersParsed, MastersError> {
        let Self {
            scanner,
            open,
            open_offset,
            container_seen,
            out,
            ..
        } = self;
        let mut parsed = out;
        parsed.warnings.extend(scanner.warnings);
        match (&result, open.first()) {
            (Err(_), Some(element)) => parsed.warnings.push(XmlWarning {
                offset: open_offset,
                message: format!("discarded incomplete <{}> element", element.name),
            }),
            (Ok(()), _) if !container_seen => parsed.warnings.push(XmlWarning {
                offset: 0,
                message: "no TALLYMESSAGE or COLLECTION element found; no masters read".to_owned(),
            }),
            (Err(_), None) | (Ok(()), _) => {}
        }
        parsed.warnings.sort_by_key(|w| w.offset);
        match result {
            Ok(()) => Ok(parsed),
            Err(error) => Err(MastersError {
                error,
                partial: Box::new(parsed),
            }),
        }
    }
}
