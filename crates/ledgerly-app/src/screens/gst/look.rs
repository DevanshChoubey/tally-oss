//! What the GST screen draws, as plain values the tests can read: the GSTR-3B table laid out as
//! the mock lays it out (`design/mock-v2/index.html:1679-1686`) and the words of the 2B and IMS
//! tab before anything is imported (index.html:1692).
//!
//! Every amount is one the engine's working already holds (rule C4). A mock row the engine has
//! no figure for, such as the credit accepted in IMS or the cash payable after set-off, is not
//! drawn; `design/PARITY-GAPS.md` lists them.

use ledgerly_api::{Gstr3bTaxAmounts, Gstr3bWorking};
use ledgerly_core::Paise;

/// The column headings of the GSTR-3B table. The mock has Table, Taxable, IGST, CGST and SGST;
/// Cess is added because the engine works it out and a figure is never dropped.
pub const GSTR3B_COLUMNS: [&str; 6] = ["Table", "Taxable", "IGST", "CGST", "SGST", "Cess"];

/// One line of the GSTR-3B table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line3b {
    /// A sub-heading across the table, such as "4 Eligible ITC" (the mock's `tr.sub`).
    Sub(&'static str),
    /// A row of figures.
    Row {
        /// What the row is.
        label: &'static str,
        /// Where the figures come from, drawn as a provenance chip (the mock's `prov`).
        source: Option<&'static str>,
        /// Indented under the sub-heading above it (the mock's `ind1`).
        indent: bool,
        /// The taxable value, when the table has one.
        taxable: Option<Paise>,
        /// IGST, CGST, SGST and cess, in that order.
        tax: [Paise; 4],
    },
}

/// The four tax heads in the order of [`GSTR3B_COLUMNS`].
fn heads(t: Gstr3bTaxAmounts) -> [Paise; 4] {
    [t.integrated, t.central, t.state_ut, t.cess]
}

/// The GSTR-3B table for `w`: table 3.1(a), then under "4 Eligible ITC" the credit in the books
/// (table 4(A)(5)) and the blocked or ineligible credit the engine keeps apart from it.
#[must_use]
pub fn gstr3b_lines(w: &Gstr3bWorking) -> Vec<Line3b> {
    vec![
        Line3b::Row {
            label: "3.1(a) Outward taxable supplies",
            source: Some("sales vouchers"),
            indent: false,
            taxable: Some(w.outward_taxable_value),
            tax: heads(w.outward_tax),
        },
        Line3b::Sub("4 Eligible ITC"),
        Line3b::Row {
            label: "In your books",
            source: Some("purchase vouchers"),
            indent: true,
            taxable: None,
            tax: heads(w.itc_all_other),
        },
        Line3b::Row {
            label: "Blocked or ineligible, not claimed",
            source: None,
            indent: true,
            taxable: None,
            tax: heads(w.itc_not_claimable),
        },
    ]
}

/// The title and sentence of the 2B and IMS tab when no 2B file has been read for
/// `period_label`. This version cannot read one, so the sentence says so and offers no import.
#[must_use]
pub fn recon_empty(period_label: &str) -> (String, String) {
    (
        format!("No 2B file for {period_label}"),
        "Reading the GSTR-2B file from the GST portal is not built in this version (engine item \
         E-3), so there is nothing to match against your purchase vouchers yet. Nothing is sent \
         anywhere."
            .to_owned(),
    )
}
