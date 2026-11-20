//! Validating a draft by recomputing through the engine and GST crates. **Card A1** implements
//! this module.
//!
//! Frozen interface (plan card A1):
//!
//! ```text
//! pub enum DraftCheck { Ok(Voucher), Blocked { field: String, reason: String } }
//! pub fn validate_draft(d: &VoucherDraft, books: &Books, rules: &RuleSet, threshold_permille: u16) -> Result<DraftCheck, AssistError>;
//! ```
//!
//! The assistant proposes, the engine computes, a person confirms (spec 6.8, rule A10). A draft
//! carries figures read from a document or typed text; this module turns it into a [`Voucher`] and
//! recomputes every derived figure. It never writes anything and never supplies a number.
//!
//! # Confidence
//!
//! A [`Transcribed`] value whose confidence is below the threshold is **blank**: it is treated as
//! if the assistant had not read it. A confidence equal to the threshold is kept. A confidence
//! above 1000 per-mille is malformed and blocks the draft. What a blank means depends on the field:
//!
//! - the date, a line's ledger name and a line's amount are required: a blank blocks the draft;
//! - the party name: the resolved party is dropped (and a sales or purchase voucher is then blocked
//!   for want of a party);
//! - the invoice number and the supplier GSTIN are left out of the voucher;
//! - the document total and document tax are not compared.
//!
//! # What is recomputed
//!
//! - The voucher must pass the store-free invariants of `ledgerly_model::validate`.
//! - A purchase or debit note is supplied by the party. The engine takes the company's registration
//!   state as the supplying state, so such a draft is `Unsupported` unless the party's state (its
//!   state field, else its GSTIN's) is known and equal to every registration state of the company.
//! - For a sales, purchase, credit-note or debit-note voucher in a GST company, the tax comes from
//!   `ledgerly_gst::compute::compute_voucher_tax`. The tax-ledger lines of the draft must equal
//!   each computed head exactly, sit on the side opposite the party line, and the party's lines
//!   must total the computed invoice value (taxable value plus tax plus the explicit round-off).
//! - For any other voucher the engine total is the debit total and the engine tax is nil.
//! - The document total and document tax, when read with enough confidence, must equal the engine's
//!   figures. A mismatch is [`DraftCheck::Blocked`], never a silent correction.
//!
//! # Not checked here
//!
//! The period lock, numbering, bill references against existing bills and stock rules belong to the
//! engine's `post_voucher`, which runs when a person accepts the draft. A draft proposes no bill
//! allocations, so a line on a bill-wise ledger is blocked. The place of supply is never guessed:
//! the caller supplies it through [`validate_draft_with_place`] and without it a GST draft is
//! `Unsupported`.

use std::collections::BTreeMap;

use ledgerly_core::{
    DrCr, Gstin, LedgerId, MoneyError, Outcome, Paise, SignedAmount, StateCode, Unsupported,
    VoucherId, VoucherTypeId,
};
use ledgerly_gst::compute::compute_voucher_tax;
use ledgerly_gst::pack::RuleSet;
use ledgerly_model::validate::validate_voucher;
use ledgerly_model::{
    BaseVoucherKind, Books, LedgerLine, TaxLedgerKind, Voucher, VoucherError, VoucherMode,
    VoucherStatus, VoucherType,
};

use crate::proposal::{Transcribed, VoucherDraft};
use crate::seam::AssistError;

/// Highest valid confidence, in per-mille.
const MAX_CONFIDENCE_PERMILLE: u16 = 1000;

/// The result of validating a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "frozen interface: the card declares `Ok(Voucher)`; the value is built once per draft"
)]
pub enum DraftCheck {
    /// The draft became a voucher that passes the invariants and whose figures the engine
    /// reproduced. It is still only a proposal: a person accepts it and the engine posts it.
    Ok(Voucher),
    /// The draft cannot be accepted as it stands.
    Blocked {
        /// The draft field at fault, such as `document_total`, `date`, `lines[1].amount` or
        /// `lines.cgst`.
        field: String,
        /// What is wrong and what the user can do.
        reason: String,
    },
}

