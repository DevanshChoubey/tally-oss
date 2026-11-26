//! Template format. **Card PR1** implements this module.
//!
//! A print template is a TOML document that says which blocks appear on a printed invoice or
//! voucher, in order. It carries no numbers: the renderer (card PR2) fills every slot from the
//! voucher and the books, so a template can never show a wrong figure (engineering rule A1).
//!
//! Frozen interface (plan card PR1):
//!
//! ```text
//! pub struct Template { .. }
//! pub fn load_template(toml: &str) -> Result<Template, PrintError>;   // PrintError defined by PR1 here
//! ```
//!
//! # Format (version 1)
//!
//! ```toml
//! format_version = 1          # required; only 1 is understood
//! name = "Tax invoice"        # required, non-empty
//! page = "A4"                 # required: "A4" or "A5"
//!
//! [[block]]                   # at least one block, printed top to bottom
//! kind = "header"             # see the block kinds below
//! slots = ["company_name", "company_gstin", "voucher_number", "voucher_date"]
//! ```
//!
//! Block kinds and the keys each takes (a key a kind does not take is an error):
//!
//! | `kind`            | required keys | optional keys |
//! |-------------------|---------------|---------------|
//! | `header`          | `slots`       | `title`       |
//! | `details`         | `slots`       | `title`       |
//! | `item_table`      | `columns`     | `title`       |
//! | `tax_table`       | `columns`     | `title`       |
//! | `totals`          | `slots`       | `title`       |
//! | `amount_in_words` |               | `title`       |
//! | `upi_qr`          |               | `title`       |
//! | `text`            | `text`        | `title`       |
//!
//! `slots` name values the renderer fills ([`Slot`]); `columns` name table columns
//! ([`ItemColumn`], [`TaxColumn`]). A name this version does not know is an error that names the
//! block, never a blank cell. At most one `item_table`, `tax_table`, `amount_in_words` and
//! `upi_qr` block may appear. A slot or column may be listed only once per block.

use serde::Deserialize;
use thiserror::Error;

/// The only template format version this crate reads.
pub const FORMAT_VERSION: u32 = 1;

/// Errors from loading or rendering a print template.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PrintError {
    /// The text is not valid TOML, or a key has the wrong type or is not part of the format.
    #[error("template is not valid: {0}")]
    Parse(String),
    /// `format_version` is not one this build reads.
    #[error("template format_version {found} is not supported (this build reads version 1)")]
    UnsupportedVersion {
        /// The version found in the template.
        found: u32,
    },
    /// The template has an empty `name`.
    #[error("template name is empty; give the template a name")]
    EmptyName,
    /// `page` is not a page size this build knows.
    #[error("template page size \"{found}\" is not known (use \"A4\" or \"A5\")")]
    UnknownPage {
        /// The page size text found.
        found: String,
    },
    /// The template lists no blocks.
    #[error("template has no [[block]] entries; add at least one")]
    NoBlocks,
    /// A block has a `kind` this build does not know.
    #[error("block {block}: unknown kind \"{found}\"")]
    UnknownBlockKind {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind text found.
        found: String,
    },
    /// A block names a slot this build does not know.
    #[error("block {block} ({kind}): unknown slot \"{found}\"")]
    UnknownSlot {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The slot name found.
        found: String,
    },
    /// A table block names a column this build does not know.
    #[error("block {block} ({kind}): unknown column \"{found}\"")]
    UnknownColumn {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The column name found.
        found: String,
    },
    /// A block lacks a key its kind needs.
    #[error("block {block} ({kind}): missing required key \"{key}\"")]
    MissingKey {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The missing key.
        key: &'static str,
    },
    /// A block has a key its kind does not take.
    #[error("block {block} ({kind}): key \"{key}\" is not used by this kind of block")]
    UnexpectedKey {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The key that does not belong.
        key: &'static str,
    },
    /// A `slots` or `columns` list is empty.
    #[error("block {block} ({kind}): \"{key}\" is empty; list at least one")]
    EmptyList {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The empty key.
        key: &'static str,
    },
    /// A slot or column is listed twice in one block.
    #[error("block {block} ({kind}): \"{found}\" is listed more than once")]
    DuplicateEntry {
        /// 1-based position of the block in the template.
        block: usize,
        /// The kind of the block.
        kind: BlockKind,
        /// The repeated slot or column name.
        found: String,
    },
    /// A block kind that may appear once appears again.
    #[error("block {block}: a {kind} block already appears at block {first}")]
    DuplicateBlock {
        /// 1-based position of the repeated block.
        block: usize,
        /// 1-based position of the first block of the same kind.
        first: usize,
        /// The repeated kind.
        kind: BlockKind,
    },
    /// Rendering failed for a reason other than the template itself (used by the PDF renderer).
    #[error("could not render: {reason}")]
    Render {
        /// What failed.
        reason: String,
    },
    /// The renderer cannot produce this input yet (engineering rule A1).
    #[error("not supported: {reason}")]
    Unsupported {
        /// What is not handled.
        reason: String,
    },
    /// A print setting (copies, page range, first page number) cannot be used with this
    /// document.
    #[error("the print settings cannot be used: {reason}")]
    Setup {
        /// What is wrong and how to fix it.
        reason: String,
    },
}

