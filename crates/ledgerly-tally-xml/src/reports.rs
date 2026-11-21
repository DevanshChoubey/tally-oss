//! Tally report exports. **Card X5** implements this module.
//!
//! Frozen interface (plan card X5):
//!
//! ```text
//! pub struct TallyReports { pub tb: Vec<TReportRow>, pub bs: Vec<TReportRow>, pub pl: Vec<TReportRow>, pub stock: Vec<TReportRow>, pub outstanding: Vec<TReportRow> }
//! pub fn report_request(company: &str, report: TallyReportKind, from: Date, to: Date) -> String;
//! pub fn parse_report(env: &Envelope, kind: TallyReportKind) -> Result<Vec<TReportRow>, TallyXmlError>;
//! ```
//!
//! # Status
//!
//! * `parse_report(&Envelope, ..)` cannot be written yet: `Envelope` is card X1's type and
//!   `envelope.rs` is still the Wave 0 contract text. [`parse_report_xml`] takes the decoded
//!   document text and yields the same rows plus warnings; `parse_report` becomes a one-line
//!   wrapper once X1 lands (same approach as `vouchers::parse_vouchers_xml`).
//! * The tag names this parser reads (`DSPACCNAME`, `DSPACCINFO`, `BSNAME`, `BSAMT`, `DSPSTKINFO`,
//!   `BILLFIXED`, ...) and the report ids in [`report_request`] are **recalled, not observed**.
//!   Research note R8 only documents the request shape (section 2.3(a)). The test fixtures are
//!   hand-built in that recalled shape, so every assertion is `derived` from the fixture text
//!   alone and the layout stays `pending` an `observed` capture (Prerequisite 4). A layout that
//!   does not match is an error or a warning, never a guess (rule A1).
//! * Sign meaning is established only where a column names its side (Trial Balance
//!   `DSPCLDRAMTA` = debit, `DSPCLCRAMTA` = credit; the R8 section 2.3(e) convention is that a
//!   debit is written negative). For every other amount the sign is kept as written
//!   ([`TReportAmount::Signed`]) and never turned into a side.
//!
//! Totals rows are flagged ([`TReportRow::is_total`]) and kept apart from detail rows by
//! [`split_totals`].

use std::iter::Peekable;
use std::slice::Iter;
use std::str::FromStr;

use ledgerly_core::{Date, Decimal, DrCr, Paise};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{TallyXmlError, XmlWarning};

/// Deepest element nesting accepted; deeper input is refused instead of recursed into.
const MAX_DEPTH: usize = 128;

/// Which Tally report to ask for or parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TallyReportKind {
    /// Trial Balance.
    TrialBalance,
    /// Balance Sheet.
    BalanceSheet,
    /// Profit and Loss account.
    ProfitAndLoss,
    /// Stock Summary.
    StockSummary,
    /// Bills receivable (party outstandings, debtors).
    BillsReceivable,
    /// Bills payable (party outstandings, creditors).
    BillsPayable,
}

impl TallyReportKind {
    /// The report id written in the request `HEADER/ID` (recalled, `pending` observed capture).
    #[must_use]
    pub const fn report_id(self) -> &'static str {
        match self {
            TallyReportKind::TrialBalance => "Trial Balance",
            TallyReportKind::BalanceSheet => "Balance Sheet",
            TallyReportKind::ProfitAndLoss => "Profit and Loss",
            TallyReportKind::StockSummary => "Stock Summary",
            TallyReportKind::BillsReceivable => "Bills Receivable",
            TallyReportKind::BillsPayable => "Bills Payable",
        }
    }
}

/// The amount of a report row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TReportAmount {
    /// Debit and credit columns, both magnitudes (never negative). A detail row has at most one
    /// non-zero side; a totals row may have both.
    Columns {
        /// Debit column.
        debit: Paise,
        /// Credit column.
        credit: Paise,
    },
    /// One amount with its sign exactly as written; the sign is not interpreted as a side.
    Signed(Paise),
    /// The row carries no amount.
    Nil,
}

