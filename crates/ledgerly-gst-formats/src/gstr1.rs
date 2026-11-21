//! GSTR-1 and GSTR-1A data shapes and JSON writer. **Card F1** implements this module.
//!
//! Frozen interface (plan card F1):
//!
//! ```text
//! pub struct Gstr1 { .. }  pub struct Gstr1a { .. }   // sections: b2b, b2cl, b2cs, cdnr, cdnur, exp, nil, hsn, doc_issue
//! pub fn gstr1_to_json(r: &Gstr1) -> Result<String, FormatError>;
//! pub fn gstr1a_to_json(r: &Gstr1a) -> Result<String, FormatError>;
//! ```
//!
//! Output rules:
//!
//! - Amounts are JSON numbers with exactly two decimals, written from [`Paise`] by integer
//!   division. No float is ever built. Rates and quantities are written from `Decimal` without
//!   trailing zeros.
//! - Keys appear in a fixed order; list entries appear in the order the builder supplied (the
//!   builder sorts, rule A7). Empty sections are left out.
//! - Dates are `dd-mm-yyyy`; place of supply is the two-digit state code; `fp` is `MMYYYY` of the
//!   last month of the period.
//!
//! Provenance: the GSTN does not publish a JSON schema for GSTR-1 (research 04-gst.md, row
//! "GSTR-1 / IFF upload"). The key names and code values here (`inum`, `idt`, `itms`, `sply_ty`,
//! `INTRB2B`, `SEWP`, and so on) are **recalled from the offline-tool layout, not yet checked
//! against an official sample**. The owner supplies a sample from the offline tool, and until then
//! the format tests carry that caveat. Invoice-number length (at most 16 characters) is CGST Rules
//! 2017 rule 46(b).

mod b2b;
mod b2c;
mod emit;
mod notes;
mod other;

use self::b2b::{b2b_json, b2ba_json, gstin_json};
use self::b2c::{b2cl_json, b2cs_json};
use self::emit::{amt, fp, kv, obj};
use self::notes::{cdnr_json, cdnur_json};
use self::other::{doc_issue_json, exp_json, hsn_json, nil_json};
use crate::FormatError;
use ledgerly_core::{Date, Gstin, Hsn, Paise, Period, Qty, Rate, StateCode};

/// How the tax on a line is split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTax {
    /// Inter-state supply: integrated tax only.
    Igst(Paise),
    /// Intra-state supply: central and state (or union-territory) tax.
    CgstSgst {
        /// Central tax.
        cgst: Paise,
        /// State or union-territory tax.
        sgst: Paise,
    },
}

/// One rate-wise line of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineItem {
    /// Total GST rate in percent (for example 18).
    pub rate: Rate,
    /// Taxable value.
    pub taxable: Paise,
    /// Tax split.
    pub tax: ItemTax,
    /// Compensation cess; zero when none.
    pub cess: Paise,
}

/// Invoice type for B2B and credit/debit-note rows (`inv_typ`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum B2bInvoiceType {
    /// Regular (`R`).
    Regular,
    /// SEZ supplies with payment of tax (`SEWP`).
    SezWithPayment,
    /// SEZ supplies without payment of tax (`SEWOP`).
    SezWithoutPayment,
    /// Deemed export (`DE`).
    DeemedExport,
}

/// A B2B invoice (table 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2bInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Place of supply.
    pub pos: StateCode,
    /// Tax payable on reverse charge.
    pub reverse_charge: bool,
    /// Invoice type.
    pub inv_type: B2bInvoiceType,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// B2B invoices for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2bParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Invoices; at least one.
    pub invoices: Vec<B2bInvoice>,
}

/// A B2C large invoice (table 5): inter-state, unregistered recipient, above the threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2clInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// B2C large invoices for one place of supply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2clGroup {
    /// Place of supply.
    pub pos: StateCode,
    /// Invoices; at least one.
    pub invoices: Vec<B2clInvoice>,
}

/// Whether a B2CS row was supplied through an e-commerce operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum B2csKind {
    /// Other than e-commerce (`OE`).
    Other,
    /// Through an e-commerce operator (`E`), identified by its GSTIN.
    Ecommerce {
        /// E-commerce operator GSTIN.
        etin: Gstin,
    },
}

/// A B2C small summary row (table 7): consolidated by place of supply and rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2csRow {
    /// `true` for an intra-state supply (`INTRA`), `false` for inter-state (`INTER`).
    pub intra_state: bool,
    /// Other or e-commerce.
    pub kind: B2csKind,
    /// Place of supply.
    pub pos: StateCode,
    /// Rate, taxable value, tax and cess.
    pub line: LineItem,
}

/// Credit note or debit note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Credit note (`C`).
    Credit,
    /// Debit note (`D`).
    Debit,
}

