//! The GSTR-1 section table: one row per table of the return, with how many records it holds, the
//! taxable value and tax in it, and whether it is ready (design mock `gst.gstr1`).
//!
//! Tables, from the GSTR-1 form as the portal's offline tool lays it out: 4A (supplies to
//! registered persons, B2B), 5 (B2C large, inter-state), 7 (B2C others), 9B (credit and debit
//! notes), 12 (HSN summary) and 13 (documents issued). Tables 6A (exports) and 8 (nil-rated,
//! exempt and non-GST supplies) get a row only when the return holds something in them, so a
//! figure the return carries is never left out of the list because the mock had no row for it.
//!
//! Source for the table list: the offline tool's worksheets b2b (4A), b2cl (5), b2cs (7), cdnr (9B),
//! hsn (12) and docs (13), per the GSTN offline tool user manual as mirrored at
//! <https://gstzen.in/a/gst-manuals-user-manuals-downloading-installing-and-uploading-invoices-in-gstr-1-taking-actions-in-gstr-2.html>
//! (read 2026-11-25); the worksheet-to-table numbering is the form's, which that page does not
//! print, so the numbers are the mock's. Table 13's mandatory status is noted in
//! <https://help.tallysolutions.com/file-gstr-1-using-offline-tool/> (read 2026-11-25).
//!
//! A voucher held back whole (`NeedsCorrection` or `Mismatch`, not `in_tables`) is in no table. It
//! is listed as its own last row, `held`, with the number of vouchers and the status the worst of
//! them has, so the other rows are never read as the whole return. A voucher that is
//! `NeedsCorrection` only because an item has no HSN is in its tables and only Table 12 is short:
//! that row is `NeedsCorrection` with the engine's reason as its note (GSTN has the HSN field in
//! Table 12 only, GSTR-1 user guide read 2026-11-25). The other rows are `Ready` when they hold a
//! record and `NotRelevant` when they hold none.

use ledgerly_core::{MoneyError, Outcome, Paise};
use ledgerly_gst::gstr1::Bucket;
use ledgerly_gst_formats::gstr1::{B2bInvoice, B2clInvoice, CdnrNote, ItemTax, LineItem};

use crate::gst::{Gstr1, Gstr1Return};

/// One row of the section table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1Section {
    /// The table's number on the form: `4A`, `5`, `7`, `9B`, `12`, `13`, `6A`, `8`; `held` for
    /// the vouchers held back.
    pub code: &'static str,
    /// The table's name as the screen shows it after its number.
    pub name: &'static str,
    /// Documents or rows the table holds, as the form counts them (table 12 counts HSN rows,
    /// table 13 counts documents including cancelled ones).
    pub records: u64,
    /// Taxable value in the table, `None` where the table has no such figure (13, held).
    pub taxable: Option<Paise>,
    /// Tax in the table, `None` where the table has no such figure (12, 13, held).
    pub tax: Option<Paise>,
    /// `Ready`, `NotRelevant` (the table is empty), or the worst bucket of the held vouchers.
    pub status: Bucket,
    /// Why the status is not `Ready`, in the engine's words (the first voucher's reason), when a
    /// voucher in this table needs correction.
    pub note: Option<String>,
}

/// GSTR-1's taxable value against GSTR-3B table 3.1(a), from the same books.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxableAgreement {
    /// Taxable value of the regular invoices in tables 4A, 5 and 7.
    pub gstr1: Paise,
    /// Taxable value of table 3.1(a) of the GSTR-3B working.
    pub gstr3b: Paise,
}

impl TaxableAgreement {
    /// Whether the two figures are equal.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.gstr1 == self.gstr3b
    }
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn line_tax(line: &LineItem) -> Result<Paise, MoneyError> {
    match line.tax {
        ItemTax::Igst(igst) => igst,
        ItemTax::CgstSgst { cgst, sgst } => cgst.checked_add(sgst)?,
    }
    .checked_add(line.cess)
}

