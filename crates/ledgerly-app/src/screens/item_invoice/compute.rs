//! Asks the API for the tax of the invoice on screen (card C2).
//!
//! The screen holds no tax logic (rule C4). It sends the draft voucher to the worker, which runs
//! `Api::invoice_gst`, and shows the answer through `ItemInvoiceScreen::set_gst`. This module only
//! moves the answer's fields into the block the screen shows: nothing is added up here.

use ledgerly_api::{Api, ApiError, InvoiceGst, PartyBill};
use ledgerly_core::Outcome;
use ledgerly_model::Voucher;

use super::{GstBlock, GstBlockLine, GstRateRow, GstResult, GstTotal};
use crate::worker::{ApiWorker, Poll, Ticket};

/// The worker's answer: the API's outcome, or the text of its error.
pub(super) type ComputeReply = Result<Outcome<InvoiceGst>, String>;

/// Queues the tax computation for `voucher` on the worker. Never blocks.
#[must_use]
pub(super) fn send(
    worker: &ApiWorker,
    voucher: Voucher,
    bills: Vec<PartyBill>,
) -> Ticket<ComputeReply> {
    worker.run(move |api: &mut Api| {
        api.invoice_gst_with_bills(&voucher, &bills)
            .map_err(|e: ApiError| e.to_string())
    })
}

/// Checks a ticket without blocking. `None` while the worker is still busy. A stopped worker is
/// a failure, never a figure.
pub(super) fn settle(ticket: &mut Ticket<ComputeReply>) -> Option<ComputeReply> {
    match ticket.poll() {
        Poll::Pending | Poll::Taken => None,
        Poll::Ready(reply) => Some(reply),
        Poll::Lost => Some(Err(
            "the engine stopped before it answered the tax computation".to_owned(),
        )),
    }
}

/// The screen's view of the worker's answer.
pub(super) fn to_result(reply: ComputeReply) -> GstResult {
    match reply {
        Ok(Outcome::Value(gst)) => GstResult::Computed(to_block(gst)),
        Ok(Outcome::Unsupported(u)) => GstResult::Unsupported(u),
        Err(message) => GstResult::Failed(message),
    }
}

/// Copies the API's figures into the block; the display order of the heads is CGST, SGST, UTGST,
/// IGST and heads that are nil are not listed.
fn to_block(gst: InvoiceGst) -> GstBlock {
    let totals = &gst.breakup.totals;
    let heads = [
        ("CGST", totals.cgst),
        ("SGST", totals.sgst),
        ("UTGST", totals.utgst),
        ("IGST", totals.igst),
    ]
    .into_iter()
    .filter(|(_, amount)| !amount.is_zero())
    .map(|(head, amount)| GstTotal {
        head: head.to_owned(),
        amount,
    })
    .collect();
    GstBlock {
        pack_version: gst.breakup.pack_version.clone(),
        lines: gst
            .breakup
            .lines
            .iter()
            .map(|l| GstBlockLine {
                taxable: l.taxable,
                rate_percent: l.rate_percent,
            })
            .collect(),
        taxable_total: gst.breakup.taxable_total,
        rate_rows: gst
            .rate_rows
            .iter()
            .map(|r| GstRateRow {
                head: r.head.clone(),
                rate_percent: r.rate_percent,
                taxable: r.taxable,
                amount: r.amount,
            })
            .collect(),
        heads,
        round_off: gst.breakup.round_off,
        invoice_total: gst.invoice_total,
        ledger_lines: gst.ledger_lines,
        stored: gst.stored,
        eligible_itc: gst.eligible_itc,
    }
}
