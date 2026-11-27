//! Bill-wise allocation state and validation. **Card E7** implements this module.
//!
//! Frozen interface (plan card E7):
//!
//! ```text
//! pub struct OpenBill { pub ledger: LedgerId, pub name: String, pub date: Date, pub due: Option<Date>, pub open: SignedAmount }
//! pub fn open_bills(books: &Books, ledger: LedgerId, as_of: Date) -> Result<Vec<OpenBill>, EngineError>;
//! pub fn validate_bill_refs(v: &Voucher, books: &Books) -> Result<(), EngineError>;
//! ```
//!
//! # Model
//!
//! A bill is identified by `(ledger, name)`. Its open amount is the signed sum (Dr positive, Cr
//! negative) of the `New`, `Advance` and `Against` allocations carrying that name on that
//! ledger. `OnAccount` allocations are not linked to a bill and are ignored. A bill whose open
//! amount is zero is closed and is not listed.
//!
//! Decisions the model leaves open, and what this module does about each:
//!
//! - **Which vouchers count** (spec section 5, invariant 3): `Regular` vouchers, and `PostDated`
//!   ones on or before the as-of date. `Optional` and `Cancelled` never count. Memorandum and
//!   reversing journals never count. Order, note and stock kinds do not post to ledgers; one that
//!   carries bill allocations is an error, not silently ignored.
//! - **Bill date and due date**: the bill is dated by its `New` allocation's voucher (else by the
//!   earliest `Advance`). A credit period counts from that voucher date. When both a due date and
//!   credit days are given they must agree, otherwise the allocation is an error. The ledger's
//!   `default_credit_days` is an entry-time default and is *not* applied here, so editing the
//!   master never moves the due date of an existing bill.
//! - **Same name twice**: two `New` allocations with one name on one ledger are a
//!   [`EngineError::Duplicate`]. An `Advance` and a `New` of one name form a single bill.
//! - **Opening balances**: the model has no opening bill breakup. A bill-wise ledger with a
//!   non-zero opening balance therefore cannot be projected and `open_bills` returns an error
//!   instead of a list that omits it.
//! - **Order**: output is sorted by `(date, name)`; voucher application order is `(date, id)`.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{Date, LedgerId, Paise, SignedAmount, VoucherId};
use ledgerly_model::{
    BaseVoucherKind, BillAllocation, BillRefKind, Books, Voucher, VoucherError, VoucherStatus,
};

use crate::EngineError;

/// One open bill of a ledger as of a date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenBill {
    /// The party ledger the bill is on.
    pub ledger: LedgerId,
    /// Bill name (usually the invoice number).
    pub name: String,
    /// Bill date: the date of the voucher that opened it.
    pub date: Date,
    /// Due date, when the opening allocation gave a due date or credit days.
    pub due: Option<Date>,
    /// Amount still open, with its side. Never zero.
    pub open: SignedAmount,
}

type BillKey = (LedgerId, String);

/// Rank of an opening allocation; the lowest rank (then earliest date) dates the bill.
const RANK_NEW: u8 = 0;
const RANK_ADVANCE: u8 = 1;

#[derive(Debug, Clone, Copy)]
struct Origin {
    rank: u8,
    date: Date,
    due: Option<Date>,
}

#[derive(Debug, Default)]
struct BillState {
    origin: Option<Origin>,
    has_new: bool,
    /// Signed open amount, Dr positive.
    open: Paise,
}

/// Which vouchers a projection includes.
#[derive(Debug, Clone, Copy)]
struct Horizon {
    /// Regular vouchers dated after this are left out; `None` includes all.
    regular_until: Option<Date>,
    /// Post-dated vouchers dated after this are left out.
    post_dated_until: Date,
}

struct Ctx<'a> {
    books: &'a Books,
    /// A voucher being altered: its stored allocations are left out.
    exclude: Option<VoucherId>,
    /// Only allocations on these ledgers are projected.
    ledgers: &'a BTreeSet<LedgerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindEffect {
    /// Posts to ledgers and can carry bill allocations.
    Posts,
    /// Never in the books (spec section 5, invariant 3).
    Excluded,
    /// Does not post to ledgers; bill allocations make no sense.
    NoBills,
}

fn kind_effect(kind: BaseVoucherKind) -> KindEffect {
    match kind {
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => KindEffect::Posts,
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => KindEffect::Excluded,
        BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => KindEffect::NoBills,
    }
}

