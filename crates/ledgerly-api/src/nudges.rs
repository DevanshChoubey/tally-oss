//! Nudges for the Home screen: things the books and the GST calendar say need a look.
//!
//! Each nudge is raised by a named rule, never by a model:
//!
//! * the GSTR-1 due date (the GST calendar);
//! * possible duplicate payments (rule D-02, our own rule, described on [`Api::home_nudges`]).
//!
//! A figure or a flag is worked out here, so the screen only words and draws it (rule C4).

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    Date, DrCr, Gstin, LedgerId, Month, Outcome, Paise, Period, Unsupported, VoucherId,
};
use ledgerly_gst::gstr1::Bucket;
use ledgerly_model::{BaseVoucherKind, BillRefKind, Books, Voucher, VoucherStatus};

use crate::error::ApiError;
use crate::facade::Api;

/// The day of the month the monthly GSTR-1 falls due, in the month after the tax period.
/// CBIC notification 83/2020-Central Tax of 10 Nov 2020, under section 37 of the CGST Act.
const GSTR1_DUE_DAY: u8 = 11;
/// Two payments are a possible duplicate only when this many days or fewer apart (rule D-02).
const DUPLICATE_WINDOW_DAYS: i64 = 30;

/// The GSTR-1 for the return period that is next due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1Due {
    /// The month the return is for.
    pub period: Period,
    /// The day it is due.
    pub due: Date,
    /// How many vouchers of the month are not settled in the return (need correction or do not
    /// agree with themselves), or why that cannot be counted.
    pub needs_correction: Outcome<usize>,
}

/// One payment voucher in a possible duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentRef {
    /// The voucher.
    pub id: VoucherId,
    /// Its number, when it has one.
    pub number: Option<String>,
    /// Its date.
    pub date: Date,
    /// `true` when the date is on or before the lock date, so a correction goes in a later month.
    pub locked: bool,
    /// The date the bank cleared the payment, from a bank line a user confirmed against a
    /// statement; `None` when no line of the payment has been matched.
    pub bank_date: Option<Date>,
}

/// Two payments rule D-02 flags: same ledger, same amount, same narration, close in date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicatePayment {
    /// The earlier payment.
    pub first: PaymentRef,
    /// The later payment.
    pub second: PaymentRef,
    /// Name of the ledger both pay.
    pub ledger: String,
    /// Name of the party both vouchers name, when both name the same one.
    pub payee: Option<String>,
    /// The amount of each.
    pub amount: Paise,
    /// The narration both carry.
    pub narration: String,
    /// Whole days from the first to the second.
    pub days_apart: i64,
}

/// What the Home screen's nudges are built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeNudges {
    /// The GSTR-1 next due; `None` when the company has no single regular GST registration.
    pub gstr1: Option<Gstr1Due>,
    /// The possible duplicate payments, oldest first.
    pub duplicates: Outcome<Vec<DuplicatePayment>>,
}

/// The monthly GSTR-1 return period that is next due on `today`, and its due date: the earliest
/// month whose due date (the 11th of the month after it, notification 83/2020-CT) is on or after
/// `today`. No holiday shift is applied, and the quarterly (QRMP) dates are not used: the GST
/// screen opens monthly returns only. `None` at the edge of the calendar.
#[must_use]
pub fn gstr1_next_due(today: Date) -> Option<(Period, Date)> {
    let this_month = today.replace_day(1).ok()?;
    let due_this_month = today.replace_day(GSTR1_DUE_DAY).ok()?;
    if today <= due_this_month {
        // The previous month's return is still due (this month's 11th has not passed).
        let last_day = this_month.previous_day()?;
        let first_day = last_day.replace_day(1).ok()?;
        return Some((Period::new(first_day, last_day).ok()?, due_this_month));
    }
    let next_year = if this_month.month() == Month::December {
        this_month.year().checked_add(1)?
    } else {
        this_month.year()
    };
    let next_month = Date::from_calendar_date(next_year, this_month.month().next(), 1).ok()?;
    let period = Period::new(this_month, next_month.previous_day()?).ok()?;
    Some((period, next_month.replace_day(GSTR1_DUE_DAY).ok()?))
}

impl Api {
    /// The Home screen's nudges as of `today`.
    ///
    /// **GSTR-1.** Raised for the monthly return next due ([`gstr1_next_due`]) when the company
    /// has one regular GST registration. No filing record is kept, so a return the user already
    /// filed is still nudged until its due date passes.
    ///
    /// **Rule D-02, possible duplicate payment (our own rule, not taken from any accounting
    /// package).** Two payment vouchers are flagged when all of these hold: both are live
    /// (regular, or post-dated and dated on or before `today`); each has exactly one debit
    /// ledger line; the debit ledger and the amount are the same; the narration is the same once
    /// trimmed, single-spaced and lower-cased, and not empty; the dates are 30 days or fewer
    /// apart; and they do not settle different bills (the bill names on the debit lines match,
    /// or neither has any). Among payments with the same ledger, amount and narration, each is
    /// compared with the one before it in date order, so a long run flags neighbouring pairs.
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn home_nudges(&self, today: Date) -> Result<HomeNudges, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => {
                return Ok(HomeNudges {
                    gstr1: None,
                    duplicates: Outcome::Unsupported(u),
                });
            }
        };
        let lock = self.lock_date();
        Ok(HomeNudges {
            gstr1: self.gstr1_due(books, today),
            duplicates: duplicate_payments(books, today, lock),
        })
    }

    /// The GSTR-1 nudge, when the company has a single regular registration with GST on.
    fn gstr1_due(&self, books: &Books, today: Date) -> Option<Gstr1Due> {
        let gstin = single_regular_gstin(books)?;
        let (period, due) = gstr1_next_due(today)?;
        let needs_correction = match self.gstr1(&gstin, period) {
            Ok(Outcome::Value(ret)) => {
                Outcome::Value(ret.buckets.iter().filter(|b| unsettled(b.bucket)).count())
            }
            Ok(Outcome::Unsupported(u)) => Outcome::Unsupported(u),
            Err(e) => Outcome::Unsupported(Unsupported {
                what: "GSTR-1".to_owned(),
                reason: format!("the return could not be worked out: {e}"),
            }),
        };
        Some(Gstr1Due {
            period,
            due,
            needs_correction,
        })
    }
}