/// One row of an exported report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TReportRow {
    /// Display name (ledger, group, stock item or bill reference).
    pub label: String,
    /// Tally GUID, when the export carries one.
    pub guid: Option<String>,
    /// Party name (bills only).
    pub party: Option<String>,
    /// Bill date text as written (bills only).
    pub date_text: Option<String>,
    /// `true` for a totals row; kept apart from detail rows (see [`split_totals`]).
    pub is_total: bool,
    /// The amount.
    pub amount: TReportAmount,
}

impl TReportRow {
    /// The side and magnitude of a [`TReportAmount::Columns`] amount with exactly one non-zero
    /// column; `None` for anything else (both sides, neither side, signed or nil).
    #[must_use]
    pub fn dr_cr(&self) -> Option<(DrCr, Paise)> {
        match self.amount {
            TReportAmount::Columns { debit, credit } => match (debit.is_zero(), credit.is_zero()) {
                (false, true) => Some((DrCr::Dr, debit)),
                (true, false) => Some((DrCr::Cr, credit)),
                (true, true) | (false, false) => None,
            },
            TReportAmount::Signed(_) | TReportAmount::Nil => None,
        }
    }
}

/// The five exported reports of one company and period.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TallyReports {
    /// Trial Balance rows.
    pub tb: Vec<TReportRow>,
    /// Balance Sheet rows.
    pub bs: Vec<TReportRow>,
    /// Profit and Loss rows.
    pub pl: Vec<TReportRow>,
    /// Stock Summary rows.
    pub stock: Vec<TReportRow>,
    /// Bills outstanding rows (receivable and payable, in the order the caller appended them).
    pub outstanding: Vec<TReportRow>,
}

/// Splits rows into `(details, totals)`, each in document order.
#[must_use]
pub fn split_totals(rows: &[TReportRow]) -> (Vec<&TReportRow>, Vec<&TReportRow>) {
    rows.iter().partition(|r| !r.is_total)
}

/// Builds the export request for one report over `from..=to`, shaped as in R8 section 2.3(a).
///
/// The company name is XML-escaped. Dates are written `YYYYMMDD`.
#[must_use]
pub fn report_request(company: &str, report: TallyReportKind, from: Date, to: Date) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><TALLYREQUEST>Export</TALLYREQUEST>\
         <TYPE>Data</TYPE><ID>{id}</ID></HEADER><BODY><DESC><STATICVARIABLES>\
         <SVCURRENTCOMPANY>{company}</SVCURRENTCOMPANY>\
         <SVFROMDATE>{from}</SVFROMDATE><SVTODATE>{to}</SVTODATE>\
         <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>\
         </STATICVARIABLES></DESC></BODY></ENVELOPE>",
        id = escape(report.report_id()),
        company = escape(company),
        from = yyyymmdd(from),
        to = yyyymmdd(to),
    )
}

fn yyyymmdd(date: Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------------

/// An element of the parsed document (attributes are not used by report exports).
struct Node {
    name: String,
    text: String,
    offset: usize,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Trimmed text at a child path, `None` when the path is absent or the text is empty.
    fn text_at(&self, path: &[&str]) -> Option<&str> {
        let mut node = self;
        for step in path {
            node = node.child(step)?;
        }
        let t = node.text.trim();
        if t.is_empty() { None } else { Some(t) }
    }

    fn paise_at(&self, path: &[&str]) -> Result<Option<Paise>, TallyXmlError> {
        let tag = path.last().copied().unwrap_or(self.name.as_str());
        self.text_at(path)
            .map(|raw| parse_paise(tag, raw))
            .transpose()
    }
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

fn warn(offset: usize, message: String) -> XmlWarning {
    XmlWarning { offset, message }
}

fn resolve_ref(r: &quick_xml::events::BytesRef<'_>) -> Option<String> {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return Some(c.to_string());
    }
    let name = r.xml_content(quick_xml::XmlVersion::Implicit1_0);
    quick_xml::escape::resolve_predefined_entity(&name).map(str::to_owned)
}

fn new_node(reader: &Reader<&[u8]>, start: &BytesStart<'_>) -> Node {
    Node {
        name: start.name().as_ref().to_owned(),
        text: String::new(),
        offset: offset_of(reader),
        children: Vec::new(),
    }
}

fn attach(
    reader: &Reader<&[u8]>,
    stack: &mut [Node],
    root: &mut Option<Node>,
    node: Node,
) -> Result<(), TallyXmlError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
        Ok(())
    } else if root.is_none() {
        *root = Some(node);
        Ok(())
    } else {
        Err(xml_err(reader, &"more than one root element"))
    }
}