/// Why validation stopped early.
enum Halt {
    /// The draft is blocked.
    Blocked { field: String, reason: String },
    /// The engine does not support what the draft needs.
    Unsupported(Unsupported),
}

type Step<T> = Result<T, Halt>;

impl Halt {
    fn blocked(field: impl Into<String>, reason: impl Into<String>) -> Halt {
        Halt::Blocked {
            field: field.into(),
            reason: reason.into(),
        }
    }

    fn unsupported(what: impl Into<String>, reason: impl Into<String>) -> Halt {
        Halt::Unsupported(Unsupported {
            what: what.into(),
            reason: reason.into(),
        })
    }
}

impl From<MoneyError> for Halt {
    fn from(e: MoneyError) -> Self {
        Halt::blocked("amount", e.to_string())
    }
}

impl From<VoucherError> for Halt {
    fn from(e: VoucherError) -> Self {
        let reason = if matches!(e, VoucherError::BillAllocationMismatch { .. }) {
            format!(
                "{e}; the assistant does not propose bill references, enter them in the voucher form"
            )
        } else {
            e.to_string()
        };
        let field = match e {
            VoucherError::Unbalanced { .. }
            | VoucherError::Empty
            | VoucherError::UnknownLedger(_)
            | VoucherError::BillOnWrongLedger { .. }
            | VoucherError::Money(_) => "lines".to_owned(),
            VoucherError::BillAllocationMismatch { line, .. } => format!("lines[{line}].bills"),
            VoucherError::NegativeAmount { line } => format!("lines[{line}].amount"),
            VoucherError::BeforeBooksBegin { .. } => "date".to_owned(),
            VoucherError::MissingParty => "party".to_owned(),
            VoucherError::UnknownVoucherType(_) => "kind".to_owned(),
            VoucherError::Invalid { field, .. } => field,
        };
        Halt::Blocked { field, reason }
    }
}

/// Tax per head, in paise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HeadSums {
    cgst: Paise,
    sgst: Paise,
    utgst: Paise,
    igst: Paise,
    cess: Paise,
}

impl HeadSums {
    fn total(&self) -> Result<Paise, MoneyError> {
        let mut sum = Paise::ZERO;
        for part in [self.cgst, self.sgst, self.utgst, self.igst, self.cess] {
            sum = sum.checked_add(part)?;
        }
        Ok(sum)
    }
}

/// The figures the engine reproduces for a voucher.
struct Expected {
    /// Invoice value (GST vouchers) or debit total.
    total: Paise,
    /// Tax per head.
    tax: HeadSums,
    /// Whether the figures came from the GST computation.
    gst: bool,
}

/// Validates a draft without a place of supply.
///
/// Equivalent to [`validate_draft_with_place`] with `None`: a draft that needs GST is
/// `Unsupported` because the place of supply is never guessed.
///
/// # Errors
/// [`AssistError::Unsupported`] when the draft needs something the engine does not compute: a
/// voucher kind the assistant does not draft, item lines, or GST the engine cannot yet compute
/// (including a missing place of supply). Every other problem with the draft is
/// [`DraftCheck::Blocked`].
pub fn validate_draft(
    d: &VoucherDraft,
    books: &Books,
    rules: &RuleSet,
    threshold_permille: u16,
) -> Result<DraftCheck, AssistError> {
    validate_draft_with_place(d, books, rules, threshold_permille, None)
}

/// Validates a draft, with the place of supply the user confirmed.
///
/// `rules` must be the rule set for the voucher's time of supply (its date, since a draft sets no
/// supply date). `threshold_permille` is the least confidence that keeps a transcribed value.
///
/// # Errors
/// [`AssistError::Unsupported`] as for [`validate_draft`].
pub fn validate_draft_with_place(
    d: &VoucherDraft,
    books: &Books,
    rules: &RuleSet,
    threshold_permille: u16,
    place_of_supply: Option<StateCode>,
) -> Result<DraftCheck, AssistError> {
    match run(d, books, rules, threshold_permille, place_of_supply) {
        Ok(voucher) => Ok(DraftCheck::Ok(voucher)),
        Err(Halt::Blocked { field, reason }) => Ok(DraftCheck::Blocked { field, reason }),
        Err(Halt::Unsupported(u)) => Err(AssistError::Unsupported(u)),
    }
}