/// A credit or debit note to a registered recipient (table 9B, CDNR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnrNote {
    /// Credit or debit.
    pub kind: NoteKind,
    /// Note number, 1 to 16 characters.
    pub num: String,
    /// Note date.
    pub date: Date,
    /// Note value including tax.
    pub value: Paise,
    /// Place of supply.
    pub pos: StateCode,
    /// Tax payable on reverse charge.
    pub reverse_charge: bool,
    /// Invoice type of the original supply.
    pub inv_type: B2bInvoiceType,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// Credit and debit notes for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnrParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Notes; at least one.
    pub notes: Vec<CdnrNote>,
}

/// What an unregistered-recipient note relates to (`typ`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdnurType {
    /// Against a B2C large invoice (`B2CL`); the place of supply is reported.
    B2cl {
        /// Place of supply.
        pos: StateCode,
    },
    /// Against an export with payment of tax (`EXPWP`).
    ExportWithPayment,
    /// Against an export without payment of tax (`EXPWOP`).
    ExportWithoutPayment,
}

/// A credit or debit note to an unregistered recipient (table 9B, CDNUR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnurNote {
    /// What the note relates to.
    pub ur_type: CdnurType,
    /// Credit or debit.
    pub kind: NoteKind,
    /// Note number, 1 to 16 characters.
    pub num: String,
    /// Note date.
    pub date: Date,
    /// Note value including tax.
    pub value: Paise,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// One rate-wise export line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpItem {
    /// Total GST rate in percent.
    pub rate: Rate,
    /// Taxable value.
    pub taxable: Paise,
    /// Integrated tax; zero for an export without payment of tax.
    pub igst: Paise,
    /// Compensation cess; zero when none.
    pub cess: Paise,
}

/// An export invoice (table 6A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Port code, when a shipping bill exists.
    pub port_code: Option<String>,
    /// Shipping bill number and date, when it exists.
    pub shipping_bill: Option<(String, Date)>,
    /// Rate-wise lines; at least one.
    pub items: Vec<ExpItem>,
}

/// Export invoices of one kind (`exp_typ`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpGroup {
    /// `true` for export with payment of tax (`WPAY`), `false` without (`WOPAY`).
    pub with_payment: bool,
    /// Invoices; at least one.
    pub invoices: Vec<ExpInvoice>,
}

/// Which nil-rated, exempt and non-GST supplies a row covers (`sply_ty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NilSupplyKind {
    /// Inter-state supplies to registered persons (`INTRB2B`).
    InterB2b,
    /// Inter-state supplies to unregistered persons (`INTRB2C`).
    InterB2c,
    /// Intra-state supplies to registered persons (`INTRAB2B`).
    IntraB2b,
    /// Intra-state supplies to unregistered persons (`INTRAB2C`).
    IntraB2c,
}

/// A nil-rated, exempt and non-GST supply row (table 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NilRow {
    /// Supply kind.
    pub kind: NilSupplyKind,
    /// Nil-rated supplies.
    pub nil_rated: Paise,
    /// Exempted supplies.
    pub exempt: Paise,
    /// Non-GST supplies.
    pub non_gst: Paise,
}

/// An HSN summary row (table 12), one per code, unit and rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsnRow {
    /// HSN or SAC code.
    pub hsn: Hsn,
    /// Description; may be empty.
    pub description: String,
    /// Unit quantity code (`KGS`, `NOS`; `NA` for services).
    pub uqc: String,
    /// Total quantity.
    pub qty: Qty,
    /// Total value including tax.
    pub value: Paise,
    /// Total taxable value.
    pub taxable: Paise,
    /// Rate in percent.
    pub rate: Rate,
    /// Integrated tax.
    pub igst: Paise,
    /// Central tax.
    pub cgst: Paise,
    /// State or union-territory tax.
    pub sgst: Paise,
    /// Compensation cess.
    pub cess: Paise,
}

/// The kind of document a series covers (table 13, `doc_num`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    /// Invoices for outward supply (1).
    OutwardInvoice,
    /// Invoices for inward supply from an unregistered person (2).
    InwardFromUnregistered,
    /// Revised invoice (3).
    RevisedInvoice,
    /// Debit note (4).
    DebitNote,
    /// Credit note (5).
    CreditNote,
    /// Receipt voucher (6).
    ReceiptVoucher,
    /// Payment voucher (7).
    PaymentVoucher,
    /// Refund voucher (8).
    RefundVoucher,
    /// Delivery challan for job work (9).
    ChallanJobWork,
    /// Delivery challan for supply on approval (10).
    ChallanOnApproval,
    /// Delivery challan in case of liquid gas (11).
    ChallanLiquidGas,
    /// Delivery challan, other (12).
    ChallanOther,
}

