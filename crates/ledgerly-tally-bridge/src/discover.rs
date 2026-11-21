//! Company discovery (card B1): ask a running Tally which companies it has loaded.
//!
//! Frozen interface (plan card B1):
//!
//! ```text
//! pub fn list_companies(t: &dyn TallyTransport) -> Result<Vec<String>, BridgeError>;   // BridgeError defined by B1 here
//! ```
//!
//! [`BridgeError`] is the error type for the whole crate; the other cards (pull, mapping,
//! reconcile) return it too.
//!
//! The reply is read with a small purpose-built scanner, not the Tally XML crate, because that
//! crate's envelope parser (card X1) is not built yet. The scanner is strict: an unreadable
//! document, an invalid character or an unknown entity is an error with the byte offset, never
//! repaired silently (rule A1).
//!
//! Tag and attribute names are matched ignoring ASCII case. This leniency is deliberate: the
//! casing Tally uses in replies has not been observed, and the names involved cannot collide.
//! Whether Tally accepts a UTF-8 request body is also unobserved.
//!
//! Shape assumption, **not yet observed**: the request is an inline collection over `Company`
//! (research R8 section 2.3 example (b)), and the reply lists each company as
//! `<COMPANY NAME="..."><NAME>...</NAME></COMPANY>` inside `<COLLECTION>`. Both the attribute and
//! the child element are accepted. Until a real TallyPrime reply is captured the reply shape is
//! `pending`; a reply of any other shape returns [`BridgeError::UnexpectedResponse`].

use ledgerly_tally_xml::TallyXmlError;
use thiserror::Error;

use crate::seam::{TallyTransport, TransportError};

/// Errors from the bridge. Each says what failed and, where it can, what to do.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BridgeError {
    /// Tally could not be reached, or answered with an HTTP error.
    #[error(
        "{0}. Check that TallyPrime is open and its HTTP server is enabled (F12, Advanced Configuration)."
    )]
    Transport(#[from] TransportError),
    /// Tally's XML could not be read by the XML crate.
    #[error(transparent)]
    Xml(#[from] TallyXmlError),
    /// The reply is not well-formed enough to read.
    #[error("Tally's reply cannot be read at byte {offset}: {reason}")]
    MalformedResponse {
        /// Byte offset in the decoded reply.
        offset: usize,
        /// What is wrong.
        reason: String,
    },
    /// The reply is readable but is not the answer to the question asked.
    #[error("Tally's reply is not the expected answer: {reason}")]
    UnexpectedResponse {
        /// What was missing or different.
        reason: String,
    },
    /// Tally understood the request and reported a problem.
    #[error("Tally reported a problem: {message}")]
    TallyReported {
        /// Tally's own text.
        message: String,
    },
    /// The user cancelled the operation; nothing partial was kept.
    #[error("cancelled")]
    Cancelled,
    /// A file could not be read or written (capture and staging cards use this).
    #[error("file {path}: {reason}")]
    Io {
        /// The file.
        path: String,
        /// The cause.
        reason: String,
    },
}

/// The request sent to Tally to list its loaded companies.
fn list_companies_request() -> String {
    "<ENVELOPE><HEADER><VERSION>1</VERSION><TALLYREQUEST>Export</TALLYREQUEST>\
     <TYPE>Collection</TYPE><ID>LedgerlyCompanies</ID></HEADER>\
     <BODY><DESC><STATICVARIABLES><SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT></STATICVARIABLES>\
     <TDL><TDLMESSAGE><COLLECTION NAME=\"LedgerlyCompanies\"><TYPE>Company</TYPE>\
     <FETCH>Name</FETCH></COLLECTION></TDLMESSAGE></TDL></DESC></BODY></ENVELOPE>"
        .to_owned()
}

/// Lists the names of the companies Tally has loaded, in the order Tally gives them, each once.
///
/// An empty list means Tally answered and has no company loaded.
///
/// # Errors
/// [`BridgeError::Transport`] when Tally is not running (the error names the port) or answers
/// with an HTTP error; [`BridgeError::MalformedResponse`] when the reply cannot be read;
/// [`BridgeError::TallyReported`] when Tally reports an error; [`BridgeError::UnexpectedResponse`]
/// when the reply has no company collection or a company without a name.
pub fn list_companies(t: &dyn TallyTransport) -> Result<Vec<String>, BridgeError> {
    let reply = t.post(&list_companies_request())?;
    parse_company_list(&reply)
}

/// One lexical item of the reply.
#[derive(Debug)]
enum Tok {
    Start {
        name: String,
        attrs: String,
        /// Byte offset of the tag's `<`.
        offset: usize,
        /// Byte offset where `attrs` begins.
        attrs_offset: usize,
        empty: bool,
    },
    End {
        name: String,
        offset: usize,
    },
    Text {
        text: String,
    },
}

fn malformed(offset: usize, reason: &str) -> BridgeError {
    BridgeError::MalformedResponse {
        offset,
        reason: reason.to_owned(),
    }
}

fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// Resolves entities in `raw`, rejecting characters XML does not allow (for example the
/// `&#4;` Tally sometimes emits, research R8 2.1). `base` is the byte offset of `raw`.
fn decode_text(raw: &str, base: usize) -> Result<String, BridgeError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut consumed = 0usize;
    while let Some(amp) = rest.find('&') {
        let (before, after) = rest.split_at(amp);
        push_checked(&mut out, before, base.saturating_add(consumed))?;
        let at = base.saturating_add(consumed).saturating_add(amp);
        let Some(semi) = after.find(';') else {
            return Err(malformed(at, "'&' without a closing ';'"));
        };
        let name = after.get(1..semi).unwrap_or("");
        let c = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            numeric => numeric_entity(numeric)
                .ok_or_else(|| malformed(at, "unknown entity or a character XML does not allow"))?,
        };
        out.push(c);
        let step = semi.saturating_add(1);
        consumed = consumed.saturating_add(amp).saturating_add(step);
        rest = after.get(step..).unwrap_or("");
    }
    push_checked(&mut out, rest, base.saturating_add(consumed))?;
    Ok(out)
}

