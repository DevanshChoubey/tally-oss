//! GST on a voucher at the API (Wave 3 card C2; spec invariant 9: "GST tax on a voucher is
//! recomputed by the engine from masters + rule pack").
//!
//! Three things live here.
//!
//! * [`Api::voucher_gst`] runs `ledgerly_gst::compute::compute_voucher_tax` on a draft voucher with
//!   the bundled slab pack and the time of supply of the draft.
//! * [`Api::invoice_gst`] turns that answer into what an item invoice posts: the accounting lines
//!   and the stored GST block. The item invoice screen asks for it, so the screen computes nothing.
//! * [`with_gst`] runs inside `PostVoucher` and `AlterVoucher`. For an invoice-mode voucher that
//!   posts to GST ledgers (a ledger with a CGST, SGST, UTGST, IGST or cess tax kind), it recomputes
//!   the tax and either fills `Voucher::gst` or refuses the voucher.
//!
//! # The rule at posting
//!
//! The model has no flagged override fields yet, so no hand-typed tax may differ from the engine
//! (invariant 9 allows overrides only on flagged fields). Per tax head the lines posted to that
//! head's ledgers must total the engine's figure, on the side the voucher kind requires: a sales
//! invoice or debit note credits tax, a purchase or credit note debits it. Otherwise the post is
//! refused with `EngineError::Invalid { field: "GST", .. }`, saying which head and both figures
//! (purchases and debit notes included: the hand-typed tax of a supplier's bill is checked too).
//!
//! * Computed and agreeing: `Voucher::gst` is filled with the engine's block (pack version,
//!   one line per head and rate, the engine's round-off).
//! * The engine does not compute the voucher (`Unsupported`, for example a composition supplier,
//!   reverse charge or a missing place of supply): nothing can be compared, so the voucher is
//!   posted as typed with `gst` left empty. Changing this to a refusal is one line in [`with_gst`];
//!   it is not done because Wave 2 vouchers and tests post such vouchers today.
//! * A voucher with no GST ledger line, a plain Dr/Cr voucher, a company without the GST feature
//!   and a voucher kind that is not sales, purchase, credit note or debit note are not touched.
//!
//! # What an item invoice posts
//!
//! Party line for the invoice total, one line per sales or purchase ledger of the inventory lines,
//! and one line per stored tax line. The tax ledger of a head is the only ledger of that tax kind
//! (or, with rate-specific ledgers, the one whose rate equals the head's percent). Several
//! candidates, none, or a non-zero round-off (the model has no "invoice rounding" ledger type,
//! which TallyPrime uses; see
//! <https://help.tallysolutions.com/tally-prime/gst-master-setup/india-gst-expense-income-non-revenue-invoice-round-off-ledgers-tally/>,
//! read 2026-11-21) make the answer `Unsupported`, never a guess.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    Date, Decimal, DrCr, LedgerId, MoneyError, Outcome, Paise, Rate, RoundingRule, SignedAmount,
    VoucherTypeId, round_rupees_to_paise,
};
use ledgerly_engine::EngineError;
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{GstBreakup, LineTax, compute_voucher_tax};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_model::{
    BaseVoucherKind, BillAllocation, BillRefKind, Books, GstLine, LedgerLine, NumberingMethod,
    ReservedLedger, StoredGst, TaxLedgerKind, Voucher, VoucherMode,
};

use crate::error::ApiError;
use crate::facade::Api;
use crate::gst::BUNDLED_RATE_PACK;

/// The accounting lines and the GST block of an item invoice, from the engine's tax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceGst {
    /// The engine's tax computation the rest is built from.
    pub breakup: GstBreakup,
    /// The invoice value: taxable value plus the tax heads plus the round-off to the whole rupee.
    pub invoice_total: Paise,
    /// The accounting lines: party, sales or purchase ledgers, tax ledgers. They balance.
    pub ledger_lines: Vec<LedgerLine>,
    /// The block to store on the voucher.
    pub stored: StoredGst,
    /// The tax box rows: each head at each rate, by rate then head, as "CGST 9% on 1,30,085.00".
    /// Empty when the rounded rows would not add up to the head totals (the box then lists the
    /// head totals alone, since showing rows that disagree with the posting would be a wrong
    /// number).
    pub rate_rows: Vec<RateRow>,
    /// The input credit of a purchase draft; `None` for any other kind of voucher (never zero).
    pub eligible_itc: Option<Outcome<EligibleItc>>,
}

