//! Export request builders. **Card X1**.
//!
//! Frozen interface (plan card X1):
//!
//! ```text
//! pub fn list_companies_request() -> String;
//! pub fn masters_request(company: &str, collection: MasterCollection) -> String;
//! pub fn vouchers_request(company: &str, from: Date, to: Date) -> String;
//! ```
//!
//! # Shape and provenance
//!
//! Every request is an `ENVELOPE` with a `HEADER` (`VERSION` 1, `TALLYREQUEST` `Export`, `TYPE`,
//! `ID`) and a `BODY/DESC` holding `STATICVARIABLES` (`SVCURRENTCOMPANY`, `SVFROMDATE`,
//! `SVTODATE`, `SVEXPORTFORMAT`) and, for masters and vouchers, an inline `TDL/TDLMESSAGE`
//! `COLLECTION` naming an object type and the fields to `FETCH`. That shape is the documented one
//! in `docs/research/08-data-interop-editions.md` sections 2.2 and 2.3 (examples a and b). Dates
//! are `YYYYMMDD`.
//!
//! Not yet observed against a running TallyPrime (so the tests that cover them are `derived` from
//! the documented shape only, and a captured export must confirm them): the report name
//! `List of Companies` and its `TYPE`; the object names in `COLLECTION/TYPE` (`Stock Item`,
//! `Voucher Type`, ...); and the `FETCH` field names, including the nested voucher entries.
//! Research 08 says the same of every field name.
//!
//! # Invalid input
//!
//! The three frozen builders return a `String` and cannot fail. A company name is escaped
//! (`&`, `<`, `>`), and a character XML cannot carry is written as a numeric reference, which the
//! receiver rejects as malformed rather than reading a different company. Use
//! [`try_masters_request`] and [`try_vouchers_request`] to reject bad input before sending.

use ledgerly_core::Date;

use crate::TallyXmlError;
use crate::envelope::is_xml_char;

/// `SVEXPORTFORMAT` value that asks for XML.
const EXPORT_FORMAT_XML: &str = "$$SysName:XML";
/// The common fields fetched for every master kind.
const COMMON_FETCH: &str = "GUID, AlterID, Name, Parent";
/// Ledger fields fetched in addition (names as in research 08 section 2.3 example b).
const LEDGER_FETCH: &str = "GUID, AlterID, Name, Parent, OpeningBalance, PartyGSTIN";
/// Voucher fields fetched, with the nested entry lists.
const VOUCHER_FETCH: &str = "GUID, AlterID, Date, VoucherTypeName, VoucherNumber, \
PartyLedgerName, Narration, AllLedgerEntries.*, AllInventoryEntries.*";

/// A kind of master that can be exported as a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterCollection {
    /// Account groups (`GROUP`).
    Groups,
    /// Ledgers (`LEDGER`).
    Ledgers,
    /// Stock items (`STOCKITEM`).
    StockItems,
    /// Units of measure (`UNIT`).
    Units,
    /// Godowns (`GODOWN`).
    Godowns,
    /// Voucher types (`VOUCHERTYPE`).
    VoucherTypes,
    /// Cost centres (`COSTCENTRE`).
    CostCentres,
    /// Cost categories (`COSTCATEGORY`).
    CostCategories,
    /// Stock groups (`STOCKGROUP`).
    StockGroups,
}

impl MasterCollection {
    /// Every kind, in the order the importer reads them (parents before children).
    pub const ALL: [MasterCollection; 9] = [
        MasterCollection::Groups,
        MasterCollection::Ledgers,
        MasterCollection::Units,
        MasterCollection::StockGroups,
        MasterCollection::StockItems,
        MasterCollection::Godowns,
        MasterCollection::VoucherTypes,
        MasterCollection::CostCategories,
        MasterCollection::CostCentres,
    ];