fn numeric_entity(name: &str) -> Option<char> {
    let digits = name.strip_prefix('#')?;
    let code = match digits.strip_prefix('x') {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<u32>().ok()?,
    };
    char::from_u32(code).filter(|c| is_xml_char(*c))
}

fn push_checked(out: &mut String, piece: &str, base: usize) -> Result<(), BridgeError> {
    for (i, c) in piece.char_indices() {
        if !is_xml_char(c) {
            return Err(malformed(
                base.saturating_add(i),
                "character that XML does not allow",
            ));
        }
    }
    out.push_str(piece);
    Ok(())
}

/// Index of the `>` that closes the tag in `s` (which starts after the `<`), skipping quoted values.
#[expect(
    clippy::match_same_arms,
    reason = "the empty arms are distinct states of the quote scanner, kept apart for reading"
)]
fn tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(i),
            None => {}
        }
    }
    None
}

/// Splits the reply into tags and text. Comments and processing instructions are dropped;
/// CDATA becomes text.
fn tokenize(text: &str) -> Result<Vec<Tok>, BridgeError> {
    let mut toks = Vec::new();
    let mut pos = 0usize;
    while let Some(rest) = text.get(pos..).filter(|r| !r.is_empty()) {
        let advance = if rest.starts_with('<') {
            lex_markup(rest, pos, &mut toks)?
        } else {
            let n = rest.find('<').unwrap_or(rest.len());
            let raw = rest.get(..n).unwrap_or("");
            toks.push(Tok::Text {
                text: decode_text(raw, pos)?,
            });
            n
        };
        pos = pos.saturating_add(advance);
    }
    Ok(toks)
}