fn run(
    d: &VoucherDraft,
    books: &Books,
    rules: &RuleSet,
    threshold: u16,
    place: Option<StateCode>,
) -> Step<Voucher> {
    let voucher = build_voucher(d, books, threshold, place)?;
    validate_voucher(&voucher, &books.masters, &books.company)?;
    let expected = recompute(&voucher, d.kind, books, rules)?;
    check_figures(&voucher, books, &expected)?;
    check_document(d, &expected, threshold)?;
    Ok(voucher)
}

/// Reads a transcribed value: `None` when absent or below the threshold, an error when the
/// confidence itself is out of range.
fn read<'a, T>(t: Option<&'a Transcribed<T>>, field: &str, threshold: u16) -> Step<Option<&'a T>> {
    let Some(t) = t else {
        return Ok(None);
    };
    if t.confidence_permille > MAX_CONFIDENCE_PERMILLE {
        return Err(Halt::blocked(
            field,
            format!(
                "confidence {} is above {MAX_CONFIDENCE_PERMILLE} per-mille; the proposal is malformed",
                t.confidence_permille
            ),
        ));
    }
    Ok(t.is_confident(threshold).then_some(&t.value))
}

/// The voucher mode for a kind the assistant drafts.
fn mode_for(kind: BaseVoucherKind) -> Step<VoucherMode> {
    match kind {
        BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => Ok(VoucherMode::AccountingInvoice),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal => Ok(VoucherMode::AsVoucher),
        BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => Err(Halt::unsupported(
            format!("assistant drafts of {kind:?} vouchers"),
            "the assistant drafts sales, purchase, credit-note, debit-note, payment, receipt, \
             contra and journal vouchers; enter this kind in the voucher form",
        )),
    }
}

/// Whether GST is computed on this kind of voucher.
fn is_taxed_kind(kind: BaseVoucherKind) -> bool {
    match kind {
        BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => true,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => false,
    }
}

/// The one active voucher type of a kind.
fn pick_type(books: &Books, kind: BaseVoucherKind) -> Step<VoucherTypeId> {
    let matching: Vec<&VoucherType> = books
        .masters
        .voucher_types
        .values()
        .filter(|t| t.active && t.base == kind)
        .collect();
    match matching.as_slice() {
        [only] => Ok(only.id),
        [] => Err(Halt::blocked(
            "kind",
            format!("the company has no active {kind:?} voucher type; create one first"),
        )),
        [_, ..] => Err(Halt::blocked(
            "kind",
            format!("the company has several active {kind:?} voucher types; choose one"),
        )),
    }
}

/// The party ledger, dropped when its name was read below the threshold.
fn resolve_party(d: &VoucherDraft, books: &Books, threshold: u16) -> Step<Option<LedgerId>> {
    let Some(id) = d.party else {
        return Ok(None);
    };
    if read(d.party_name.as_ref(), "party_name", threshold)?.is_none() && d.party_name.is_some() {
        return Ok(None);
    }
    if !books.masters.ledgers.contains_key(&id) {
        return Err(Halt::blocked(
            "party",
            "the party ledger does not exist in this company",
        ));
    }
    Ok(Some(id))
}

/// Whose GSTIN the supplier GSTIN on a document should be.
enum SupplierIs {
    /// The party ledger (a purchase or a debit note: the party is the supplier).
    Party,
    /// The company itself (a sale: the company is the supplier).
    Company,
    /// Not known from the kind; only the format is checked.
    Unchecked,
}

fn supplier_is(kind: BaseVoucherKind) -> SupplierIs {
    match kind {
        BaseVoucherKind::Purchase | BaseVoucherKind::DebitNote => SupplierIs::Party,
        BaseVoucherKind::Sales => SupplierIs::Company,
        BaseVoucherKind::CreditNote
        | BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => SupplierIs::Unchecked,
    }
}