/// The input tax credit of a purchase draft, by the rule GSTR-3B 4(A) uses. It cannot know the
/// s.16(2) conditions (the supplier's bill in 2B, receipt, payment within 180 days), so it is the
/// credit that is eligible before those are met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibleItc {
    /// Tax on lines whose credit the rule allows: each head summed unrounded over those lines,
    /// rounded once to the paisa, then added.
    pub eligible: Paise,
    /// Tax on lines whose credit is ineligible or blocked.
    pub not_claimable: Paise,
}

/// One row of the tax box: a tax head at one rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateRow {
    /// `CGST`, `SGST`, `UTGST` or `IGST`.
    pub head: String,
    /// The head's own rate in percent (half the slab for CGST, SGST and UTGST).
    pub rate_percent: Decimal,
    /// The taxable value of the lines at this rate.
    pub taxable: Paise,
    /// The head's tax on them, rounded to the paisa from the unrounded line tax.
    pub amount: Paise,
}

/// One bill reference the user typed for the party line of an invoice (TallyPrime's Bill-wise
/// Details screen, <https://help.tallysolutions.com/advances-received/>, read 2026-11-25).
///
/// A bill-wise party with no references gets one [`BillRefKind::New`] for the whole invoice value.
/// References split the value: those with an amount take it, and at most one without an amount
/// takes what is left. They must add up to the invoice value exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyBill {
    /// New Ref, Agst Ref, Advance or On Account.
    pub kind: BillRefKind,
    /// The bill's name. Empty on a New Ref means the voucher number: the engine names it when the
    /// voucher is posted and has its number. Ignored on On Account.
    pub name: String,
    /// The part of the invoice value on this reference; `None` takes the remainder.
    pub amount: Option<Paise>,
    /// Credit days from the voucher date; the party master's default when `None` on a New Ref.
    pub credit_days: Option<u32>,
    /// Due date, instead of credit days.
    pub due_date: Option<Date>,
}

/// A tax ledger of the masters or of a voucher.
pub(crate) struct TaxLedger {
    pub(crate) id: LedgerId,
    pub(crate) kind: TaxLedgerKind,
    pub(crate) rate: Option<Rate>,
}

