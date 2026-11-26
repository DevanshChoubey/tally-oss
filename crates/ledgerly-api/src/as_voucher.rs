//! GST for an entry made as ledger lines (As Voucher mode, Ctrl+H on a sales or purchase voucher).
//!
//! The accounting product's pages (read 2026-11-26) say: "Press Ctrl+H (Change mode) > select As
//! Voucher mode. In As Voucher mode, the voucher displays By and To instead of Dr and Cr"
//! (<https://help.tallysolutions.com/sales-of-goods-services/>,
//! <https://help.tallysolutions.com/purchase-goods-services/>); for GST the user "selects the
//! applicable GST ledgers: CGST and SGST/UTGST for local sales, IGST for interstate sales"
//! (<https://help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/>), and "if
//! Place of Supply and GST ledger do not match, GST will not be calculated correctly"
//! (<https://help.tallysolutions.com/tally-prime/other-scenarios/gst-not-calculated-tally/>).
//! The pages do not describe tax lines in this mode; the rules below are therefore a product
//! choice built on those two statements:
//!
//! * the entry names the party (`Voucher::party`), a place of supply, taxable lines (ledgers with
//!   a GST classification, on the side the voucher kind posts them) and the tax ledgers of the
//!   supply: CGST and SGST (or UTGST) for a local supply, IGST for an inter-state one, exactly
//!   that set;
//! * an amount typed on a tax line is ignored: the engine's figure replaces it (invariant 9);
//! * a party line typed by the user is dropped and rebuilt as the invoice value;
//! * a tax ledger that does not suit the place of supply, no tax ledger, a cess ledger, or stock
//!   allocation on the entry is `Unsupported`, never a figure.
//!
//! The tax itself is the accounting-invoice computation: the entry is turned into an accounting
//! invoice (taxable lines only) and handed to the same engine, so the figures cannot differ.

use ledgerly_core::Outcome;
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{GstBreakup, TaxHeads};
use ledgerly_gst::pack::RulePack;
use ledgerly_model::{Books, TaxLedgerKind, Voucher, VoucherMode};

use crate::voucher_gst::{
    InvoiceGst, PartyBill, TaxLedger, build_invoice, compute_for, is_gst_head, tax_side,
};

/// The entry as an accounting invoice, with the tax ledgers the user selected.
struct Parts {
    accounting: Voucher,
    tax_ledgers: Vec<TaxLedger>,
}

fn unsupported<T>(what: &str, reason: impl Into<String>) -> Result<Outcome<T>, GstError> {
    Ok(Outcome::unsupported(what, reason))
}

/// Splits the entry's lines into the party, the tax ledgers and the taxable particulars.
fn split(books: &Books, v: &Voucher) -> Result<Outcome<Parts>, GstError> {
    if !v.inventory_lines.is_empty() {
        return unsupported(
            "stock item allocation in As Voucher mode",
            "the rate of a stock item is not combined with ledger lines here; use the item invoice for stock items",
        );
    }
    let Some(party) = v.party else {
        return unsupported("GST on an as-voucher entry", "set the party first");
    };
    let base = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .and_then(|t| tax_side(t.base));
    if base.is_none() {
        return unsupported(
            "GST on an as-voucher entry",
            "only sales, purchase, credit-note and debit-note vouchers are taxed",
        );
    }
    let mut particulars = Vec::new();
    let mut tax_ledgers: Vec<TaxLedger> = Vec::new();
    for line in &v.ledger_lines {
        if line.ledger == party {
            continue;
        }
        let tax = books
            .masters
            .ledgers
            .get(&line.ledger)
            .and_then(|l| l.tax)
            .filter(|t| is_gst_head(t.kind));
        match tax {
            Some(t) => {
                if t.kind == TaxLedgerKind::Cess {
                    return unsupported(
                        "GST cess in As Voucher mode",
                        "no verified cess rate is available",
                    );
                }
                if !tax_ledgers.iter().any(|l| l.id == line.ledger) {
                    tax_ledgers.push(TaxLedger {
                        id: line.ledger,
                        kind: t.kind,
                        rate: t.rate_percent,
                    });
                }
            }
            None => particulars.push(line.clone()),
        }
    }
    if tax_ledgers.is_empty() {
        return unsupported(
            "tax ledger in As Voucher mode",
            "select the tax ledger lines: CGST and SGST for a local supply, IGST for an inter-state one; the tax is calculated from the ledgers selected",
        );
    }
    let accounting = Voucher {
        mode: VoucherMode::AccountingInvoice,
        ledger_lines: particulars,
        ..v.clone()
    };
    Ok(Outcome::Value(Parts {
        accounting,
        tax_ledgers,
    }))
}

/// `Unsupported` unless the selected tax ledgers are exactly the heads of the supply.
fn check_heads(breakup: &GstBreakup, chosen: &[TaxLedger]) -> Outcome<()> {
    let (needed, label): (&[TaxLedgerKind], &str) = match breakup.heads {
        TaxHeads::CgstSgst => (&[TaxLedgerKind::Cgst, TaxLedgerKind::Sgst], "CGST and SGST"),
        TaxHeads::CgstUtgst => (
            &[TaxLedgerKind::Cgst, TaxLedgerKind::Utgst],
            "CGST and UTGST",
        ),
        TaxHeads::Igst => (&[TaxLedgerKind::Igst], "IGST"),
    };
    let exact = needed.iter().all(|k| chosen.iter().any(|c| c.kind == *k))
        && chosen.iter().all(|c| needed.contains(&c.kind));
    if exact {
        Outcome::Value(())
    } else {
        Outcome::unsupported(
            "tax ledgers that do not match the place of supply",
            format!(
                "this supply takes {label}; GST is not calculated correctly when the tax ledgers and the place of supply do not match, so select exactly {label}"
            ),
        )
    }
}

/// The engine's tax for an entry made as ledger lines.
pub(crate) fn breakup(
    books: &Books,
    v: &Voucher,
    pack: &RulePack,
) -> Result<Outcome<GstBreakup>, GstError> {
    let parts = match split(books, v)? {
        Outcome::Value(p) => p,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let breakup = match compute_for(books, &parts.accounting, pack)? {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    match check_heads(&breakup, &parts.tax_ledgers) {
        Outcome::Value(()) => Ok(Outcome::Value(breakup)),
        Outcome::Unsupported(u) => Ok(Outcome::Unsupported(u)),
    }
}

/// The accounting lines and stored GST of an entry made as ledger lines.
pub(crate) fn invoice(
    books: &Books,
    v: &Voucher,
    pack: &RulePack,
    bills: &[PartyBill],
) -> Result<Outcome<InvoiceGst>, GstError> {
    let parts = match split(books, v)? {
        Outcome::Value(p) => p,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let breakup = match compute_for(books, &parts.accounting, pack)? {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    if let Outcome::Unsupported(u) = check_heads(&breakup, &parts.tax_ledgers) {
        return Ok(Outcome::Unsupported(u));
    }
    build_invoice(
        books,
        &parts.accounting,
        breakup,
        bills,
        &parts.tax_ledgers,
    )
}