    /// The name of the inline collection, also the request `ID`.
    #[must_use]
    pub fn collection_name(self) -> &'static str {
        match self {
            MasterCollection::Groups => "LedgerlyGroups",
            MasterCollection::Ledgers => "LedgerlyLedgers",
            MasterCollection::StockItems => "LedgerlyStockItems",
            MasterCollection::Units => "LedgerlyUnits",
            MasterCollection::Godowns => "LedgerlyGodowns",
            MasterCollection::VoucherTypes => "LedgerlyVoucherTypes",
            MasterCollection::CostCentres => "LedgerlyCostCentres",
            MasterCollection::CostCategories => "LedgerlyCostCategories",
            MasterCollection::StockGroups => "LedgerlyStockGroups",
        }
    }

    /// The object type named in `COLLECTION/TYPE`.
    #[must_use]
    pub fn object_type(self) -> &'static str {
        match self {
            MasterCollection::Groups => "Group",
            MasterCollection::Ledgers => "Ledger",
            MasterCollection::StockItems => "Stock Item",
            MasterCollection::Units => "Unit",
            MasterCollection::Godowns => "Godown",
            MasterCollection::VoucherTypes => "Voucher Type",
            MasterCollection::CostCentres => "Cost Centre",
            MasterCollection::CostCategories => "Cost Category",
            MasterCollection::StockGroups => "Stock Group",
        }
    }

    /// The `FETCH` list sent for this kind.
    #[must_use]
    pub fn fetch_fields(self) -> &'static str {
        match self {
            MasterCollection::Ledgers => LEDGER_FETCH,
            MasterCollection::Groups
            | MasterCollection::StockItems
            | MasterCollection::Units
            | MasterCollection::Godowns
            | MasterCollection::VoucherTypes
            | MasterCollection::CostCentres
            | MasterCollection::CostCategories
            | MasterCollection::StockGroups => COMMON_FETCH,
        }
    }
}

/// An inline `COLLECTION` definition.
struct CollectionDef<'a> {
    name: &'a str,
    object_type: &'a str,
    fetch: &'a str,
}

/// Lines of XML with two-space indentation.
struct Doc {
    lines: Vec<String>,
}

impl Doc {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn line(&mut self, depth: usize, text: &str) {
        self.lines.push(format!("{}{}", "  ".repeat(depth), text));
    }

    fn finish(self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }
}

/// Renders an export envelope. `variables` values must already be escaped.
fn render(
    kind: &str,
    id: &str,
    variables: &[(&str, String)],
    collection: Option<&CollectionDef<'_>>,
) -> String {
    let mut doc = Doc::new();
    doc.line(0, "<ENVELOPE>");
    doc.line(1, "<HEADER>");
    doc.line(2, "<VERSION>1</VERSION>");
    doc.line(2, "<TALLYREQUEST>Export</TALLYREQUEST>");
    doc.line(2, &format!("<TYPE>{kind}</TYPE>"));
    doc.line(2, &format!("<ID>{id}</ID>"));
    doc.line(1, "</HEADER>");
    doc.line(1, "<BODY>");
    doc.line(2, "<DESC>");
    doc.line(3, "<STATICVARIABLES>");
    for (tag, value) in variables {
        doc.line(4, &format!("<{tag}>{value}</{tag}>"));
    }
    doc.line(3, "</STATICVARIABLES>");
    if let Some(def) = collection {
        doc.line(3, "<TDL>");
        doc.line(4, "<TDLMESSAGE>");
        doc.line(5, &format!("<COLLECTION NAME=\"{}\">", def.name));
        doc.line(6, &format!("<TYPE>{}</TYPE>", def.object_type));
        doc.line(6, &format!("<FETCH>{}</FETCH>", def.fetch));
        doc.line(5, "</COLLECTION>");
        doc.line(4, "</TDLMESSAGE>");
        doc.line(3, "</TDL>");
    }
    doc.line(2, "</DESC>");
    doc.line(1, "</BODY>");
    doc.line(0, "</ENVELOPE>");
    doc.finish()
}

/// Escapes element text. `&`, `<` and `>` become entities; a character XML 1.0 forbids becomes
/// a numeric reference (which a receiver rejects as malformed).
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other if is_xml_char(other) => out.push(other),
            other => {
                out.push_str("&#");
                out.push_str(&u32::from(other).to_string());
                out.push(';');
            }
        }
    }
    out
}