#[derive(Default)]
struct Sum {
    taxable: Paise,
    tax: Paise,
}

impl Sum {
    fn lines(&mut self, items: &[LineItem]) -> Result<(), MoneyError> {
        for line in items {
            self.taxable = self.taxable.checked_add(line.taxable)?;
            self.tax = self.tax.checked_add(line_tax(line)?)?;
        }
        Ok(())
    }
}

fn row(
    code: &'static str,
    name: &'static str,
    records: u64,
    sum: Option<Sum>,
    with_tax: bool,
) -> Gstr1Section {
    Gstr1Section {
        code,
        name,
        records,
        taxable: sum.as_ref().map(|s| s.taxable),
        tax: sum.filter(|_| with_tax).map(|s| s.tax),
        status: if records == 0 {
            Bucket::NotRelevant
        } else {
            Bucket::Ready
        },
        note: None,
    }
}

type Rows = Result<Gstr1Section, MoneyError>;

fn b2b_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    let mut n = 0_usize;
    for party in &r.b2b {
        for inv in &party.invoices {
            let B2bInvoice { items, .. } = inv;
            sum.lines(items)?;
            n = n.saturating_add(1);
        }
    }
    Ok(row("4A", "B2B invoices", count(n), Some(sum), true))
}

fn b2cl_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    let mut n = 0_usize;
    for group in &r.b2cl {
        for inv in &group.invoices {
            let B2clInvoice { items, .. } = inv;
            sum.lines(items)?;
            n = n.saturating_add(1);
        }
    }
    let name = "B2C large (inter-state)";
    Ok(row("5", name, count(n), Some(sum), true))
}

fn b2cs_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    for line in &r.b2cs {
        sum.lines(std::slice::from_ref(&line.line))?;
    }
    Ok(row("7", "B2C others", count(r.b2cs.len()), Some(sum), true))
}

fn cdn_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    let mut n = 0_usize;
    for party in &r.cdnr {
        for note in &party.notes {
            let CdnrNote { items, .. } = note;
            sum.lines(items)?;
            n = n.saturating_add(1);
        }
    }
    for note in &r.cdnur {
        sum.lines(&note.items)?;
        n = n.saturating_add(1);
    }
    let name = "Credit and debit notes";
    Ok(row("9B", name, count(n), Some(sum), true))
}

fn export_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    let mut n = 0_usize;
    for group in &r.exp {
        for inv in &group.invoices {
            for item in &inv.items {
                sum.taxable = sum.taxable.checked_add(item.taxable)?;
                sum.tax = sum.tax.checked_add(item.igst)?.checked_add(item.cess)?;
            }
            n = n.saturating_add(1);
        }
    }
    Ok(row("6A", "Exports", count(n), Some(sum), true))
}

fn nil_row(r: &Gstr1) -> Rows {
    let mut sum = Sum::default();
    for line in &r.nil {
        sum.taxable = sum
            .taxable
            .checked_add(line.nil_rated)?
            .checked_add(line.exempt)?
            .checked_add(line.non_gst)?;
    }
    let name = "Nil-rated, exempt and non-GST";
    Ok(row("8", name, count(r.nil.len()), Some(sum), false))
}

fn hsn_row(ret: &Gstr1Return) -> Rows {
    let mut sum = Sum::default();
    for line in &ret.gstr1.hsn {
        sum.taxable = sum.taxable.checked_add(line.taxable)?;
    }
    let mut section = row(
        "12",
        "HSN summary",
        count(ret.gstr1.hsn.len()),
        Some(sum),
        false,
    );
    let short = ret
        .buckets
        .iter()
        .find(|b| b.in_tables && b.bucket == Bucket::NeedsCorrection);
    if let Some(b) = short {
        section.status = Bucket::NeedsCorrection;
        section.note.clone_from(&b.reason);
    }
    Ok(section)
}