/// Paper size of a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    /// 210 mm by 297 mm.
    A4,
    /// 148 mm by 210 mm.
    A5,
}

impl PageSize {
    /// Width and height in millimetres, portrait.
    #[must_use]
    pub const fn millimetres(self) -> (u16, u16) {
        match self {
            Self::A4 => (210, 297),
            Self::A5 => (148, 210),
        }
    }
}

/// The kind of a template block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockKind {
    /// Company and document identity (name, GSTIN, number, date).
    Header,
    /// Party and supply details (buyer, place of supply).
    Details,
    /// The line-item table.
    ItemTable,
    /// The tax summary table (by HSN or rate).
    TaxTable,
    /// Totals and round-off.
    Totals,
    /// The grand total written in words.
    AmountInWords,
    /// The UPI payment QR code.
    UpiQr,
    /// Fixed text such as terms or a signature line.
    Text,
}

impl BlockKind {
    /// Every kind, in documentation order.
    pub const ALL: [Self; 8] = [
        Self::Header,
        Self::Details,
        Self::ItemTable,
        Self::TaxTable,
        Self::Totals,
        Self::AmountInWords,
        Self::UpiQr,
        Self::Text,
    ];

    /// The name used in the TOML `kind` key.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Details => "details",
            Self::ItemTable => "item_table",
            Self::TaxTable => "tax_table",
            Self::Totals => "totals",
            Self::AmountInWords => "amount_in_words",
            Self::UpiQr => "upi_qr",
            Self::Text => "text",
        }
    }

    /// Finds a kind by its TOML name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.name() == name)
    }

    /// Whether more than one block of this kind makes no sense.
    const fn at_most_once(self) -> bool {
        match self {
            Self::ItemTable | Self::TaxTable | Self::AmountInWords | Self::UpiQr => true,
            Self::Header | Self::Details | Self::Totals | Self::Text => false,
        }
    }
}

impl std::fmt::Display for BlockKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A value the renderer fills in from the voucher and the books.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// The company's name.
    CompanyName,
    /// The company's address.
    CompanyAddress,
    /// The company's GSTIN.
    CompanyGstin,
    /// The document title, such as "Tax Invoice".
    DocumentTitle,
    /// The voucher number.
    VoucherNumber,
    /// The voucher date.
    VoucherDate,
    /// The party's name.
    PartyName,
    /// The party's address.
    PartyAddress,
    /// The party's GSTIN.
    PartyGstin,
    /// The place of supply.
    PlaceOfSupply,
    /// The total taxable value.
    TaxableTotal,
    /// The total tax.
    TaxTotal,
    /// The round-off amount.
    RoundOff,
    /// The grand total.
    GrandTotal,
}

impl Slot {
    /// Every slot, in documentation order.
    pub const ALL: [Self; 14] = [
        Self::CompanyName,
        Self::CompanyAddress,
        Self::CompanyGstin,
        Self::DocumentTitle,
        Self::VoucherNumber,
        Self::VoucherDate,
        Self::PartyName,
        Self::PartyAddress,
        Self::PartyGstin,
        Self::PlaceOfSupply,
        Self::TaxableTotal,
        Self::TaxTotal,
        Self::RoundOff,
        Self::GrandTotal,
    ];