/// `YYYYMMDD`, the date form of `SVFROMDATE` and `SVTODATE`.
fn format_date(date: Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn check_company(company: &str) -> Result<(), TallyXmlError> {
    if company.trim().is_empty() {
        return Err(TallyXmlError::Value {
            tag: "SVCURRENTCOMPANY".to_owned(),
            value: company.to_owned(),
            reason: "the company name is empty".to_owned(),
        });
    }
    if let Some(c) = company.chars().find(|c| !is_xml_char(*c)) {
        return Err(TallyXmlError::Value {
            tag: "SVCURRENTCOMPANY".to_owned(),
            value: company.to_owned(),
            reason: format!(
                "the company name has the character U+{:04X}, which XML cannot carry",
                u32::from(c)
            ),
        });
    }
    Ok(())
}

fn check_date(tag: &str, date: Date) -> Result<(), TallyXmlError> {
    if (1..=9999).contains(&date.year()) {
        return Ok(());
    }
    Err(TallyXmlError::Value {
        tag: tag.to_owned(),
        value: format_date(date),
        reason: "the year must be between 1 and 9999 to fit YYYYMMDD".to_owned(),
    })
}

/// The request for the list of companies Tally has open or loaded.
///
/// `TYPE` is `Collection` and `ID` is `List of Companies`; no company is selected, since the
/// answer names the companies. Neither value is observed yet (see the module documentation).
#[must_use]
pub fn list_companies_request() -> String {
    let variables = [("SVEXPORTFORMAT", EXPORT_FORMAT_XML.to_owned())];
    render("Collection", "List of Companies", &variables, None)
}

/// The request for every master of one kind in `company`.
///
/// The company name is escaped; see the module documentation for what happens to a character XML
/// cannot carry. Use [`try_masters_request`] to reject such a name instead.
#[must_use]
pub fn masters_request(company: &str, collection: MasterCollection) -> String {
    let variables = [
        ("SVCURRENTCOMPANY", escape_text(company)),
        ("SVEXPORTFORMAT", EXPORT_FORMAT_XML.to_owned()),
    ];
    let def = CollectionDef {
        name: collection.collection_name(),
        object_type: collection.object_type(),
        fetch: collection.fetch_fields(),
    };
    render("Collection", def.name, &variables, Some(&def))
}

/// The request for every voucher dated from `from` to `to` (both inclusive) in `company`.
///
/// Dates are written `YYYYMMDD`. The caller is expected to pass `from <= to` and four-digit
/// years; [`try_vouchers_request`] checks both.
#[must_use]
pub fn vouchers_request(company: &str, from: Date, to: Date) -> String {
    let variables = [
        ("SVCURRENTCOMPANY", escape_text(company)),
        ("SVFROMDATE", format_date(from)),
        ("SVTODATE", format_date(to)),
        ("SVEXPORTFORMAT", EXPORT_FORMAT_XML.to_owned()),
    ];
    let def = CollectionDef {
        name: "LedgerlyVouchers",
        object_type: "Voucher",
        fetch: VOUCHER_FETCH,
    };
    render("Collection", def.name, &variables, Some(&def))
}

/// [`masters_request`] after checking the company name.
///
/// # Errors
///
/// [`TallyXmlError::Value`] when the company name is empty or has a character XML cannot carry.
pub fn try_masters_request(
    company: &str,
    collection: MasterCollection,
) -> Result<String, TallyXmlError> {
    check_company(company)?;
    Ok(masters_request(company, collection))
}

/// [`vouchers_request`] after checking the company name and the dates.
///
/// # Errors
///
/// [`TallyXmlError::Value`] when the company name is empty or has a character XML cannot carry,
/// when a year is outside 1 to 9999, or when `to` is before `from`.
pub fn try_vouchers_request(company: &str, from: Date, to: Date) -> Result<String, TallyXmlError> {
    check_company(company)?;
    check_date("SVFROMDATE", from)?;
    check_date("SVTODATE", to)?;
    if to < from {
        return Err(TallyXmlError::Value {
            tag: "SVTODATE".to_owned(),
            value: format_date(to),
            reason: format!(
                "the end date is before the start date {}",
                format_date(from)
            ),
        });
    }
    Ok(vouchers_request(company, from, to))
}