fn docs_row(r: &Gstr1) -> Gstr1Section {
    let mut docs = 0_u64;
    for series in &r.doc_issue {
        for range in &series.ranges {
            docs = docs.saturating_add(u64::from(range.total));
        }
    }
    row("13", "Documents issued", docs, None, false)
}

impl Gstr1Return {
    /// Compares the taxable value of tables 4A, 5 and 7 with table 3.1(a) of `working`.
    ///
    /// Table 3.1(a) holds the outward taxable supplies other than zero rated, nil rated and
    /// exempted ones, which GSTR-1 reports in 4A, 5 and 7 (regular invoices). The comparison is
    /// refused (`Unsupported`) where the two forms do not line up by construction: credit and debit
    /// notes (how 3.1(a) nets them is pending, CGST Act s.34), and SEZ or deemed export invoices
    /// (zero rated, so 3.1(b) not 3.1(a)).
    ///
    /// # Errors
    /// [`MoneyError`] when a sum overflows.
    pub fn taxable_vs_gstr3b(
        &self,
        working: &ledgerly_gst::gstr3b::Gstr3bWorking,
    ) -> Result<Outcome<TaxableAgreement>, MoneyError> {
        let r = &self.gstr1;
        if !r.cdnr.is_empty() || !r.cdnur.is_empty() {
            return Ok(Outcome::unsupported(
                "comparing GSTR-1 with GSTR-3B table 3.1(a) when the return has credit or debit notes",
                "how table 3.1(a) nets a note is pending (CGST Act s.34 not yet checked)",
            ));
        }
        let mut sum = Sum::default();
        for party in &r.b2b {
            for inv in &party.invoices {
                if inv.inv_type != ledgerly_gst_formats::gstr1::B2bInvoiceType::Regular {
                    return Ok(Outcome::unsupported(
                        "comparing GSTR-1 with GSTR-3B table 3.1(a) when the return has SEZ or deemed export invoices",
                        "those are zero rated, so they belong to table 3.1(b), not 3.1(a)",
                    ));
                }
                sum.lines(&inv.items)?;
            }
        }
        for group in &r.b2cl {
            for inv in &group.invoices {
                sum.lines(&inv.items)?;
            }
        }
        for line in &r.b2cs {
            sum.lines(std::slice::from_ref(&line.line))?;
        }
        Ok(Outcome::Value(TaxableAgreement {
            gstr1: sum.taxable,
            gstr3b: working.outward_taxable_value,
        }))
    }

    /// The section table of this return, in the form's order.
    ///
    /// # Errors
    /// [`MoneyError`] when a sum overflows.
    pub fn sections(&self) -> Result<Vec<Gstr1Section>, MoneyError> {
        let r = &self.gstr1;
        let mut out = vec![b2b_row(r)?, b2cl_row(r)?, b2cs_row(r)?, cdn_row(r)?];
        if !r.exp.is_empty() {
            out.push(export_row(r)?);
        }
        if !r.nil.is_empty() {
            out.push(nil_row(r)?);
        }
        out.push(hsn_row(self)?);
        out.push(docs_row(r));
        out.extend(self.held_row());
        Ok(out)
    }

    /// The last row: the vouchers held back, with the worst status among them.
    fn held_row(&self) -> Option<Gstr1Section> {
        let held: Vec<Bucket> = self
            .buckets
            .iter()
            .filter(|b| !b.in_tables)
            .map(|b| b.bucket)
            .filter(|b| matches!(b, Bucket::NeedsCorrection | Bucket::Mismatch))
            .collect();
        if held.is_empty() {
            return None;
        }
        let status = if held.contains(&Bucket::NeedsCorrection) {
            Bucket::NeedsCorrection
        } else {
            Bucket::Mismatch
        };
        Some(Gstr1Section {
            code: "held",
            name: "Vouchers held back",
            records: count(held.len()),
            taxable: None,
            tax: None,
            status,
            note: None,
        })
    }
}