fn invalid(field: impl Into<String>, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.into(),
        reason: reason.into(),
    }
}

fn effect_of(books: &Books, v: &Voucher) -> Result<KindEffect, EngineError> {
    let vt = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .ok_or(VoucherError::UnknownVoucherType(v.type_id))?;
    let effect = kind_effect(vt.base);
    let has_bills = v.ledger_lines.iter().any(|l| !l.bills.is_empty());
    if effect == KindEffect::NoBills && has_bills {
        return Err(invalid(
            format!("voucher type {}", vt.name),
            "this voucher type does not post to ledgers, so bill allocations cannot be applied; \
             remove them",
        ));
    }
    Ok(effect)
}

fn voucher_counts(books: &Books, v: &Voucher, h: Horizon) -> Result<bool, EngineError> {
    let in_time = match v.status {
        VoucherStatus::Regular => h.regular_until.is_none_or(|d| v.date <= d),
        VoucherStatus::PostDated => v.date <= h.post_dated_until,
        VoucherStatus::Optional | VoucherStatus::Cancelled => return Ok(false),
    };
    if !in_time {
        return Ok(false);
    }
    Ok(effect_of(books, v)? == KindEffect::Posts)
}

fn signed_amount(a: &BillAllocation) -> Result<Paise, EngineError> {
    if a.amount.amount.is_negative() {
        return Err(invalid(
            format!("bill '{}'", a.name),
            "allocation amount is negative; give a positive amount with a Dr/Cr side",
        ));
    }
    if a.name.trim().is_empty() {
        return Err(invalid(
            "bill name",
            "a bill reference needs a name; use On Account for an unlinked amount",
        ));
    }
    Ok(a.amount.to_signed()?)
}

fn add_days(date: Date, days: u32, bill: &str) -> Result<Date, EngineError> {
    date.checked_add(time::Duration::days(i64::from(days)))
        .ok_or_else(|| {
            invalid(
                format!("bill '{bill}'"),
                format!("{days} credit days after {date} is outside the supported calendar"),
            )
        })
}

fn resolve_due(date: Date, a: &BillAllocation) -> Result<Option<Date>, EngineError> {
    match (a.due_date, a.credit_days) {
        (None, None) => Ok(None),
        (Some(due), None) => Ok(Some(due)),
        (None, Some(days)) => add_days(date, days, &a.name).map(Some),
        (Some(due), Some(days)) => {
            let computed = add_days(date, days, &a.name)?;
            if computed == due {
                Ok(Some(due))
            } else {
                Err(invalid(
                    format!("bill '{}'", a.name),
                    format!(
                        "due date {due} disagrees with {days} credit days from {date} ({computed}); \
                         keep one of them"
                    ),
                ))
            }
        }
    }
}

/// `ledger 'Name'`, as the user knows the ledger; its id only when the books do not hold it.
fn ledger_label(books: &Books, ledger: LedgerId) -> String {
    books.masters.ledgers.get(&ledger).map_or_else(
        || format!("ledger {}", ledger.0),
        |l| format!("ledger '{}'", l.name),
    )
}

fn open_bill(
    books: &Books,
    map: &mut BTreeMap<BillKey, BillState>,
    ledger: LedgerId,
    date: Date,
    a: &BillAllocation,
    rank: u8,
) -> Result<(), EngineError> {
    let signed = signed_amount(a)?;
    let due = resolve_due(date, a)?;
    let st = map.entry((ledger, a.name.clone())).or_default();
    if a.kind == BillRefKind::New {
        if st.has_new {
            return Err(EngineError::Duplicate {
                what: format!("bill '{}' on {}", a.name, ledger_label(books, ledger)),
            });
        }
        st.has_new = true;
    }
    if st.origin.is_none_or(|o| (rank, date) < (o.rank, o.date)) {
        st.origin = Some(Origin { rank, date, due });
    }
    st.open = st.open.checked_add(signed)?;
    Ok(())
}

