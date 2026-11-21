//! The facts an invoice prints, read from the voucher and the books (card PR2).
//!
//! Every figure printed here comes from the voucher's stored GST and lines, never from a template
//! (rule A1). Before any total is shown it is checked against the amount posted to the party
//! ledger, and the item lines are checked against the stored taxable value; a mismatch is an error
//! that names the voucher, not a number.

use std::collections::BTreeSet;

use ledgerly_core::format::{amount_in_words, format_inr};
use ledgerly_core::{Date, Decimal, Paise, Rate};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, GstLine, GstRegistration, InventoryLine, Ledger,
    RegistrationType, StockItem, StoredGst, TaxLedgerKind, Unit, Voucher, VoucherStatus,
};

use crate::template::{ItemColumn, PrintError, Slot, TaxColumn};

/// The kind of document a voucher prints as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocKind {
    TaxInvoice,
    CreditNote,
    DebitNote,
}

impl DocKind {
    fn title(self) -> &'static str {
        match self {
            Self::TaxInvoice => "Tax Invoice",
            Self::CreditNote => "Credit Note",
            Self::DebitNote => "Debit Note",
        }
    }

    fn number_label(self) -> &'static str {
        match self {
            Self::TaxInvoice => "Invoice No",
            Self::CreditNote => "Credit Note No",
            Self::DebitNote => "Debit Note No",
        }
    }
}

pub(super) fn unsupported(reason: impl Into<String>) -> PrintError {
    PrintError::Unsupported {
        reason: reason.into(),
    }
}

fn missing(what: impl Into<String>) -> PrintError {
    PrintError::Render {
        reason: what.into(),
    }
}

fn overflow() -> PrintError {
    missing("an amount on the invoice is too large to add")
}

fn add(a: Paise, b: Paise) -> Result<Paise, PrintError> {
    a.checked_add(b).map_err(|_| overflow())
}

fn add_decimal(a: Decimal, b: Decimal) -> Result<Decimal, PrintError> {
    a.checked_add(b).ok_or_else(overflow)
}

fn sum(values: impl IntoIterator<Item = Paise>) -> Result<Paise, PrintError> {
    values.into_iter().try_fold(Paise::ZERO, add)
}

fn plain(d: Decimal) -> String {
    d.normalize().to_string()
}

fn two_places(d: Decimal) -> String {
    let mut n = d.normalize();
    if n.scale() < 2 {
        n.rescale(2);
    }
    n.to_string()
}

fn percent(rate: Rate) -> String {
    format!("{}%", plain(rate.0))
}

/// `dd-mm-yyyy`.
fn date_text(d: Date) -> String {
    format!("{:02}-{:02}-{}", d.day(), u8::from(d.month()), d.year())
}

