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
    Decimal, DrCr, LedgerId, MoneyError, Outcome, Paise, Rate, RoundingRule, SignedAmount,
    round_rupees_to_paise,
};
use ledgerly_engine::EngineError;
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{GstBreakup, LineTax, compute_voucher_tax};
use ledgerly_gst::pack::{RulePack, load_pack};
use ledgerly_model::{
    BaseVoucherKind, Books, GstLine, LedgerLine, StoredGst, TaxLedgerKind, Voucher, VoucherMode,
};

use crate::error::ApiError;
use crate::facade::Api;
use crate::gst::BUNDLED_RATE_PACK;

/// The accounting lines and the GST block of an item invoice, from the engine's tax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceGst {
    /// The engine's tax computation the rest is built from.
    pub breakup: GstBreakup,
    /// The invoice value: taxable value plus the tax heads (the round-off is zero here).
    pub invoice_total: Paise,
    /// The accounting lines: party, sales or purchase ledgers, tax ledgers. They balance.
    pub ledger_lines: Vec<LedgerLine>,
    /// The block to store on the voucher.
    pub stored: StoredGst,
}

/// A tax ledger of the masters or of a voucher.
struct TaxLedger {
    id: LedgerId,
    kind: TaxLedgerKind,
    rate: Option<Rate>,
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
        Ok(compute_for(books, v, &pack)?)
    }

    /// The accounting lines and stored GST block of an item invoice (a draft with inventory lines
    /// and no ledger lines), built from [`Api::voucher_gst`].
    ///
    /// # Errors
    /// As [`Api::voucher_gst`].
    pub fn invoice_gst(&self, v: &Voucher) -> Result<Outcome<InvoiceGst>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
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
        let pack = load_pack(BUNDLED_RATE_PACK)?;
        let breakup = match compute_for(books, v, &pack)? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(build_invoice(books, v, breakup)?)
    }
}

/// Runs the engine for `v` on the rule set of its time of supply.
fn compute_for(
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
fn tax_side(base: BaseVoucherKind) -> Option<DrCr> {
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
fn is_gst_head(kind: TaxLedgerKind) -> bool {
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

/// Builds the lines and block of an item invoice from the engine's tax.
fn build_invoice(
    books: &Books,
    v: &Voucher,
    breakup: GstBreakup,
) -> Result<Outcome<InvoiceGst>, GstError> {
    if !breakup.round_off.is_zero() {
        return Ok(Outcome::unsupported(
            "invoice round-off line",
            format!(
                "the engine rounds this invoice by Rs {}, and the masters have no invoice-rounding ledger to post it to (TallyPrime sets one with the ledger type Invoice Rounding); not posted",
                breakup.round_off.to_rupees()
            ),
        ));
    }
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
    let ledgers = all_tax_ledgers(books);
    let Some(stored) = stored_gst(&breakup, &ledgers)? else {
        return Ok(Outcome::unsupported(
            "choice of tax ledgers",
            "each tax head needs exactly one ledger of its kind (or one ledger of that kind with the head's rate); keep one CGST, SGST/UTGST and IGST ledger, or give rate-specific ledgers",
        ));
    };
    let mut tax_total = Paise::ZERO;
    for tax_line in &stored.lines {
        tax_total = tax_total.checked_add(tax_line.tax)?;
    }
    let invoice_total = breakup.taxable_total.checked_add(tax_total)?;
    let mut lines = vec![line(
        party,
        SignedAmount {
            amount: invoice_total,
            side: side.opposite(),
        },
    )];
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
    Ok(Outcome::Value(InvoiceGst {
        breakup,
        invoice_total,
        ledger_lines: lines,
        stored,
    }))
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
fn all_tax_ledgers(books: &Books) -> Vec<TaxLedger> {
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
