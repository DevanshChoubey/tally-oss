//! Decoding and the envelope (`UTF-8`/`UTF-16`, invalid characters, truncation). **Card X1**.
//!
//! Frozen interface (plan card X1):
//!
//! ```text
//! pub struct Envelope { pub header: Header, pub messages: Vec<RawMessage> }
//! pub fn decode(bytes: &[u8]) -> Result<(String, Vec<XmlWarning>), TallyXmlError>;
//! pub fn parse_envelope(text: &str) -> Result<(Envelope, Vec<XmlWarning>), TallyXmlError>;
//! ```
//!
//! # Shape
//!
//! The envelope shape is the one documented in `docs/research/08-data-interop-editions.md`
//! section 2.2: `ENVELOPE` holds a `HEADER` and a `BODY`. [`Header`] reads the `HEADER` fields.
//! Inside `BODY` the wrapper elements `DATA`, `IMPORTDATA` and `REQUESTDATA` are looked through;
//! every `TALLYMESSAGE` (import form) and `COLLECTION` (export form) found that way becomes a
//! [`RawMessage`] holding its child elements verbatim. Every other element of the body (`DESC`,
//! `REQUESTDESC`, result counters, ...) and every other child of `ENVELOPE` is kept whole in
//! [`Envelope::extras`], so nothing is dropped (rule A9). The element-level parsers (cards X2 and
//! X3) read the elements of a message.
//!
//! # Leniency
//!
//! Tally's output is not always well-formed XML (research 08 section 2.1). [`decode`] drops raw
//! characters XML forbids and reports each; the parser drops characters it still meets, decodes
//! `&#4;`-style references to forbidden characters away, and keeps an unknown or bare `&`
//! literally, each time with an [`XmlWarning`]. Structure is strict: a mismatched end tag, a
//! `DOCTYPE`, a repeated attribute or nesting deeper than 128 levels is an error.
//!
//! # Offsets
//!
//! [`decode`] reports decode errors as byte offsets into the raw input and warnings as byte
//! offsets into the decoded text before repair. Parser warnings and errors are byte offsets into
//! the text given to the parser.
//!
//! # Truncation
//!
//! [`parse_envelope`] returns an error when the document ends early. [`parse_envelope_partial`]
//! returns the same error together with everything read before it; a message that was still open
//! is discarded and reported as a warning.

use crate::masters::RawElement;
use crate::{TallyXmlError, XmlWarning};

/// Deepest element nesting accepted, so hostile input cannot exhaust the stack or memory.
const MAX_DEPTH: usize = 128;
/// Longest entity or character reference name that is looked at.
const MAX_ENTITY_LEN: usize = 32;
/// Elements below `BODY` that only wrap messages and are looked through.
const WRAPPERS: [&str; 3] = ["DATA", "IMPORTDATA", "REQUESTDATA"];

/// The `HEADER` of an envelope. A field the document does not carry is `None`; text is trimmed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Header {
    /// `VERSION`.
    pub version: Option<String>,
    /// `TALLYREQUEST` (`Import`, `Export`, `Execute`, `Import Data`) of a request.
    pub request: Option<String>,
    /// `TYPE` (`Data`, `Collection`, `Object`, `Function`) of a request.
    pub kind: Option<String>,
    /// `SUBTYPE`.
    pub sub_type: Option<String>,
    /// `ID`: the report, collection, object or function a request names.
    pub id: Option<String>,
    /// `STATUS` of a response (`1` success, `0` failure per research 08 section 2.2).
    pub status: Option<String>,
    /// Header children this card does not model, in document order.
    pub extra: Vec<RawElement>,
}

/// One `TALLYMESSAGE` (import form) or `COLLECTION` (export form) with its child elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMessage {
    /// `TALLYMESSAGE` or `COLLECTION`.
    pub tag: String,
    /// Attributes of the message element (for example `xmlns:UDF`), in document order.
    pub attributes: Vec<(String, String)>,
    /// Child elements in document order, kept verbatim.
    pub elements: Vec<RawElement>,
    /// Byte offset of the message's start tag in the parsed text.
    pub offset: usize,
}