impl Api {
    /// Runs the GST engine on a draft voucher with the bundled slab pack, selected by the
    /// voucher's time of supply (its supply date, else its date).
    ///
    /// The draft needs no number or id. `Unsupported` carries the engine's reason (for example no
    /// place of supply, a composition supplier, or a rate with no verified slab).
    ///
    /// # Errors
    /// [`ApiError::Gst`] when a master the voucher names is missing, the pack is malformed or
    /// arithmetic overflows; [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn voucher_gst(&self, v: &Voucher) -> Result<Outcome<GstBreakup>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let pack = load_pack(BUNDLED_RATE_PACK)?;
        if v.mode == VoucherMode::AsVoucher {
            return Ok(crate::as_voucher::breakup(books, v, &pack)?);
        }
        Ok(compute_for(books, v, &pack)?)
    }

    /// The accounting lines and stored GST block of an item invoice (a draft with inventory lines
    /// and no ledger lines), built from [`Api::voucher_gst`].
    ///
    /// # Errors
    /// As [`Api::voucher_gst`].
    pub fn invoice_gst(&self, v: &Voucher) -> Result<Outcome<InvoiceGst>, ApiError> {
        self.invoice_gst_with_bills(v, &[])
    }

    /// [`Api::invoice_gst`] with the bill references typed for the party line. See [`PartyBill`]:
    /// no references on a bill-wise party means one New Ref for the invoice value; references on a
    /// party that is not bill-wise, or ones that do not add up to the value, are `Unsupported`.
    ///
    /// # Errors
    /// As [`Api::voucher_gst`].
    pub fn invoice_gst_with_bills(
        &self,
        v: &Voucher,
        bills: &[PartyBill],
    ) -> Result<Outcome<InvoiceGst>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let pack = load_pack(BUNDLED_RATE_PACK)?;
        if v.mode == VoucherMode::AsVoucher {
            return Ok(crate::as_voucher::invoice(books, v, &pack, bills)?);
        }
        match v.mode {
            VoucherMode::ItemInvoice if v.ledger_lines.is_empty() => {}
            VoucherMode::AccountingInvoice if v.inventory_lines.is_empty() => {}
            VoucherMode::ItemInvoice | VoucherMode::AccountingInvoice | VoucherMode::AsVoucher => {
                return Ok(Outcome::unsupported(
                    "accounting lines for this voucher",
                    "only an item invoice without ledger lines, or an accounting invoice with only its particulars, gets its lines from the tax engine",
                ));
            }
        }
        let breakup = match compute_for(books, v, &pack)? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(build_invoice(
            books,
            v,
            breakup,
            bills,
            &all_tax_ledgers(books),
        )?)
    }

    /// The bills of `ledger` still open on `as_of` (the list of the payment and receipt screens'
    /// bill-wise details). A ledger that does not track bills has none.
    ///
    /// # Errors
    /// [`ApiError::Engine`] when the ledger does not exist or its bills cannot be worked out (for
    /// example a bill-wise ledger with an opening balance, which has no bill breakup);
    /// [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn open_bills(
        &self,
        ledger: LedgerId,
        as_of: Date,
    ) -> Result<Outcome<Vec<ledgerly_engine::bills::OpenBill>>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(Outcome::Value(ledgerly_engine::bills::open_bills(
            books, ledger, as_of,
        )?))
    }

    /// The number the engine will give the next voucher of type `type_id` dated `date`, for the
    /// title of a new voucher ("Sales invoice SI/0418"). `None` when the type takes a number typed
    /// in or has no numbers. `Unsupported` when the type has several series (the screen has no
    /// series choice yet).
    ///
    /// The number is a preview: it is allocated when the voucher is posted.
    ///
    /// # Errors
    /// [`ApiError::Engine`] when the type is unknown or numbering cannot continue;
    /// [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn next_voucher_number(
        &self,
        type_id: VoucherTypeId,
        date: Date,
    ) -> Result<Outcome<Option<String>>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let vt =
            books
                .masters
                .voucher_types
                .get(&type_id)
                .ok_or_else(|| EngineError::NotFound {
                    what: format!("voucher type {}", type_id.0),
                })?;
        match vt.numbering.method {
            NumberingMethod::Manual | NumberingMethod::None => return Ok(Outcome::Value(None)),
            NumberingMethod::Automatic => {}
        }
        if !vt.numbering.series.is_empty() {
            return Ok(Outcome::unsupported(
                "next number of a voucher type with several series",
                "choose the series first; the screen has no series choice yet",
            ));
        }
        let number = ledgerly_engine::numbering::next_number(vt, None, date, books)?;
        Ok(Outcome::Value(Some(number)))
    }
}

/// Runs the engine for `v` on the rule set of its time of supply.
pub(crate) fn compute_for(
    books: &Books,
    v: &Voucher,
    pack: &RulePack,
) -> Result<Outcome<GstBreakup>, GstError> {
    let tos = v.supply_date.unwrap_or(v.date);
    let rules = match pack.for_time_of_supply(tos) {
        Outcome::Value(rules) => rules,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    compute_voucher_tax(v, &books.masters, &rules, &books.company)
}

/// The side the tax lines of a voucher kind are posted on, or `None` for a kind that is not taxed.
pub(crate) fn tax_side(base: BaseVoucherKind) -> Option<DrCr> {
    match base {
        BaseVoucherKind::Sales | BaseVoucherKind::DebitNote => Some(DrCr::Cr),
        BaseVoucherKind::Purchase | BaseVoucherKind::CreditNote => Some(DrCr::Dr),
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
        | BaseVoucherKind::PurchaseOrder => None,
    }
}

/// `true` for the tax kinds the GST engine computes.
pub(crate) fn is_gst_head(kind: TaxLedgerKind) -> bool {
    match kind {
        TaxLedgerKind::Cgst
        | TaxLedgerKind::Sgst
        | TaxLedgerKind::Utgst
        | TaxLedgerKind::Igst
        | TaxLedgerKind::Cess => true,
        TaxLedgerKind::Tds | TaxLedgerKind::Tcs | TaxLedgerKind::Other => false,
    }
}

fn head_label(kind: TaxLedgerKind) -> &'static str {
    match kind {
        TaxLedgerKind::Cgst => "CGST",
        TaxLedgerKind::Sgst => "SGST",
        TaxLedgerKind::Utgst => "UTGST",
        TaxLedgerKind::Igst => "IGST",
        TaxLedgerKind::Cess => "cess",
        TaxLedgerKind::Tds => "TDS",
        TaxLedgerKind::Tcs => "TCS",
        TaxLedgerKind::Other => "other duty",
    }
}

fn refused(reason: String) -> EngineError {
    EngineError::Invalid {
        field: "GST".to_owned(),
        reason,
    }
}

