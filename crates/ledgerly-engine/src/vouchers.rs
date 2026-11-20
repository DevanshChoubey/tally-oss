//! Voucher post, alter, cancel and delete. **Card E3** implements this module.
//!
//! Frozen interface (plan card E3):
//!
//! ```text
//! pub fn post_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn alter_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn cancel_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! pub fn delete_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError>;
//! ```
//!
//! # The period lock
//!
//! [`Books`] carries no lock date (see the E5 note in [`crate::locks`]), and the frozen signatures
//! above take nothing else, so the four frozen functions cannot know whether the books are locked
//! and **check no lock** (they behave as `lock = None`). Every caller that holds a lock date must
//! use the `_with_lock` variants, which take the lock as an argument and apply invariant 8 (spec
//! section 5) with [`check_unlocked`]. An interface request asks for the lock to travel in
//! [`Books`] so the frozen functions can enforce it themselves.
//!
//! # What each command checks
//!
//! - **post**: the id is new; the date is unlocked; model validation (invariants 1, 2 and 6, see
//!   `ledgerly_model::validate`); the voucher type is active; bill references against the bills
//!   already in the books (invariant 2, via [`validate_bill_refs`]); the number (automatic types
//!   get one allocated by [`next_number`], manual types must bring one, both must be unique within
//!   their type, series and numbering period; GST sales invoices obey CGST Rules 2017, rule 46(b)).
//! - **alter**: the voucher exists, is not cancelled and keeps its type; both the stored date and
//!   the new date are unlocked; the same validation as post; a number or series change is refused
//!   for sales-side and `prevent_renumbering` types, allowed for others only if still unique; a
//!   new date with the same number must keep the number unique in its new numbering period
//!   (invariant 7); other vouchers that settle a bill this one opens must stay valid.
//! - **cancel**: the voucher exists, is not already cancelled and its date is unlocked. The number
//!   and series stay; lines and GST are dropped (see [`cancelled_form`]). A bill that other
//!   vouchers settle blocks the cancel.
//! - **delete**: the voucher exists and its date is unlocked. Sales-side types (`Sales`, and
//!   `CreditNote`/`DebitNote` of a GST company) and any type with `prevent_renumbering` are never
//!   deleted: [`EngineError::SalesSeriesNoDelete`], cancel instead (invariant 7). A bill that
//!   other vouchers settle blocks the delete.
//!
//! The 16-character GST number check applies to `Sales` only; credit and debit notes of a GST
//! company are sales-side for renumbering and delete, but are deliberately exempt from the check.
//! Posting a voucher whose status is already `Cancelled` is refused: use `cancel`.
//!
//! Not checked here, because other cards own them: GST tax recomputation (invariant 9), negative
//! stock and godown quantity sums, bank-reconciled instruments.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{Date, FinancialYear, LedgerId, Month, Outcome, VoucherId};
use ledgerly_model::validate::validate_voucher;
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, Books, NumberingMethod, RestartPeriod, Voucher, VoucherStatus,
    VoucherType,
};
use ledgerly_store::Event;

use crate::EngineError;
use crate::bills::validate_bill_refs;
use crate::locks::check_unlocked;
use crate::numbering::next_number;

/// Maximum length of a GST invoice serial number (CGST Rules 2017, rule 46(b)).
const GST_NUMBER_MAX: usize = 16;

type BillKey = (LedgerId, String);

fn invalid(field: &str, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn not_found(id: VoucherId) -> EngineError {
    EngineError::NotFound {
        what: format!("voucher {}", id.0),
    }
}

fn type_of<'a>(books: &'a Books, v: &Voucher) -> Result<&'a VoucherType, EngineError> {
    books
        .masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| EngineError::NotFound {
            what: format!("voucher type {}", v.type_id.0),
        })
}

