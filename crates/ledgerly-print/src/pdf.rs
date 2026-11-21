//! PDF rendering. **Card PR2** implements this module.
//!
//! Frozen interface (plan card PR2):
//!
//! ```text
//! pub fn render_invoice(t: &Template, v: &Voucher, books: &Books, gst: &StoredGst) -> Result<Vec<u8>, PrintError>;
//! ```
//!
//! The renderer prints a sales invoice, credit note or debit note on the page size and in the
//! block order of a [`Template`]. It adds no figure of its own: every number comes from the
//! voucher's stored GST and lines, and the totals are checked against the amount posted to the
//! party ledger before they are shown (rule A1). What it cannot print it refuses with
//! [`PrintError::Unsupported`], never a blank or a guess:
//!
//! * characters outside plain ASCII (the built-in Helvetica font covers nothing else);
//! * a voucher that is not a sales invoice, credit note or debit note, or is optional or
//!   cancelled;
//! * a company with several GSTINs or a composition registration;
//! * item or tax tables that cannot be tied to the stored GST (stored GST lines carry no HSN, and
//!   item lines carry no tax rate, so an HSN or rate column is printed only when one code or one
//!   rate covers the whole invoice);
//! * a `upi_qr` block when no payee is given (use [`render_invoice_with_upi`]).
//!
//! The PDF is written uncompressed so that the text stays extractable and the output is
//! byte-for-byte the same for the same input.

mod data {
    //! The facts an invoice prints, read from the voucher and the books (card PR2).
    //!
    //! Every figure printed here comes from the voucher's stored GST and lines, never from a template
    //! (rule A1). Before any total is shown it is checked against the amount posted to the party
    //! ledger, and the item lines are checked against the stored taxable value; a mismatch is an error
    //! that names the voucher, not a number.

    use std::collections::BTreeSet;