/// The ledger lines of `v` that post to a GST tax ledger, with the ledger.
fn typed_tax_lines<'v>(v: &'v Voucher, books: &Books) -> Vec<(&'v LedgerLine, TaxLedger)> {
    v.ledger_lines
        .iter()
        .filter_map(|line| {
            let tax = books.masters.ledgers.get(&line.ledger)?.tax?;
            is_gst_head(tax.kind).then_some((
                line,
                TaxLedger {
                    id: line.ledger,
                    kind: tax.kind,
                    rate: tax.rate_percent,
                },
            ))
        })
        .collect()
}

/// Recomputes the GST of an invoice-mode voucher at `PostVoucher` and `AlterVoucher`.
///
/// See the module docs for the rule. Returns the voucher with `gst` filled (or emptied when the
/// engine cannot compute it), or refuses it.
///
/// # Errors
/// [`EngineError::Invalid`] on field `GST` when a hand-typed tax line disagrees with the engine
/// (the reason names the head and both amounts), or when the engine fails on the masters.
pub(crate) fn with_gst(mut v: Voucher, books: &Books) -> Result<Voucher, EngineError> {
    if !books.company.features.gst {
        return Ok(v);
    }
    match v.mode {
        VoucherMode::AccountingInvoice | VoucherMode::ItemInvoice => {}
        VoucherMode::AsVoucher => return Ok(v),
    }
    let Some(side) = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .and_then(|t| tax_side(t.base))
    else {
        return Ok(v);
    };
    let typed = typed_tax_lines(&v, books);
    if typed.is_empty() {
        return Ok(v);
    }
    let pack = load_pack(BUNDLED_RATE_PACK).map_err(|e| refused(e.to_string()))?;
    let breakup = match compute_for(books, &v, &pack) {
        Ok(Outcome::Value(b)) => b,
        Ok(Outcome::Unsupported(_)) => {
            v.gst = None;
            return Ok(v);
        }
        // A missing master is reported by the engine's own voucher checks.
        Err(GstError::NotFound { .. }) => return Ok(v),
        Err(e) => return Err(refused(e.to_string())),
    };
    check_typed_lines(&typed, side, &breakup)?;
    let ledgers: Vec<TaxLedger> = typed.into_iter().map(|(_, ledger)| ledger).collect();
    v.gst = stored_gst(&breakup, &ledgers)?;
    Ok(v)
}

/// Refuses the voucher when its tax lines differ from the engine's totals per head.
fn check_typed_lines(
    typed: &[(&LedgerLine, TaxLedger)],
    side: DrCr,
    breakup: &GstBreakup,
) -> Result<(), EngineError> {
    let totals = &breakup.totals;
    let heads = [
        (TaxLedgerKind::Cgst, totals.cgst),
        (TaxLedgerKind::Sgst, totals.sgst),
        (TaxLedgerKind::Utgst, totals.utgst),
        (TaxLedgerKind::Igst, totals.igst),
        (TaxLedgerKind::Cess, totals.cess),
    ];
    for (kind, want) in heads {
        let mut on_side = Paise::ZERO;
        let mut other_side = Paise::ZERO;
        for (line, _) in typed.iter().filter(|(_, l)| l.kind == kind) {
            if line.amount.side == side {
                on_side = on_side.checked_add(line.amount.amount)?;
            } else {
                other_side = other_side.checked_add(line.amount.amount)?;
            }
        }
        if !other_side.is_zero() {
            return Err(refused(format!(
                "{} is posted on the wrong side: this kind of voucher posts it on the {side:?} side, and the lines on the other side total Rs {}",
                head_label(kind),
                other_side.to_rupees()
            )));
        }
        if on_side != want {
            return Err(refused(format!(
                "{} lines total Rs {} but the GST engine computes Rs {} from the masters and rule pack {} ({:?}); remove the hand-typed tax lines or correct them",
                head_label(kind),
                on_side.to_rupees(),
                want.to_rupees(),
                breakup.pack_version,
                breakup.heads,
            )));
        }
    }
    Ok(())
}