fn address_text(a: &Address) -> Option<String> {
    let mut parts: Vec<String> = a
        .lines
        .iter()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    if let Some(state) = a.state {
        parts.push(state.name().to_owned());
    }
    if let Some(pin) = a.pincode.as_deref().filter(|p| !p.trim().is_empty()) {
        parts.push(pin.trim().to_owned());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// One tax component of a tax row.
#[derive(Debug, Clone, Copy)]
struct Component {
    rate: Rate,
    tax: Paise,
}

/// One row of the tax table: a taxable value and the tax on it.
#[derive(Debug, Clone, Copy)]
struct TaxRow {
    taxable: Paise,
    central: Option<Component>,
    state: Option<Component>,
    integrated: Option<Component>,
    cess: Option<Component>,
}

impl TaxRow {
    fn components(&self) -> [Option<Component>; 4] {
        [self.central, self.state, self.integrated, self.cess]
    }

    fn total_tax(&self) -> Result<Paise, PrintError> {
        sum(self.components().into_iter().flatten().map(|c| c.tax))
    }

    /// The GST rate of the row: central and state (or integrated), cess not included.
    fn gst_rate(&self) -> Result<Decimal, PrintError> {
        [self.central, self.state, self.integrated]
            .into_iter()
            .flatten()
            .try_fold(Decimal::ZERO, |a, c| add_decimal(a, c.rate.0))
    }
}

fn rate_cell(c: Option<Component>) -> String {
    c.map_or_else(|| "-".to_owned(), |c| percent(c.rate))
}

fn tax_cell(c: Option<Component>) -> String {
    c.map_or_else(|| "-".to_owned(), |c| format_inr(c.tax))
}

/// Groups the stored tax lines into rows.
///
/// Central and state (or union territory) lines with the same taxable value and rate make one
/// row; each integrated line is a row; a cess line joins the first row with the same taxable
/// value that has no cess. Anything that does not pair up is `Unsupported`: the stored lines
/// carry no HSN, so a different grouping cannot be proven right.
fn tax_rows(gst: &StoredGst) -> Result<Vec<TaxRow>, PrintError> {
    let mut central: Vec<&GstLine> = Vec::new();
    let mut state: Vec<&GstLine> = Vec::new();
    let mut integrated: Vec<&GstLine> = Vec::new();
    let mut cess: Vec<&GstLine> = Vec::new();
    for line in &gst.lines {
        match line.kind {
            TaxLedgerKind::Cgst => central.push(line),
            TaxLedgerKind::Sgst | TaxLedgerKind::Utgst => state.push(line),
            TaxLedgerKind::Igst => integrated.push(line),
            TaxLedgerKind::Cess => cess.push(line),
            TaxLedgerKind::Tds | TaxLedgerKind::Tcs | TaxLedgerKind::Other => {
                return Err(unsupported(
                    "an invoice with TDS, TCS or other duty lines cannot be printed yet",
                ));
            }
        }
    }
    let comp = |l: &GstLine| Component {
        rate: l.rate_percent,
        tax: l.tax,
    };
    let mut rows: Vec<TaxRow> = Vec::new();
    for c in central {
        let found = state
            .iter()
            .position(|s| s.taxable == c.taxable && s.rate_percent == c.rate_percent);
        let Some(at) = found else {
            return Err(unsupported(
                "a central GST line has no state GST line of the same value and rate",
            ));
        };
        let s = state.remove(at);
        rows.push(TaxRow {
            taxable: c.taxable,
            central: Some(comp(c)),
            state: Some(comp(s)),
            integrated: None,
            cess: None,
        });
    }
    if !state.is_empty() {
        return Err(unsupported(
            "a state GST line has no central GST line of the same value and rate",
        ));
    }
    for i in integrated {
        rows.push(TaxRow {
            taxable: i.taxable,
            central: None,
            state: None,
            integrated: Some(comp(i)),
            cess: None,
        });
    }
    for c in cess {
        let Some(row) = rows
            .iter_mut()
            .find(|r| r.taxable == c.taxable && r.cess.is_none())
        else {
            return Err(unsupported(
                "a cess line does not match the taxable value of any tax row",
            ));
        };
        row.cess = Some(comp(c));
    }
    Ok(rows)
}

/// The figures of the totals block.
#[derive(Debug, Clone, Copy)]
struct Totals {
    taxable: Paise,
    tax: Paise,
    round_off: Paise,
    grand: Paise,
}

/// A voucher ready to print, with the books it is read against.
pub(super) struct Invoice<'a> {
    books: &'a Books,
    v: &'a Voucher,
    gst: &'a StoredGst,
    kind: DocKind,
}

impl<'a> Invoice<'a> {
    /// Checks that the voucher can print as a tax document at all.
    pub(super) fn new(
        books: &'a Books,
        v: &'a Voucher,
        gst: &'a StoredGst,
    ) -> Result<Self, PrintError> {
        let mut inv = Self {
            books,
            v,
            gst,
            kind: DocKind::TaxInvoice,
        };
        match v.status {
            VoucherStatus::Regular | VoucherStatus::PostDated => {}
            VoucherStatus::Optional => {
                return Err(unsupported(format!(
                    "{}: an optional voucher is not in the books and is not printed",
                    inv.label()
                )));
            }
            VoucherStatus::Cancelled => {
                return Err(unsupported(format!(
                    "{}: a cancelled voucher is not printed",
                    inv.label()
                )));
            }
        }
        if v.gst.as_ref().is_some_and(|stored| stored != gst) {
            return Err(missing(format!(
                "{}: the GST given for printing differs from the GST stored on the voucher",
                inv.label()
            )));
        }
        inv.kind = inv.doc_kind()?;
        Ok(inv)
    }

    fn label(&self) -> String {
        match &self.v.number {
            Some(n) => format!("voucher {n}"),
            None => format!("voucher id {}", self.v.id.0),
        }
    }

    fn doc_kind(&self) -> Result<DocKind, PrintError> {
        let vt = self
            .books
            .masters
            .voucher_types
            .get(&self.v.type_id)
            .ok_or_else(|| {
                missing(format!(
                    "{}: its voucher type is not in the books",
                    self.label()
                ))
            })?;
        match vt.base {
            BaseVoucherKind::Sales => Ok(DocKind::TaxInvoice),
            BaseVoucherKind::CreditNote => Ok(DocKind::CreditNote),
            BaseVoucherKind::DebitNote => Ok(DocKind::DebitNote),
            BaseVoucherKind::Contra
            | BaseVoucherKind::Payment
            | BaseVoucherKind::Receipt
            | BaseVoucherKind::Journal
            | BaseVoucherKind::Purchase
            | BaseVoucherKind::Memorandum
            | BaseVoucherKind::ReversingJournal
            | BaseVoucherKind::ReceiptNote
            | BaseVoucherKind::DeliveryNote
            | BaseVoucherKind::RejectionIn
            | BaseVoucherKind::RejectionOut
            | BaseVoucherKind::StockJournal
            | BaseVoucherKind::PhysicalStock
            | BaseVoucherKind::SalesOrder
            | BaseVoucherKind::PurchaseOrder => Err(unsupported(format!(
                "{}: only sales invoices, credit notes and debit notes can be printed",
                self.label()
            ))),
        }
    }

    /// The company's registration in force on the voucher date. A composition registration
    /// issues a bill of supply, which this renderer does not print.
    fn registration(&self) -> Result<&'a GstRegistration, PrintError> {
        let mut in_force = self
            .books
            .company
            .registrations
            .iter()
            .filter(|r| r.effective_from <= self.v.date);
        let first = in_force.next().ok_or_else(|| {
            missing(format!(
                "{}: the company has no GST registration in force on {}",
                self.label(),
                date_text(self.v.date)
            ))
        })?;
        if in_force.next().is_some() {
            return Err(unsupported(format!(
                "{}: the company has several GSTINs and the voucher does not say which issues it",
                self.label()
            )));
        }
        if first.composition {
            return Err(unsupported(format!(
                "{}: a composition taxpayer issues a bill of supply, not a tax invoice",
                self.label()
            )));
        }
        Ok(first)
    }

    fn party(&self) -> Result<&'a Ledger, PrintError> {
        let id = self
            .v
            .party
            .ok_or_else(|| missing(format!("{}: it has no party", self.label())))?;
        self.books.masters.ledgers.get(&id).ok_or_else(|| {
            missing(format!(
                "{}: its party ledger is not in the books",
                self.label()
            ))
        })
    }

    /// The label and text a slot prints.
    pub(super) fn slot(&self, slot: Slot) -> Result<(&'static str, String), PrintError> {
        let company = &self.books.company;
        match slot {
            Slot::CompanyName => Ok((
                "",
                company
                    .mailing_name
                    .clone()
                    .unwrap_or_else(|| company.name.clone()),
            )),
            Slot::CompanyAddress => address_text(&company.address)
                .map(|a| ("", a))
                .ok_or_else(|| missing("the company has no address")),
            Slot::CompanyGstin => Ok(("GSTIN", self.registration()?.gstin.as_str().to_owned())),
            Slot::DocumentTitle => {
                self.registration()?;
                Ok(("", self.kind.title().to_owned()))
            }
            Slot::VoucherNumber => self
                .v
                .number
                .clone()
                .map(|n| (self.kind.number_label(), n))
                .ok_or_else(|| missing(format!("{}: it has no number yet", self.label()))),
            Slot::VoucherDate => Ok(("Date", date_text(self.v.date))),
            Slot::PartyName => Ok(("Name", self.party()?.name.clone())),
            Slot::PartyAddress => self
                .party()?
                .party
                .as_ref()
                .and_then(|p| address_text(&p.address))
                .map(|a| ("Address", a))
                .ok_or_else(|| missing(format!("{}: the party has no address", self.label()))),
            Slot::PartyGstin => self.party_gstin().map(|g| ("GSTIN", g)),
            Slot::PlaceOfSupply => self
                .v
                .place_of_supply
                .map(|s| ("Place of supply", format!("{:02}-{}", s.get(), s.name())))
                .ok_or_else(|| {
                    unsupported(format!(
                        "{}: the voucher has no place of supply to print",
                        self.label()
                    ))
                }),
            Slot::TaxableTotal => Ok(("Taxable value", format_inr(self.totals()?.taxable))),
            Slot::TaxTotal => Ok(("Total tax", format_inr(self.totals()?.tax))),
            Slot::RoundOff => Ok(("Round off", format_inr(self.totals()?.round_off))),
            Slot::GrandTotal => Ok(("Grand total", format_inr(self.totals()?.grand))),
        }
    }

    fn party_gstin(&self) -> Result<String, PrintError> {
        let ledger = self.party()?;
        let details = ledger
            .party
            .as_ref()
            .ok_or_else(|| missing(format!("{}: the party has no party details", self.label())))?;
        if let Some(g) = &details.gstin {
            return Ok(g.as_str().to_owned());
        }
        match details.registration {
            RegistrationType::Unregistered | RegistrationType::Consumer => {
                Ok("Unregistered".to_owned())
            }
            RegistrationType::Regular
            | RegistrationType::Composition
            | RegistrationType::Sez
            | RegistrationType::Overseas
            | RegistrationType::Uin => Err(missing(format!(
                "{}: the party is registered but has no GSTIN on its ledger",
                self.label()
            ))),
        }
    }

    /// Totals, checked against the stored lines and the amount posted to the party.
    fn totals(&self) -> Result<Totals, PrintError> {
        let rows = tax_rows(self.gst)?;
        let taxable = sum(rows.iter().map(|r| r.taxable))?;
        let tax = sum(self.gst.lines.iter().map(|l| l.tax))?;
        let round_off = self.gst.round_off;
        let grand = add(add(taxable, tax)?, round_off)?;
        if !self.v.inventory_lines.is_empty() {
            let items = sum(self.v.inventory_lines.iter().map(|l| l.amount))?;
            if items != taxable {
                return Err(unsupported(format!(
                    "{}: the item lines total {} but the stored taxable value is {}; \
                     charges added to the taxable value are not apportioned to items yet",
                    self.label(),
                    format_inr(items),
                    format_inr(taxable)
                )));
            }
        }
        let party = self.v.party.ok_or_else(|| {
            unsupported(format!(
                "{}: no party line confirms the total",
                self.label()
            ))
        })?;
        let posted = sum(self
            .v
            .ledger_lines
            .iter()
            .filter(|l| l.ledger == party)
            .map(|l| l.amount.amount))?;
        if posted != grand {
            return Err(missing(format!(
                "{}: the invoice totals {} but {} is posted to the party ledger",
                self.label(),
                format_inr(grand),
                format_inr(posted)
            )));
        }
        Ok(Totals {
            taxable,
            tax,
            round_off,
            grand,
        })
    }

    /// The grand total.
    pub(super) fn grand_total(&self) -> Result<Paise, PrintError> {
        Ok(self.totals()?.grand)
    }

    /// The grand total in words.
    pub(super) fn words(&self) -> Result<String, PrintError> {
        Ok(amount_in_words(self.grand_total()?))
    }

    /// The voucher number, for a payment note.
    pub(super) fn number(&self) -> Option<&str> {
        self.v.number.as_deref()
    }

    fn item(&self, line: &InventoryLine) -> Result<&'a StockItem, PrintError> {
        self.books
            .masters
            .stock_items
            .get(&line.item)
            .ok_or_else(|| {
                missing(format!(
                    "{}: a stock item is not in the books",
                    self.label()
                ))
            })
    }

    fn unit(&self, line: &InventoryLine) -> Result<&'a Unit, PrintError> {
        self.books
            .masters
            .units
            .get(&line.unit)
            .ok_or_else(|| missing(format!("{}: a unit is not in the books", self.label())))
    }

    /// The single HSN or SAC of every item line.
    fn single_hsn(&self) -> Result<String, PrintError> {
        let mut codes = BTreeSet::new();
        for line in &self.v.inventory_lines {
            let item = self.item(line)?;
            let hsn = item.hsn.as_ref().ok_or_else(|| {
                missing(format!(
                    "{}: item \"{}\" has no HSN or SAC code",
                    self.label(),
                    item.name
                ))
            })?;
            codes.insert(hsn.as_str().to_owned());
        }
        let mut it = codes.into_iter();
        match (it.next(), it.next()) {
            (Some(one), None) => Ok(one),
            (None, _) => Err(unsupported(format!(
                "{}: the tax table's HSN column needs item lines",
                self.label()
            ))),
            (Some(_), Some(_)) => Err(unsupported(format!(
                "{}: the tax table's HSN column with several HSN codes needs the HSN on each stored GST line",
                self.label()
            ))),
        }
    }

    /// The GST rate of the invoice when it has exactly one tax row.
    fn single_rate(&self) -> Result<String, PrintError> {
        let rows = tax_rows(self.gst)?;
        match rows.as_slice() {
            [row] => Ok(format!("{}%", plain(row.gst_rate()?))),
            [] | [_, _, ..] => Err(unsupported(format!(
                "{}: an item's GST rate needs a rate on each stored item line; this invoice has {} tax rows",
                self.label(),
                rows.len()
            ))),
        }
    }

    /// Cells of the item table, one row per inventory line.
    pub(super) fn item_rows(&self, cols: &[ItemColumn]) -> Result<Vec<Vec<String>>, PrintError> {
        if self.v.inventory_lines.is_empty() {
            return Err(unsupported(format!(
                "{}: the item table needs item lines; accounting invoices are not printed yet",
                self.label()
            )));
        }
        self.totals()?;
        let rate = if cols.contains(&ItemColumn::TaxRate) {
            self.single_rate()?
        } else {
            String::new()
        };
        let mut rows = Vec::with_capacity(self.v.inventory_lines.len());
        let mut serial: u32 = 0;
        for line in &self.v.inventory_lines {
            serial = serial.saturating_add(1);
            let item = self.item(line)?;
            let unit = self.unit(line)?;
            let mut row = Vec::with_capacity(cols.len());
            for col in cols {
                row.push(self.item_cell(*col, serial, (line, item, unit), &rate)?);
            }
            rows.push(row);
        }
        Ok(rows)
    }

    fn item_cell(
        &self,
        col: ItemColumn,
        serial: u32,
        (line, item, unit): (&InventoryLine, &StockItem, &Unit),
        rate: &str,
    ) -> Result<String, PrintError> {
        Ok(match col {
            ItemColumn::Serial => serial.to_string(),
            ItemColumn::Description => item.name.clone(),
            ItemColumn::Hsn => item
                .hsn
                .as_ref()
                .map(|h| h.as_str().to_owned())
                .ok_or_else(|| {
                    missing(format!(
                        "{}: item \"{}\" has no HSN or SAC code",
                        self.label(),
                        item.name
                    ))
                })?,
            ItemColumn::Quantity => plain(line.qty.0),
            ItemColumn::Unit => unit.symbol.clone(),
            ItemColumn::Rate => two_places(line.rate.0),
            ItemColumn::Discount => line
                .discount_percent
                .map_or_else(|| "-".to_owned(), percent),
            ItemColumn::TaxableValue | ItemColumn::Amount => format_inr(line.amount),
            ItemColumn::TaxRate => rate.to_owned(),
        })
    }

    /// Cells of the tax table, one row per tax row.
    pub(super) fn tax_table_rows(
        &self,
        cols: &[TaxColumn],
    ) -> Result<Vec<Vec<String>>, PrintError> {
        self.totals()?;
        let hsn = if cols.contains(&TaxColumn::Hsn) {
            self.single_hsn()?
        } else {
            String::new()
        };
        let mut out = Vec::new();
        for row in tax_rows(self.gst)? {
            let mut cells = Vec::with_capacity(cols.len());
            for col in cols {
                cells.push(match col {
                    TaxColumn::Hsn => hsn.clone(),
                    TaxColumn::TaxableValue => format_inr(row.taxable),
                    TaxColumn::CgstRate => rate_cell(row.central),
                    TaxColumn::CgstAmount => tax_cell(row.central),
                    TaxColumn::SgstRate => rate_cell(row.state),
                    TaxColumn::SgstAmount => tax_cell(row.state),
                    TaxColumn::IgstRate => rate_cell(row.integrated),
                    TaxColumn::IgstAmount => tax_cell(row.integrated),
                    TaxColumn::CessAmount => tax_cell(row.cess),
                    TaxColumn::TotalTax => format_inr(row.total_tax()?),
                });
            }
            out.push(cells);
        }
        Ok(out)
    }
}