/// Lexes one `<...>` item at the start of `rest` (which sits at byte `pos`); returns its length.
fn lex_markup(rest: &str, pos: usize, toks: &mut Vec<Tok>) -> Result<usize, BridgeError> {
    if let Some(body) = rest.strip_prefix("<!--") {
        let end = body
            .find("-->")
            .ok_or_else(|| malformed(pos, "comment is not closed"))?;
        return Ok(end.saturating_add(7));
    }
    if let Some(body) = rest.strip_prefix("<![CDATA[") {
        let end = body
            .find("]]>")
            .ok_or_else(|| malformed(pos, "CDATA section is not closed"))?;
        let mut checked = String::new();
        push_checked(
            &mut checked,
            body.get(..end).unwrap_or(""),
            pos.saturating_add(9),
        )?;
        toks.push(Tok::Text { text: checked });
        return Ok(end.saturating_add(12));
    }
    if let Some(body) = rest.strip_prefix("<?") {
        let end = body
            .find("?>")
            .ok_or_else(|| malformed(pos, "processing instruction is not closed"))?;
        return Ok(end.saturating_add(4));
    }
    if rest.starts_with("<!") {
        let end = rest
            .find('>')
            .ok_or_else(|| malformed(pos, "declaration is not closed"))?;
        return Ok(end.saturating_add(1));
    }
    let after_lt = rest.get(1..).unwrap_or("");
    let end = tag_end(after_lt).ok_or_else(|| malformed(pos, "tag is not closed"))?;
    toks.push(tag_token(after_lt.get(..end).unwrap_or(""), pos)?);
    Ok(end.saturating_add(2))
}

fn tag_token(inner: &str, offset: usize) -> Result<Tok, BridgeError> {
    if let Some(name) = inner.strip_prefix('/') {
        let name = name.trim();
        if name.is_empty() {
            return Err(malformed(offset, "closing tag has no name"));
        }
        return Ok(Tok::End {
            name: name.to_owned(),
            offset,
        });
    }
    let trimmed = inner.trim_end();
    let (body, empty) = match trimmed.strip_suffix('/') {
        Some(b) => (b, true),
        None => (trimmed, false),
    };
    let name_len = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = body.get(..name_len).unwrap_or("");
    if name.is_empty() {
        return Err(malformed(offset, "tag has no name"));
    }
    Ok(Tok::Start {
        name: name.to_owned(),
        attrs: body.get(name_len..).unwrap_or("").to_owned(),
        offset,
        attrs_offset: offset.saturating_add(1).saturating_add(name_len),
        empty,
    })
}

/// The decoded value of attribute `want` (compared without regard to case) in `attrs`, which
/// begins at byte `base` of the reply.
fn attribute(attrs: &str, want: &str, base: usize) -> Result<Option<String>, BridgeError> {
    let at = |rest: &str| base.saturating_add(attrs.len().saturating_sub(rest.len()));
    let mut rest = attrs.trim_start();
    let mut found = None;
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else {
            return Err(malformed(at(rest), "attribute without '='"));
        };
        let name = rest.get(..eq).unwrap_or("").trim();
        let after = rest.get(eq.saturating_add(1)..).unwrap_or("").trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            return Err(malformed(at(after), "attribute value is not quoted"));
        };
        let value_area = after.get(1..).unwrap_or("");
        let Some(close) = value_area.find(quote) else {
            return Err(malformed(at(value_area), "attribute value is not closed"));
        };
        if name.eq_ignore_ascii_case(want) && found.is_none() {
            let raw = value_area.get(..close).unwrap_or("");
            found = Some(decode_text(raw, at(value_area))?);
        }
        rest = value_area
            .get(close.saturating_add(1)..)
            .unwrap_or("")
            .trim_start();
    }
    Ok(found)
}

/// State while walking the tokens of a reply.
#[derive(Default)]
struct Scan {
    stack: Vec<String>,
    names: Vec<String>,
    saw_envelope: bool,
    saw_collection: bool,
    /// The company being read: byte offset of its tag and the name from its NAME attribute.
    company: Option<(usize, Option<String>)>,
    child_name: Option<String>,
    /// Text of a `RESPONSE` or `LINEERROR` element: Tally reporting a problem.
    message: Option<String>,
    status_zero: bool,
}

