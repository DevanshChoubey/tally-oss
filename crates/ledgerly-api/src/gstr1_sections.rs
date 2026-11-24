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
//! A voucher held back (`NeedsCorrection` or `Mismatch`) is in no table. It is listed as its own
//! last row, `held`, with the number of vouchers and the status the worst of them has, so the
//! other rows are never read as the whole return. The other rows are `Ready` when they hold a
//! record and `NotRelevant` when they hold none.

use ledgerly_core::{MoneyError, Paise};
use ledgerly_gst::gstr1::Bucket;
use ledgerly_gst_formats::gstr1::{B2bInvoice, B2clInvoice, CdnrNote, ItemTax, LineItem};

use crate::gst::Gstr1Return;

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
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn line_tax(line: &LineItem) -> Result<Paise, MoneyError> {
    Ok(match line.tax {
        ItemTax::Igst(igst) => igst,
        ItemTax::CgstSgst { cgst, sgst } => cgst.checked_add(sgst)?,
    }
    .checked_add(line.cess)?)
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
    }
}

impl Gstr1Return {
    /// The section table of this return, in the form's order.
    ///
    /// # Errors
    /// [`MoneyError`] when a sum overflows.
    pub fn sections(&self) -> Result<Vec<Gstr1Section>, MoneyError> {
        let r = &self.gstr1;
        let mut out = Vec::with_capacity(8);

        let mut b2b = Sum::default();
        let mut n = 0_usize;
        for party in &r.b2b {
            for inv in &party.invoices {
                let B2bInvoice { items, .. } = inv;
                b2b.lines(items)?;
                n = n.saturating_add(1);
            }
        }
        out.push(row("4A", "B2B invoices", count(n), Some(b2b), true));

        let mut b2cl = Sum::default();
        let mut n = 0_usize;
        for group in &r.b2cl {
            for inv in &group.invoices {
                let B2clInvoice { items, .. } = inv;
                b2cl.lines(items)?;
                n = n.saturating_add(1);
            }
        }
        out.push(row(
            "5",
            "B2C large (inter-state)",
            count(n),
            Some(b2cl),
            true,
        ));

        let mut b2cs = Sum::default();
        for line in &r.b2cs {
            b2cs.lines(std::slice::from_ref(&line.line))?;
        }
        out.push(row(
            "7",
            "B2C others",
            count(r.b2cs.len()),
            Some(b2cs),
            true,
        ));

        let mut cdn = Sum::default();
        let mut n = 0_usize;
        for party in &r.cdnr {
            for note in &party.notes {
                let CdnrNote { items, .. } = note;
                cdn.lines(items)?;
                n = n.saturating_add(1);
            }
        }
        for note in &r.cdnur {
            cdn.lines(&note.items)?;
            n = n.saturating_add(1);
        }
        out.push(row(
            "9B",
            "Credit and debit notes",
            count(n),
            Some(cdn),
            true,
        ));

        if !r.exp.is_empty() {
            let mut exp = Sum::default();
            let mut n = 0_usize;
            for group in &r.exp {
                for inv in &group.invoices {
                    for item in &inv.items {
                        exp.taxable = exp.taxable.checked_add(item.taxable)?;
                        exp.tax = exp.tax.checked_add(item.igst)?.checked_add(item.cess)?;
                    }
                    n = n.saturating_add(1);
                }
            }
            out.push(row("6A", "Exports", count(n), Some(exp), true));
        }

        if !r.nil.is_empty() {
            let mut nil = Sum::default();
            for line in &r.nil {
                nil.taxable = nil
                    .taxable
                    .checked_add(line.nil_rated)?
                    .checked_add(line.exempt)?
                    .checked_add(line.non_gst)?;
            }
            out.push(row(
                "8",
                "Nil-rated, exempt and non-GST",
                count(r.nil.len()),
                Some(nil),
                false,
            ));
        }

        let mut hsn = Sum::default();
        for line in &r.hsn {
            hsn.taxable = hsn.taxable.checked_add(line.taxable)?;
        }
        out.push(row(
            "12",
            "HSN summary",
            count(r.hsn.len()),
            Some(hsn),
            false,
        ));

        let mut docs = 0_u64;
        for series in &r.doc_issue {
            for range in &series.ranges {
                docs = docs.saturating_add(u64::from(range.total));
            }
        }
        out.push(row("13", "Documents issued", docs, None, false));

        let held: Vec<Bucket> = self
            .buckets
            .iter()
            .map(|b| b.bucket)
            .filter(|b| matches!(b, Bucket::NeedsCorrection | Bucket::Mismatch))
            .collect();
        if !held.is_empty() {
            let status = if held.contains(&Bucket::NeedsCorrection) {
                Bucket::NeedsCorrection
            } else {
                Bucket::Mismatch
            };
            out.push(Gstr1Section {
                code: "held",
                name: "Vouchers held back",
                records: count(held.len()),
                taxable: None,
                tax: None,
                status,
            });
        }
        Ok(out)
    }
}
