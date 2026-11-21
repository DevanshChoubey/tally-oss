//! GSTR-1 builder with exception buckets. **Card G8** implements this module.
//!
//! Frozen interface (plan card G8):
//!
//! ```text
//! pub enum Bucket { Ready, NotRelevant, Mismatch, NeedsCorrection }
//! pub struct BucketedVoucher { pub voucher: VoucherId, pub bucket: Bucket, pub reason: Option<String> }
//! pub fn build_gstr1(books: &Books, gstin: &Gstin, period: Period, pack: &RulePack) -> Result<Outcome<(Gstr1, Vec<BucketedVoucher>)>, GstError>;
//! ```
//!
//! # What is built
//!
//! Every voucher dated inside `period` gets exactly one [`BucketedVoucher`]; vouchers dated
//! outside it are not part of this return and get none. The return holds Table 4 (B2B), 5
//! (B2CL), 7 (B2CS), 8 (nil, exempt, non-GST), 9B for registered recipients (CDNR), 12 (HSN) and
//! 13 (documents issued). `cdnur` and `exp` are always empty (see below).
//!
//! # Buckets
//!
//! * [`Bucket::Ready`]: the voucher's figures are in the return.
//! * [`Bucket::NotRelevant`]: the voucher is not a GSTR-1 document: purchases and debit notes
//!   (inward), payments, journals, orders and stock vouchers, receipts with no advance, optional
//!   vouchers (memoranda) and cancelled vouchers (a cancelled sales voucher still counts as
//!   cancelled in Table 13).
//! * [`Bucket::Mismatch`]: the books disagree with themselves. Either the tax stored on the
//!   voucher (`Voucher::gst`) differs from the tax recomputed by [`compute_voucher_tax`], or the
//!   rate-wise rows (each rate rounded to the paisa) do not add up to the voucher's head totals.
//!   Books hold no portal data, so there is no comparison with the portal here.
//! * [`Bucket::NeedsCorrection`]: the voucher cannot be placed until the user fixes data (no
//!   number, a number over 16 characters, a registered party with no GSTIN, no HSN, no unit
//!   quantity code, a duplicate document number) **or** a rule is `Unsupported` (the reason then
//!   starts with `unsupported:` and names the rule). Nothing is guessed; the voucher is left out
//!   of the return and listed here, so the return is complete only when every in-period voucher
//!   is `Ready` or `NotRelevant`.
//!
//! # Rules
//!
//! * A sales voucher is a B2B invoice (registered recipient, GSTIN required), or for an
//!   unregistered or unknown recipient a B2CL invoice (inter-state and invoice value strictly
//!   above the pack's threshold) or a B2CS contribution. The invoice value is the taxable value
//!   plus the rounded tax heads plus the round-off line of the voucher's [`GstBreakup`].
//! * Intra-state versus inter-state is the voucher's [`TaxHeads`], not a second opinion.
//! * Rate-wise rows are the taxable lines of the voucher grouped by rate; the **unrounded** line
//!   tax of a rate is summed and rounded half away from zero to the paisa once per rate and head.
//!   Union-territory tax goes in the state-tax column. B2CS rows add the per-voucher rate rows of
//!   all vouchers with the same place of supply, intra/inter flag and rate.
//! * Table 8 takes exempt, nil-rated and non-GST lines by inter/intra and registered/not.
//! * A credit note to a registered recipient is a CDNR note. Notes to unregistered recipients
//!   (CDNUR or a Table 7 adjustment, depending on the original invoice, which a voucher does not
//!   identify) and notes carrying exempt, nil-rated or non-GST lines are `NeedsCorrection`
//!   (`unsupported:`).
//! * HSN rows (Table 12) are summed per HSN, unit quantity code and rate, over **all** outward
//!   lines (taxable and not). Services (SAC) report `NA` and a zero quantity. The tax of a row is
//!   the sum of unrounded line tax rounded once. Credit notes reduce the rows only when the rule
//!   pack says so (`gstr1.hsn_credit_notes`, parameter `treatment` = `net` or `exclude`); with no
//!   verified entry a credit note is `NeedsCorrection` (`unsupported:`). There is no B2B/B2C
//!   split of Table 12 in the data shape yet, so one combined table is built.
//! * Table 13 lists each run of consecutive numbers (same prefix, numeric tail one apart) of
//!   invoices and of credit notes, with the cancelled ones counted. A gap starts a new run; the
//!   builder never invents a document. Receipt-voucher series are not summarised.
//!
//! # Pack entries read
//!
//! * `gstr1.b2cl_threshold`, parameter `invoice_value_rupees` (research 04-gst section 2.5: Rs 1
//!   lakh from 1 Aug 2024, N.12/2024-CT, tag `[V]`). Missing: inter-state sales to unregistered
//!   recipients are `NeedsCorrection`.
//! * `gstr1.hsn_credit_notes`, parameter `treatment`. Missing: credit notes are `NeedsCorrection`.
//!
//! # Not built (`unsupported:` per voucher)
//!
//! Exports, SEZ supplies and reverse charge (the tax computation refuses them), advances
//! (Tables 11A and 11B have no data shape), e-commerce operator supplies (all B2CS rows are "other
//! than e-commerce"), amendments, and the time-of-supply rules of card G12 beyond the voucher's
//! own `supply_date` (the same date rule as [`compute_voucher_tax`] is used, because it demands
//! the rule set of exactly that date).

use std::collections::BTreeMap;

use ledgerly_core::{
    Date, Decimal, Gstin, Hsn, MoneyError, Outcome, Paise, Period, Qty, Rate, RoundingRule,
    StateCode, Unsupported, VoucherId, round_rupees_to_paise,
};
use ledgerly_gst_formats::gstr1::{
    B2bInvoice, B2bInvoiceType, B2bParty, B2clGroup, B2clInvoice, B2csKind, B2csRow, CdnrNote,
    CdnrParty, DocIssue, DocRange, DocType, Gstr1, HsnRow, ItemTax, LineItem, NilRow,
    NilSupplyKind, NoteKind,
};
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, Books, RegistrationType, TaxLedgerKind, Voucher, VoucherStatus,
};