impl Scan {
    fn push_company(&mut self, name: &str, offset: usize) -> Result<(), BridgeError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BridgeError::UnexpectedResponse {
                reason: format!("a company entry at byte {offset} has no name"),
            });
        }
        if !self.names.iter().any(|n| n == name) {
            self.names.push(name.to_owned());
        }
        Ok(())
    }

    fn start_message(&mut self) {
        if self.message.is_none() {
            self.message = Some(String::new());
        }
    }

    fn start(
        &mut self,
        name: &str,
        attrs: &str,
        offset: usize,
        attrs_offset: usize,
        empty: bool,
    ) -> Result<(), BridgeError> {
        if self.stack.is_empty() && !self.saw_envelope {
            if name.eq_ignore_ascii_case("ENVELOPE") {
                self.saw_envelope = true;
            } else if name.eq_ignore_ascii_case("RESPONSE") {
                // Tally answers a request it cannot process with a bare <RESPONSE>text.
                self.start_message();
            } else {
                return Err(BridgeError::UnexpectedResponse {
                    reason: format!("the reply starts with <{name}>, not <ENVELOPE>"),
                });
            }
        }
        if name.eq_ignore_ascii_case("COLLECTION") {
            self.saw_collection = true;
        }
        if name.eq_ignore_ascii_case("LINEERROR") && !empty {
            self.start_message();
        }
        if name.eq_ignore_ascii_case("COMPANY") && self.company.is_none() {
            let attr = attribute(attrs, "NAME", attrs_offset)?;
            if empty {
                self.push_company(attr.as_deref().unwrap_or(""), offset)?;
            } else {
                self.company = Some((offset, attr));
                self.child_name = None;
            }
        }
        if !empty {
            self.stack.push(name.to_owned());
        }
        Ok(())
    }

    fn end(&mut self, name: &str, offset: usize) -> Result<(), BridgeError> {
        match self.stack.pop() {
            Some(open) if open.eq_ignore_ascii_case(name) => {}
            Some(open) => return Err(malformed(offset, &format!("</{name}> closes <{open}>"))),
            None => {
                return Err(malformed(
                    offset,
                    &format!("</{name}> has no matching start tag"),
                ));
            }
        }
        if name.eq_ignore_ascii_case("COMPANY")
            && let Some((tag_offset, attr)) = self.company.take()
        {
            let chosen = self.child_name.take().or(attr).unwrap_or_default();
            self.push_company(&chosen, tag_offset)?;
        }
        Ok(())
    }

    fn text(&mut self, text: &str) {
        let parent = self.stack.last().map_or("", String::as_str);
        if (parent.eq_ignore_ascii_case("RESPONSE") || parent.eq_ignore_ascii_case("LINEERROR"))
            && let Some(m) = self.message.as_mut()
        {
            m.push_str(text);
        }
        if parent.eq_ignore_ascii_case("STATUS") && text.trim() == "0" {
            self.status_zero = true;
        }
        if self.company.is_some() && parent.eq_ignore_ascii_case("NAME") {
            self.child_name
                .get_or_insert_with(String::new)
                .push_str(text);
        }
    }

    fn finish(self, reply_len: usize) -> Result<Vec<String>, BridgeError> {
        if let Some(open) = self.stack.last() {
            return Err(malformed(reply_len, &format!("reply ends inside <{open}>")));
        }
        if let Some(m) = self
            .message
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
        {
            return Err(BridgeError::TallyReported {
                message: m.to_owned(),
            });
        }
        if self.status_zero {
            return Err(BridgeError::TallyReported {
                message: "the reply header has STATUS 0 (failure)".to_owned(),
            });
        }
        if !self.saw_envelope {
            return Err(BridgeError::UnexpectedResponse {
                reason: "the reply has no <ENVELOPE>".to_owned(),
            });
        }
        if !self.saw_collection {
            return Err(BridgeError::UnexpectedResponse {
                reason: "the reply has no <COLLECTION> of companies".to_owned(),
            });
        }
        Ok(self.names)
    }
}