    /// The name used in a TOML `slots` list.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CompanyName => "company_name",
            Self::CompanyAddress => "company_address",
            Self::CompanyGstin => "company_gstin",
            Self::DocumentTitle => "document_title",
            Self::VoucherNumber => "voucher_number",
            Self::VoucherDate => "voucher_date",
            Self::PartyName => "party_name",
            Self::PartyAddress => "party_address",
            Self::PartyGstin => "party_gstin",
            Self::PlaceOfSupply => "place_of_supply",
            Self::TaxableTotal => "taxable_total",
            Self::TaxTotal => "tax_total",
            Self::RoundOff => "round_off",
            Self::GrandTotal => "grand_total",
        }
    }

    /// Finds a slot by its TOML name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.name() == name)
    }
}

/// A column of the item table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemColumn {
    /// Line serial number.
    Serial,
    /// Item description.
    Description,
    /// HSN or SAC code.
    Hsn,
    /// Quantity.
    Quantity,
    /// Unit of measure.
    Unit,
    /// Rate per unit.
    Rate,
    /// Discount.
    Discount,
    /// Taxable value of the line.
    TaxableValue,
    /// GST rate of the line.
    TaxRate,
    /// Line amount.
    Amount,
}

impl ItemColumn {
    /// Every column, in documentation order.
    pub const ALL: [Self; 10] = [
        Self::Serial,
        Self::Description,
        Self::Hsn,
        Self::Quantity,
        Self::Unit,
        Self::Rate,
        Self::Discount,
        Self::TaxableValue,
        Self::TaxRate,
        Self::Amount,
    ];

    /// The name used in a TOML `columns` list.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::Description => "description",
            Self::Hsn => "hsn",
            Self::Quantity => "quantity",
            Self::Unit => "unit",
            Self::Rate => "rate",
            Self::Discount => "discount",
            Self::TaxableValue => "taxable_value",
            Self::TaxRate => "tax_rate",
            Self::Amount => "amount",
        }
    }

    /// Finds a column by its TOML name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}

/// A column of the tax table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaxColumn {
    /// HSN or SAC code.
    Hsn,
    /// Taxable value.
    TaxableValue,
    /// CGST rate.
    CgstRate,
    /// CGST amount.
    CgstAmount,
    /// SGST or UTGST rate.
    SgstRate,
    /// SGST or UTGST amount.
    SgstAmount,
    /// IGST rate.
    IgstRate,
    /// IGST amount.
    IgstAmount,
    /// Cess amount.
    CessAmount,
    /// Total tax of the row.
    TotalTax,
}

impl TaxColumn {
    /// Every column, in documentation order.
    pub const ALL: [Self; 10] = [
        Self::Hsn,
        Self::TaxableValue,
        Self::CgstRate,
        Self::CgstAmount,
        Self::SgstRate,
        Self::SgstAmount,
        Self::IgstRate,
        Self::IgstAmount,
        Self::CessAmount,
        Self::TotalTax,
    ];

    /// The name used in a TOML `columns` list.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hsn => "hsn",
            Self::TaxableValue => "taxable_value",
            Self::CgstRate => "cgst_rate",
            Self::CgstAmount => "cgst_amount",
            Self::SgstRate => "sgst_rate",
            Self::SgstAmount => "sgst_amount",
            Self::IgstRate => "igst_rate",
            Self::IgstAmount => "igst_amount",
            Self::CessAmount => "cess_amount",
            Self::TotalTax => "total_tax",
        }
    }

    /// Finds a column by its TOML name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}

/// What a block prints. Slot and column lists are never empty and never repeat an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockBody {
    /// A `header` block.
    Header {
        /// The slots, in print order.
        slots: Vec<Slot>,
    },
    /// A `details` block.
    Details {
        /// The slots, in print order.
        slots: Vec<Slot>,
    },
    /// An `item_table` block.
    ItemTable {
        /// The columns, left to right.
        columns: Vec<ItemColumn>,
    },
    /// A `tax_table` block.
    TaxTable {
        /// The columns, left to right.
        columns: Vec<TaxColumn>,
    },
    /// A `totals` block.
    Totals {
        /// The slots, in print order.
        slots: Vec<Slot>,
    },
    /// An `amount_in_words` block: the grand total in words, filled by the renderer.
    AmountInWords,
    /// A `upi_qr` block: the UPI payment QR code, filled by the renderer.
    UpiQr,
    /// A `text` block with fixed text.
    Text {
        /// The text to print as written.
        text: String,
    },
}