/// A confident supplier GSTIN must be well formed and, where the kind says who the supplier is,
/// agree with the GSTIN on file for that party or company.
fn check_gstin(
    d: &VoucherDraft,
    books: &Books,
    party: Option<LedgerId>,
    threshold: u16,
) -> Step<()> {
    let Some(text) = read(d.supplier_gstin.as_ref(), "supplier_gstin", threshold)? else {
        return Ok(());
    };
    let parsed = Gstin::parse(text).map_err(|e| {
        Halt::blocked(
            "supplier_gstin",
            format!("'{text}' is not a valid GSTIN ({e}); correct it or clear it"),
        )
    })?;
    let known: Vec<&str> = match supplier_is(d.kind) {
        SupplierIs::Party => party
            .and_then(|id| books.masters.ledgers.get(&id))
            .and_then(|l| l.party.as_ref())
            .and_then(|p| p.gstin.as_ref())
            .map(|g| vec![g.as_str()])
            .unwrap_or_default(),
        SupplierIs::Company => books
            .company
            .registrations
            .iter()
            .map(|r| r.gstin.as_str())
            .collect(),
        SupplierIs::Unchecked => return Ok(()),
    };
    if !known.is_empty() && !known.contains(&parsed.as_str()) {
        return Err(Halt::blocked(
            "supplier_gstin",
            format!(
                "the GSTIN read from the document ({}) differs from the one on file ({})",
                parsed.as_str(),
                known.join(", ")
            ),
        ));
    }
    Ok(())
}

/// The ledger lines of the draft. Ids are never invented: a line without a resolved ledger blocks.
fn build_lines(d: &VoucherDraft, books: &Books, threshold: u16) -> Step<Vec<LedgerLine>> {
    let mut lines = Vec::with_capacity(d.lines.len());
    for (index, line) in d.lines.iter().enumerate() {
        let ledger_field = format!("lines[{index}].ledger");
        let amount_field = format!("lines[{index}].amount");
        let Some(name) = read(Some(&line.ledger_name), &ledger_field, threshold)? else {
            return Err(Halt::blocked(
                ledger_field,
                "the ledger name was read with too little confidence; choose the ledger yourself",
            ));
        };
        let Some(id) = line.ledger else {
            return Err(Halt::blocked(
                ledger_field,
                format!("no ledger matches '{name}'; choose an existing ledger or create one"),
            ));
        };
        if !books.masters.ledgers.contains_key(&id) {
            return Err(Halt::blocked(
                ledger_field,
                format!("the ledger for '{name}' does not exist in this company"),
            ));
        }
        let Some(amount) = read(Some(&line.amount), &amount_field, threshold)? else {
            return Err(Halt::blocked(
                amount_field,
                "the amount was read with too little confidence; type it in",
            ));
        };
        let signed = match line.side {
            DrCr::Dr => SignedAmount::dr(*amount),
            DrCr::Cr => SignedAmount::cr(*amount),
        };
        lines.push(LedgerLine {
            ledger: id,
            amount: signed,
            bills: Vec::new(),
            cost: Vec::new(),
            bank: None,
        });
    }
    Ok(lines)
}

fn build_voucher(
    d: &VoucherDraft,
    books: &Books,
    threshold: u16,
    place: Option<StateCode>,
) -> Step<Voucher> {
    if !d.items.is_empty() {
        return Err(Halt::unsupported(
            "assistant drafts with item lines",
            "stock items are not resolved from names yet; enter the item lines in the voucher form",
        ));
    }
    let mode = mode_for(d.kind)?;
    let type_id = pick_type(books, d.kind)?;
    let Some(date) = read(d.date.as_ref(), "date", threshold)? else {
        return Err(Halt::blocked(
            "date",
            "the date is missing or was read with too little confidence; type it in",
        ));
    };
    if !d.create_ledgers.is_empty() {
        return Err(Halt::blocked(
            "create_ledgers",
            format!(
                "create these ledgers first: {}",
                d.create_ledgers.join(", ")
            ),
        ));
    }
    let party = resolve_party(d, books, threshold)?;
    check_gstin(d, books, party, threshold)?;
    let ledger_lines = build_lines(d, books, threshold)?;
    let reference = read(d.invoice_number.as_ref(), "invoice_number", threshold)?
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let next_id = books
        .vouchers
        .keys()
        .next_back()
        .map_or(Some(1_u64), |id| id.0.checked_add(1));
    let Some(id) = next_id else {
        return Err(Halt::blocked("id", "voucher ids are exhausted"));
    };
    Ok(Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: *date,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode,
        party,
        place_of_supply: place,
        reverse_charge: false,
        reference,
        reference_date: None,
        narration: d.narration.clone(),
        ledger_lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    })
}