/// The error for an Against reference to a bill that is not on `ledger`.
fn missing_bill(ctx: &Ctx<'_>, ledger: LedgerId, name: &str) -> EngineError {
    let elsewhere = ctx
        .books
        .vouchers
        .values()
        .filter(|v| Some(v.id) != ctx.exclude)
        .flat_map(|v| &v.ledger_lines)
        .filter(|l| l.ledger != ledger)
        .flat_map(|l| &l.bills)
        .any(|b| b.name == name && matches!(b.kind, BillRefKind::New | BillRefKind::Advance));
    if elsewhere {
        VoucherError::BillOnWrongLedger {
            bill: name.to_string(),
            ledger,
        }
        .into()
    } else {
        EngineError::NotFound {
            what: format!("bill '{name}' on {}", ledger_label(ctx.books, ledger)),
        }
    }
}

fn settle(
    ctx: &Ctx<'_>,
    map: &mut BTreeMap<BillKey, BillState>,
    ledger: LedgerId,
    a: &BillAllocation,
) -> Result<(), EngineError> {
    let signed = signed_amount(a)?;
    let settling = a.amount.amount;
    if settling.is_zero() {
        return Err(invalid(
            format!("bill '{}'", a.name),
            "a settlement of zero changes nothing; remove it",
        ));
    }
    let Some(st) = map.get_mut(&(ledger, a.name.clone())) else {
        return Err(missing_bill(ctx, ledger, &a.name));
    };
    if st.open.is_zero() {
        return Err(EngineError::OverSettled {
            bill: a.name.clone(),
            open: Paise::ZERO,
            settling,
        });
    }
    if st.open.is_negative() == signed.is_negative() {
        return Err(invalid(
            format!("bill '{}'", a.name),
            "an Against reference must be on the opposite side (Dr/Cr) of the bill it settles",
        ));
    }
    let open = st.open.checked_abs()?;
    if settling > open {
        return Err(EngineError::OverSettled {
            bill: a.name.clone(),
            open,
            settling,
        });
    }
    st.open = st.open.checked_add(signed)?;
    Ok(())
}

fn apply_origins(
    ctx: &Ctx<'_>,
    map: &mut BTreeMap<BillKey, BillState>,
    v: &Voucher,
) -> Result<(), EngineError> {
    for line in v
        .ledger_lines
        .iter()
        .filter(|l| ctx.ledgers.contains(&l.ledger))
    {
        for a in &line.bills {
            match a.kind {
                BillRefKind::New => open_bill(ctx.books, map, line.ledger, v.date, a, RANK_NEW)?,
                BillRefKind::Advance => {
                    open_bill(ctx.books, map, line.ledger, v.date, a, RANK_ADVANCE)?;
                }
                BillRefKind::Against | BillRefKind::OnAccount => {}
            }
        }
    }
    Ok(())
}

fn apply_settlements(
    ctx: &Ctx<'_>,
    map: &mut BTreeMap<BillKey, BillState>,
    v: &Voucher,
) -> Result<(), EngineError> {
    for line in v
        .ledger_lines
        .iter()
        .filter(|l| ctx.ledgers.contains(&l.ledger))
    {
        for a in &line.bills {
            match a.kind {
                BillRefKind::Against => settle(ctx, map, line.ledger, a)?,
                BillRefKind::New | BillRefKind::Advance | BillRefKind::OnAccount => {}
            }
        }
    }
    Ok(())
}

/// Projects every counted voucher: all origins first (so order of entry cannot matter), then
/// all settlements in `(date, id)` order.
fn project(ctx: &Ctx<'_>, h: Horizon) -> Result<BTreeMap<BillKey, BillState>, EngineError> {
    let mut counted: Vec<&Voucher> = Vec::new();
    for v in ctx.books.vouchers.values() {
        if Some(v.id) != ctx.exclude && voucher_counts(ctx.books, v, h)? {
            counted.push(v);
        }
    }
    counted.sort_by_key(|v| (v.date, v.id));
    let mut map = BTreeMap::new();
    for v in &counted {
        apply_origins(ctx, &mut map, v)?;
    }
    for v in &counted {
        apply_settlements(ctx, &mut map, v)?;
    }
    Ok(map)
}