/// The one ledger of `ledgers` that takes a head of `percent`, if there is exactly one.
fn pick_ledger(ledgers: &[TaxLedger], kind: TaxLedgerKind, percent: Decimal) -> Option<LedgerId> {
    let of_kind = || ledgers.iter().filter(|l| l.kind == kind);
    let exact: BTreeSet<LedgerId> = of_kind()
        .filter(|l| l.rate.is_some_and(|r| r.0 == percent))
        .map(|l| l.id)
        .collect();
    let pool: BTreeSet<LedgerId> = if exact.is_empty() {
        of_kind()
            .filter(|l| l.rate.is_none())
            .map(|l| l.id)
            .collect()
    } else {
        exact
    };
    let mut ids = pool.into_iter();
    match (ids.next(), ids.next()) {
        (Some(only), None) => Some(only),
        (None | Some(_), _) => None,
    }
}

/// The tax of one head over the lines of one rate: unrounded sum in rupees.
fn head_sum(lines: &[&LineTax], pick: impl Fn(&LineTax) -> Decimal) -> Result<Decimal, MoneyError> {
    let mut sum = Decimal::ZERO;
    for line in lines {
        sum = sum.checked_add(pick(line)).ok_or(MoneyError::Overflow)?;
    }
    Ok(sum)
}

/// The block to store: one line per head and rate. `None` when a ledger cannot be chosen or the
/// per-rate rounding does not add up to the engine's head totals (nothing is then stored).
fn stored_gst(
    breakup: &GstBreakup,
    ledgers: &[TaxLedger],
) -> Result<Option<StoredGst>, MoneyError> {
    let mut by_rate: BTreeMap<Decimal, Vec<&LineTax>> = BTreeMap::new();
    for line in breakup.lines.iter().filter(|l| !l.rate_percent.is_zero()) {
        by_rate.entry(line.rate_percent).or_default().push(line);
    }
    let mut out: Vec<GstLine> = Vec::new();
    for lines in by_rate.values() {
        let mut taxable = Paise::ZERO;
        for line in lines {
            taxable = taxable.checked_add(line.taxable)?;
        }
        let heads: [(TaxLedgerKind, Decimal); 4] = [
            (TaxLedgerKind::Cgst, head_sum(lines, |l| l.cgst)?),
            (TaxLedgerKind::Sgst, head_sum(lines, |l| l.sgst)?),
            (TaxLedgerKind::Utgst, head_sum(lines, |l| l.utgst)?),
            (TaxLedgerKind::Igst, head_sum(lines, |l| l.igst)?),
        ];
        for (kind, unrounded) in heads {
            if unrounded.is_zero() {
                continue;
            }
            let Some(percent) = unrounded
                .checked_mul(Decimal::ONE_HUNDRED)
                .and_then(|x| x.checked_div(taxable.to_rupees()))
            else {
                return Ok(None);
            };
            let percent = percent.normalize();
            let Some(ledger) = pick_ledger(ledgers, kind, percent) else {
                return Ok(None);
            };
            out.push(GstLine {
                ledger,
                kind,
                taxable,
                rate_percent: Rate(percent),
                tax: round_rupees_to_paise(unrounded, RoundingRule::HalfUpToPaise)?,
            });
        }
    }
    if !agrees_with_totals(&out, breakup)? {
        return Ok(None);
    }
    Ok(Some(StoredGst {
        pack_version: breakup.pack_version.clone(),
        lines: out,
        round_off: breakup.round_off,
    }))
}

/// `true` when the stored lines add up, per head, to the engine's rounded head totals.
fn agrees_with_totals(lines: &[GstLine], breakup: &GstBreakup) -> Result<bool, MoneyError> {
    let totals = &breakup.totals;
    for (kind, want) in [
        (TaxLedgerKind::Cgst, totals.cgst),
        (TaxLedgerKind::Sgst, totals.sgst),
        (TaxLedgerKind::Utgst, totals.utgst),
        (TaxLedgerKind::Igst, totals.igst),
    ] {
        let mut sum = Paise::ZERO;
        for line in lines.iter().filter(|l| l.kind == kind) {
            sum = sum.checked_add(line.tax)?;
        }
        if sum != want {
            return Ok(false);
        }
    }
    Ok(true)
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
        ineligible_itc: None,
    }
}

/// The invoice-rounding ledger (reserved role [`ReservedLedger::InvoiceRounding`]), if the masters
/// have one.
fn rounding_ledger(books: &Books) -> Option<LedgerId> {
    books
        .masters
        .ledgers
        .values()
        .find(|l| l.reserved == Some(ReservedLedger::InvoiceRounding))
        .map(|l| l.id)
}