fn debit_total(v: &Voucher) -> Result<Paise, MoneyError> {
    let mut total = Paise::ZERO;
    for line in &v.ledger_lines {
        match line.amount.side {
            DrCr::Dr => total = total.checked_add(line.amount.amount)?,
            DrCr::Cr => {}
        }
    }
    Ok(total)
}

/// Card G4 takes the company's registration state as the supplier's state whatever the voucher
/// kind. That is right when the company supplies (sales) and wrong when the party supplies
/// (purchase, debit note), unless the party is in the company's own state. Anything else would be
/// a guess about CGST+SGST against IGST, so it is `Unsupported` (rule A1).
fn check_supplier_state(v: &Voucher, kind: BaseVoucherKind, books: &Books) -> Step<()> {
    if !matches!(supplier_is(kind), SupplierIs::Party) {
        return Ok(());
    }
    let Some(party) = v.party else {
        return Err(Halt::blocked("party", "a taxed voucher needs a party"));
    };
    let details = books
        .masters
        .ledgers
        .get(&party)
        .and_then(|l| l.party.as_ref());
    let from_state = details.and_then(|p| p.state);
    let from_gstin = details
        .and_then(|p| p.gstin.as_ref())
        .and_then(Gstin::state);
    let party_state = match (from_state, from_gstin) {
        (Some(a), Some(b)) if a != b => {
            return Err(Halt::unsupported(
                "GST on a supply from a party whose state is ambiguous",
                "the party ledger's state and its GSTIN's state differ; correct the party ledger",
            ));
        }
        (Some(a), _) | (None, Some(a)) => a,
        (None, None) => {
            return Err(Halt::unsupported(
                "GST on a supply from a party of unknown state",
                "the party ledger has neither a state nor a GSTIN; set the supplier's state on the \
                 party ledger or enter this voucher in the voucher form",
            ));
        }
    };
    if books
        .company
        .registrations
        .iter()
        .all(|r| r.state == party_state)
    {
        return Ok(());
    }
    Err(Halt::unsupported(
        "GST on a supply from a supplier outside the company's registration state",
        "the engine takes the company's registration state as the supplier's state, which is \
         wrong when the party supplies from another state (the tax would be IGST); enter this \
         voucher in the voucher form",
    ))
}

/// What the engine says the voucher's total and tax are.
fn recompute(v: &Voucher, kind: BaseVoucherKind, books: &Books, rules: &RuleSet) -> Step<Expected> {
    if !(is_taxed_kind(kind) && books.company.features.gst) {
        return Ok(Expected {
            total: debit_total(v)?,
            tax: HeadSums::default(),
            gst: false,
        });
    }
    check_supplier_state(v, kind, books)?;
    let breakup = match compute_voucher_tax(v, &books.masters, rules, &books.company) {
        Ok(Outcome::Value(b)) => b,
        Ok(Outcome::Unsupported(u)) => return Err(Halt::Unsupported(u)),
        Err(e) => return Err(Halt::blocked("gst", e.to_string())),
    };
    Ok(Expected {
        total: breakup
            .total_before_round_off
            .checked_add(breakup.round_off)?,
        tax: HeadSums {
            cgst: breakup.totals.cgst,
            sgst: breakup.totals.sgst,
            utgst: breakup.totals.utgst,
            igst: breakup.totals.igst,
            cess: breakup.totals.cess,
        },
        gst: true,
    })
}