/// A parsed envelope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Envelope {
    /// The `HEADER`; all fields `None` when the document has none (a warning is raised).
    pub header: Header,
    /// Every message in the body, in document order.
    pub messages: Vec<RawMessage>,
    /// Body and envelope elements that are neither the header nor a message (`DESC`, result
    /// counters, ...), kept whole. An addition to the frozen interface, for rule A9.
    pub extras: Vec<RawElement>,
}

/// The result of [`parse_envelope_partial`]: what was read, and the error that stopped it, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEnvelope {
    /// Everything read before the error (or the whole envelope when `error` is `None`).
    pub envelope: Envelope,
    /// Non-fatal problems, ordered by byte offset.
    pub warnings: Vec<XmlWarning>,
    /// Why parsing stopped early.
    pub error: Option<TallyXmlError>,
}

// ---------------------------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------------------------

/// Decodes `bytes` to text. Accepts `UTF-8` (with or without a byte order mark) and `UTF-16`
/// (little or big endian, with a byte order mark, or without one when the document starts with
/// `<` as XML 1.0 appendix F.1 detects). Raw characters XML forbids are dropped and each is
/// reported as a warning; character references such as `&#4;` are left to the parser.
///
/// # Errors
///
/// [`TallyXmlError::Decode`] with the byte offset in `bytes` of an invalid `UTF-8` sequence, an
/// unpaired `UTF-16` surrogate, or an odd trailing byte in `UTF-16` input.
pub fn decode(bytes: &[u8]) -> Result<(String, Vec<XmlWarning>), TallyXmlError> {
    let text = decode_bytes(bytes)?;
    Ok(strip_invalid_chars(text))
}