    use ledgerly_core::format::{amount_in_words, format_inr};
    use ledgerly_core::{Date, Decimal, DrCr, Paise, Rate};
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
    /// row; each integrated line is a row; a cess line joins the one row with the same taxable
    /// value that has no cess. Two rows sharing that value make the cess ambiguous, and it is
    /// `Unsupported`: the stored lines carry no HSN or item link that says which row it belongs
    /// to. Anything else that does not pair up is `Unsupported` too.
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
            let candidates: Vec<usize> = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.taxable == c.taxable && r.cess.is_none())
                .map(|(at, _)| at)
                .collect();
            let [at] = candidates.as_slice() else {
                return Err(unsupported(if candidates.is_empty() {
                    "a cess line does not match the taxable value of any tax row"
                } else {
                    "a cess line matches more than one tax row of the same taxable value; \
                     the stored lines do not say which row it belongs to"
                }));
            };
            let Some(row) = rows.get_mut(*at) else {
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
            let details = ledger.party.as_ref().ok_or_else(|| {
                missing(format!("{}: the party has no party details", self.label()))
            })?;
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
            // Debits count plus and credits minus, so a balanced pair nets to nothing and a credit
            // cannot stand in for the debit the invoice total needs.
            let mut net: i64 = 0;
            for l in self.v.ledger_lines.iter().filter(|l| l.ledger == party) {
                let value = match l.amount.side {
                    DrCr::Dr => l.amount.amount.get(),
                    DrCr::Cr => l.amount.amount.get().checked_neg().ok_or_else(overflow)?,
                };
                net = net.checked_add(value).ok_or_else(overflow)?;
            }
            if net != grand.get() {
                let posted = if net < 0 {
                    "a net credit".to_owned()
                } else {
                    format_inr(Paise::new(net))
                };
                return Err(missing(format!(
                    "{}: the invoice totals {} but {} is posted to the party ledger (debits less credits)",
                    self.label(),
                    format_inr(grand),
                    posted
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
        pub(super) fn item_rows(
            &self,
            cols: &[ItemColumn],
        ) -> Result<Vec<Vec<String>>, PrintError> {
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
}
mod page {
    //! Page layout and PDF writing for the invoice renderer (card PR2).
    //!
    //! All layout is whole points in `i32`, measured from the top-left of the page. Floats appear
    //! only at the `pdf-writer` boundary, converted from integers with `f32::from` (rule A3: no float
    //! arithmetic). Text uses the built-in Helvetica and Helvetica-Bold fonts with `WinAnsiEncoding`;
    //! only plain ASCII (code points 32 to 126) can be printed. Any other character is reported as
    //! [`PrintError::Unsupported`] instead of being replaced (rule A1).

    use pdf_writer::{Content, Name, Pdf, Rect, Ref, Str};

    use crate::template::PrintError;

    /// Page margin in points.
    pub(super) const MARGIN: i32 = 36;
    /// Space kept free at the bottom of every page for the page number.
    const FOOTER: i32 = 28;
    /// Padding inside table cells.
    const PAD: i32 = 3;
    /// Body text size.
    pub(super) const BODY: i32 = 10;
    /// Table text size.
    const TABLE: i32 = 9;

    /// Advance widths of Helvetica for code points 32 to 126, in 1000ths of the font size.
    ///
    /// Provenance: recalled from the published Helvetica AFM metrics, not re-checked. They only
    /// affect where text is aligned and wrapped, never which characters are printed.
    const HELVETICA: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
        278, // space to /
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0 to 9
        278, 278, 584, 584, 584, 556, 1015, // : to @
        667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // A to M
        722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N to Z
        278, 278, 278, 469, 556, 333, // [ to `
        556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // a to m
        556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // n to z
        334, 260, 334, 584, // { to ~
    ];

    /// Width used for a character outside the table (layout only; such text is refused earlier).
    const FALLBACK_WIDTH: u16 = 556;

    /// Converts a layout value to the `f32` the PDF writer takes, clamped to the `i16` range.
    fn pt(v: i32) -> f32 {
        let clamped = v.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        f32::from(i16::try_from(clamped).unwrap_or(0))
    }

    fn to_i32(n: usize) -> i32 {
        i32::try_from(n).unwrap_or(i32::MAX)
    }

    /// Width of `text` in points at `size`. Bold text is estimated 10 percent wider.
    pub(super) fn text_width(text: &str, size: i32, bold: bool) -> i32 {
        let mut units: i32 = 0;
        for ch in text.chars() {
            let width = u8::try_from(ch)
                .ok()
                .and_then(|b| b.checked_sub(32))
                .and_then(|i| HELVETICA.get(usize::from(i)).copied())
                .unwrap_or(FALLBACK_WIDTH);
            units = units.saturating_add(i32::from(width));
        }
        if bold {
            units = units.saturating_add(units / 10);
        }
        units.saturating_mul(size) / 1000
    }

    /// Splits `text` into lines no wider than `max_width`, breaking at spaces and, for a word wider
    /// than the line, between characters. Explicit line breaks in `text` are kept.
    pub(super) fn wrap(text: &str, max_width: i32, size: i32, bold: bool) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.lines() {
            wrap_paragraph(paragraph, max_width, size, bold, &mut lines);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    fn wrap_paragraph(text: &str, max_width: i32, size: i32, bold: bool, lines: &mut Vec<String>) {
        let mut current = String::new();
        for word in text.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if text_width(&candidate, size, bold) <= max_width {
                current = candidate;
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if text_width(word, size, bold) <= max_width {
                word.clone_into(&mut current);
                continue;
            }
            for ch in word.chars() {
                let mut next = current.clone();
                next.push(ch);
                if text_width(&next, size, bold) > max_width && !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    current.push(ch);
                } else {
                    current = next;
                }
            }
        }
        lines.push(current);
    }

    /// Line height for text of `size`.
    pub(super) fn leading(size: i32) -> i32 {
        size.saturating_add(size / 4).saturating_add(1)
    }

    /// One drawing operation on a page. Positions are in points from the top-left.
    enum Op {
        Text {
            x: i32,
            baseline: i32,
            size: i32,
            bold: bool,
            text: String,
        },
        Rule {
            x1: i32,
            x2: i32,
            y: i32,
        },
        Fill {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
        },
    }

    /// How a table column aligns its text.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum Align {
        Left,
        Right,
    }

    /// A table column: heading, share of the width and alignment.
    pub(super) struct Column {
        pub(super) title: &'static str,
        pub(super) weight: i32,
        pub(super) align: Align,
    }

    fn refuse_char(ch: char) -> PrintError {
        PrintError::Unsupported {
            reason: format!(
                "the character {ch:?} cannot be printed: the built-in font covers plain ASCII only"
            ),
        }
    }

    fn push_text(
        ops: &mut Vec<Op>,
        x: i32,
        baseline: i32,
        size: i32,
        bold: bool,
        text: &str,
    ) -> Result<(), PrintError> {
        if let Some(bad) = text.chars().find(|c| !matches!(c, ' '..='~')) {
            return Err(refuse_char(bad));
        }
        ops.push(Op::Text {
            x,
            baseline,
            size,
            bold,
            text: text.to_owned(),
        });
        Ok(())
    }

    /// The page being laid out, top to bottom, with a new page started when one fills up.
    pub(super) struct Canvas {
        width: i32,
        height: i32,
        done: Vec<Vec<Op>>,
        current: Vec<Op>,
        y: i32,
    }

    impl Canvas {
        /// A canvas of one empty page, `width` by `height` points.
        pub(super) fn new(width: i32, height: i32) -> Self {
            Self {
                width,
                height,
                done: Vec::new(),
                current: Vec::new(),
                y: MARGIN,
            }
        }

        /// Width available between the margins.
        pub(super) fn content_width(&self) -> i32 {
            self.width.saturating_sub(MARGIN.saturating_mul(2))
        }

        /// X of the right margin.
        pub(super) fn right(&self) -> i32 {
            self.width.saturating_sub(MARGIN)
        }

        fn bottom(&self) -> i32 {
            self.height.saturating_sub(MARGIN).saturating_sub(FOOTER)
        }

        /// Moves down `n` points.
        pub(super) fn skip(&mut self, n: i32) {
            self.y = self.y.saturating_add(n);
        }

        /// Starts a new page when `h` more points do not fit; returns whether it did.
        pub(super) fn ensure(&mut self, h: i32) -> bool {
            if self.y.saturating_add(h) > self.bottom() && self.y > MARGIN {
                self.done.push(std::mem::take(&mut self.current));
                self.y = MARGIN;
                true
            } else {
                false
            }
        }

        /// Prints `text` at `x` on the current line (the line is `size` tall, not yet advanced).
        pub(super) fn text_at(
            &mut self,
            x: i32,
            size: i32,
            bold: bool,
            text: &str,
        ) -> Result<(), PrintError> {
            let baseline = self.y.saturating_add(size);
            push_text(&mut self.current, x, baseline, size, bold, text)
        }

        /// Prints wrapped text from the left margin, one line after another.
        pub(super) fn paragraph(
            &mut self,
            text: &str,
            size: i32,
            bold: bool,
        ) -> Result<(), PrintError> {
            for line in wrap(text, self.content_width(), size, bold) {
                self.ensure(leading(size));
                self.text_at(MARGIN, size, bold, &line)?;
                self.skip(leading(size));
            }
            Ok(())
        }

        /// Prints `label` at the left margin and `value` against the right margin on one line.
        pub(super) fn label_value(
            &mut self,
            label: &str,
            value: &str,
            size: i32,
            bold: bool,
        ) -> Result<(), PrintError> {
            self.ensure(leading(size));
            self.text_at(MARGIN, size, bold, label)?;
            let x = self
                .right()
                .saturating_sub(text_width(value, size, bold))
                .max(MARGIN);
            self.text_at(x, size, bold, value)?;
            self.skip(leading(size));
            Ok(())
        }

        fn rule(&mut self) {
            let (x1, x2, y) = (MARGIN, self.right(), self.y);
            self.current.push(Op::Rule { x1, x2, y });
        }

        /// Draws a square-module code with its top-left at the current line.
        ///
        /// `modules` holds `side * side` entries, row by row, `true` for a dark module.
        pub(super) fn modules(
            &mut self,
            modules: &[bool],
            side: usize,
            module: i32,
        ) -> Result<(), PrintError> {
            if side == 0 || modules.len() != side.saturating_mul(side) {
                return Err(PrintError::Render {
                    reason: "the QR code is not square".to_owned(),
                });
            }
            let total = to_i32(side).saturating_mul(module);
            self.ensure(total);
            let mut y = self.y;
            for row in modules.chunks(side) {
                let mut x = MARGIN;
                for dark in row {
                    if *dark {
                        self.current.push(Op::Fill {
                            x,
                            y,
                            w: module,
                            h: module,
                        });
                    }
                    x = x.saturating_add(module);
                }
                y = y.saturating_add(module);
            }
            self.y = y;
            Ok(())
        }

        fn column_widths(&self, cols: &[Column]) -> Vec<i32> {
            let total: i32 = cols
                .iter()
                .fold(0_i32, |a, c| a.saturating_add(c.weight.max(1)));
            let width = self.content_width();
            let mut used = 0_i32;
            let mut widths = Vec::with_capacity(cols.len());
            for (n, col) in cols.iter().enumerate() {
                let w = if n.saturating_add(1) == cols.len() {
                    width.saturating_sub(used)
                } else {
                    width.saturating_mul(col.weight.max(1)) / total.max(1)
                };
                used = used.saturating_add(w);
                widths.push(w);
            }
            widths
        }

        /// Prints a table: a heading row and then `rows`. A row that does not fit starts a new page,
        /// which repeats the heading. Every row must have one cell per column.
        pub(super) fn table(
            &mut self,
            cols: &[Column],
            rows: &[Vec<String>],
        ) -> Result<(), PrintError> {
            let widths = self.column_widths(cols);
            let heading: Vec<String> = cols.iter().map(|c| c.title.to_owned()).collect();
            self.table_row(cols, &widths, &heading, true)?;
            for row in rows {
                if row.len() != cols.len() {
                    return Err(PrintError::Render {
                        reason: "a table row has a different number of cells than columns"
                            .to_owned(),
                    });
                }
                self.table_row(cols, &widths, row, false)?;
            }
            Ok(())
        }

        fn table_row(
            &mut self,
            cols: &[Column],
            widths: &[i32],
            cells: &[String],
            heading: bool,
        ) -> Result<(), PrintError> {
            let wrapped: Vec<Vec<String>> = cells
                .iter()
                .zip(widths)
                .map(|(t, w)| wrap(t, w.saturating_sub(PAD.saturating_mul(2)), TABLE, heading))
                .collect();
            let lines = wrapped.iter().map(Vec::len).max().unwrap_or(1);
            let height = to_i32(lines)
                .saturating_mul(leading(TABLE))
                .saturating_add(PAD.saturating_mul(2));
            // The heading is checked for room too, so it never ends a page with its rows on the
            // next one. A data row that breaks the page repeats the heading on the new page.
            let broke = self.ensure(height.saturating_add(leading(TABLE)));
            if broke && !heading {
                let titles: Vec<String> = cols.iter().map(|c| c.title.to_owned()).collect();
                self.table_row(cols, widths, &titles, true)?;
            }
            let mut x0 = MARGIN;
            for ((col, width), lines) in cols.iter().zip(widths).zip(&wrapped) {
                let mut baseline = self.y.saturating_add(PAD).saturating_add(TABLE);
                for line in lines {
                    let x = match col.align {
                        Align::Left => x0.saturating_add(PAD),
                        Align::Right => x0
                            .saturating_add(*width)
                            .saturating_sub(PAD)
                            .saturating_sub(text_width(line, TABLE, heading)),
                    };
                    push_text(&mut self.current, x, baseline, TABLE, heading, line)?;
                    baseline = baseline.saturating_add(leading(TABLE));
                }
                x0 = x0.saturating_add(*width);
            }
            self.skip(height);
            self.rule();
            Ok(())
        }

        /// Adds the page numbers and writes the document.
        pub(super) fn finish(mut self) -> Result<Vec<u8>, PrintError> {
            self.done.push(std::mem::take(&mut self.current));
            let total = self.done.len();
            let baseline = self.height.saturating_sub(MARGIN);
            let centre = self.width / 2;
            let mut number = 0_usize;
            for ops in &mut self.done {
                number = number.saturating_add(1);
                let label = format!("Page {number} of {total}");
                let x = centre.saturating_sub(text_width(&label, TABLE, false) / 2);
                push_text(ops, x, baseline, TABLE, false, &label)?;
            }
            write_pdf(self.width, self.height, &self.done)
        }
    }

    fn too_large() -> PrintError {
        PrintError::Render {
            reason: "the document has too many pages to number".to_owned(),
        }
    }

    /// Object numbers: 1 catalog, 2 page tree, 3 and 4 fonts, then a page and its content per page.
    fn page_refs(index: usize) -> Result<(Ref, Ref), PrintError> {
        let page = index
            .checked_mul(2)
            .and_then(|n| n.checked_add(5))
            .and_then(|n| i32::try_from(n).ok())
            .filter(|n| *n > 0 && *n < i32::MAX)
            .ok_or_else(too_large)?;
        Ok((Ref::new(page), Ref::new(page + 1)))
    }

    fn content_stream(height: i32, ops: &[Op]) -> Vec<u8> {
        let mut content = Content::new();
        for op in ops {
            match op {
                Op::Text {
                    x,
                    baseline,
                    size,
                    bold,
                    text,
                } => {
                    let font: &[u8] = if *bold { b"F2" } else { b"F1" };
                    content.begin_text();
                    content.set_font(Name(font), pt(*size));
                    content.next_line(pt(*x), pt(height.saturating_sub(*baseline)));
                    content.show(Str(text.as_bytes()));
                    content.end_text();
                }
                Op::Rule { x1, x2, y } => {
                    let y = pt(height.saturating_sub(*y));
                    content.set_line_width(0.5);
                    content.move_to(pt(*x1), y);
                    content.line_to(pt(*x2), y);
                    content.stroke();
                }
                Op::Fill { x, y, w, h } => {
                    content.rect(
                        pt(*x),
                        pt(height.saturating_sub(y.saturating_add(*h))),
                        pt(*w),
                        pt(*h),
                    );
                    content.fill_nonzero();
                }
            }
        }
        let buf = content.finish();
        buf.to_vec()
    }

    fn write_pdf(width: i32, height: i32, pages: &[Vec<Op>]) -> Result<Vec<u8>, PrintError> {
        let catalog = Ref::new(1);
        let tree = Ref::new(2);
        let regular = Ref::new(3);
        let bold = Ref::new(4);
        let mut pdf = Pdf::new();
        pdf.catalog(catalog).pages(tree);
        pdf.type1_font(regular)
            .base_font(Name(b"Helvetica"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(bold)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        let mut kids = Vec::with_capacity(pages.len());
        for (index, ops) in pages.iter().enumerate() {
            let (page_ref, content_ref) = page_refs(index)?;
            kids.push(page_ref);
            {
                let mut page = pdf.page(page_ref);
                page.parent(tree)
                    .media_box(Rect::new(0.0, 0.0, pt(width), pt(height)))
                    .contents(content_ref);
                page.resources()
                    .fonts()
                    .pair(Name(b"F1"), regular)
                    .pair(Name(b"F2"), bold);
            }
            let data = content_stream(height, ops);
            pdf.stream(content_ref, &data);
        }
        let count = i32::try_from(kids.len()).map_err(|_| too_large())?;
        pdf.pages(tree).kids(kids).count(count);
        Ok(pdf.finish())
    }
}

use ledgerly_bank::upi::upi_payload;
use ledgerly_core::Paise;
use ledgerly_model::{Books, StoredGst, Voucher};
use qrcode::{Color, QrCode};

use crate::template::{Block, BlockBody, ItemColumn, PrintError, Slot, TaxColumn, Template};
use data::{Invoice, unsupported};
use page::{Align, BODY, Canvas, Column};

/// Size of one QR module in points.
const QR_MODULE: i32 = 3;

/// Where a `upi_qr` block sends the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpiPayee<'a> {
    /// The payee's UPI address, such as `shop@bank`.
    pub vpa: &'a str,
    /// The payee's name.
    pub name: &'a str,
}

/// Renders the voucher on the template and returns the PDF bytes.
///
/// A template with a `upi_qr` block is `Unsupported` here, because the books hold no UPI address;
/// [`render_invoice_with_upi`] takes one.
///
/// # Errors
///
/// [`PrintError::Unsupported`] for input the renderer does not handle (see the module docs) and
/// [`PrintError::Render`] when the voucher or the books lack something the template prints, or
/// when the invoice totals disagree with what is posted.
pub fn render_invoice(
    t: &Template,
    v: &Voucher,
    books: &Books,
    gst: &StoredGst,
) -> Result<Vec<u8>, PrintError> {
    render_invoice_with_upi(t, v, books, gst, None)
}

/// Like [`render_invoice`], with the payee a `upi_qr` block pays to.
///
/// # Errors
///
/// As [`render_invoice`]; a `upi_qr` block with `upi` of `None` is `Unsupported`.
pub fn render_invoice_with_upi(
    t: &Template,
    v: &Voucher,
    books: &Books,
    gst: &StoredGst,
    upi: Option<&UpiPayee<'_>>,
) -> Result<Vec<u8>, PrintError> {
    let invoice = Invoice::new(books, v, gst)?;
    let (width_mm, height_mm) = t.page.millimetres();
    let mut canvas = Canvas::new(mm_to_points(width_mm), mm_to_points(height_mm));
    for block in &t.blocks {
        render_block(&mut canvas, &invoice, block, upi)?;
        canvas.skip(8);
    }
    canvas.finish()
}

/// Millimetres to whole points (72 per 25.4 mm), rounded to the nearest point.
fn mm_to_points(mm: u16) -> i32 {
    (i32::from(mm) * 720 + 127) / 254
}

fn render_block(
    canvas: &mut Canvas,
    invoice: &Invoice<'_>,
    block: &Block,
    upi: Option<&UpiPayee<'_>>,
) -> Result<(), PrintError> {
    if let Some(title) = &block.title {
        canvas.paragraph(title, BODY, true)?;
    }
    match &block.body {
        BlockBody::Header { slots } | BlockBody::Details { slots } => {
            for slot in slots {
                render_slot(canvas, invoice, *slot)?;
            }
            Ok(())
        }
        BlockBody::Totals { slots } => {
            for slot in slots {
                let (label, value) = invoice.slot(*slot)?;
                let bold = matches!(slot, Slot::GrandTotal);
                canvas.label_value(label, &value, BODY, bold)?;
            }
            Ok(())
        }
        BlockBody::ItemTable { columns } => {
            let rows = invoice.item_rows(columns)?;
            canvas.table(&item_columns(columns), &rows)
        }
        BlockBody::TaxTable { columns } => {
            let rows = invoice.tax_table_rows(columns)?;
            canvas.table(&tax_columns(columns), &rows)
        }
        BlockBody::AmountInWords => canvas.paragraph(&invoice.words()?, BODY, false),
        BlockBody::UpiQr => render_upi(canvas, invoice, upi),
        BlockBody::Text { text } => canvas.paragraph(text, BODY, false),
    }
}

/// Prints one slot of a header or details block: the company name and the document title large,
/// everything else as `label: value`.
fn render_slot(canvas: &mut Canvas, invoice: &Invoice<'_>, slot: Slot) -> Result<(), PrintError> {
    let (label, value) = invoice.slot(slot)?;
    let text = if label.is_empty() {
        value
    } else {
        format!("{label}: {value}")
    };
    match slot {
        Slot::CompanyName => canvas.paragraph(&text, 14, true),
        Slot::DocumentTitle => canvas.paragraph(&text, 16, true),
        Slot::CompanyAddress
        | Slot::CompanyGstin
        | Slot::VoucherNumber
        | Slot::VoucherDate
        | Slot::PartyName
        | Slot::PartyAddress
        | Slot::PartyGstin
        | Slot::PlaceOfSupply
        | Slot::TaxableTotal
        | Slot::TaxTotal
        | Slot::RoundOff
        | Slot::GrandTotal => canvas.paragraph(&text, BODY, false),
    }
}

fn render_upi(
    canvas: &mut Canvas,
    invoice: &Invoice<'_>,
    upi: Option<&UpiPayee<'_>>,
) -> Result<(), PrintError> {
    let payee = upi.ok_or_else(|| {
        unsupported("a upi_qr block needs the payee's UPI address, which the books do not hold")
    })?;
    let grand: Paise = invoice.grand_total()?;
    let payload = upi_payload(payee.vpa, payee.name, grand, invoice.number().unwrap_or(""))
        .map_err(|e| PrintError::Render {
            reason: e.to_string(),
        })?;
    let code = QrCode::new(payload.as_bytes()).map_err(|e| PrintError::Render {
        reason: format!("could not encode the UPI QR code: {e}"),
    })?;
    let side = code.width();
    let dark: Vec<bool> = code
        .into_colors()
        .into_iter()
        .map(|c| matches!(c, Color::Dark))
        .collect();
    canvas.paragraph(&format!("Pay to: {}", payee.vpa), BODY, false)?;
    canvas.modules(&dark, side, QR_MODULE)
}

fn item_columns(cols: &[ItemColumn]) -> Vec<Column> {
    cols.iter()
        .map(|c| match c {
            ItemColumn::Serial => column("#", 4, Align::Left),
            ItemColumn::Description => column("Description", 24, Align::Left),
            ItemColumn::Hsn => column("HSN/SAC", 9, Align::Left),
            ItemColumn::Quantity => column("Qty", 7, Align::Right),
            ItemColumn::Unit => column("Unit", 5, Align::Left),
            ItemColumn::Rate => column("Rate", 10, Align::Right),
            ItemColumn::Discount => column("Disc", 6, Align::Right),
            ItemColumn::TaxableValue => column("Taxable value", 12, Align::Right),
            ItemColumn::TaxRate => column("GST", 6, Align::Right),
            ItemColumn::Amount => column("Amount", 12, Align::Right),
        })
        .collect()
}

fn tax_columns(cols: &[TaxColumn]) -> Vec<Column> {
    cols.iter()
        .map(|c| match c {
            TaxColumn::Hsn => column("HSN/SAC", 9, Align::Left),
            TaxColumn::TaxableValue => column("Taxable value", 12, Align::Right),
            TaxColumn::CgstRate => column("CGST rate", 7, Align::Right),
            TaxColumn::CgstAmount => column("CGST", 10, Align::Right),
            TaxColumn::SgstRate => column("SGST rate", 7, Align::Right),
            TaxColumn::SgstAmount => column("SGST", 10, Align::Right),
            TaxColumn::IgstRate => column("IGST rate", 7, Align::Right),
            TaxColumn::IgstAmount => column("IGST", 10, Align::Right),
            TaxColumn::CessAmount => column("Cess", 8, Align::Right),
            TaxColumn::TotalTax => column("Total tax", 11, Align::Right),
        })
        .collect()
}

fn column(title: &'static str, weight: i32, align: Align) -> Column {
    Column {
        title,
        weight,
        align,
    }
}