/// Reads the whole document into one tree and returns its root element.
fn build_tree(text: &str) -> Result<Node, TallyXmlError> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    loop {
        let event = reader.read_event().map_err(|e| xml_err(&reader, &e))?;
        match event {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(xml_err(&reader, &"elements nested too deeply"));
                }
                stack.push(new_node(&reader, &e));
            }
            Event::Empty(e) => {
                let node = new_node(&reader, &e);
                attach(&reader, &mut stack, &mut root, node)?;
            }
            Event::End(_) => {
                let Some(node) = stack.pop() else {
                    return Err(xml_err(&reader, &"closing tag without an opening tag"));
                };
                attach(&reader, &mut stack, &mut root, node)?;
            }
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
                    let resolved = resolve_ref(&r)
                        .ok_or_else(|| xml_err(&reader, &"unknown entity reference"))?;
                    top.text.push_str(&resolved);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if !stack.is_empty() {
        return Err(TallyXmlError::Truncated {
            offset: offset_of(&reader),
        });
    }
    root.ok_or_else(|| missing("ENVELOPE", "document"))
}

/// Parses a decimal rupee amount into exact paise; refuses anything that would need rounding.
fn parse_paise(tag: &str, raw: &str) -> Result<Paise, TallyXmlError> {
    let dec = Decimal::from_str(raw.trim())
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

fn is_total_label(label: &str) -> bool {
    let t = label.trim();
    t.eq_ignore_ascii_case("Grand Total") || t.eq_ignore_ascii_case("Total")
}

/// Parses one exported report from the decoded document text (what `envelope::decode` returns).
///
/// Rows come back in document order; totals rows are flagged, not dropped. Elements the layout
/// does not model are reported as warnings with their byte offset (rule A9), never discarded
/// silently.
///
/// # Errors
/// [`TallyXmlError::Xml`] for malformed XML or nesting deeper than 128 levels,
/// [`TallyXmlError::Truncated`] when the document ends inside an element,
/// [`TallyXmlError::Missing`] when there is no root element, a row has no name or no amount
/// element, or an amount element appears with no row name before it, and
/// [`TallyXmlError::Value`] for an amount that is not exact paise, a trial balance amount whose
/// sign contradicts its column, a detail row with both debit and credit, or a balance sheet or
/// profit and loss row carrying both a main and a sub amount.
pub fn parse_report_xml(
    text: &str,
    kind: TallyReportKind,
) -> Result<(Vec<TReportRow>, Vec<XmlWarning>), TallyXmlError> {
    let root = build_tree(text)?;
    if root.name != "ENVELOPE" {
        return Err(missing("ENVELOPE", "document"));
    }
    let mut warnings = Vec::new();
    let mut rows = Vec::new();
    let mut it = root.children.iter().peekable();
    while let Some(node) = it.next() {
        let row = match kind {
            TallyReportKind::TrialBalance => tb_row(node, &mut it, &mut warnings)?,
            TallyReportKind::BalanceSheet | TallyReportKind::ProfitAndLoss => {
                bs_row(node, &mut it, &mut warnings)?
            }
            TallyReportKind::StockSummary => stock_row(node, &mut it, &mut warnings)?,
            TallyReportKind::BillsReceivable | TallyReportKind::BillsPayable => {
                bill_row(node, &mut it, &mut warnings)?
            }
        };
        rows.extend(row);
    }
    Ok((rows, warnings))
}

type Nodes<'a> = Peekable<Iter<'a, Node>>;

/// Takes the body element that must follow a row name.
fn take_body<'a>(
    it: &mut Nodes<'a>,
    body: &str,
    name_tag: &str,
    label: &str,
) -> Result<&'a Node, TallyXmlError> {
    it.next_if(|n| n.name == body)
        .ok_or_else(|| missing(body, &format!("{name_tag} '{label}'")))
}