impl BlockBody {
    /// The kind of this body.
    #[must_use]
    pub const fn kind(&self) -> BlockKind {
        match self {
            Self::Header { .. } => BlockKind::Header,
            Self::Details { .. } => BlockKind::Details,
            Self::ItemTable { .. } => BlockKind::ItemTable,
            Self::TaxTable { .. } => BlockKind::TaxTable,
            Self::Totals { .. } => BlockKind::Totals,
            Self::AmountInWords => BlockKind::AmountInWords,
            Self::UpiQr => BlockKind::UpiQr,
            Self::Text { .. } => BlockKind::Text,
        }
    }
}

/// One block of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// An optional heading printed above the block.
    pub title: Option<String>,
    /// What the block prints.
    pub body: BlockBody,
}

/// A loaded, validated print template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// The template's name.
    pub name: String,
    /// The paper size.
    pub page: PageSize,
    /// The blocks, printed top to bottom. Never empty.
    pub blocks: Vec<Block>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    format_version: u32,
    name: String,
    page: String,
    #[serde(default)]
    block: Vec<RawBlock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlock {
    kind: String,
    title: Option<String>,
    slots: Option<Vec<String>>,
    columns: Option<Vec<String>>,
    text: Option<String>,
}

/// Loads and validates a template from TOML text.
///
/// # Errors
///
/// [`PrintError::Parse`] for invalid TOML or an unknown key; the other variants name the block
/// (1-based) and the slot, column, kind or key at fault. Nothing is defaulted: a missing
/// required key is an error.
pub fn load_template(toml: &str) -> Result<Template, PrintError> {
    let raw: RawTemplate = ::toml::from_str(toml).map_err(|e| PrintError::Parse(e.to_string()))?;
    if raw.format_version != FORMAT_VERSION {
        return Err(PrintError::UnsupportedVersion {
            found: raw.format_version,
        });
    }
    if raw.name.trim().is_empty() {
        return Err(PrintError::EmptyName);
    }
    let page = match raw.page.as_str() {
        "A4" => PageSize::A4,
        "A5" => PageSize::A5,
        other => {
            return Err(PrintError::UnknownPage {
                found: other.to_owned(),
            });
        }
    };
    if raw.block.is_empty() {
        return Err(PrintError::NoBlocks);
    }
    let mut blocks: Vec<Block> = Vec::with_capacity(raw.block.len());
    for (i, raw_block) in raw.block.iter().enumerate() {
        let number = i.saturating_add(1);
        let block = convert_block(number, raw_block)?;
        let kind = block.body.kind();
        if kind.at_most_once() {
            let first = blocks
                .iter()
                .position(|b| b.body.kind() == kind)
                .map(|p| p.saturating_add(1));
            if let Some(first) = first {
                return Err(PrintError::DuplicateBlock {
                    block: number,
                    first,
                    kind,
                });
            }
        }
        blocks.push(block);
    }
    Ok(Template {
        name: raw.name,
        page,
        blocks,
    })
}