/// Whether the base kind is a sales-side document whose number never changes and which is never
/// deleted. Credit and debit notes count only for a GST company, where they are GST documents.
fn is_sales_side(base: BaseVoucherKind, gst: bool) -> bool {
    match base {
        BaseVoucherKind::Sales => true,
        BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => gst,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Purchase
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

fn never_renumbered(vt: &VoucherType, books: &Books) -> bool {
    vt.numbering.prevent_renumbering || is_sales_side(vt.base, books.company.features.gst)
}

/// Whether two dates are in the same numbering period of `restart`.
fn same_period(
    restart: RestartPeriod,
    a: Date,
    b: Date,
    books: &Books,
) -> Result<bool, EngineError> {
    match restart {
        RestartPeriod::Never => Ok(true),
        RestartPeriod::Monthly => Ok(a.year() == b.year() && a.month() == b.month()),
        RestartPeriod::Yearly => {
            let fy_start = books.company.fy_start;
            if fy_start.month() != Month::April || fy_start.day() != 1 {
                return Err(invalid(
                    "numbering",
                    "yearly restart is only supported for financial years starting on 1 April",
                ));
            }
            let fa = FinancialYear::containing(a)
                .map_err(|_| invalid("date", "date is outside the supported calendar range"))?;
            let fb = FinancialYear::containing(b)
                .map_err(|_| invalid("date", "date is outside the supported calendar range"))?;
            Ok(fa == fb)
        }
    }
}

/// The lock check for every date a command touches.
fn check_dates(dates: &[Date], lock: Option<Date>) -> Result<(), EngineError> {
    for date in dates {
        check_unlocked(*date, lock)?;
    }
    Ok(())
}

/// The series must be one the type configures (or none when it configures none).
fn check_series(vt: &VoucherType, series: Option<&str>) -> Result<(), EngineError> {
    match series {
        Some(s) if !vt.numbering.series.iter().any(|c| c == s) => Err(EngineError::NotFound {
            what: format!("series '{s}' of voucher type '{}'", vt.name),
        }),
        None if !vt.numbering.series.is_empty() => Err(invalid(
            "series",
            format!("'{}' has several series; choose one", vt.name),
        )),
        Some(_) | None => Ok(()),
    }
}

/// CGST Rules 2017, rule 46(b): at most 16 characters of letters, digits, hyphen and slash.
///
/// This must match the private `check_gst_number` in `numbering.rs` (card E4); it is copied because
/// that one is private (interface request: make it `pub(crate)`). Only `Sales` invoices are
/// checked: credit and debit notes are numbered by their own series and are exempt here by choice.
fn check_gst_number(number: &str) -> Result<(), EngineError> {
    if !number
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/')
    {
        return Err(invalid(
            "number",
            format!(
                "'{number}' has characters a GST invoice number cannot contain; \
                 use only letters, digits, hyphen and slash (CGST Rules, rule 46(b))"
            ),
        ));
    }
    if number.chars().count() > GST_NUMBER_MAX {
        return Err(EngineError::NumberTooLong {
            number: number.to_owned(),
            max: GST_NUMBER_MAX,
        });
    }
    Ok(())
}

/// Refuses a number already used by another voucher of the same type, series and period.
fn check_unique(
    v: &Voucher,
    number: &str,
    vt: &VoucherType,
    books: &Books,
) -> Result<(), EngineError> {
    for other in books.vouchers.values() {
        if other.id == v.id
            || other.type_id != v.type_id
            || other.series != v.series
            || other.number.as_deref() != Some(number)
        {
            continue;
        }
        if same_period(vt.numbering.restart, other.date, v.date, books)? {
            return Err(EngineError::Duplicate {
                what: format!("voucher number '{number}' of '{}'", vt.name),
            });
        }
    }
    Ok(())
}

/// Checks a number that is already on the voucher.
fn check_given_number(v: &Voucher, vt: &VoucherType, books: &Books) -> Result<(), EngineError> {
    let Some(number) = v.number.as_deref() else {
        return Ok(());
    };
    match vt.numbering.method {
        NumberingMethod::Automatic | NumberingMethod::Manual => {}
        NumberingMethod::None => {
            return Err(invalid(
                "number",
                format!(
                    "'{}' does not use voucher numbers; remove the number",
                    vt.name
                ),
            ));
        }
    }
    if number.is_empty() {
        return Err(invalid(
            "number",
            "the number is empty; type one or leave it out",
        ));
    }
    check_series(vt, v.series.as_deref())?;
    if books.company.features.gst && vt.base == BaseVoucherKind::Sales {
        check_gst_number(number)?;
    }
    check_unique(v, number, vt, books)
}

/// Gives a new voucher its number: allocated for automatic types, required for manual ones.
fn assign_number(mut v: Voucher, vt: &VoucherType, books: &Books) -> Result<Voucher, EngineError> {
    match (vt.numbering.method, v.number.is_some()) {
        (NumberingMethod::Automatic, false) => {
            v.number = Some(next_number(vt, v.series.as_deref(), v.date, books)?);
        }
        (NumberingMethod::Manual, false) => {
            return Err(invalid(
                "number",
                format!("'{}' uses manual numbering; type the number in", vt.name),
            ));
        }
        (NumberingMethod::None, false)
        | (NumberingMethod::Automatic | NumberingMethod::Manual | NumberingMethod::None, true) => {}
    }
    check_given_number(&v, vt, books)?;
    Ok(v)
}

/// The (ledger, bill name) pairs a voucher opens.
fn opened_bills(v: &Voucher) -> BTreeSet<BillKey> {
    let mut keys = BTreeSet::new();
    for line in &v.ledger_lines {
        for a in &line.bills {
            match a.kind {
                BillRefKind::New | BillRefKind::Advance => {
                    keys.insert((line.ledger, a.name.clone()));
                }
                BillRefKind::Against | BillRefKind::OnAccount => {}
            }
        }
    }
    keys
}

/// Whether `v` settles (`Against`) any of the bills in `keys`.
fn settles_any(v: &Voucher, keys: &BTreeSet<BillKey>) -> bool {
    v.ledger_lines.iter().any(|line| {
        line.bills.iter().any(|a| {
            let against = match a.kind {
                BillRefKind::Against => true,
                BillRefKind::New | BillRefKind::Advance | BillRefKind::OnAccount => false,
            };
            against && keys.contains(&(line.ledger, a.name.clone()))
        })
    })
}

/// The live vouchers, other than `changed`, that settle a bill `changed` or `after` opens.
fn dependents_of<'a>(
    books: &'a Books,
    changed: &Voucher,
    after: Option<&Voucher>,
) -> Vec<&'a Voucher> {
    let mut keys = opened_bills(changed);
    if let Some(a) = after {
        keys.extend(opened_bills(a));
    }
    if keys.is_empty() {
        return Vec::new();
    }
    books
        .vouchers
        .values()
        .filter(|w| w.id != changed.id && w.status != VoucherStatus::Cancelled)
        .filter(|w| settles_any(w, &keys))
        .collect()
}