fn decode_bytes(bytes: &[u8]) -> Result<String, TallyXmlError> {
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        decode_utf16(rest, true, 2)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        decode_utf16(rest, false, 2)
    } else if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        decode_utf8(rest, 3)
    } else if bytes.starts_with(b"<\x00") {
        decode_utf16(bytes, true, 0)
    } else if bytes.starts_with(b"\x00<") {
        decode_utf16(bytes, false, 0)
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

/// Decodes `UTF-16` code units. `base` is the byte order mark length, so offsets point into the
/// raw input.
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

/// Drops the characters XML 1.0 forbids, one warning each, offset into `text` before repair.
fn strip_invalid_chars(text: String) -> (String, Vec<XmlWarning>) {
    if text.chars().all(is_xml_char) {
        return (text, Vec::new());
    }
    let mut out = String::with_capacity(text.len());
    let mut warnings = Vec::new();
    for (offset, c) in text.char_indices() {
        if is_xml_char(c) {
            out.push(c);
        } else {
            warnings.push(XmlWarning {
                offset,
                message: format!("dropped invalid character U+{:04X}", u32::from(c)),
            });
        }
    }
    (out, warnings)
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
pub(crate) fn is_xml_char(c: char) -> bool {
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
// Tree building and routing
// ---------------------------------------------------------------------------------------------

/// An element being read. `router` frames (`ENVELOPE`, `BODY` and the body wrappers) hold no
/// children: what they contain is routed straight into the [`Envelope`].
struct Frame {
    node: RawElement,
    offset: usize,
    router: bool,
}

struct Builder<'a> {
    scanner: Scanner<'a>,
    stack: Vec<Frame>,
    header: Option<Header>,
    messages: Vec<RawMessage>,
    extras: Vec<RawElement>,
    done: bool,
}

impl<'a> Builder<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            scanner: Scanner::new(text),
            stack: Vec::new(),
            header: None,
            messages: Vec::new(),
            extras: Vec::new(),
            done: false,
        }
    }

    fn run(&mut self) -> Result<(), TallyXmlError> {
        loop {
            let token = self.scanner.next_token()?;
            if self.done {
                match &token {
                    Token::Eof => return Ok(()),
                    Token::Text { text, .. } if text.trim().is_empty() => {}
                    Token::Text { offset, .. }
                    | Token::Start { offset, .. }
                    | Token::End { offset, .. } => {
                        self.scanner.warn(
                            *offset,
                            "content after the closing </ENVELOPE> ignored".to_owned(),
                        );
                        return Ok(());
                    }
                }
                continue;
            }
            match token {
                Token::Eof => return Err(self.scanner.truncated()),
                Token::Text { text, offset } => self.on_text(&text, offset)?,
                Token::Start {
                    name,
                    attributes,
                    self_closing,
                    offset,
                } => self.on_start(name, attributes, self_closing, offset)?,
                Token::End { name, offset } => self.on_end(&name, offset)?,
            }
        }
    }

    fn on_text(&mut self, text: &str, offset: usize) -> Result<(), TallyXmlError> {
        match self.stack.last_mut() {
            Some(top) => top.node.text.push_str(text),
            None if text.trim().is_empty() => {}
            None => return Err(xml_error(offset, "text before the root element")),
        }
        Ok(())
    }

    fn on_start(
        &mut self,
        name: String,
        attributes: Vec<(String, String)>,
        self_closing: bool,
        offset: usize,
    ) -> Result<(), TallyXmlError> {
        let router = match self.stack.last() {
            None => {
                if name != "ENVELOPE" {
                    return Err(TallyXmlError::Missing {
                        tag: "ENVELOPE".to_owned(),
                        path: "/".to_owned(),
                    });
                }
                true
            }
            Some(parent) => {
                if self.stack.len() >= MAX_DEPTH {
                    return Err(xml_error(
                        offset,
                        format!("elements nested deeper than {MAX_DEPTH} levels"),
                    ));
                }
                is_router_child(parent, &name)
            }
        };
        self.stack.push(Frame {
            node: RawElement {
                name,
                attributes,
                text: String::new(),
                children: Vec::new(),
            },
            offset,
            router,
        });
        if self_closing {
            self.close_top()?;
        }
        Ok(())
    }

    fn on_end(&mut self, name: &str, offset: usize) -> Result<(), TallyXmlError> {
        let Some(top) = self.stack.last() else {
            return Err(xml_error(offset, format!("unexpected end tag </{name}>")));
        };
        if top.node.name != name {
            return Err(xml_error(
                offset,
                format!(
                    "end tag </{name}> does not match <{}> opened at byte {}",
                    top.node.name, top.offset
                ),
            ));
        }
        self.close_top()
    }

    /// Closes the innermost element and puts it where it belongs.
    fn close_top(&mut self) -> Result<(), TallyXmlError> {
        let Some(mut frame) = self.stack.pop() else {
            return Ok(());
        };
        if !frame.node.children.is_empty() {
            frame.node.text = frame.node.text.trim().to_owned();
        }
        if frame.router {
            self.check_router_content(&frame);
        }
        let Some(parent) = self.stack.last() else {
            self.done = true;
            return Ok(());
        };
        let parent_router = parent.router;
        let parent_is_root = self.stack.len() == 1;
        if !parent_router {
            if let Some(parent) = self.stack.last_mut() {
                parent.node.children.push(frame.node);
            }
            return Ok(());
        }
        if frame.router {
            return Ok(());
        }
        self.route(frame, parent_is_root)
    }

    /// Warns about content of a router element that has nowhere to go.
    fn check_router_content(&mut self, frame: &Frame) {
        if !frame.node.attributes.is_empty() {
            let message = format!("attributes of <{}> ignored", frame.node.name);
            self.scanner.warn(frame.offset, message);
        }
        if !frame.node.text.trim().is_empty() {
            let message = format!("text inside <{}> ignored", frame.node.name);
            self.scanner.warn(frame.offset, message);
        }
    }

    /// Places a closed child of a router element: header, message or extra.
    fn route(&mut self, frame: Frame, parent_is_root: bool) -> Result<(), TallyXmlError> {
        let Frame { node, offset, .. } = frame;
        let is_header = parent_is_root && node.name == "HEADER";
        let is_message =
            !parent_is_root && (node.name == "TALLYMESSAGE" || node.name == "COLLECTION");
        if is_header {
            if self.header.is_some() {
                return Err(TallyXmlError::Value {
                    tag: "HEADER".to_owned(),
                    value: String::new(),
                    reason: "more than one HEADER in the envelope".to_owned(),
                });
            }
            self.header = Some(build_header(node)?);
        } else if is_message {
            if !node.text.trim().is_empty() {
                let message = format!("text inside <{}> ignored", node.name);
                self.scanner.warn(offset, message);
            }
            self.messages.push(RawMessage {
                tag: node.name,
                attributes: node.attributes,
                elements: node.children,
                offset,
            });
        } else {
            self.extras.push(node);
        }
        Ok(())
    }

    /// Warnings for messages that were open when parsing stopped, then the result.
    fn finish(mut self, error: Option<TallyXmlError>) -> ParsedEnvelope {
        if error.is_some() {
            let open: Vec<(usize, String)> = self
                .stack
                .iter()
                .zip(self.stack.iter().skip(1))
                .filter(|(parent, child)| {
                    parent.router
                        && !child.router
                        && (child.node.name == "TALLYMESSAGE" || child.node.name == "COLLECTION")
                })
                .map(|(_, child)| (child.offset, child.node.name.clone()))
                .collect();
            for (offset, name) in open {
                self.scanner
                    .warn(offset, format!("incomplete <{name}> discarded"));
            }
        } else if self.header.is_none() {
            self.scanner
                .warn(0, "the envelope has no HEADER".to_owned());
        }
        let mut warnings = self.scanner.warnings;
        warnings.sort_by_key(|w| w.offset);
        ParsedEnvelope {
            envelope: Envelope {
                header: self.header.unwrap_or_default(),
                messages: self.messages,
                extras: self.extras,
            },
            warnings,
            error,
        }
    }
}

