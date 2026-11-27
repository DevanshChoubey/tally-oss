//! S23: the 180-day rule's inputs read from the books' bill references.
//!
//! A purchase opens a bill on the supplier's ledger (a `New` reference); payments settle it with
//! `Against` (or earlier `Advance`) references of the same name on the same ledger. That link is
//! what `itc_conditions::unpaid_invoice_from_bills` reads. Anything that settles the bill other
//! than a payment voucher, or a payment to the supplier not linked to any bill, leaves the unpaid
//! amount unknown and is `Unsupported`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{LedgerId, Outcome, Paise, SignedAmount, VoucherId, VoucherTypeId};
use ledgerly_gst::gstr3b::Gstr3bTaxAmounts;
use ledgerly_gst::itc_conditions::{UnpaidInvoice, unpaid_invoice_from_bills};
use ledgerly_model::{
    BaseVoucherKind, BillAllocation, BillRefKind, LedgerLine, Voucher, VoucherMode, VoucherStatus,
};
use time::Date;
use time::macros::date;

const SUPPLIER: LedgerId = LedgerId(7);
const BANK: LedgerId = LedgerId(8);
const PURCHASES: LedgerId = LedgerId(9);

fn itc() -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: Paise::new(18_000),
        central: Paise::ZERO,
        state_ut: Paise::ZERO,
        cess: Paise::ZERO,
    }
}

fn voucher(id: u64, on: Date) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(id.to_string()),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: Some(SUPPLIER),
        place_of_supply: None,
        reverse_charge: false,
        reference: Some("INV-1".to_owned()),
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn bill(kind: BillRefKind, name: &str, amount: SignedAmount) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.to_owned(),
        due_date: None,
        credit_days: None,
        amount,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    LedgerLine {
        ledger,
        amount,
        bills,
        cost: vec![],
        bank: None,
        ineligible_itc: None,
    }
}

/// A purchase of 1,000.00 plus IGST 180.00 dated 12 Jan 2026, on the supplier's invoice INV-1 of
/// 10 Jan 2026, opening bill INV-1 for 1,180.00.
fn purchase() -> Voucher {
    let mut v = voucher(1, date!(2026 - 01 - 12));
    v.reference_date = Some(date!(2026 - 01 - 10));
    let total = SignedAmount::cr(Paise::new(118_000));
    v.ledger_lines = vec![
        line(
            SUPPLIER,
            total,
            vec![bill(BillRefKind::New, "INV-1", total)],
        ),
        line(PURCHASES, SignedAmount::dr(Paise::new(118_000)), vec![]),
    ];
    v
}

/// A payment of `paise` to the supplier on `on`, settling bill `name` with reference `kind`.
fn payment(id: u64, on: Date, kind: BillRefKind, name: &str, paise: i64) -> Voucher {
    let mut v = voucher(id, on);
    let amount = SignedAmount::dr(Paise::new(paise));
    v.ledger_lines = vec![
        line(SUPPLIER, amount, vec![bill(kind, name, amount)]),
        line(BANK, SignedAmount::cr(Paise::new(paise)), vec![]),
    ];
    v
}

fn value(o: Outcome<UnpaidInvoice>) -> UnpaidInvoice {
    match o {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {}: {}", u.what, u.reason),
    }
}

#[test]
fn payments_against_the_bill_are_the_invoices_payments() {
    // provenance: derived. CGST Rules, Rule 37(1) (CBIC tax information portal, read
    // 2026-11-27): the reversal is proportionate to "the amount not paid" of "the value of supply
    // along with tax". The purchase's bill INV-1 holds that value, 1,180.00; the two payments
    // against INV-1 (400.00 on 20 Feb 2026, 300.00 on 15 Mar 2026) are its payments; the invoice
    // date is the supplier's invoice date (the voucher's reference date), 10 Jan 2026.
    let p = purchase();
    let pay1 = payment(
        2,
        date!(2026 - 02 - 20),
        BillRefKind::Against,
        "INV-1",
        40_000,
    );
    let pay2 = payment(
        3,
        date!(2026 - 03 - 15),
        BillRefKind::Against,
        "INV-1",
        30_000,
    );
    let got = value(
        unpaid_invoice_from_bills(
            &p,
            SUPPLIER,
            [
                (BaseVoucherKind::Payment, &pay1),
                (BaseVoucherKind::Payment, &pay2),
            ],
            itc(),
        )
        .unwrap(),
    );
    assert_eq!(
        got,
        UnpaidInvoice {
            invoice_date: date!(2026 - 01 - 10),
            payable: Paise::new(118_000),
            deemed_paid: Paise::ZERO,
            payments: vec![
                (date!(2026 - 02 - 20), Paise::new(40_000)),
                (date!(2026 - 03 - 15), Paise::new(30_000)),
            ],
            itc: itc(),
            reverse_charge: false,
        }
    );
}

