//! What the GST screen draws, as plain values the tests can read: the GSTR-3B table laid out as
//! the mock lays it out (`design/mock-v2/index.html:1679-1686`) and the words of the 2B and IMS
//! tab before anything is imported (index.html:1692).
//!
//! Every amount is one the engine's working already holds (rule C4). A mock row the engine has
//! no figure for, such as the credit accepted in IMS or the cash payable after set-off, is not
//! drawn; `design/PARITY-GAPS.md` lists them.

use ledgerly_api::{Gstr3bPayment, Gstr3bTaxAmounts, Gstr3bWorking, TaxableAgreement};
use ledgerly_core::Outcome;
use ledgerly_core::Paise;
use ledgerly_core::format::format_inr;

use crate::i18n::Tr;

/// The English words of the GSTR-3B table, its figure headings and its sources, with the key of
/// each one's translation. The words are the ones [`gstr3b_lines`] and [`GSTR3B_COLUMNS`] build
/// the table from; the form names and tax heads (IGST, CGST, SGST, Cess) stay in Latin letters.
const TABLE_WORDS: [(&str, &str); 10] = [
    ("Table", "gst.3b.table"),
    ("Taxable", "gst.3b.taxable"),
    ("3.1(a) Outward taxable supplies", "gst.3b.outward"),
    ("4 Eligible ITC", "gst.3b.eligible_itc"),
    ("In your books", "gst.3b.in_books"),
    ("Blocked or ineligible, not claimed", "gst.3b.blocked"),
    ("sales vouchers", "gst.3b.src_sales"),
    ("purchase vouchers", "gst.3b.src_purchase"),
    ("From the return", "gst.figures.from_return"),
    ("Amount", "gst.figures.amount"),
];

/// A GSTR-3B table word in the words of `tr`. A word with no key in the table of words is shown
/// as it is, so a new table row is never blank.
#[must_use]
pub fn text_in(text: &str, tr: &Tr) -> String {
    TABLE_WORDS
        .iter()
        .find(|(en, _)| *en == text)
        .map_or_else(|| text.to_owned(), |(_, key)| tr.t(key))
}

/// The column headings of the GSTR-3B table. The mock has Table, Taxable, IGST, CGST and SGST;
/// Cess is added because the engine works it out and a figure is never dropped.
pub const GSTR3B_COLUMNS: [&str; 6] = ["Table", "Taxable", "IGST", "CGST", "SGST", "Cess"];

/// One line of the GSTR-3B table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line3b {
    /// A sub-heading across the table, such as "4 Eligible ITC" (the mock's `tr.sub`).
    Sub(&'static str),
    /// A sub-heading with a provenance chip after it (the mock's `tr.sub` with a `prov`).
    SubSourced {
        /// The heading.
        label: &'static str,
        /// Where the rows below come from.
        source: &'static str,
    },
    /// The closing line of the table, drawn bold (the mock's `tr.total`): a figure in the
    /// Taxable column and the four tax heads.
    Total {
        /// What the line is.
        label: &'static str,
        /// The figure in the Taxable column (here the four heads added).
        taxable: Paise,
        /// IGST, CGST, SGST and cess, in that order.
        tax: [Paise; 4],
    },
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

/// The table with the payment of tax (table 6.1) the engine worked out: under "4 Eligible ITC"
/// the credit accepted in IMS and the credit with no action yet (shown apart, not used), then
/// "6.1 Payment of tax" with what is left after the accepted credit and the cash payable. When
/// the engine has no payment (`Unsupported`), the table is the working's own rows alone; the
/// reason is [`payment_note`].
#[must_use]
pub fn gstr3b_lines_with_payment(
    w: &Gstr3bWorking,
    payment: &Outcome<Gstr3bPayment>,
) -> Vec<Line3b> {
    let mut lines = gstr3b_lines(w);
    let Outcome::Value(p) = payment else {
        return lines;
    };
    let ind =
        |label: &'static str, source: Option<&'static str>, t: Gstr3bTaxAmounts| Line3b::Row {
            label,
            source,
            indent: true,
            taxable: None,
            tax: heads(t),
        };
    // After "In your books" (index 2), before the blocked row.
    lines.insert(
        3,
        ind("Accepted in IMS", Some("your actions"), p.credit_accepted),
    );
    lines.insert(
        4,
        ind(
            "No action yet: deemed accepted when 2B is generated",
            None,
            p.credit_no_action,
        ),
    );
    lines.push(Line3b::SubSourced {
        label: "6.1 Payment of tax",
        source: "engine set-off",
    });
    lines.push(ind(
        "Left after using accepted credit",
        None,
        p.set_off.left_to_pay,
    ));
    // The cash total is the four heads added; an overflow leaves the line out (no wrong number).
    if let Ok(total) = p.set_off.cash_total() {
        lines.push(Line3b::Total {
            label: "Payable in cash, rounded to rupees",
            taxable: total,
            tax: heads(p.set_off.cash_payable),
        });
    }
    lines
}

/// The line under the table when the engine has no payment of tax to show, `None` when it has.
#[must_use]
pub fn payment_note(payment: &Outcome<Gstr3bPayment>) -> Option<String> {
    match payment {
        Outcome::Value(_) => None,
        Outcome::Unsupported(u) => Some(format!(
            "Payment of tax not worked out: {}: {}.",
            u.what, u.reason
        )),
    }
}

/// The month word of a period label such as `November 2026`.
#[must_use]
pub fn month_word(period_label: &str) -> &str {
    period_label
        .split_whitespace()
        .next()
        .unwrap_or(period_label)
}

/// The title and sentence of the 2B and IMS tab when no 2B file has been imported for
/// `period_label` (the mock's `recon-empty`, index.html:1692). The mock's "Drop it here" is
/// left out: the screen takes no dropped files.
#[must_use]
pub fn recon_empty(period_label: &str) -> (String, String) {
    let month = month_word(period_label);
    (
        format!("No 2B file for {month} yet"),
        format!(
            "On the GST portal, open Returns, then GSTR-2B, choose {period_label} and download \
             the JSON. Press Ctrl+O to choose it. The engine matches it to your purchase \
             vouchers; nothing is sent anywhere."
        ),
    )
}

/// The words of the lock bar of a period whose GSTR-3B was marked filed (the mock's
/// `recon-locked`, index.html:1702): the bold lead and the rest. The mock's closing sentence
/// about an amendment in a later month is left out: no published GST source for it was found.
#[must_use]
pub fn lock_words(period_label: &str, filed_on: &str) -> (String, String) {
    let month = month_word(period_label);
    (
        format!("{month} is final."),
        format!(
            "You marked GSTR-3B for {month} filed on {filed_on}, so these IMS actions are read-only."
        ),
    )
}

/// The first line of the refusal note for a 2B file the engine did not take.
#[must_use]
pub fn import_refusal(reason: &str) -> String {
    format!("Not imported: {reason}.")
}

/// The line under the GSTR-1 section table (the mock's "Taxable value agrees with GSTR-3B table
/// 3.1(a): ..."), from the engine's comparison. When the two figures differ it says both and never
/// says they agree.
#[must_use]
pub fn agreement_text(a: &TaxableAgreement) -> String {
    if a.agrees() {
        crate::i18n::tf("gst.agreement.agrees", &[("amount", &format_inr(a.gstr1))])
    } else {
        crate::i18n::tf(
            "gst.agreement.differs",
            &[
                ("gstr1", &format_inr(a.gstr1)),
                ("gstr3b", &format_inr(a.gstr3b)),
            ],
        )
    }
}