/// Per-head tax carried by the draft's tax-ledger lines, with the side of each such line.
fn drafted_heads(v: &Voucher, books: &Books) -> Step<(HeadSums, Vec<DrCr>)> {
    let mut sums = HeadSums::default();
    let mut sides = Vec::new();
    for line in &v.ledger_lines {
        let Some(tax) = books
            .masters
            .ledgers
            .get(&line.ledger)
            .and_then(|l| l.tax.as_ref())
        else {
            continue;
        };
        let slot = match tax.kind {
            TaxLedgerKind::Cgst => &mut sums.cgst,
            TaxLedgerKind::Sgst => &mut sums.sgst,
            TaxLedgerKind::Utgst => &mut sums.utgst,
            TaxLedgerKind::Igst => &mut sums.igst,
            TaxLedgerKind::Cess => &mut sums.cess,
            TaxLedgerKind::Tds | TaxLedgerKind::Tcs | TaxLedgerKind::Other => continue,
        };
        *slot = slot.checked_add(line.amount.amount)?;
        sides.push(line.amount.side);
    }
    Ok((sums, sides))
}

/// The draft's own lines against the engine's figures.
fn check_figures(v: &Voucher, books: &Books, expected: &Expected) -> Step<()> {
    let (drafted, tax_sides) = drafted_heads(v, books)?;
    let heads = [
        ("cgst", expected.tax.cgst, drafted.cgst),
        ("sgst", expected.tax.sgst, drafted.sgst),
        ("utgst", expected.tax.utgst, drafted.utgst),
        ("igst", expected.tax.igst, drafted.igst),
        ("cess", expected.tax.cess, drafted.cess),
    ];
    for (name, want, got) in heads {
        if want != got {
            let (want, got) = (want.to_rupees(), got.to_rupees());
            let basis = if expected.gst {
                "the engine computes"
            } else {
                "no GST is computed for this voucher, expected"
            };
            return Err(Halt::blocked(
                format!("lines.{name}"),
                format!("the lines carry {got} of {name} tax but {basis} {want}"),
            ));
        }
    }
    if !expected.gst {
        return Ok(());
    }
    let (party_side, party_total) = party_lines(v)?;
    if tax_sides.contains(&party_side) {
        return Err(Halt::blocked(
            "lines",
            "a tax line is on the same side as the party line; tax goes on the opposite side",
        ));
    }
    if party_total != expected.total {
        let (party_total, total) = (party_total.to_rupees(), expected.total.to_rupees());
        return Err(Halt::blocked(
            "lines",
            format!(
                "the party's lines total {party_total} but the engine computes an invoice value \
                 of {total} (taxable value plus tax plus round-off)"
            ),
        ));
    }
    Ok(())
}

/// The side of the party's first line and the total of the party's lines on that side. The party
/// carries the invoice value whichever way the round-off falls (the round-off line is a separate
/// line on the party's side when the value rounds down).
fn party_lines(v: &Voucher) -> Step<(DrCr, Paise)> {
    let Some(party) = v.party else {
        return Err(Halt::blocked("party", "a taxed voucher needs a party"));
    };
    let Some(first) = v.ledger_lines.iter().find(|l| l.ledger == party) else {
        return Err(Halt::blocked(
            "lines",
            "no line posts to the party ledger; add the party's line",
        ));
    };
    let side = first.amount.side;
    let mut total = Paise::ZERO;
    for line in v
        .ledger_lines
        .iter()
        .filter(|l| l.ledger == party && l.amount.side == side)
    {
        total = total.checked_add(line.amount.amount)?;
    }
    Ok((side, total))
}

/// The document's own totals against the engine's. Blank (low-confidence) figures are skipped.
fn check_document(d: &VoucherDraft, expected: &Expected, threshold: u16) -> Step<()> {
    if let Some(read_total) = read(d.document_total.as_ref(), "document_total", threshold)?
        && *read_total != expected.total
    {
        let (read_total, total) = (read_total.to_rupees(), expected.total.to_rupees());
        return Err(Halt::blocked(
            "document_total",
            format!("the document total read is {read_total} but the engine computes {total}"),
        ));
    }
    if let Some(read_tax) = read(d.document_tax.as_ref(), "document_tax", threshold)? {
        let tax = expected.tax.total()?;
        if *read_tax != tax {
            let (read_tax, tax) = (read_tax.to_rupees(), tax.to_rupees());
            return Err(Halt::blocked(
                "document_tax",
                format!("the document tax read is {read_tax} but the engine computes {tax}"),
            ));
        }
    }
    Ok(())
}