fn orphan(node: &Node) -> TallyXmlError {
    missing("row name", &format!("before <{}>", node.name))
}

fn unknown(node: &Node, warnings: &mut Vec<XmlWarning>) -> Option<TReportRow> {
    warnings.push(warn(
        node.offset,
        format!(
            "element <{}> is not part of this report layout; ignored",
            node.name
        ),
    ));
    None
}

fn warn_unmodelled(parent: &Node, known: &[&str], warnings: &mut Vec<XmlWarning>) {
    for c in parent
        .children
        .iter()
        .filter(|c| !known.contains(&c.name.as_str()))
    {
        warnings.push(warn(
            c.offset,
            format!(
                "element <{}> inside <{}> is not modelled; ignored",
                c.name, parent.name
            ),
        ));
    }
}

fn label_of(name_node: &Node, path: &[&str]) -> Result<String, TallyXmlError> {
    name_node
        .text_at(path)
        .map(str::to_owned)
        .ok_or_else(|| missing(path.last().copied().unwrap_or("name"), &name_node.name))
}

fn guid_of(name_node: &Node) -> Option<String> {
    name_node.text_at(&["GUID"]).map(str::to_owned)
}

fn columns(
    label: &str,
    is_total: bool,
    dr: Option<Paise>,
    cr: Option<Paise>,
) -> Result<TReportAmount, TallyXmlError> {
    if dr.is_none() && cr.is_none() {
        return Ok(TReportAmount::Nil);
    }
    // R8 2.3(e): a debit is written negative, so the debit column holds zero or a negative.
    let debit = match dr {
        None => Paise::ZERO,
        Some(p) if p.is_negative() => p
            .checked_neg()
            .map_err(|e| value_err("DSPCLDRAMTA", label, &e.to_string()))?,
        Some(p) if p.is_zero() => p,
        Some(p) => {
            return Err(value_err(
                "DSPCLDRAMTA",
                &p.to_rupees().to_string(),
                "positive amount in the debit column: sign contradicts the column",
            ));
        }
    };
    let credit = match cr {
        None => Paise::ZERO,
        Some(p) if p.is_negative() => {
            return Err(value_err(
                "DSPCLCRAMTA",
                &p.to_rupees().to_string(),
                "negative amount in the credit column: sign contradicts the column",
            ));
        }
        Some(p) => p,
    };
    if !is_total && !debit.is_zero() && !credit.is_zero() {
        return Err(value_err(
            "DSPACCINFO",
            label,
            "a detail row has both a debit and a credit closing amount",
        ));
    }
    Ok(TReportAmount::Columns { debit, credit })
}

fn tb_row(
    node: &Node,
    it: &mut Nodes<'_>,
    warnings: &mut Vec<XmlWarning>,
) -> Result<Option<TReportRow>, TallyXmlError> {
    match node.name.as_str() {
        "DSPACCNAME" => {
            let label = label_of(node, &["DSPDISPNAME"])?;
            let info = take_body(it, "DSPACCINFO", "DSPACCNAME", &label)?;
            warn_unmodelled(info, &["DSPCLDRAMT", "DSPCLCRAMT"], warnings);
            let dr = info.paise_at(&["DSPCLDRAMT", "DSPCLDRAMTA"])?;
            let cr = info.paise_at(&["DSPCLCRAMT", "DSPCLCRAMTA"])?;
            let is_total = is_total_label(&label);
            let amount = columns(&label, is_total, dr, cr)?;
            Ok(Some(TReportRow {
                guid: guid_of(node),
                label,
                party: None,
                date_text: None,
                is_total,
                amount,
            }))
        }
        "DSPACCINFO" => Err(orphan(node)),
        _ => Ok(unknown(node, warnings)),
    }
}