/// The bills of `ledger` with a non-zero open amount as of `as_of`, sorted by `(date, name)`.
///
/// A ledger without bill-wise tracking has no bills. Counted vouchers are listed in the module
/// documentation.
///
/// # Errors
/// - [`EngineError::NotFound`] when the ledger or a settled bill does not exist.
/// - [`EngineError::Invalid`] when a bill-wise ledger has an opening balance (the model has no
///   opening bill breakup), when a due date and credit days disagree, or on malformed
///   allocations.
/// - [`EngineError::Duplicate`] for two `New` allocations of one name.
/// - [`EngineError::OverSettled`] when recorded settlements exceed a bill.
/// - [`EngineError::Money`] on overflow.
pub fn open_bills(
    books: &Books,
    ledger: LedgerId,
    as_of: Date,
) -> Result<Vec<OpenBill>, EngineError> {
    let master = books
        .masters
        .ledgers
        .get(&ledger)
        .ok_or_else(|| EngineError::NotFound {
            what: format!("ledger {}", ledger.0),
        })?;
    if master.bill_wise.is_none() {
        return Ok(Vec::new());
    }
    if !master.opening.amount.is_zero() {
        return Err(invalid(
            format!("ledger '{}' opening balance", master.name),
            "the opening balance has no bill-by-bill breakup, so open bills would omit it; \
             this is not supported yet",
        ));
    }
    let ledgers = BTreeSet::from([ledger]);
    let ctx = Ctx {
        books,
        exclude: None,
        ledgers: &ledgers,
    };
    let horizon = Horizon {
        regular_until: Some(as_of),
        post_dated_until: as_of,
    };
    let mut out = Vec::new();
    for ((_, name), st) in project(&ctx, horizon)? {
        if st.open.is_zero() {
            continue;
        }
        let origin = st
            .origin
            .ok_or_else(|| invalid(format!("bill '{name}'"), "bill has no opening allocation"))?;
        out.push(OpenBill {
            ledger,
            name,
            date: origin.date,
            due: origin.due,
            open: SignedAmount::from_signed(st.open)?,
        });
    }
    out.sort_by(|a, b| (a.date, &a.name).cmp(&(b.date, &b.name)));
    Ok(out)
}

/// Checks the bill references on `v` against the bills already in `books` (spec section 5,
/// invariant 2): every `Against` must name an existing bill on the same ledger, on the opposite
/// side, and must not settle more than is open. Settlements are checked cumulatively, so two
/// lines of one voucher cannot together over-settle. If `v` is already stored in `books`
/// (an alteration), its stored allocations are ignored. Settlements are checked against every
/// counted voucher regardless of date, so a back-dated entry cannot over-settle a bill that a
/// later voucher already settled.
///
/// That allocations add up to their line is checked by model validation, not here. A cancelled
/// voucher carries no amounts and is accepted.
///
/// # Errors
/// - [`EngineError::OverSettled`] when a settlement exceeds the open amount (including a closed
///   bill, open amount zero).
/// - [`EngineError::Voucher`] with [`VoucherError::BillOnWrongLedger`],
///   [`VoucherError::UnknownLedger`] or [`VoucherError::UnknownVoucherType`].
/// - [`EngineError::NotFound`] when the named bill does not exist anywhere.
/// - [`EngineError::Duplicate`] for a `New` whose name is already a bill on the ledger.
/// - [`EngineError::Invalid`] for allocations on a ledger that is not bill-wise, an `Against` on
///   the same side as the bill, a zero or negative amount, or a due date that contradicts the
///   credit days.
pub fn validate_bill_refs(v: &Voucher, books: &Books) -> Result<(), EngineError> {
    match v.status {
        VoucherStatus::Cancelled => return Ok(()),
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    let mut touched = BTreeSet::new();
    for line in v.ledger_lines.iter().filter(|l| !l.bills.is_empty()) {
        let master = books
            .masters
            .ledgers
            .get(&line.ledger)
            .ok_or(VoucherError::UnknownLedger(line.ledger))?;
        if master.bill_wise.is_none() {
            return Err(invalid(
                format!("ledger '{}'", master.name),
                "bill references need a bill-wise ledger; enable bill-wise tracking on it first",
            ));
        }
        touched.insert(line.ledger);
    }
    if touched.is_empty() {
        return Ok(());
    }
    effect_of(books, v)?;
    let ctx = Ctx {
        books,
        exclude: Some(v.id),
        ledgers: &touched,
    };
    let horizon = Horizon {
        regular_until: None,
        post_dated_until: v.date,
    };
    let mut map = project(&ctx, horizon)?;
    apply_origins(&ctx, &mut map, v)?;
    apply_settlements(&ctx, &mut map, v)
}