/// Whether `name`, a child of `parent`, is itself a router element.
fn is_router_child(parent: &Frame, name: &str) -> bool {
    if !parent.router {
        return false;
    }
    if parent.node.name == "ENVELOPE" {
        name == "BODY"
    } else {
        WRAPPERS.contains(&name)
    }
}

fn build_header(node: RawElement) -> Result<Header, TallyXmlError> {
    let mut header = Header::default();
    for child in node.children {
        let slot = match child.name.as_str() {
            "VERSION" => Some(&mut header.version),
            "TALLYREQUEST" => Some(&mut header.request),
            "TYPE" => Some(&mut header.kind),
            "SUBTYPE" => Some(&mut header.sub_type),
            "ID" => Some(&mut header.id),
            "STATUS" => Some(&mut header.status),
            _ => None,
        };
        let Some(slot) = slot else {
            header.extra.push(child);
            continue;
        };
        if !child.children.is_empty() {
            return Err(TallyXmlError::Value {
                tag: child.name,
                value: String::new(),
                reason: "expected text, found child elements".to_owned(),
            });
        }
        let value = child.text.trim().to_owned();
        if slot.is_some() {
            return Err(TallyXmlError::Value {
                tag: child.name,
                value,
                reason: "repeated in HEADER".to_owned(),
            });
        }
        *slot = Some(value);
    }
    Ok(header)
}

/// Parses an envelope, keeping what was read when the document is bad.
///
/// Never panics. When `error` is `Some`, `envelope` holds every header and message completed
/// before it; a message still open is discarded and reported in `warnings`.
#[must_use]
pub fn parse_envelope_partial(text: &str) -> ParsedEnvelope {
    let mut builder = Builder::new(text);
    let error = builder.run().err();
    builder.finish(error)
}

/// Parses a decoded Tally XML document into an [`Envelope`].
///
/// # Errors
///
/// [`TallyXmlError::Truncated`] (offset is `text.len()`) when the document ends early,
/// [`TallyXmlError::Xml`] for malformed XML, a mismatched end tag, a `DOCTYPE` or nesting beyond
/// 128 levels, [`TallyXmlError::Missing`] when the root element is not `ENVELOPE`, and
/// [`TallyXmlError::Value`] for a repeated `HEADER` or repeated or nested header field. Use
/// [`parse_envelope_partial`] to keep what was read before an error.
pub fn parse_envelope(text: &str) -> Result<(Envelope, Vec<XmlWarning>), TallyXmlError> {
    let parsed = parse_envelope_partial(text);
    match parsed.error {
        Some(error) => Err(error),
        None => Ok((parsed.envelope, parsed.warnings)),
    }
}