fn bs_row(
    node: &Node,
    it: &mut Nodes<'_>,
    warnings: &mut Vec<XmlWarning>,
) -> Result<Option<TReportRow>, TallyXmlError> {
    match node.name.as_str() {
        "BSNAME" => {
            let label = label_of(node, &["DSPACCNAME", "DSPDISPNAME"])?;
            let Some(body) = it.next_if(|n| n.name == "BSAMT" || n.name == "PLAMT") else {
                return Err(missing("BSAMT", &format!("BSNAME '{label}'")));
            };
            warn_unmodelled(
                body,
                &["BSSUBAMT", "BSMAINAMT", "PLSUBAMT", "PLMAINAMT"],
                warnings,
            );
            let sub = body
                .paise_at(&["BSSUBAMT"])?
                .or(body.paise_at(&["PLSUBAMT"])?);
            let main = body
                .paise_at(&["BSMAINAMT"])?
                .or(body.paise_at(&["PLMAINAMT"])?);
            let amount = match (sub, main) {
                (Some(_), Some(_)) => {
                    return Err(value_err(
                        "BSAMT",
                        &label,
                        "both a main and a sub amount: which one the row means is not established",
                    ));
                }
                (Some(p), None) | (None, Some(p)) => TReportAmount::Signed(p),
                (None, None) => TReportAmount::Nil,
            };
            Ok(Some(TReportRow {
                guid: guid_of(node),
                is_total: is_total_label(&label),
                label,
                party: None,
                date_text: None,
                amount,
            }))
        }
        "BSAMT" | "PLAMT" => Err(orphan(node)),
        _ => Ok(unknown(node, warnings)),
    }
}

fn stock_row(
    node: &Node,
    it: &mut Nodes<'_>,
    warnings: &mut Vec<XmlWarning>,
) -> Result<Option<TReportRow>, TallyXmlError> {
    match node.name.as_str() {
        "DSPACCNAME" => {
            let label = label_of(node, &["DSPDISPNAME"])?;
            let info = take_body(it, "DSPSTKINFO", "DSPACCNAME", &label)?;
            warn_unmodelled(info, &["DSPSTKCL"], warnings);
            if let Some(cl) = info.child("DSPSTKCL") {
                warn_unmodelled(cl, &["DSPCLAMTA"], warnings);
            }
            let amount = info
                .paise_at(&["DSPSTKCL", "DSPCLAMTA"])?
                .map_or(TReportAmount::Nil, TReportAmount::Signed);
            Ok(Some(TReportRow {
                guid: guid_of(node),
                is_total: is_total_label(&label),
                label,
                party: None,
                date_text: None,
                amount,
            }))
        }
        "DSPSTKINFO" => Err(orphan(node)),
        _ => Ok(unknown(node, warnings)),
    }
}

fn bill_row(
    node: &Node,
    it: &mut Nodes<'_>,
    warnings: &mut Vec<XmlWarning>,
) -> Result<Option<TReportRow>, TallyXmlError> {
    match node.name.as_str() {
        "BILLFIXED" => {
            let label = label_of(node, &["BILLREF"])?;
            warn_unmodelled(
                node,
                &["BILLREF", "BILLPARTY", "BILLDATE", "GUID"],
                warnings,
            );
            let closing = take_body(it, "BILLCL", "BILLFIXED", &label)?;
            let amount = closing
                .paise_at(&[])?
                .map_or(TReportAmount::Nil, TReportAmount::Signed);
            Ok(Some(TReportRow {
                guid: guid_of(node),
                label,
                party: node.text_at(&["BILLPARTY"]).map(str::to_owned),
                date_text: node.text_at(&["BILLDATE"]).map(str::to_owned),
                is_total: false,
                amount,
            }))
        }
        "BILLCL" => Err(orphan(node)),
        _ => Ok(unknown(node, warnings)),
    }
}
