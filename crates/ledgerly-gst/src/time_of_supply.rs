//! Time of supply (CGST s.12, s.13, s.14). **Card G12** implements this module.
//!
//! Frozen interface (plan card G12):
//!
//! ```text
//! pub enum GoodsOrServices { Goods, Services }
//! pub struct SupplyDates { pub invoice: Option<Date>, pub supply: Option<Date>, pub payment: Option<Date>, pub kind: GoodsOrServices }
//! pub enum RateSide { Before, After }
//! pub fn time_of_supply(d: &SupplyDates) -> Outcome<Date>;
//! pub fn rate_change_rule(d: &SupplyDates, change_date: Date) -> Outcome<RateSide>;
//! ```
//!
//! Conventions:
//!
//! - `payment: None` means no payment has been received. Callers must pass the earliest receipt
//!   (the earlier of the bank credit date and the date of entry in the books, the proviso to s.14).
//! - A rate change takes effect on `change_date` itself: a date before it is "before the change",
//!   a date on or after it is "after the change".
//! - Anything the dates alone cannot decide is `Unsupported`, never a guess: a missing needed date,
//!   a goods advance (it depends on the registration type, N.66/2017-CT and N.40/2017-CT), a
//!   service invoice 31 to 45 days after the supply (the 30-day period of rule 47 becomes 45 days
//!   for insurers, banks and financial institutions, and the dates do not say which supplier this
//!   is). Reverse charge (s.12(3), s.13(3)), continuous supplies (s.31(5)) and vouchers (s.12(4),
//!   s.13(4)) are not modelled; the caller must not use this module for them.

use ledgerly_core::{Date, Outcome};

/// Days within which a service invoice must be issued (CGST Rules 2017, rule 47(1)).
const SERVICE_INVOICE_DAYS: i64 = 30;
/// The longer period for insurers, banks, financial institutions and NBFCs (rule 47(1) proviso).
const SERVICE_INVOICE_DAYS_FINANCIAL: i64 = 45;

/// Whether the supply is of goods (s.12) or services (s.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoodsOrServices {
    /// Goods: s.12.
    Goods,
    /// Services: s.13.
    Services,
}

/// The dates that decide time of supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyDates {
    /// Date the invoice was issued; `None` if it has not been issued.
    pub invoice: Option<Date>,
    /// Date goods were removed or delivered, or services were provided; `None` if unknown.
    pub supply: Option<Date>,
    /// Date payment was received; `None` if no payment has been received.
    pub payment: Option<Date>,
    /// Goods or services.
    pub kind: GoodsOrServices,
}

/// Which side of a rate change a supply falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateSide {
    /// The old rate applies (time of supply before the change).
    Before,
    /// The new rate applies (time of supply on or after the change).
    After,
}

/// The earlier of a date and an optional second date.
fn earliest(a: Date, b: Option<Date>) -> Date {
    match b {
        Some(b) => a.min(b),
        None => a,
    }
}

/// Time of supply under s.12 (goods) or s.13 (services), outside a rate change and outside reverse
/// charge. Use [`rate_change_rule`] to pick the rate when a rate changes near the supply.
pub fn time_of_supply(d: &SupplyDates) -> Outcome<Date> {
    let Some(invoice) = d.invoice else {
        return Outcome::unsupported(
            "GST time of supply: invoice date missing",
            "Time of supply needs the invoice date; issue the invoice or enter its date.",
        );
    };
    let Some(supply) = d.supply else {
        return Outcome::unsupported(
            "GST time of supply: supply date missing",
            "Time of supply needs the date goods were removed or delivered, or services provided.",
        );
    };
    match d.kind {
        GoodsOrServices::Goods => goods_time(invoice, supply, d.payment),
        GoodsOrServices::Services => services_time(invoice, supply, d.payment),
    }
}

/// s.12(2): the earlier of the invoice date (or the last date it had to be issued) and the date of
/// payment. The last date for an invoice (s.31(1)) is the removal or delivery date, `supply`.
fn goods_time(invoice: Date, supply: Date, payment: Option<Date>) -> Outcome<Date> {
    if let Some(paid) = payment
        && paid < invoice
    {
        return Outcome::unsupported(
            "GST time of supply: advance received before the invoice for goods",
            "s.12(2)(b) taxes the advance, but N.66/2017-CT exempts it for registered persons other \
             than composition dealers (N.40/2017-CT); the registration type is not an input here.",
        );
    }
    Outcome::Value(invoice.min(supply))
}

/// s.13(2): invoice within the rule 47 period gives the earlier of invoice and payment; otherwise
/// the earlier of the supply date and payment.
fn services_time(invoice: Date, supply: Date, payment: Option<Date>) -> Outcome<Date> {
    let gap = (invoice - supply).whole_days();
    let within_invoice_rule = earliest(invoice, payment);
    let late_invoice_rule = earliest(supply, payment);
    if gap <= SERVICE_INVOICE_DAYS {
        return Outcome::Value(within_invoice_rule);
    }
    if gap > SERVICE_INVOICE_DAYS_FINANCIAL || within_invoice_rule == late_invoice_rule {
        return Outcome::Value(late_invoice_rule);
    }
    Outcome::unsupported(
        "GST time of supply: service invoice 31 to 45 days after the supply",
        "Rule 47 allows 30 days, or 45 days for insurers, banks and financial institutions, and \
         the two readings give different dates; the supplier type is not an input here.",
    )
}

/// Which rate applies when the rate changes on `change_date` (CGST s.14, which overrides s.12 and
/// s.13). The rate is that of the side the time of supply falls on:
///
/// - supply before the change: the old rate if the invoice or the payment is before the change,
///   the new rate only if both are on or after it (s.14(1)(a));
/// - supply on or after the change: the old rate only if both invoice and payment are before the
///   change, otherwise the new rate (s.14(1)(b)).
///
/// A missing invoice or payment date is `Unsupported` only when the answer depends on it.
pub fn rate_change_rule(d: &SupplyDates, change_date: Date) -> Outcome<RateSide> {
    let Some(supply) = d.supply else {
        return Outcome::unsupported(
            "GST rate change: supply date missing",
            "s.14 needs the date of supply to pick the clause; enter the supply date.",
        );
    };
    let side = |date: Option<Date>| {
        date.map(|v| {
            if v < change_date {
                RateSide::Before
            } else {
                RateSide::After
            }
        })
    };
    let pair = (side(d.invoice), side(d.payment));
    let decided = if supply < change_date {
        match pair {
            (Some(RateSide::Before), _) | (_, Some(RateSide::Before)) => Some(RateSide::Before),
            (Some(RateSide::After), Some(RateSide::After)) => Some(RateSide::After),
            (Some(RateSide::After), None) | (None, Some(RateSide::After)) | (None, None) => None,
        }
    } else {
        match pair {
            (Some(RateSide::After), _) | (_, Some(RateSide::After)) => Some(RateSide::After),
            (Some(RateSide::Before), Some(RateSide::Before)) => Some(RateSide::Before),
            (Some(RateSide::Before), None) | (None, Some(RateSide::Before)) | (None, None) => None,
        }
    };
    match decided {
        Some(side) => Outcome::Value(side),
        None => Outcome::unsupported(
            "GST rate change: invoice or payment date missing",
            "s.14 result depends on a date that is not known yet (invoice not issued or payment \
             not received); it cannot be decided until it is.",
        ),
    }
}