/// Reads the company names out of a decoded reply. See the module docs for the assumed shape.
fn parse_company_list(reply: &str) -> Result<Vec<String>, BridgeError> {
    let mut scan = Scan::default();
    for tok in tokenize(reply)? {
        match tok {
            Tok::Start {
                name,
                attrs,
                offset,
                attrs_offset,
                empty,
            } => scan.start(&name, &attrs, offset, attrs_offset, empty)?,
            Tok::End { name, offset } => scan.end(&name, offset)?,
            Tok::Text { text } => scan.text(&text),
        }
    }
    scan.finish(reply.len())
}

#[cfg(test)]
mod tests {
    // provenance: derived. Every fixture is a hand-written, fictional document in the assumed
    // reply shape (module docs); every expected value is a literal typed into the fixture
    // itself (a company name, a byte position counted by hand), never our own output. No
    // fixture was captured from a TallyPrime release, so the reply shape stays `pending`.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: rule B2 lets tests unwrap and panic"
    )]

    use super::*;

    fn collection(inner: &str) -> String {
        format!(
            "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>\
             <COLLECTION>{inner}</COLLECTION></DATA></BODY></ENVELOPE>"
        )
    }

    #[test]
    fn request_is_an_export_of_the_company_collection() {
        let r = list_companies_request();
        assert!(r.starts_with("<ENVELOPE>"));
        assert!(r.contains("<TALLYREQUEST>Export</TALLYREQUEST>"));
        assert!(r.contains("<TYPE>Company</TYPE>"));
    }

    #[test]
    fn names_from_child_elements() {
        let doc = collection(
            "<COMPANY><NAME TYPE=\"String\">Acme Traders</NAME></COMPANY>\
             <COMPANY><NAME>Beta Stores</NAME></COMPANY>",
        );
        assert_eq!(
            parse_company_list(&doc).unwrap(),
            vec!["Acme Traders".to_owned(), "Beta Stores".to_owned()]
        );
    }

    #[test]
    fn names_from_attributes_and_self_closing_tags() {
        let doc =
            collection("<COMPANY NAME=\"Gamma &amp; Sons\"/><COMPANY NAME='Delta'></COMPANY>");
        assert_eq!(
            parse_company_list(&doc).unwrap(),
            vec!["Gamma & Sons".to_owned(), "Delta".to_owned()]
        );
    }

    #[test]
    fn child_name_wins_over_attribute() {
        let doc = collection("<COMPANY NAME=\"Old\"><NAME>New</NAME></COMPANY>");
        assert_eq!(parse_company_list(&doc).unwrap(), vec!["New".to_owned()]);
    }

    #[test]
    fn duplicates_are_listed_once_in_first_seen_order() {
        let doc = collection("<COMPANY NAME=\"B\"/><COMPANY NAME=\"A\"/><COMPANY NAME=\"B\"/>");
        assert_eq!(
            parse_company_list(&doc).unwrap(),
            vec!["B".to_owned(), "A".to_owned()]
        );
    }

    #[test]
    fn empty_collection_means_no_company_loaded() {
        assert_eq!(
            parse_company_list(&collection("")).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn rupee_sign_and_numeric_entities_decode() {
        // &#8377; is U+20B9 (decimal 8377 = 0x20B9); &#x41; is 'A'.
        let doc = collection("<COMPANY NAME=\"&#8377; &#x41;\"/>");
        assert_eq!(
            parse_company_list(&doc).unwrap(),
            vec!["\u{20B9} A".to_owned()]
        );
    }

    #[test]
    fn invalid_character_entity_is_an_error_with_position() {
        // &#4; is the stray sequence Tally emits (R8 2.1); it is not an allowed XML character.
        let prefix = "<ENVELOPE><COLLECTION><COMPANY NAME=\"";
        let doc = format!("{prefix}A&#4;B\"/></COLLECTION></ENVELOPE>");
        let err = parse_company_list(&doc).unwrap_err();
        // '<ENVELOPE><COLLECTION><COMPANY NAME="' is 10+12+8+7 = 37 bytes, then 'A' is byte 37 and '&' is byte 38.
        assert_eq!(
            err,
            BridgeError::MalformedResponse {
                offset: 38,
                reason: "unknown entity or a character XML does not allow".to_owned(),
            }
        );
    }

    #[test]
    fn raw_control_character_is_an_error() {
        let doc = collection("<COMPANY><NAME>A\u{4}B</NAME></COMPANY>");
        assert!(matches!(
            parse_company_list(&doc),
            Err(BridgeError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn truncated_reply_is_an_error() {
        let doc = "<ENVELOPE><BODY><COLLECTION><COMPANY NAME=\"A\"/>";
        assert!(matches!(
            parse_company_list(doc),
            Err(BridgeError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn unterminated_tag_reports_its_offset() {
        let err = parse_company_list("<ENVELOPE><COLL").unwrap_err();
        assert_eq!(
            err,
            BridgeError::MalformedResponse {
                offset: 10,
                reason: "tag is not closed".to_owned(),
            }
        );
    }

    #[test]
    fn mismatched_closing_tag_is_an_error() {
        let err = parse_company_list("<ENVELOPE></BODY></ENVELOPE>").unwrap_err();
        assert!(matches!(err, BridgeError::MalformedResponse { .. }));
    }

    #[test]
    fn bare_response_text_is_tally_reporting_a_problem() {
        let err = parse_company_list("<RESPONSE>Unknown Request, cannot be processed</RESPONSE>")
            .unwrap_err();
        assert_eq!(
            err,
            BridgeError::TallyReported {
                message: "Unknown Request, cannot be processed".to_owned()
            }
        );
    }

    #[test]
    fn line_error_is_reported() {
        let doc = "<ENVELOPE><BODY><DATA><LINEERROR>Could not set SVCurrentCompany</LINEERROR></DATA></BODY></ENVELOPE>";
        assert_eq!(
            parse_company_list(doc).unwrap_err(),
            BridgeError::TallyReported {
                message: "Could not set SVCurrentCompany".to_owned()
            }
        );
    }

    #[test]
    fn status_zero_is_a_failure() {
        let doc =
            "<ENVELOPE><HEADER><STATUS>0</STATUS></HEADER><BODY><COLLECTION/></BODY></ENVELOPE>";
        assert!(matches!(
            parse_company_list(doc),
            Err(BridgeError::TallyReported { .. })
        ));
    }

    #[test]
    fn reply_that_is_not_an_envelope_is_unexpected() {
        assert!(matches!(
            parse_company_list("<HTML><BODY>hi</BODY></HTML>"),
            Err(BridgeError::UnexpectedResponse { .. })
        ));
        assert!(matches!(
            parse_company_list(""),
            Err(BridgeError::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn envelope_without_collection_is_unexpected() {
        assert!(matches!(
            parse_company_list("<ENVELOPE><BODY/></ENVELOPE>"),
            Err(BridgeError::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn company_without_a_name_is_unexpected() {
        assert!(matches!(
            parse_company_list(&collection("<COMPANY></COMPANY>")),
            Err(BridgeError::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn comments_declarations_and_cdata_are_skipped_or_kept() {
        let doc = "<?xml version=\"1.0\"?><!-- hi --><ENVELOPE><COLLECTION>\
                   <COMPANY><NAME><![CDATA[A <b> Co]]></NAME></COMPANY></COLLECTION></ENVELOPE>";
        assert_eq!(
            parse_company_list(doc).unwrap(),
            vec!["A <b> Co".to_owned()]
        );
    }

    #[test]
    fn display_names_the_port_and_a_next_step() {
        let e = BridgeError::from(TransportError::TallyNotRunning { port: 9000 });
        let text = e.to_string();
        assert!(text.contains("9000"));
        assert!(text.contains("HTTP server"));
    }
}