use crate::GstError;
use crate::classify::{SupplyKind, classify_line};
use crate::compute::{GstBreakup, LineSource, LineTax, TaxHeads, compute_voucher_tax};
use crate::pack::{RulePack, RuleSet};

/// Pack entry id of the B2CL invoice-value threshold (parameter `invoice_value_rupees`).
pub const B2CL_THRESHOLD_ID: &str = "gstr1.b2cl_threshold";

/// Pack entry id of the treatment of credit notes in the HSN summary (parameter `treatment`,
/// `net` or `exclude`).
pub const HSN_CREDIT_NOTES_ID: &str = "gstr1.hsn_credit_notes";

/// Where a voucher stands for this return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// The voucher's figures are in the return.
    Ready,
    /// The voucher is not a GSTR-1 document.
    NotRelevant,
    /// The books disagree with themselves about this voucher.
    Mismatch,
    /// The voucher is left out until the user fixes data, or because a rule is unsupported.
    NeedsCorrection,
}

/// One voucher and the bucket it landed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketedVoucher {
    /// The voucher.
    pub voucher: VoucherId,
    /// Its bucket.
    pub bucket: Bucket,
    /// Why, for every bucket except `Ready`.
    pub reason: Option<String>,
}

/// Builds GSTR-1 for `gstin` and `period` from the books, with every in-period voucher bucketed.
///
/// `period` must be one calendar month or one calendar quarter (Jan-Mar, Apr-Jun, Jul-Sep,
/// Oct-Dec); anything else, a company without GST enabled and a composition registration are
/// `Unsupported`. Each voucher's rules are selected from `pack` by its time of supply.
///
/// # Errors
/// * [`GstError::Invalid`] when `gstin` is not a registration of the company, or a number is too
///   large to count.
/// * [`GstError::NotFound`] when a voucher refers to a voucher type, ledger, item or unit that is
///   not in the masters.
/// * [`GstError::Pack`] when a pack entry that is read is malformed.
/// * [`GstError::Money`] on arithmetic overflow.
pub fn build_gstr1(
    books: &Books,
    gstin: &Gstin,
    period: Period,
    pack: &RulePack,
) -> Result<Outcome<(Gstr1, Vec<BucketedVoucher>)>, GstError> {
    if let Outcome::Unsupported(u) = check_period(period) {
        return Ok(Outcome::Unsupported(u));
    }
    if !books.company.features.gst {
        return Ok(Outcome::unsupported(
            "GSTR-1 for a company without GST",
            "GST is not enabled for this company; enable it in the company features",
        ));
    }
    let registration = books
        .company
        .registrations
        .iter()
        .find(|r| &r.gstin == gstin)
        .ok_or_else(|| GstError::Invalid {
            field: "gstin".to_owned(),
            reason: format!("{gstin} is not a registration of this company"),
        })?;
    if registration.composition {
        return Ok(Outcome::unsupported(
            "GSTR-1 for a composition registration",
            "a composition taxpayer does not file GSTR-1",
        ));
    }
    let mut evals = Vec::new();
    for v in books.vouchers.values().filter(|v| period.contains(v.date)) {
        evals.push(evaluate(books, v, pack)?);
    }
    flag_duplicates(&mut evals);
    let gstr1 = assemble(gstin, period, &evals)?;
    let buckets = evals
        .iter()
        .map(|e| BucketedVoucher {
            voucher: e.id,
            bucket: e.bucket,
            reason: e.reason.clone(),
        })
        .collect();
    Ok(Outcome::Value((gstr1, buckets)))
}

// ---------------------------------------------------------------------------------------------
// Internal types

/// Why a voucher is not in the return, and which bucket that puts it in.
struct Stop {
    bucket: Bucket,
    reason: String,
}

/// An outer error, or a voucher that stops, or a value.
type Step<T> = Result<Result<T, Stop>, GstError>;

/// Unwraps a `Result<T, Stop>` or leaves a [`Step`] function with the stop.
macro_rules! go {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(stop) => return Ok(Err(stop)),
        }
    };
}

/// Unwraps an `Outcome` or leaves a [`Step`] function with an `unsupported:` stop.
macro_rules! sup {
    ($e:expr) => {
        match $e {
            Outcome::Value(v) => v,
            Outcome::Unsupported(u) => return Ok(Err(stop_unsupported(&u))),
        }
    };
}

fn stop(bucket: Bucket, reason: impl Into<String>) -> Stop {
    Stop {
        bucket,
        reason: reason.into(),
    }
}

fn needs(reason: impl Into<String>) -> Stop {
    stop(Bucket::NeedsCorrection, reason)
}

fn stop_unsupported(u: &Unsupported) -> Stop {
    needs(format!("unsupported: {}: {}", u.what, u.reason))
}

/// The recipient of an outward supply.
enum Recipient {
    Registered(Gstin),
    Unregistered,
}