/// `true` when the voucher is not settled in the return.
fn unsettled(bucket: Bucket) -> bool {
    match bucket {
        Bucket::Ready | Bucket::NotRelevant => false,
        Bucket::Mismatch | Bucket::NeedsCorrection => true,
    }
}

/// The GSTIN of the company's one regular registration, when GST is on and there is exactly one.
fn single_regular_gstin(books: &Books) -> Option<Gstin> {
    if !books.company.features.gst {
        return None;
    }
    match books.company.registrations.as_slice() {
        [only] if !only.composition => Some(only.gstin.clone()),
        _ => None,
    }
}

/// What rule D-02 matches on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    ledger: LedgerId,
    amount: i64,
    narration: String,
}

/// One live payment, ready to compare.
struct Candidate<'a> {
    voucher: &'a Voucher,
    bills: BTreeSet<&'a str>,
}

/// Rule D-02 over `books` (see [`Api::home_nudges`]).
fn duplicate_payments(
    books: &Books,
    today: Date,
    lock: Option<Date>,
) -> Outcome<Vec<DuplicatePayment>> {
    let mut groups: BTreeMap<Key, Vec<Candidate<'_>>> = BTreeMap::new();
    for voucher in books.vouchers.values() {
        let is_payment = books
            .masters
            .voucher_types
            .get(&voucher.type_id)
            .is_some_and(|t| t.base == BaseVoucherKind::Payment);
        if !is_payment || !live(voucher, today) {
            continue;
        }
        let mut debits = voucher
            .ledger_lines
            .iter()
            .filter(|l| l.amount.side == DrCr::Dr);
        let (Some(debit), None) = (debits.next(), debits.next()) else {
            continue;
        };
        let narration = normalise(&voucher.narration);
        if narration.is_empty() {
            continue;
        }
        let key = Key {
            ledger: debit.ledger,
            amount: debit.amount.amount.get(),
            narration,
        };
        let bills = debit
            .bills
            .iter()
            .filter(|b| match b.kind {
                BillRefKind::New | BillRefKind::Against => true,
                BillRefKind::Advance | BillRefKind::OnAccount => false,
            })
            .map(|b| b.name.as_str())
            .collect();
        groups
            .entry(key)
            .or_default()
            .push(Candidate { voucher, bills });
    }
    let mut found = Vec::new();
    for (key, mut group) in groups {
        group.sort_by_key(|c| (c.voucher.date, c.voucher.id));
        let Some(ledger) = books.masters.ledgers.get(&key.ledger) else {
            return Outcome::unsupported(
                "possible duplicate payments (rule D-02)",
                format!("ledger {} is not in the books", key.ledger.0),
            );
        };
        for pair in group.windows(2) {
            let [a, b] = pair else { continue };
            let days_apart = (b.voucher.date - a.voucher.date).whole_days();
            if days_apart > DUPLICATE_WINDOW_DAYS || a.bills != b.bills {
                continue;
            }
            let payee = match (a.voucher.party, b.voucher.party) {
                (Some(x), Some(y)) if x == y => {
                    books.masters.ledgers.get(&x).map(|l| l.name.clone())
                }
                _ => None,
            };
            found.push(DuplicatePayment {
                first: payment_ref(a.voucher, lock),
                second: payment_ref(b.voucher, lock),
                ledger: ledger.name.clone(),
                payee,
                amount: Paise::new(key.amount),
                narration: b.voucher.narration.trim().to_owned(),
                days_apart,
            });
        }
    }
    found.sort_by_key(|d| (d.second.date, d.second.id));
    Outcome::Value(found)
}

/// Regular vouchers, and post-dated ones dated on or before `today`.
fn live(voucher: &Voucher, today: Date) -> bool {
    match voucher.status {
        VoucherStatus::Regular => true,
        VoucherStatus::PostDated => voucher.date <= today,
        VoucherStatus::Optional | VoucherStatus::Cancelled => false,
    }
}

/// Trimmed, single-spaced and lower-cased.
fn normalise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn payment_ref(voucher: &Voucher, lock: Option<Date>) -> PaymentRef {
    PaymentRef {
        id: voucher.id,
        number: voucher.number.clone(),
        date: voucher.date,
        locked: lock.is_some_and(|lock| voucher.date <= lock),
        bank_date: voucher
            .ledger_lines
            .iter()
            .filter(|l| l.amount.side == DrCr::Cr)
            .find_map(|l| l.bank.as_ref().and_then(|b| b.bank_date)),
    }
}