fn convert_block(number: usize, raw: &RawBlock) -> Result<Block, PrintError> {
    let kind = BlockKind::parse(&raw.kind).ok_or_else(|| PrintError::UnknownBlockKind {
        block: number,
        found: raw.kind.clone(),
    })?;
    let body = match kind {
        BlockKind::Header => {
            reject(number, kind, "columns", raw.columns.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            BlockBody::Header {
                slots: slots(number, kind, raw)?,
            }
        }
        BlockKind::Details => {
            reject(number, kind, "columns", raw.columns.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            BlockBody::Details {
                slots: slots(number, kind, raw)?,
            }
        }
        BlockKind::Totals => {
            reject(number, kind, "columns", raw.columns.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            BlockBody::Totals {
                slots: slots(number, kind, raw)?,
            }
        }
        BlockKind::ItemTable => {
            reject(number, kind, "slots", raw.slots.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            let names = required(number, kind, "columns", raw.columns.as_deref())?;
            BlockBody::ItemTable {
                columns: parse_list(number, kind, names, ItemColumn::parse, column_error)?,
            }
        }
        BlockKind::TaxTable => {
            reject(number, kind, "slots", raw.slots.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            let names = required(number, kind, "columns", raw.columns.as_deref())?;
            BlockBody::TaxTable {
                columns: parse_list(number, kind, names, TaxColumn::parse, column_error)?,
            }
        }
        BlockKind::AmountInWords | BlockKind::UpiQr => {
            reject(number, kind, "slots", raw.slots.is_some())?;
            reject(number, kind, "columns", raw.columns.is_some())?;
            reject(number, kind, "text", raw.text.is_some())?;
            if kind == BlockKind::UpiQr {
                BlockBody::UpiQr
            } else {
                BlockBody::AmountInWords
            }
        }
        BlockKind::Text => {
            reject(number, kind, "slots", raw.slots.is_some())?;
            reject(number, kind, "columns", raw.columns.is_some())?;
            let text = raw.text.clone().ok_or(PrintError::MissingKey {
                block: number,
                kind,
                key: "text",
            })?;
            BlockBody::Text { text }
        }
    };
    Ok(Block {
        title: raw.title.clone(),
        body,
    })
}

fn reject(
    number: usize,
    kind: BlockKind,
    key: &'static str,
    present: bool,
) -> Result<(), PrintError> {
    if present {
        Err(PrintError::UnexpectedKey {
            block: number,
            kind,
            key,
        })
    } else {
        Ok(())
    }
}

fn required<'a>(
    number: usize,
    kind: BlockKind,
    key: &'static str,
    value: Option<&'a [String]>,
) -> Result<&'a [String], PrintError> {
    value.ok_or(PrintError::MissingKey {
        block: number,
        kind,
        key,
    })
}

fn slots(number: usize, kind: BlockKind, raw: &RawBlock) -> Result<Vec<Slot>, PrintError> {
    let names = required(number, kind, "slots", raw.slots.as_deref())?;
    parse_list(number, kind, names, Slot::parse, slot_error)
}

fn slot_error(block: usize, kind: BlockKind, found: &str) -> PrintError {
    PrintError::UnknownSlot {
        block,
        kind,
        found: found.to_owned(),
    }
}

fn column_error(block: usize, kind: BlockKind, found: &str) -> PrintError {
    PrintError::UnknownColumn {
        block,
        kind,
        found: found.to_owned(),
    }
}

/// Parses a non-empty list of names into values, rejecting unknown and repeated names.
fn parse_list<T: Copy + PartialEq>(
    number: usize,
    kind: BlockKind,
    names: &[String],
    parse: fn(&str) -> Option<T>,
    unknown: fn(usize, BlockKind, &str) -> PrintError,
) -> Result<Vec<T>, PrintError> {
    if names.is_empty() {
        let key = match kind {
            BlockKind::ItemTable | BlockKind::TaxTable => "columns",
            BlockKind::Header
            | BlockKind::Details
            | BlockKind::Totals
            | BlockKind::AmountInWords
            | BlockKind::UpiQr
            | BlockKind::Text => "slots",
        };
        return Err(PrintError::EmptyList {
            block: number,
            kind,
            key,
        });
    }
    let mut out: Vec<T> = Vec::with_capacity(names.len());
    for name in names {
        let value = parse(name).ok_or_else(|| unknown(number, kind, name))?;
        if out.contains(&value) {
            return Err(PrintError::DuplicateEntry {
                block: number,
                kind,
                found: name.clone(),
            });
        }
        out.push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // provenance: derived — format contract in this module's docs: every name must parse back to
    // itself, otherwise a template would print a different slot than it names.
    #[test]
    fn names_round_trip() {
        for s in Slot::ALL {
            assert_eq!(Slot::parse(s.name()), Some(s));
        }
        for c in ItemColumn::ALL {
            assert_eq!(ItemColumn::parse(c.name()), Some(c));
        }
        for c in TaxColumn::ALL {
            assert_eq!(TaxColumn::parse(c.name()), Some(c));
        }
        for k in BlockKind::ALL {
            assert_eq!(BlockKind::parse(k.name()), Some(k));
        }
    }

    // provenance: derived — ISO 216 sizes: A4 is 210 x 297 mm, A5 is 148 x 210 mm.
    #[test]
    fn page_sizes_match_iso_216() {
        assert_eq!(PageSize::A4.millimetres(), (210, 297));
        assert_eq!(PageSize::A5.millimetres(), (148, 210));
    }
}
