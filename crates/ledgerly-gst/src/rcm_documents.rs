//! The documents a recipient issues for a supply under reverse charge, and when (round s23).
//!
//! # The law
//!
//! Read on CBIC's tax information portal (taxinformation.cbic.gov.in) on 2026-11-27:
//!
//! * CGST Act s.31(3)(f): a registered person liable to pay tax under s.9(3) or s.9(4) "shall,
//!   within the period as may be prescribed, issue an invoice in respect of goods or services or
//!   both received by him from the supplier who is not registered on the date of receipt". The
//!   Explanation counts a supplier "registered solely for the purpose of deduction of tax under
//!   section 51" as not registered. Both the period and the Explanation were inserted by s.122 of
//!   the Finance (No. 2) Act, 2024.
//! * CGST Act s.31(3)(g): the same person "shall issue a payment voucher at the time of making
//!   payment to the supplier", whatever the supplier's registration. Its particulars are those of
//!   Rule 52.
//! * Rule 47A of the CGST Rules, inserted w.e.f. 01.11.2024 by Notification 20/2024-CT: the
//!   s.31(3)(f) invoice is issued "within a period of thirty days from the date of receipt of the
//!   said supply".
//!
//! # What is decided
//!
//! Which of the two documents a reverse-charge purchase needs, and the last day for the
//! recipient's invoice: the date of receipt plus thirty days. A receipt before 1 November 2024 is
//! `Unsupported`: no period had been prescribed in the text read, and whether Rule 47A reaches an
//! earlier receipt is not settled by it.
//!
//! The documents' particulars (Rule 46 for the invoice, Rule 52 for the payment voucher) and their
//! numbering are not produced here; this module only says what is due.

use ledgerly_core::{Date, Outcome};
use time::Duration;
use time::macros::date;

/// The day Rule 47A comes into force (Notification 20/2024-CT).
const RULE_47A_FROM: Date = date!(2024 - 11 - 01);

/// Days within which the recipient's invoice is issued (Rule 47A).
const RULE_47A_DAYS: i64 = 30;

/// The supplier's registration on the date of receipt, as s.31(3)(f) reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupplierStatus {
    /// Registered (other than only for tax deduction): the supplier issues the invoice.
    Registered,
    /// Registered solely to deduct tax under s.51: "not registered" by the Explanation.
    TaxDeductorOnly,
    /// Not registered.
    Unregistered,
}

/// What the recipient issues for one reverse-charge purchase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcmDocuments {
    /// `Some` when the recipient issues the invoice (s.31(3)(f)): the last day for it, or
    /// `Unsupported` when no time limit is known. `None` when the supplier invoices.
    pub self_invoice_due: Option<Outcome<Date>>,
    /// A payment voucher is issued when the supplier is paid (s.31(3)(g)). Always `true` under
    /// reverse charge; kept as a field so the caller does not assume it.
    pub payment_voucher: bool,
}

/// The last day to issue the recipient's invoice for a supply received on `receipt` (Rule 47A).
pub fn self_invoice_due(receipt: Date) -> Outcome<Date> {
    if receipt < RULE_47A_FROM {
        return Outcome::unsupported(
            format!("the time limit for the recipient's invoice for a receipt on {receipt}"),
            "Rule 47A applies from 1 November 2024; no time limit was read for earlier receipts",
        );
    }
    match receipt.checked_add(Duration::days(RULE_47A_DAYS)) {
        Some(due) => Outcome::Value(due),
        None => Outcome::unsupported(
            format!("the time limit for the recipient's invoice for a receipt on {receipt}"),
            "the date thirty days later is outside the supported calendar",
        ),
    }
}

/// The documents due for a reverse-charge purchase from a supplier of `status`, received on
/// `receipt`.
pub fn rcm_documents(status: SupplierStatus, receipt: Date) -> RcmDocuments {
    let self_invoice_due = match status {
        SupplierStatus::Registered => None,
        SupplierStatus::TaxDeductorOnly | SupplierStatus::Unregistered => {
            Some(self_invoice_due(receipt))
        }
    };
    RcmDocuments {
        self_invoice_due,
        payment_voucher: true,
    }
}