#[test]
fn an_advance_of_the_same_name_counts_and_other_bills_do_not() {
    // provenance: derived. An `Advance` reference named INV-1 joins the same bill (ledgerly-engine
    // bills.rs: "An Advance and a New of one name form a single bill"), so the 200.00 advance of
    // 5 Jan 2026 is paid on the invoice; a payment against bill INV-2 pays another invoice.
    let p = purchase();
    let adv = payment(
        2,
        date!(2026 - 01 - 05),
        BillRefKind::Advance,
        "INV-1",
        20_000,
    );
    let other = payment(
        3,
        date!(2026 - 02 - 01),
        BillRefKind::Against,
        "INV-2",
        50_000,
    );
    let got = value(
        unpaid_invoice_from_bills(
            &p,
            SUPPLIER,
            [
                (BaseVoucherKind::Payment, &adv),
                (BaseVoucherKind::Payment, &other),
            ],
            itc(),
        )
        .unwrap(),
    );
    assert_eq!(
        got.payments,
        vec![(date!(2026 - 01 - 05), Paise::new(20_000))]
    );
}

#[test]
fn optional_and_cancelled_payments_are_not_paid() {
    // provenance: derived. Optional and cancelled vouchers are kept out of the books
    // (ledgerly-model VoucherStatus), so they pay nothing.
    let p = purchase();
    let mut opt = payment(
        2,
        date!(2026 - 02 - 01),
        BillRefKind::Against,
        "INV-1",
        10_000,
    );
    opt.status = VoucherStatus::Optional;
    let mut can = payment(
        3,
        date!(2026 - 02 - 02),
        BillRefKind::Against,
        "INV-1",
        10_000,
    );
    can.status = VoucherStatus::Cancelled;
    let got = value(
        unpaid_invoice_from_bills(
            &p,
            SUPPLIER,
            [
                (BaseVoucherKind::Payment, &opt),
                (BaseVoucherKind::Payment, &can),
            ],
            itc(),
        )
        .unwrap(),
    );
    assert_eq!(got.payments, vec![]);
}

#[test]
fn a_purchase_marked_reverse_charge_says_so() {
    // provenance: derived. Rule 37(1) excludes supplies "on which tax is payable under reverse
    // charge"; the voucher's mark is carried so that rule37_reversal returns no reversal.
    let mut p = purchase();
    p.reverse_charge = true;
    let got = value(unpaid_invoice_from_bills(&p, SUPPLIER, [], itc()).unwrap());
    assert!(got.reverse_charge);
}

fn unsupported(o: Outcome<UnpaidInvoice>) -> String {
    match o {
        Outcome::Value(v) => panic!("expected Unsupported, got {v:?}"),
        Outcome::Unsupported(u) => u.reason,
    }
}

#[test]
fn a_purchase_with_no_bill_reference_is_unsupported() {
    // provenance: derived (never a wrong number). Without a bill on the supplier's line no
    // payment can be tied to this invoice, so the amount not paid is unknown.
    let mut p = purchase();
    p.ledger_lines[0].bills.clear();
    let reason = unsupported(unpaid_invoice_from_bills(&p, SUPPLIER, [], itc()).unwrap());
    assert!(reason.contains("bill"), "{reason}");
}

#[test]
fn a_purchase_with_no_supplier_invoice_date_is_unsupported() {
    // provenance: derived. Rule 37(1) counts from "the date of issue of invoice", the
    // supplier's date; the voucher date is the date the purchase was entered, not that.
    let mut p = purchase();
    p.reference_date = None;
    let reason = unsupported(unpaid_invoice_from_bills(&p, SUPPLIER, [], itc()).unwrap());
    assert!(reason.contains("invoice date"), "{reason}");
}

#[test]
fn a_bill_settled_by_a_debit_note_is_unsupported() {
    // provenance: derived. A debit note against the bill lowers what is owed but is not a
    // payment; whether it reduces "the amount not paid" under Rule 37 was not read.
    let p = purchase();
    let mut dn = payment(
        2,
        date!(2026 - 02 - 01),
        BillRefKind::Against,
        "INV-1",
        10_000,
    );
    dn.ledger_lines[1].ledger = PURCHASES;
    let reason = unsupported(
        unpaid_invoice_from_bills(&p, SUPPLIER, [(BaseVoucherKind::DebitNote, &dn)], itc())
            .unwrap(),
    );
    assert!(reason.contains("DebitNote"), "{reason}");
}

#[test]
fn an_on_account_payment_to_the_supplier_is_unsupported() {
    // provenance: derived. A payment "on account" is not tied to a bill, so it may or may not
    // pay this invoice; the amount not paid is unknown.
    let p = purchase();
    let pay = payment(2, date!(2026 - 02 - 01), BillRefKind::OnAccount, "", 10_000);
    let reason = unsupported(
        unpaid_invoice_from_bills(&p, SUPPLIER, [(BaseVoucherKind::Payment, &pay)], itc()).unwrap(),
    );
    assert!(reason.contains("on account"), "{reason}");
}

#[test]
fn a_reference_that_adds_to_the_bill_is_unsupported() {
    // provenance: derived. A credit `Against` INV-1 in another voucher raises the bill above the
    // invoice value; which part is "the value of supply along with tax" is then unknown.
    let p = purchase();
    let mut more = payment(
        2,
        date!(2026 - 02 - 01),
        BillRefKind::Against,
        "INV-1",
        10_000,
    );
    more.ledger_lines[0].amount = SignedAmount::cr(Paise::new(10_000));
    more.ledger_lines[0].bills[0].amount = SignedAmount::cr(Paise::new(10_000));
    let reason = unsupported(
        unpaid_invoice_from_bills(&p, SUPPLIER, [(BaseVoucherKind::Journal, &more)], itc())
            .unwrap(),
    );
    assert!(reason.contains("adds"), "{reason}");
}