/// One consecutive run of document numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocRange {
    /// First number in the run.
    pub from: String,
    /// Last number in the run.
    pub to: String,
    /// Total documents in the run, cancelled ones included.
    pub total: u32,
    /// Cancelled documents; at most `total`.
    pub cancelled: u32,
}

/// Document series of one type (table 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocIssue {
    /// Document type.
    pub doc_type: DocType,
    /// Runs; at least one.
    pub ranges: Vec<DocRange>,
}

/// A GSTR-1 return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1 {
    /// Supplier GSTIN.
    pub gstin: Gstin,
    /// Return period (one month, or a quarter under QRMP).
    pub period: Period,
    /// Aggregate turnover of the preceding financial year, when reported (`gt`).
    pub aggregate_turnover: Option<Paise>,
    /// Gross turnover of the current year up to the period, when reported (`cur_gt`).
    pub current_turnover: Option<Paise>,
    /// Table 4.
    pub b2b: Vec<B2bParty>,
    /// Table 5.
    pub b2cl: Vec<B2clGroup>,
    /// Table 7.
    pub b2cs: Vec<B2csRow>,
    /// Table 9B, registered recipients.
    pub cdnr: Vec<CdnrParty>,
    /// Table 9B, unregistered recipients.
    pub cdnur: Vec<CdnurNote>,
    /// Table 6A.
    pub exp: Vec<ExpGroup>,
    /// Table 8.
    pub nil: Vec<NilRow>,
    /// Table 12.
    pub hsn: Vec<HsnRow>,
    /// Table 13.
    pub doc_issue: Vec<DocIssue>,
}

/// An amended B2B invoice (table 9A, B2BA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2baInvoice {
    /// Number of the original invoice.
    pub original_num: String,
    /// Date of the original invoice.
    pub original_date: Date,
    /// The invoice as amended.
    pub amended: B2bInvoice,
}

/// Amended B2B invoices for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2baParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Amended invoices; at least one.
    pub invoices: Vec<B2baInvoice>,
}

/// A GSTR-1A return (amendments made after GSTR-1 is filed and before GSTR-3B).
///
/// Only the B2B amendment section (`b2ba`) is written. The other amendment sections (B2CLA, B2CSA,
/// CDNRA, CDNURA, EXPA) are not modelled yet and have no field, so nothing can be dropped silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1a {
    /// Supplier GSTIN.
    pub gstin: Gstin,
    /// Return period.
    pub period: Period,
    /// Amended B2B invoices.
    pub b2ba: Vec<B2baParty>,
}

/// Writes a GSTR-1 return as JSON.
///
/// # Errors
/// [`FormatError::Field`] naming the JSON path when a document number is empty or longer than 16
/// characters, a table has no lines, a unit quantity code is empty, or cancelled documents exceed
/// the total issued.
pub fn gstr1_to_json(r: &Gstr1) -> Result<String, FormatError> {
    let mut f = vec![kv("gstin", &gstin_json(&r.gstin)?), kv("fp", &fp(r.period))];
    if let Some(gt) = r.aggregate_turnover {
        f.push(kv("gt", &amt(gt)));
    }
    if let Some(gt) = r.current_turnover {
        f.push(kv("cur_gt", &amt(gt)));
    }
    if !r.b2b.is_empty() {
        f.push(kv("b2b", &b2b_json(&r.b2b, "b2b")?));
    }
    if !r.b2cl.is_empty() {
        f.push(kv("b2cl", &b2cl_json(&r.b2cl)?));
    }
    if !r.b2cs.is_empty() {
        f.push(kv("b2cs", &b2cs_json(&r.b2cs)?));
    }
    if !r.cdnr.is_empty() {
        f.push(kv("cdnr", &cdnr_json(&r.cdnr)?));
    }
    if !r.cdnur.is_empty() {
        f.push(kv("cdnur", &cdnur_json(&r.cdnur)?));
    }
    if !r.exp.is_empty() {
        f.push(kv("exp", &exp_json(&r.exp)?));
    }
    if !r.nil.is_empty() {
        f.push(kv("nil", &nil_json(&r.nil)?));
    }
    if !r.hsn.is_empty() {
        f.push(kv("hsn", &hsn_json(&r.hsn)?));
    }
    if !r.doc_issue.is_empty() {
        f.push(kv("doc_issue", &doc_issue_json(&r.doc_issue)?));
    }
    Ok(obj(&f))
}

/// Writes a GSTR-1A return as JSON.
///
/// # Errors
/// [`FormatError::Field`] naming the JSON path, as for [`gstr1_to_json`].
pub fn gstr1a_to_json(r: &Gstr1a) -> Result<String, FormatError> {
    let mut f = vec![kv("gstin", &gstin_json(&r.gstin)?), kv("fp", &fp(r.period))];
    if !r.b2ba.is_empty() {
        f.push(kv("b2ba", &b2ba_json(&r.b2ba)?));
    }
    Ok(obj(&f))
}