/// The bill allocations of the party line, for an invoice value `total` on `side`.
///
/// A party that is not bill-wise has none (and takes none). A bill-wise party with no typed
/// references gets one New Ref for the whole value with the master's credit days. Typed references
/// are resolved as [`PartyBill`] says.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the typed references, with a refusal per rule; splitting it would scatter one rule"
)]
fn party_bills(
    books: &Books,
    party: LedgerId,
    total: Paise,
    side: DrCr,
    typed: &[PartyBill],
) -> Result<Outcome<Vec<BillAllocation>>, GstError> {
    let Some(master) = books.masters.ledgers.get(&party) else {
        return Ok(Outcome::unsupported(
            "bill references",
            format!("party ledger {} does not exist", party.0),
        ));
    };
    let Some(config) = master.bill_wise else {
        if typed.is_empty() {
            return Ok(Outcome::Value(Vec::new()));
        }
        return Ok(Outcome::unsupported(
            "bill references on a party that is not bill-wise",
            format!(
                "'{}' does not track bills; turn bill-wise on in its master or remove the references",
                master.name
            ),
        ));
    };
    let amount = |paise: Paise| SignedAmount {
        amount: paise,
        side,
    };
    if typed.is_empty() {
        return Ok(Outcome::Value(vec![BillAllocation {
            kind: BillRefKind::New,
            name: String::new(),
            due_date: None,
            credit_days: config.default_credit_days,
            amount: amount(total),
        }]));
    }
    let blanks = typed.iter().filter(|b| b.amount.is_none()).count();
    if blanks > 1 {
        return Ok(Outcome::unsupported(
            "bill references without an amount",
            "only one reference can take the remainder of the invoice value; type the others' amounts",
        ));
    }
    let mut typed_sum = Paise::ZERO;
    for b in typed {
        if let Some(a) = b.amount {
            typed_sum = typed_sum.checked_add(a)?;
        }
    }
    let rest = total.checked_sub(typed_sum)?;
    let fits = if blanks == 1 {
        rest.get() > 0
    } else {
        rest.is_zero()
    };
    if !fits {
        return Ok(Outcome::unsupported(
            "bill references that do not add up to the invoice value",
            format!(
                "the references total Rs {} but the invoice value is Rs {}; the amounts must add up exactly",
                format_rupees(typed_sum),
                format_rupees(total)
            ),
        ));
    }
    let mut out = Vec::with_capacity(typed.len());
    for b in typed {
        let paise = b.amount.unwrap_or(rest);
        if paise.get() <= 0 {
            return Ok(Outcome::unsupported(
                "bill reference with no amount",
                "every reference needs an amount above zero",
            ));
        }
        let new_ref = b.kind == BillRefKind::New;
        out.push(BillAllocation {
            kind: b.kind,
            name: match b.kind {
                BillRefKind::OnAccount => String::new(),
                BillRefKind::New | BillRefKind::Against | BillRefKind::Advance => b.name.clone(),
            },
            due_date: b.due_date,
            credit_days: if b.due_date.is_none() && new_ref {
                b.credit_days.or(config.default_credit_days)
            } else {
                b.credit_days
            },
            amount: amount(paise),
        });
    }
    Ok(Outcome::Value(out))
}

/// Rupees with Indian digit grouping and two decimals, for messages.
fn format_rupees(p: Paise) -> String {
    ledgerly_core::format::format_inr(p)
}