/// What a line is reported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineClass {
    Taxable,
    Deemed,
    Nil(NilClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NilClass {
    NilRated,
    Exempt,
    NonGst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NilKey {
    InterB2b,
    InterB2c,
    IntraB2b,
    IntraB2c,
}

impl NilKey {
    fn of(inter: bool, registered: bool) -> NilKey {
        match (inter, registered) {
            (true, true) => NilKey::InterB2b,
            (true, false) => NilKey::InterB2c,
            (false, true) => NilKey::IntraB2b,
            (false, false) => NilKey::IntraB2c,
        }
    }

    fn kind(self) -> NilSupplyKind {
        match self {
            NilKey::InterB2b => NilSupplyKind::InterB2b,
            NilKey::InterB2c => NilSupplyKind::InterB2c,
            NilKey::IntraB2b => NilSupplyKind::IntraB2b,
            NilKey::IntraB2c => NilSupplyKind::IntraB2c,
        }
    }
}

/// The kinds of document in Table 13 that this module summarises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DocKind {
    Invoice,
    CreditNote,
}

struct Doc {
    kind: DocKind,
    number: String,
    cancelled: bool,
}

/// Amounts of one rate, in paise.
#[derive(Debug, Clone, Copy, Default)]
struct Sums {
    taxable: Paise,
    cgst: Paise,
    sgst: Paise,
    igst: Paise,
    cess: Paise,
}

impl Sums {
    fn plus(self, o: Sums) -> Result<Sums, GstError> {
        Ok(Sums {
            taxable: self.taxable.checked_add(o.taxable)?,
            cgst: self.cgst.checked_add(o.cgst)?,
            sgst: self.sgst.checked_add(o.sgst)?,
            igst: self.igst.checked_add(o.igst)?,
            cess: self.cess.checked_add(o.cess)?,
        })
    }
}

/// One rate of one voucher, rounded.
#[derive(Debug, Clone, Copy)]
struct Group {
    rate: Decimal,
    amt: Sums,
}

impl Group {
    fn item(&self, inter: bool) -> LineItem {
        let tax = if inter {
            ItemTax::Igst(self.amt.igst)
        } else {
            ItemTax::CgstSgst {
                cgst: self.amt.cgst,
                sgst: self.amt.sgst,
            }
        };
        LineItem {
            rate: Rate(self.rate),
            taxable: self.amt.taxable,
            tax,
            cess: self.amt.cess,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HsnKey {
    hsn: Hsn,
    uqc: String,
    rate: Decimal,
}

/// Unrounded rupee amounts of an HSN row.
#[derive(Debug, Clone, Copy, Default)]
struct HsnAmounts {
    qty: Decimal,
    taxable: Decimal,
    cgst: Decimal,
    sgst: Decimal,
    igst: Decimal,
    cess: Decimal,
}

impl HsnAmounts {
    fn plus(self, o: HsnAmounts) -> Result<HsnAmounts, GstError> {
        Ok(HsnAmounts {
            qty: dec_add(self.qty, o.qty)?,
            taxable: dec_add(self.taxable, o.taxable)?,
            cgst: dec_add(self.cgst, o.cgst)?,
            sgst: dec_add(self.sgst, o.sgst)?,
            igst: dec_add(self.igst, o.igst)?,
            cess: dec_add(self.cess, o.cess)?,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct NilAmounts {
    nil_rated: Paise,
    exempt: Paise,
    non_gst: Paise,
}

/// Where a voucher's taxable lines go.
enum Placement {
    /// No taxable line: the voucher is only in Tables 8, 12 and 13.
    NilOnly,
    B2b(Gstin, B2bInvoice),
    B2cl(StateCode, B2clInvoice),
    Cdnr(Gstin, CdnrNote),
    B2cs {
        pos: StateCode,
        intra: bool,
        groups: Vec<Group>,
    },
}

/// Everything one voucher adds to the return.
struct Part {
    placement: Placement,
    nil: Vec<(NilKey, NilClass, Paise)>,
    hsn: Vec<(HsnKey, HsnAmounts)>,
}

struct Eval {
    id: VoucherId,
    bucket: Bucket,
    reason: Option<String>,
    doc: Option<Doc>,
    part: Option<Part>,
}

/// What `build_part` works on.
struct Ctx<'a> {
    books: &'a Books,
    v: &'a Voucher,
    rules: &'a RuleSet,
    breakup: &'a GstBreakup,
}

/// The facts of one invoice or note, before its table is chosen.
struct Inv {
    number: String,
    value: Paise,
    pos: StateCode,
    inter: bool,
    inv_type: B2bInvoiceType,
    items: Vec<LineItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HsnNotes {
    Net,
    Exclude,
}

fn dec_add(a: Decimal, b: Decimal) -> Result<Decimal, GstError> {
    a.checked_add(b)
        .ok_or_else(|| GstError::from(MoneyError::Overflow))
}

fn round_paise(rupees: Decimal) -> Result<Paise, GstError> {
    round_rupees_to_paise(rupees, RoundingRule::HalfUpToPaise).map_err(GstError::from)
}

fn not_found(what: String) -> GstError {
    GstError::NotFound { what }
}

fn invalid(field: &str, reason: String) -> GstError {
    GstError::Invalid {
        field: field.to_owned(),
        reason,
    }
}

// ---------------------------------------------------------------------------------------------
// Period and time of supply

/// `Unsupported` unless the period is one calendar month or one calendar quarter.
fn check_period(period: Period) -> Outcome<()> {
    let (from, to) = (period.from(), period.to());
    let months = (to.year() - from.year()) * 12 + i32::from(u8::from(to.month()))
        - i32::from(u8::from(from.month()))
        + 1;
    let starts_on_first = from.day() == 1;
    let ends_on_last = to.next_day().is_none_or(|n| n.month() != to.month());
    let quarter_start = u8::from(from.month()) % 3 == 1;
    if starts_on_first && ends_on_last && (months == 1 || (months == 3 && quarter_start)) {
        Outcome::Value(())
    } else {
        Outcome::unsupported(
            format!("GSTR-1 for the period {from} to {to}"),
            "a return covers one calendar month, or one calendar quarter under QRMP",
        )
    }
}

/// The time of supply taken from the voucher: its `supply_date`, else its date. This is the same
/// rule as `compute_voucher_tax` and `classify_line` apply, which demand a rule set of exactly
/// that date.
fn time_of_supply(v: &Voucher) -> Outcome<Date> {
    match (v.supply_date, v.payment_date) {
        (Some(d), Some(_) | None) => Outcome::Value(d),
        (None, Some(p)) if p < v.date => Outcome::unsupported(
            "time of supply with a payment dated before the invoice",
            "set the supply date explicitly; the time-of-supply rules are not applied here",
        ),
        (None, Some(_) | None) => Outcome::Value(v.date),
    }
}

// ---------------------------------------------------------------------------------------------
// One voucher

fn evaluate(books: &Books, v: &Voucher, pack: &RulePack) -> Result<Eval, GstError> {
    let base = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| not_found(format!("voucher type {}", v.type_id.0)))?
        .base;
    let doc = doc_entry(v, base);
    let judged = match base {
        BaseVoucherKind::Sales => judge_outward(books, v, false, pack)?,
        BaseVoucherKind::CreditNote => judge_outward(books, v, true, pack)?,
        BaseVoucherKind::Receipt => judge_receipt(v),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => Err(stop(
            Bucket::NotRelevant,
            format!("a {base:?} voucher is not reported in GSTR-1"),
        )),
    };
    Ok(match judged {
        Ok(part) => Eval {
            id: v.id,
            bucket: Bucket::Ready,
            reason: None,
            doc,
            part: Some(part),
        },
        Err(s) => Eval {
            id: v.id,
            bucket: s.bucket,
            reason: Some(s.reason),
            doc,
            part: None,
        },
    })
}

fn doc_kind(base: BaseVoucherKind) -> Option<DocKind> {
    match base {
        BaseVoucherKind::Sales => Some(DocKind::Invoice),
        BaseVoucherKind::CreditNote => Some(DocKind::CreditNote),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => None,
    }
}

/// The Table 13 entry of a voucher: invoices and credit notes that were issued (cancelled ones
/// count as cancelled; optional vouchers were never issued).
fn doc_entry(v: &Voucher, base: BaseVoucherKind) -> Option<Doc> {
    let kind = doc_kind(base)?;
    let cancelled = match v.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => false,
        VoucherStatus::Cancelled => true,
        VoucherStatus::Optional => return None,
    };
    let number = v
        .number
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())?;
    Some(Doc {
        kind,
        number: number.to_owned(),
        cancelled,
    })
}

fn status_gate(v: &Voucher) -> Result<(), Stop> {
    match v.status {
        VoucherStatus::Regular => Ok(()),
        VoucherStatus::Optional => Err(stop(
            Bucket::NotRelevant,
            "an optional voucher is a memorandum, not a supply",
        )),
        VoucherStatus::Cancelled => Err(stop(
            Bucket::NotRelevant,
            "a cancelled voucher is not a supply; it counts as cancelled in Table 13",
        )),
        VoucherStatus::PostDated => Err(needs(
            "unsupported: a post-dated voucher: whether it is reported before its date is not decided",
        )),
    }
}

/// A receipt is relevant only when it carries an advance, and advances are not built.
fn judge_receipt(v: &Voucher) -> Result<Part, Stop> {
    status_gate(v)?;
    let has_advance = v
        .ledger_lines
        .iter()
        .any(|l| l.bills.iter().any(|b| b.kind == BillRefKind::Advance));
    if has_advance {
        Err(needs(
            "unsupported: an advance received: Tables 11A and 11B are not built, so the tax on this advance is missing from the return",
        ))
    } else {
        Err(stop(
            Bucket::NotRelevant,
            "a receipt with no advance is not reported in GSTR-1",
        ))
    }
}

fn document_number(v: &Voucher) -> Result<String, Stop> {
    let Some(n) = v.number.as_deref().map(str::trim).filter(|n| !n.is_empty()) else {
        return Err(needs(
            "the voucher has no number; every invoice and note needs one",
        ));
    };
    let len = n.chars().count();
    if len > 16 {
        return Err(needs(format!(
            "number `{n}` has {len} characters; the limit is 16 (CGST Rules 2017 rule 46(b))"
        )));
    }
    Ok(n.to_owned())
}

fn judge_outward(books: &Books, v: &Voucher, is_note: bool, pack: &RulePack) -> Step<Part> {
    if let Err(s) = status_gate(v) {
        return Ok(Err(s));
    }
    let number = go!(document_number(v));
    let tos = sup!(time_of_supply(v));
    let rules = sup!(pack.for_time_of_supply(tos));
    let breakup = sup!(compute_voucher_tax(
        v,
        &books.masters,
        &rules,
        &books.company
    )?);
    let recipient = go!(recipient_of(books, v)?);
    let c = Ctx {
        books,
        v,
        rules: &rules,
        breakup: &breakup,
    };
    build_part(&c, number, is_note, &recipient)
}

/// Builds the whole contribution of a voucher whose tax was computed; nothing is kept unless
/// every line succeeds.
fn build_part(c: &Ctx<'_>, number: String, is_note: bool, recipient: &Recipient) -> Step<Part> {
    let classes = go!(classify_lines(c)?);
    let inv_type = go!(shape(is_note, &classes));
    let groups = go!(rate_groups(c, &classes)?);
    if let Some(reason) = stored_gst_mismatch(c)? {
        return Ok(Err(stop(Bucket::Mismatch, reason)));
    }
    let hsn = go!(hsn_parts(c, is_note)?);
    let nil = nil_parts(c, &classes, recipient);
    let Some(pos) = c.v.place_of_supply else {
        return Ok(Err(needs("the voucher has no place of supply")));
    };
    let inter = is_inter(c.breakup.heads);
    let inv = Inv {
        number,
        value: c
            .breakup
            .total_before_round_off
            .checked_add(c.breakup.round_off)?,
        pos,
        inter,
        inv_type,
        items: groups.iter().map(|g| g.item(inter)).collect(),
    };
    let placement = go!(placement(c, is_note, recipient, inv, &groups)?);
    Ok(Ok(Part {
        placement,
        nil,
        hsn,
    }))
}

fn is_inter(heads: TaxHeads) -> bool {
    match heads {
        TaxHeads::Igst => true,
        TaxHeads::CgstSgst | TaxHeads::CgstUtgst => false,
    }
}

fn recipient_of(books: &Books, v: &Voucher) -> Step<Recipient> {
    let Some(party_id) = v.party else {
        return Ok(Ok(Recipient::Unregistered));
    };
    let ledger = books
        .masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    let Some(details) = &ledger.party else {
        return Ok(Err(needs(format!(
            "party ledger `{}` has no registration details; add them",
            ledger.name
        ))));
    };
    let name = &ledger.name;
    Ok(match (details.registration, &details.gstin) {
        (RegistrationType::Regular | RegistrationType::Composition, Some(g)) => {
            Ok(Recipient::Registered(g.clone()))
        }
        (RegistrationType::Regular | RegistrationType::Composition, None) => Err(needs(format!(
            "party `{name}` is a registered taxpayer but has no GSTIN; add it to the party ledger"
        ))),
        (RegistrationType::Unregistered | RegistrationType::Consumer, None) => {
            Ok(Recipient::Unregistered)
        }
        (RegistrationType::Unregistered | RegistrationType::Consumer, Some(_)) => Err(needs(
            format!("party `{name}` is marked unregistered but carries a GSTIN; fix one of them"),
        )),
        (
            RegistrationType::Sez | RegistrationType::Overseas | RegistrationType::Uin,
            Some(_) | None,
        ) => Err(needs(format!(
            "unsupported: a supply to party `{name}`: SEZ, overseas and UIN recipients are not built"
        ))),
    })
}

fn line_class(kind: SupplyKind) -> Result<LineClass, Stop> {
    match kind {
        SupplyKind::Taxable => Ok(LineClass::Taxable),
        SupplyKind::DeemedExport => Ok(LineClass::Deemed),
        SupplyKind::Exempt => Ok(LineClass::Nil(NilClass::Exempt)),
        SupplyKind::NilRated => Ok(LineClass::Nil(NilClass::NilRated)),
        SupplyKind::NonGst => Ok(LineClass::Nil(NilClass::NonGst)),
        SupplyKind::ExportWithPayment
        | SupplyKind::ExportUnderLut
        | SupplyKind::SezWithPayment
        | SupplyKind::SezUnderLut
        | SupplyKind::Advance => Err(needs(format!(
            "unsupported: a {kind:?} line: this kind of supply is not built into GSTR-1"
        ))),
    }
}

/// Classifies every taxed line of the breakup, in the breakup's order.
fn classify_lines(c: &Ctx<'_>) -> Step<Vec<LineClass>> {
    // Ledger lines of an accounting invoice that carry a GST classification, in voucher order;
    // the breakup lists its ledger lines in the same order.
    let positions: Vec<usize> =
        c.v.ledger_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                c.books
                    .masters
                    .ledgers
                    .get(&l.ledger)
                    .is_some_and(|m| m.gst.is_some())
            })
            .map(|(i, _)| i)
            .collect();
    let mut out = Vec::with_capacity(c.breakup.lines.len());
    for (ordinal, line) in c.breakup.lines.iter().enumerate() {
        let index = match line.source {
            LineSource::Inventory { index } => index,
            LineSource::Ledger { .. } => *positions.get(ordinal).ok_or_else(|| {
                invalid(
                    "line",
                    format!("taxed line {ordinal} has no matching ledger line"),
                )
            })?,
        };
        let kind = sup!(classify_line(c.v, index, &c.books.masters, c.rules)?);
        out.push(go!(line_class(kind)));
    }
    Ok(Ok(out))
}

fn is_nil(class: LineClass) -> bool {
    match class {
        LineClass::Nil(_) => true,
        LineClass::Taxable | LineClass::Deemed => false,
    }
}

/// Checks that the lines can be reported together and returns the invoice type.
fn shape(is_note: bool, classes: &[LineClass]) -> Result<B2bInvoiceType, Stop> {
    if is_note && classes.iter().any(|c| is_nil(*c)) {
        return Err(needs(
            "unsupported: a credit note with exempt, nil-rated or non-GST lines: netting it in Table 8 is not decided",
        ));
    }
    let taxable = classes.contains(&LineClass::Taxable);
    let deemed = classes.contains(&LineClass::Deemed);
    match (taxable, deemed) {
        (true, true) => Err(needs(
            "unsupported: a voucher with both ordinary and deemed-export lines",
        )),
        (false, true) => Ok(B2bInvoiceType::DeemedExport),
        (true, false) | (false, false) => Ok(B2bInvoiceType::Regular),
    }
}

// ---------------------------------------------------------------------------------------------
// Rate rows, stored-tax check

#[derive(Default)]
struct Raw {
    taxable: Paise,
    cgst: Decimal,
    sgst: Decimal,
    igst: Decimal,
    cess: Decimal,
}

/// The taxable lines grouped by rate, each head rounded once per rate. `Mismatch` when the rounded
/// rows do not add up to the voucher's head totals.
fn rate_groups(c: &Ctx<'_>, classes: &[LineClass]) -> Step<Vec<Group>> {
    let mut raw: BTreeMap<Decimal, Raw> = BTreeMap::new();
    for (line, class) in c.breakup.lines.iter().zip(classes) {
        if is_nil(*class) {
            continue;
        }
        let r = raw.entry(line.rate_percent).or_default();
        r.taxable = r.taxable.checked_add(line.taxable)?;
        r.cgst = dec_add(r.cgst, line.cgst)?;
        r.sgst = dec_add(r.sgst, dec_add(line.sgst, line.utgst)?)?;
        r.igst = dec_add(r.igst, line.igst)?;
        r.cess = dec_add(r.cess, line.cess)?;
    }
    let mut groups = Vec::with_capacity(raw.len());
    for (rate, r) in raw {
        groups.push(Group {
            rate,
            amt: Sums {
                taxable: r.taxable,
                cgst: round_paise(r.cgst)?,
                sgst: round_paise(r.sgst)?,
                igst: round_paise(r.igst)?,
                cess: round_paise(r.cess)?,
            },
        });
    }
    let t = c.breakup.totals;
    let checks = [
        ("CGST", sum_by(&groups, |g| g.amt.cgst)?, t.cgst),
        (
            "state tax",
            sum_by(&groups, |g| g.amt.sgst)?,
            t.sgst.checked_add(t.utgst)?,
        ),
        ("IGST", sum_by(&groups, |g| g.amt.igst)?, t.igst),
        ("cess", sum_by(&groups, |g| g.amt.cess)?, t.cess),
    ];
    for (name, rows, voucher) in checks {
        if rows != voucher {
            return Ok(Err(stop(
                Bucket::Mismatch,
                format!(
                    "the rate-wise {name} adds up to {} paise but the voucher's {name} is {} paise",
                    rows.get(),
                    voucher.get()
                ),
            )));
        }
    }
    Ok(Ok(groups))
}

fn sum_by(groups: &[Group], f: impl Fn(&Group) -> Paise) -> Result<Paise, GstError> {
    let mut sum = Paise::ZERO;
    for g in groups {
        sum = sum.checked_add(f(g))?;
    }
    Ok(sum)
}

/// A description of the first difference between the tax stored on the voucher and the tax
/// recomputed from the masters; `None` when the voucher stores none or they agree.
fn stored_gst_mismatch(c: &Ctx<'_>) -> Result<Option<String>, GstError> {
    let Some(stored) = &c.v.gst else {
        return Ok(None);
    };
    let mut sums: BTreeMap<TaxLedgerKind, Paise> = BTreeMap::new();
    for l in &stored.lines {
        let slot = sums.entry(l.kind).or_insert(Paise::ZERO);
        *slot = slot.checked_add(l.tax)?;
    }
    let t = c.breakup.totals;
    let checks = [
        ("CGST", TaxLedgerKind::Cgst, t.cgst),
        ("SGST", TaxLedgerKind::Sgst, t.sgst),
        ("UTGST", TaxLedgerKind::Utgst, t.utgst),
        ("IGST", TaxLedgerKind::Igst, t.igst),
        ("cess", TaxLedgerKind::Cess, t.cess),
    ];
    for (name, kind, computed) in checks {
        let stored_sum = sums.get(&kind).copied().unwrap_or(Paise::ZERO);
        if stored_sum != computed {
            return Ok(Some(format!(
                "the voucher stores {name} of {} paise but the masters give {} paise",
                stored_sum.get(),
                computed.get()
            )));
        }
    }
    if stored.round_off != c.breakup.round_off {
        return Ok(Some(format!(
            "the voucher stores a round-off of {} paise but the masters give {} paise",
            stored.round_off.get(),
            c.breakup.round_off.get()
        )));
    }
    Ok(None)
}

// ---------------------------------------------------------------------------------------------
// Tables 4, 5, 7, 9B placement

fn placement(
    c: &Ctx<'_>,
    is_note: bool,
    recipient: &Recipient,
    inv: Inv,
    groups: &[Group],
) -> Step<Placement> {
    if inv.items.is_empty() {
        return Ok(Ok(Placement::NilOnly));
    }
    let date = c.v.date;
    match (recipient, is_note) {
        (Recipient::Registered(ctin), false) => Ok(Ok(Placement::B2b(
            ctin.clone(),
            B2bInvoice {
                num: inv.number,
                date,
                value: inv.value,
                pos: inv.pos,
                reverse_charge: false,
                inv_type: inv.inv_type,
                items: inv.items,
            },
        ))),
        (Recipient::Registered(ctin), true) => Ok(Ok(Placement::Cdnr(
            ctin.clone(),
            CdnrNote {
                kind: NoteKind::Credit,
                num: inv.number,
                date,
                value: inv.value,
                pos: inv.pos,
                reverse_charge: false,
                inv_type: inv.inv_type,
                items: inv.items,
            },
        ))),
        (Recipient::Unregistered, true) => Ok(Err(needs(
            "unsupported: a credit note to an unregistered recipient: whether it belongs in CDNUR or adjusts Table 7 depends on the original invoice",
        ))),
        (Recipient::Unregistered, false) => unregistered_invoice(c, inv, groups),
    }
}

/// B2CL when inter-state and strictly above the pack's threshold, otherwise B2CS.
fn unregistered_invoice(c: &Ctx<'_>, inv: Inv, groups: &[Group]) -> Step<Placement> {
    if inv.inter {
        let limit = go!(rupee_param(
            c.rules,
            B2CL_THRESHOLD_ID,
            "invoice_value_rupees"
        )?);
        if inv.value > limit {
            return Ok(Ok(Placement::B2cl(
                inv.pos,
                B2clInvoice {
                    num: inv.number,
                    date: c.v.date,
                    value: inv.value,
                    items: inv.items,
                },
            )));
        }
    }
    Ok(Ok(Placement::B2cs {
        pos: inv.pos,
        intra: !inv.inter,
        groups: groups.to_vec(),
    }))
}

/// A rupee amount from a verified pack entry.
fn rupee_param(rules: &RuleSet, id: &str, key: &str) -> Step<Paise> {
    let entry = sup!(rules.get(id));
    let text = entry.params().get(key).ok_or_else(|| GstError::Pack {
        reason: format!("entry `{id}` has no `{key}` parameter"),
    })?;
    let rupees = text.trim().parse::<Decimal>().map_err(|e| GstError::Pack {
        reason: format!("entry `{id}`: {key} `{text}` is not a decimal number ({e})"),
    })?;
    Ok(Ok(round_paise(rupees)?))
}

// ---------------------------------------------------------------------------------------------
// Tables 8 and 12

fn nil_parts(
    c: &Ctx<'_>,
    classes: &[LineClass],
    recipient: &Recipient,
) -> Vec<(NilKey, NilClass, Paise)> {
    let registered = match recipient {
        Recipient::Registered(_) => true,
        Recipient::Unregistered => false,
    };
    let key = NilKey::of(is_inter(c.breakup.heads), registered);
    let mut out = Vec::new();
    for (line, class) in c.breakup.lines.iter().zip(classes) {
        if let LineClass::Nil(n) = class {
            out.push((key, *n, line.taxable));
        }
    }
    out
}

fn credit_note_hsn(rules: &RuleSet) -> Step<HsnNotes> {
    let entry = sup!(rules.get(HSN_CREDIT_NOTES_ID));
    match entry.params().get("treatment").map(String::as_str) {
        Some("net") => Ok(Ok(HsnNotes::Net)),
        Some("exclude") => Ok(Ok(HsnNotes::Exclude)),
        Some(other) => Err(GstError::Pack {
            reason: format!(
                "entry `{HSN_CREDIT_NOTES_ID}`: treatment `{other}` is not net or exclude"
            ),
        }),
        None => Err(GstError::Pack {
            reason: format!("entry `{HSN_CREDIT_NOTES_ID}` has no `treatment` parameter"),
        }),
    }
}

fn hsn_parts(c: &Ctx<'_>, is_note: bool) -> Step<Vec<(HsnKey, HsnAmounts)>> {
    let negate = if is_note {
        match go!(credit_note_hsn(c.rules)?) {
            HsnNotes::Net => true,
            HsnNotes::Exclude => return Ok(Ok(Vec::new())),
        }
    } else {
        false
    };
    let signed = |d: Decimal| if negate { -d } else { d };
    let mut out = Vec::with_capacity(c.breakup.lines.len());
    for line in &c.breakup.lines {
        let (hsn, uqc, qty) = go!(hsn_facts(c, line)?);
        out.push((
            HsnKey {
                hsn,
                uqc,
                rate: line.rate_percent,
            },
            HsnAmounts {
                qty: signed(qty),
                taxable: signed(line.taxable.to_rupees()),
                cgst: signed(line.cgst),
                sgst: signed(dec_add(line.sgst, line.utgst)?),
                igst: signed(line.igst),
                cess: signed(line.cess),
            },
        ));
    }
    Ok(Ok(out))
}

/// The HSN or SAC, unit quantity code and quantity of a line.
fn hsn_facts(c: &Ctx<'_>, line: &LineTax) -> Step<(Hsn, String, Decimal)> {
    match line.source {
        LineSource::Inventory { index } => {
            let inv =
                c.v.inventory_lines.get(index).ok_or_else(|| {
                    invalid("line", format!("inventory line {index} does not exist"))
                })?;
            let item = c
                .books
                .masters
                .stock_items
                .get(&inv.item)
                .ok_or_else(|| not_found(format!("stock item {}", inv.item.0)))?;
            let Some(hsn) = item.hsn.clone() else {
                return Ok(Err(needs(format!(
                    "item `{}` has no HSN or SAC code; Table 12 needs one",
                    item.name
                ))));
            };
            if hsn.is_sac() {
                return Ok(Ok((hsn, "NA".to_owned(), Decimal::ZERO)));
            }
            let unit = c
                .books
                .masters
                .units
                .get(&inv.unit)
                .ok_or_else(|| not_found(format!("unit {}", inv.unit.0)))?;
            let Some(uqc) = unit.uqc.clone().filter(|u| !u.trim().is_empty()) else {
                return Ok(Err(needs(format!(
                    "unit `{}` has no unit quantity code; Table 12 needs one",
                    unit.symbol
                ))));
            };
            Ok(Ok((hsn, uqc, inv.qty.0)))
        }
        LineSource::Ledger { ledger } => {
            let hsn = c
                .books
                .masters
                .ledgers
                .get(&ledger)
                .and_then(|l| l.gst.as_ref())
                .and_then(|g| g.hsn.clone());
            match hsn {
                Some(h) if h.is_sac() => Ok(Ok((h, "NA".to_owned(), Decimal::ZERO))),
                Some(h) => Ok(Err(needs(format!(
                    "unsupported: goods HSN {} on a ledger line: an accounting invoice has no quantity or unit",
                    h.as_str()
                )))),
                None => Ok(Err(needs(format!(
                    "ledger {} has no HSN or SAC code; Table 12 needs one",
                    ledger.0
                )))),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The whole return

/// A voucher number used by two or more vouchers cannot be placed: the table rows and Table 13
/// would count one document twice.
fn flag_duplicates(evals: &mut [Eval]) {
    let mut uses: BTreeMap<(DocKind, String), usize> = BTreeMap::new();
    for d in evals.iter().filter_map(|e| e.doc.as_ref()) {
        *uses.entry((d.kind, d.number.clone())).or_insert(0) += 1;
    }
    for e in &mut *evals {
        let Some(d) = &e.doc else { continue };
        let count = uses.get(&(d.kind, d.number.clone())).copied().unwrap_or(0);
        if count > 1 && e.bucket == Bucket::Ready {
            e.reason = Some(format!(
                "document number `{}` is used by {count} vouchers; every document needs its own",
                d.number
            ));
            e.bucket = Bucket::NeedsCorrection;
            e.part = None;
        }
    }
}

#[derive(Default)]
struct Agg {
    b2b: BTreeMap<Gstin, Vec<B2bInvoice>>,
    b2cl: BTreeMap<StateCode, Vec<B2clInvoice>>,
    cdnr: BTreeMap<Gstin, Vec<CdnrNote>>,
    b2cs: BTreeMap<(StateCode, bool, Decimal), Sums>,
    nil: BTreeMap<NilKey, NilAmounts>,
    hsn: BTreeMap<HsnKey, HsnAmounts>,
}

impl Agg {
    fn add(&mut self, part: &Part) -> Result<(), GstError> {
        match &part.placement {
            Placement::NilOnly => {}
            Placement::B2b(ctin, inv) => {
                self.b2b.entry(ctin.clone()).or_default().push(inv.clone());
            }
            Placement::B2cl(pos, inv) => self.b2cl.entry(*pos).or_default().push(inv.clone()),
            Placement::Cdnr(ctin, note) => {
                self.cdnr
                    .entry(ctin.clone())
                    .or_default()
                    .push(note.clone());
            }
            Placement::B2cs { pos, intra, groups } => {
                for g in groups {
                    let slot = self.b2cs.entry((*pos, *intra, g.rate)).or_default();
                    *slot = slot.plus(g.amt)?;
                }
            }
        }
        for (key, class, taxable) in &part.nil {
            let slot = self.nil.entry(*key).or_default();
            let field = match class {
                NilClass::NilRated => &mut slot.nil_rated,
                NilClass::Exempt => &mut slot.exempt,
                NilClass::NonGst => &mut slot.non_gst,
            };
            *field = field.checked_add(*taxable)?;
        }
        for (key, amounts) in &part.hsn {
            let slot = self.hsn.entry(key.clone()).or_default();
            *slot = slot.plus(*amounts)?;
        }
        Ok(())
    }
}

fn assemble(gstin: &Gstin, period: Period, evals: &[Eval]) -> Result<Gstr1, GstError> {
    let mut agg = Agg::default();
    for part in evals.iter().filter_map(|e| e.part.as_ref()) {
        agg.add(part)?;
    }
    let b2b = agg
        .b2b
        .into_iter()
        .map(|(ctin, mut invoices)| {
            invoices.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.num.cmp(&b.num)));
            B2bParty { ctin, invoices }
        })
        .collect();
    let b2cl = agg
        .b2cl
        .into_iter()
        .map(|(pos, mut invoices)| {
            invoices.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.num.cmp(&b.num)));
            B2clGroup { pos, invoices }
        })
        .collect();
    let cdnr = agg
        .cdnr
        .into_iter()
        .map(|(ctin, mut notes)| {
            notes.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.num.cmp(&b.num)));
            CdnrParty { ctin, notes }
        })
        .collect();
    let b2cs = agg
        .b2cs
        .into_iter()
        .map(|((pos, intra, rate), amt)| B2csRow {
            intra_state: intra,
            kind: B2csKind::Other,
            pos,
            line: Group { rate, amt }.item(!intra),
        })
        .collect();
    let nil = agg
        .nil
        .into_iter()
        .map(|(key, a)| NilRow {
            kind: key.kind(),
            nil_rated: a.nil_rated,
            exempt: a.exempt,
            non_gst: a.non_gst,
        })
        .collect();
    let mut hsn = Vec::with_capacity(agg.hsn.len());
    for (key, a) in agg.hsn {
        hsn.push(hsn_row(key, a)?);
    }
    Ok(Gstr1 {
        gstin: gstin.clone(),
        period,
        aggregate_turnover: None,
        current_turnover: None,
        b2b,
        b2cl,
        b2cs,
        cdnr,
        cdnur: Vec::new(),
        exp: Vec::new(),
        nil,
        hsn,
        doc_issue: doc_issue(evals)?,
    })
}

/// One Table 12 row: every amount rounded once, half away from zero, to the paisa.
fn hsn_row(key: HsnKey, a: HsnAmounts) -> Result<HsnRow, GstError> {
    let taxable = round_paise(a.taxable)?;
    let mut row = HsnRow {
        hsn: key.hsn,
        description: String::new(),
        uqc: key.uqc,
        qty: Qty(a.qty),
        value: taxable,
        taxable,
        rate: Rate(key.rate),
        igst: round_paise(a.igst)?,
        cgst: round_paise(a.cgst)?,
        sgst: round_paise(a.sgst)?,
        cess: round_paise(a.cess)?,
    };
    for tax in [row.igst, row.cgst, row.sgst, row.cess] {
        row.value = row.value.checked_add(tax)?;
    }
    Ok(row)
}

// ---------------------------------------------------------------------------------------------
// Table 13

fn doc_issue(evals: &[Eval]) -> Result<Vec<DocIssue>, GstError> {
    // number -> cancelled, per kind; a repeated number counts once (it is flagged elsewhere)
    let mut by_kind: BTreeMap<DocKind, BTreeMap<String, bool>> = BTreeMap::new();
    for d in evals.iter().filter_map(|e| e.doc.as_ref()) {
        by_kind
            .entry(d.kind)
            .or_default()
            .insert(d.number.clone(), d.cancelled);
    }
    let mut out = Vec::new();
    for (kind, numbers) in by_kind {
        let ranges = ranges_of(&numbers)?;
        if !ranges.is_empty() {
            out.push(DocIssue {
                doc_type: match kind {
                    DocKind::Invoice => DocType::OutwardInvoice,
                    DocKind::CreditNote => DocType::CreditNote,
                },
                ranges,
            });
        }
    }
    Ok(out)
}

/// Splits a document number into its prefix and the number its trailing digits spell.
fn split_number(text: &str) -> Option<(&str, u64)> {
    let prefix_len = text.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let prefix = text.get(..prefix_len)?;
    let digits = text.get(prefix_len..)?;
    if digits.is_empty() || digits.len() > 18 {
        return None;
    }
    digits.parse::<u64>().ok().map(|n| (prefix, n))
}

fn bump(n: u32) -> Result<u32, GstError> {
    n.checked_add(1)
        .ok_or_else(|| invalid("doc_issue", "too many documents to count".to_owned()))
}

fn new_run(text: &str, cancelled: bool) -> DocRange {
    DocRange {
        from: text.to_owned(),
        to: text.to_owned(),
        total: 1,
        cancelled: u32::from(cancelled),
    }
}

/// Appends the runs of consecutive numbers among `docs` (numeric tail, original text, cancelled).
fn extend_runs(mut docs: Vec<(u64, &str, bool)>, out: &mut Vec<DocRange>) -> Result<(), GstError> {
    docs.sort_unstable();
    let mut last: Option<u64> = None;
    let mut run: Option<DocRange> = None;
    for (n, text, cancelled) in docs {
        let continues = last.and_then(|l| l.checked_add(1)) == Some(n);
        match run.take() {
            Some(mut r) if continues => {
                r.to = text.to_owned();
                r.total = bump(r.total)?;
                if cancelled {
                    r.cancelled = bump(r.cancelled)?;
                }
                run = Some(r);
            }
            Some(done) => {
                out.push(done);
                run = Some(new_run(text, cancelled));
            }
            None => run = Some(new_run(text, cancelled)),
        }
        last = Some(n);
    }
    out.extend(run);
    Ok(())
}

/// Runs of consecutive numbers (same prefix, numeric tail one apart); a number that does not end
/// in digits is a run of one.
fn ranges_of(numbers: &BTreeMap<String, bool>) -> Result<Vec<DocRange>, GstError> {
    let mut by_prefix: BTreeMap<&str, Vec<(u64, &str, bool)>> = BTreeMap::new();
    let mut out = Vec::new();
    for (text, cancelled) in numbers {
        match split_number(text) {
            Some((prefix, n)) => {
                by_prefix
                    .entry(prefix)
                    .or_default()
                    .push((n, text.as_str(), *cancelled))
            }
            None => out.push(new_run(text, *cancelled)),
        }
    }
    for docs in by_prefix.into_values() {
        extend_runs(docs, &mut out)?;
    }
    out.sort_by(|a, b| a.from.cmp(&b.from));
    Ok(out)
}