/// A copy of `books` for bill checks only: `changed` is replaced by `after` (or removed when
/// `None`) and the vouchers in `omit` are removed. Postings and stock moves are not needed by
/// bill validation and are left empty.
fn hypothetical(
    books: &Books,
    changed: VoucherId,
    after: Option<&Voucher>,
    omit: &[VoucherId],
) -> Books {
    let mut vouchers: BTreeMap<VoucherId, Voucher> = books.vouchers.clone();
    match after {
        Some(a) => {
            vouchers.insert(a.id, a.clone());
        }
        None => {
            vouchers.remove(&changed);
        }
    }
    for id in omit {
        vouchers.remove(id);
    }
    Books {
        company: books.company.clone(),
        masters: books.masters.clone(),
        vouchers,
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

/// Bill checks for an altered voucher `after` that replaces `before`.
///
/// [`validate_bill_refs`] leaves the checked voucher out of its projection, so a voucher that
/// opens a bill other vouchers settle cannot be checked against books that hold those vouchers
/// (their settlements would point at a bill the projection no longer has). Such a voucher is
/// checked against the books without its dependents; the dependents are then checked, in
/// [`recheck_settlements`], against the books with the alteration applied.
fn validate_altered_bills(
    books: &Books,
    before: &Voucher,
    after: &Voucher,
) -> Result<(), EngineError> {
    let dependents = dependents_of(books, before, Some(after));
    if dependents.is_empty() {
        return validate_bill_refs(after, books);
    }
    let ids: Vec<VoucherId> = dependents.iter().map(|w| w.id).collect();
    let without = hypothetical(books, after.id, None, &ids);
    validate_bill_refs(after, &without)
}

/// After `changed` is replaced by `after` (or removed, when `None`), every other voucher that
/// settles a bill `changed` opened must still be valid. Without this, altering, cancelling or
/// deleting an invoice would leave receipts pointing at a bill that is gone or too small.
fn recheck_settlements(
    books: &Books,
    changed: &Voucher,
    after: Option<&Voucher>,
) -> Result<(), EngineError> {
    let dependents = dependents_of(books, changed, after);
    if dependents.is_empty() {
        return Ok(());
    }
    let after_books = hypothetical(books, changed.id, after, &[]);
    for w in dependents {
        validate_bill_refs(w, &after_books).map_err(|e| {
            invalid(
                &format!("voucher {}", w.id.0),
                format!(
                    "this change would break voucher {} that settles the same bill ({e}); \
                     alter or cancel that voucher first",
                    w.id.0
                ),
            )
        })?;
    }
    Ok(())
}

/// The state of a cancelled voucher: number, series, date, party and narration stay; lines and
/// GST are removed (spec section 5: cancelled, number kept, no amounts).
///
/// The store applies a `VoucherCancelled { before }` event by replacing the stored voucher with
/// this value.
pub fn cancelled_form(before: &Voucher) -> Voucher {
    let mut after = before.clone();
    after.status = VoucherStatus::Cancelled;
    after.ledger_lines = Vec::new();
    after.inventory_lines = Vec::new();
    after.gst = None;
    after
}

/// Posts a new voucher, with no period lock check. See the module note; prefer
/// [`post_voucher_with_lock`] whenever a lock date is known.
///
/// # Errors
/// As [`post_voucher_with_lock`], except [`EngineError::PeriodLocked`] is never returned.
pub fn post_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    post_voucher_with_lock(v, books, None)
}

/// Posts a new voucher (spec section 5, invariants 1, 2, 6, 7 and 8). Returns one
/// [`Event::VoucherPosted`] carrying the voucher with its allocated number.
///
/// # Errors
/// - [`EngineError::PeriodLocked`] when the date is on or before `lock`.
/// - [`EngineError::Duplicate`] when the id or the number is already used.
/// - [`EngineError::Voucher`] for a model invariant (unbalanced, unknown ledger, ...).
/// - [`EngineError::Invalid`] for a voucher already marked cancelled, an inactive type, or a
///   missing, unexpected or malformed number.
/// - [`EngineError::NumberTooLong`] for a GST sales invoice number over 16 characters.
/// - Any error of bill reference validation ([`validate_bill_refs`]) or number allocation.
pub fn post_voucher_with_lock(
    v: Voucher,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if books.vouchers.contains_key(&v.id) {
        return Err(EngineError::Duplicate {
            what: format!("voucher {}", v.id.0),
        });
    }
    check_dates(&[v.date], lock)?;
    match v.status {
        VoucherStatus::Cancelled => {
            return Err(invalid(
                "status",
                "a voucher is cancelled with cancel, not posted as cancelled",
            ));
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    validate_voucher(&v, &books.masters, &books.company)?;
    let vt = type_of(books, &v)?;
    if !vt.active {
        return Err(invalid(
            "voucher type",
            format!(
                "'{}' is not active; activate it or choose another type",
                vt.name
            ),
        ));
    }
    let v = assign_number(v, vt, books)?;
    validate_bill_refs(&v, books)?;
    Ok(Outcome::Value(vec![Event::VoucherPosted(v)]))
}

/// Alters a stored voucher, with no period lock check. See the module note; prefer
/// [`alter_voucher_with_lock`] whenever a lock date is known.
///
/// # Errors
/// As [`alter_voucher_with_lock`], except [`EngineError::PeriodLocked`] is never returned.
pub fn alter_voucher(v: Voucher, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    alter_voucher_with_lock(v, books, None)
}

/// A number or series change on an altered voucher.
fn check_renumbering(
    before: &Voucher,
    v: &Voucher,
    vt: &VoucherType,
    books: &Books,
) -> Result<(), EngineError> {
    if before.number == v.number && before.series == v.series {
        // Same number, but a new date can move the voucher into a numbering period (Monthly or
        // Yearly restart) that already uses it (spec section 5, invariant 7). A redate is allowed
        // for every type, sales-side included, when the number is still unique there.
        if before.date != v.date
            && let Some(number) = v.number.as_deref()
        {
            return check_unique(v, number, vt, books);
        }
        return Ok(());
    }
    if never_renumbered(vt, books) {
        return Err(invalid(
            "number",
            format!(
                "'{}' vouchers keep their number and series; cancel this voucher and post a new one",
                vt.name
            ),
        ));
    }
    if before.number.is_some() && v.number.is_none() {
        return Err(invalid(
            "number",
            "a numbered voucher cannot lose its number",
        ));
    }
    check_given_number(v, vt, books)
}

/// The status rules of an alteration: a cancelled voucher stays as it is, and cancelling goes
/// through [`cancel_voucher`].
fn check_alter_status(before: VoucherStatus, after: VoucherStatus) -> Result<(), EngineError> {
    match before {
        VoucherStatus::Cancelled => {
            return Err(invalid(
                "status",
                "a cancelled voucher cannot be altered; post a new voucher instead",
            ));
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    match after {
        VoucherStatus::Cancelled => Err(invalid(
            "status",
            "use cancel to cancel a voucher; an alteration keeps it live",
        )),
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => Ok(()),
    }
}

/// Alters a stored voucher (matched by `v.id`). Returns one [`Event::VoucherAltered`] holding the
/// stored voucher as `before` and `v` as `after`, or no events when nothing changed.
///
/// # Errors
/// - [`EngineError::NotFound`] when no voucher has this id.
/// - [`EngineError::PeriodLocked`] when the stored date or the new date is on or before `lock`.
/// - [`EngineError::Invalid`] when the voucher is cancelled, changes type, is being cancelled
///   through an alteration, or renumbers when it may not.
/// - The model, number and bill errors listed for [`post_voucher_with_lock`].
pub fn alter_voucher_with_lock(
    v: Voucher,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books.vouchers.get(&v.id).ok_or_else(|| not_found(v.id))?;
    check_dates(&[before.date, v.date], lock)?;
    check_alter_status(before.status, v.status)?;
    if before.type_id != v.type_id {
        return Err(invalid(
            "voucher type",
            "the type of a posted voucher cannot change; cancel it and post a new one",
        ));
    }
    validate_voucher(&v, &books.masters, &books.company)?;
    let vt = type_of(books, &v)?;
    check_renumbering(before, &v, vt, books)?;
    validate_altered_bills(books, before, &v)?;
    recheck_settlements(books, before, Some(&v))?;
    if *before == v {
        return Ok(Outcome::Value(Vec::new()));
    }
    Ok(Outcome::Value(vec![Event::VoucherAltered {
        before: before.clone(),
        after: v,
    }]))
}

/// Cancels a voucher, with no period lock check. See the module note; prefer
/// [`cancel_voucher_with_lock`] whenever a lock date is known.
///
/// # Errors
/// As [`cancel_voucher_with_lock`], except [`EngineError::PeriodLocked`] is never returned.
pub fn cancel_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    cancel_voucher_with_lock(id, books, None)
}

/// Cancels a voucher: it keeps its number and loses its amounts (see [`cancelled_form`]).
/// Returns one [`Event::VoucherCancelled`] holding the stored voucher.
///
/// # Errors
/// - [`EngineError::NotFound`] when no voucher has this id.
/// - [`EngineError::PeriodLocked`] when its date is on or before `lock`.
/// - [`EngineError::Invalid`] when it is already cancelled, or when another voucher settles a bill
///   it opens.
/// - [`EngineError::Voucher`] when the cancelled form fails model validation.
pub fn cancel_voucher_with_lock(
    id: VoucherId,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books.vouchers.get(&id).ok_or_else(|| not_found(id))?;
    check_dates(&[before.date], lock)?;
    match before.status {
        VoucherStatus::Cancelled => {
            return Err(invalid("status", "the voucher is already cancelled"));
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    let after = cancelled_form(before);
    validate_voucher(&after, &books.masters, &books.company)?;
    recheck_settlements(books, before, Some(&after))?;
    Ok(Outcome::Value(vec![Event::VoucherCancelled {
        before: before.clone(),
    }]))
}

/// Deletes a voucher, with no period lock check. See the module note; prefer
/// [`delete_voucher_with_lock`] whenever a lock date is known.
///
/// # Errors
/// As [`delete_voucher_with_lock`], except [`EngineError::PeriodLocked`] is never returned.
pub fn delete_voucher(id: VoucherId, books: &Books) -> Result<Outcome<Vec<Event>>, EngineError> {
    delete_voucher_with_lock(id, books, None)
}

/// Deletes a voucher (spec section 5, invariants 7 and 8). Returns one [`Event::VoucherDeleted`]
/// holding the stored voucher.
///
/// # Errors
/// - [`EngineError::NotFound`] when no voucher has this id.
/// - [`EngineError::PeriodLocked`] when its date is on or before `lock`.
/// - [`EngineError::SalesSeriesNoDelete`] for sales-side vouchers and types that never renumber;
///   cancel them instead. A cancelled one is still not deletable: its number stays used.
/// - [`EngineError::Invalid`] when another voucher settles a bill it opens.
pub fn delete_voucher_with_lock(
    id: VoucherId,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books.vouchers.get(&id).ok_or_else(|| not_found(id))?;
    check_dates(&[before.date], lock)?;
    let vt = type_of(books, before)?;
    if never_renumbered(vt, books) {
        return Err(EngineError::SalesSeriesNoDelete);
    }
    recheck_settlements(books, before, None)?;
    Ok(Outcome::Value(vec![Event::VoucherDeleted {
        before: before.clone(),
    }]))
}