/// Builds the lines and block of an item invoice from the engine's tax.
#[expect(
    clippy::too_many_lines,
    reason = "one pass that builds the lines, the per-rate rows and the party bills from one breakup"
)]
pub(crate) fn build_invoice(
    books: &Books,
    v: &Voucher,
    breakup: GstBreakup,
    bills: &[PartyBill],
    ledgers: &[TaxLedger],
) -> Result<Outcome<InvoiceGst>, GstError> {
    let round_ledger = if breakup.round_off.is_zero() {
        None
    } else {
        let Some(ledger) = rounding_ledger(books) else {
            return Ok(Outcome::unsupported(
                "invoice round-off line",
                format!(
                    "the engine rounds this invoice by Rs {}, and the masters have no invoice-rounding ledger to post it to (the previous accounting app sets one with the ledger type Invoice Rounding); not posted",
                    breakup.round_off.to_rupees()
                ),
            ));
        };
        Some(ledger)
    };
    let Some(side) = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .and_then(|t| tax_side(t.base))
    else {
        return Ok(Outcome::unsupported(
            "accounting lines for this voucher",
            "only sales, purchase, credit-note and debit-note invoices are taxed",
        ));
    };
    let Some(party) = v.party else {
        return Ok(Outcome::unsupported(
            "accounting lines for an invoice without a party",
            "set the party first",
        ));
    };
    let accounts = match v.mode {
        VoucherMode::ItemInvoice => item_accounts(v)?,
        VoucherMode::AccountingInvoice => particulars(books, v, party, side, &breakup)?,
        VoucherMode::AsVoucher => {
            return Ok(Outcome::unsupported(
                "accounting lines for a voucher entered as plain Dr/Cr lines",
                "only invoices get their lines from the tax engine",
            ));
        }
    };
    let accounts = match accounts {
        Outcome::Value(accounts) => accounts,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let Some(stored) = stored_gst(&breakup, ledgers)? else {
        return Ok(Outcome::unsupported(
            "choice of tax ledgers",
            "each tax head needs exactly one ledger of its kind (or one ledger of that kind with the head's rate); keep one CGST, SGST/UTGST and IGST ledger, or give rate-specific ledgers",
        ));
    };
    let mut tax_total = Paise::ZERO;
    for tax_line in &stored.lines {
        tax_total = tax_total.checked_add(tax_line.tax)?;
    }
    let before_round = breakup.taxable_total.checked_add(tax_total)?;
    let invoice_total = before_round.checked_add(breakup.round_off)?;
    let party_bills = match party_bills(books, party, invoice_total, side.opposite(), bills)? {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let mut party_line = line(
        party,
        SignedAmount {
            amount: invoice_total,
            side: side.opposite(),
        },
    );
    party_line.bills = party_bills;
    let mut lines = vec![party_line];
    for (ledger, amount) in accounts {
        lines.push(line(ledger, SignedAmount { amount, side }));
    }
    for tax_line in &stored.lines {
        lines.push(line(
            tax_line.ledger,
            SignedAmount {
                amount: tax_line.tax,
                side,
            },
        ));
    }
    if let Some(ledger) = round_ledger {
        // The party line carries the round-off, so a positive one makes the party side bigger and
        // the rounding ledger takes the tax side; a negative one takes the party's side.
        let magnitude = breakup.round_off.checked_abs()?;
        let on_tax_side = !breakup.round_off.is_negative();
        let round_side = if on_tax_side { side } else { side.opposite() };
        lines.push(line(
            ledger,
            SignedAmount {
                amount: magnitude,
                side: round_side,
            },
        ));
    }
    let rate_rows = rate_rows(&breakup)?;
    let with_lines = Voucher {
        ledger_lines: lines.clone(),
        ..v.clone()
    };
    let eligible_itc = crate::itc_draft::eligible_itc(books, &with_lines, &breakup)?;
    Ok(Outcome::Value(InvoiceGst {
        breakup,
        invoice_total,
        ledger_lines: lines,
        stored,
        rate_rows,
        eligible_itc,
    }))
}

/// Per-rate sums of one voucher: taxable value and the unrounded tax of each head.
#[derive(Default)]
struct RateSums {
    taxable: Paise,
    cgst: Decimal,
    sgst: Decimal,
    utgst: Decimal,
    igst: Decimal,
}

/// The tax box rows of `breakup`: for each rate (ascending) the heads with tax, each summed over
/// the lines at that rate and rounded once to the paisa. Empty when any head's rows do not add up
/// to the head total the invoice posts, so the box never shows rows that disagree with it.
fn rate_rows(breakup: &GstBreakup) -> Result<Vec<RateRow>, GstError> {
    let mut by_rate: BTreeMap<Decimal, RateSums> = BTreeMap::new();
    for l in &breakup.lines {
        let sums = by_rate.entry(l.rate_percent.normalize()).or_default();
        sums.taxable = sums.taxable.checked_add(l.taxable)?;
        sums.cgst = sums.cgst.checked_add(l.cgst).ok_or(MoneyError::Overflow)?;
        sums.sgst = sums.sgst.checked_add(l.sgst).ok_or(MoneyError::Overflow)?;
        sums.utgst = sums
            .utgst
            .checked_add(l.utgst)
            .ok_or(MoneyError::Overflow)?;
        sums.igst = sums.igst.checked_add(l.igst).ok_or(MoneyError::Overflow)?;
    }
    let two = Decimal::from(2);
    let mut rows = Vec::new();
    let (mut cgst, mut sgst, mut utgst, mut igst) =
        (Paise::ZERO, Paise::ZERO, Paise::ZERO, Paise::ZERO);
    for (rate, sums) in &by_rate {
        let half = rate.checked_div(two).ok_or(MoneyError::Overflow)?;
        let heads = [
            ("CGST", half, sums.cgst, &mut cgst),
            ("SGST", half, sums.sgst, &mut sgst),
            ("UTGST", half, sums.utgst, &mut utgst),
            ("IGST", *rate, sums.igst, &mut igst),
        ];
        for (head, head_rate, unrounded, total) in heads {
            if unrounded.is_zero() {
                continue;
            }
            let amount = round_rupees_to_paise(unrounded, RoundingRule::HalfUpToPaise)?;
            *total = total.checked_add(amount)?;
            rows.push(RateRow {
                head: head.to_owned(),
                rate_percent: head_rate,
                taxable: sums.taxable,
                amount,
            });
        }
    }
    let t = &breakup.totals;
    if cgst == t.cgst && sgst == t.sgst && utgst == t.utgst && igst == t.igst {
        Ok(rows)
    } else {
        Ok(Vec::new())
    }
}

/// The sales or purchase ledgers of an item invoice, with the sum of its inventory lines on each.
fn item_accounts(v: &Voucher) -> Result<Outcome<Vec<(LedgerId, Paise)>>, GstError> {
    let mut accounts: BTreeMap<LedgerId, Paise> = BTreeMap::new();
    for inventory in &v.inventory_lines {
        let Some(ledger) = inventory.ledger else {
            return Ok(Outcome::unsupported(
                "accounting lines for an inventory line without a ledger",
                "every line needs the sales or purchase ledger it posts to",
            ));
        };
        let sum = accounts.entry(ledger).or_insert(Paise::ZERO);
        *sum = sum.checked_add(inventory.amount)?;
    }
    Ok(Outcome::Value(accounts.into_iter().collect()))
}

/// The particulars of an accounting invoice, as typed, in order. Each must be on the tax side,
/// on a ledger with a GST classification, and neither the party nor a tax ledger: the party line
/// is the engine's taxable value plus tax, so a particular the engine does not tax would be left
/// out of it, and the tax lines are the engine's own. Their sum must be the engine's taxable value.
fn particulars(
    books: &Books,
    v: &Voucher,
    party: LedgerId,
    side: DrCr,
    breakup: &GstBreakup,
) -> Result<Outcome<Vec<(LedgerId, Paise)>>, GstError> {
    let refuse = |reason: String| {
        Ok(Outcome::unsupported(
            "accounting lines for this accounting invoice",
            reason,
        ))
    };
    if v.ledger_lines.is_empty() {
        return refuse("type at least one ledger under Particulars".to_owned());
    }
    let mut out = Vec::with_capacity(v.ledger_lines.len());
    let mut sum = Paise::ZERO;
    for l in &v.ledger_lines {
        let Some(ledger) = books.masters.ledgers.get(&l.ledger) else {
            return refuse(format!("ledger {} does not exist", l.ledger.0));
        };
        if l.ledger == party {
            return refuse(format!(
                "'{}' is the party; it is not a particular",
                ledger.name
            ));
        }
        if ledger.tax.is_some() {
            return refuse(format!(
                "'{}' is a tax ledger; the engine adds the tax lines itself",
                ledger.name
            ));
        }
        if ledger.gst.is_none() {
            return refuse(format!(
                "'{}' has no GST classification, so the engine does not tax it and the party line would leave it out; set its GST details in the ledger master",
                ledger.name
            ));
        }
        if l.amount.side != side {
            return refuse(format!(
                "'{}' is on the {:?} side; this kind of invoice posts its particulars on the {side:?} side",
                ledger.name, l.amount.side
            ));
        }
        sum = sum.checked_add(l.amount.amount)?;
        out.push((l.ledger, l.amount.amount));
    }
    if sum != breakup.taxable_total {
        return refuse(format!(
            "the particulars total Rs {} but the engine's taxable value is Rs {}",
            sum.to_rupees(),
            breakup.taxable_total.to_rupees()
        ));
    }
    Ok(Outcome::Value(out))
}

/// Every GST tax ledger of the masters.
pub(crate) fn all_tax_ledgers(books: &Books) -> Vec<TaxLedger> {
    books
        .masters
        .ledgers
        .values()
        .filter_map(|l| {
            let tax = l.tax?;
            is_gst_head(tax.kind).then_some(TaxLedger {
                id: l.id,
                kind: tax.kind,
                rate: tax.rate_percent,
            })
        })
        .collect()
}
