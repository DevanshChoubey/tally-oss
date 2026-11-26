//! What the invoice screen asks the engine for besides tax: the number the voucher will get (the
//! "SI/0418" of the title) and the party's balance now and after saving (the line under the
//! narration). Both come from the API through the worker; the screen shows the answers and adds
//! nothing to them (rule C4).

use ledgerly_api::{Api, BalanceChange, InvoicedAgainst, OpenBill};
use ledgerly_core::{Date, LedgerId, Outcome, VoucherTypeId};
use ledgerly_model::{BaseVoucherKind, Voucher};

use crate::worker::{ApiWorker, Poll, Ticket};

/// The worker's answer for the next voucher number.
type NumberReply = Result<Outcome<Option<String>>, String>;

/// The worker's answer for the balance change.
type BalanceReply = Result<Outcome<BalanceChange>, String>;

/// The worker's answer for a ledger's open bills.
type OpenReply = Result<Outcome<Vec<OpenBill>>, String>;

/// The worker's answer for the invoice a note is against.
type InvoicedReply = Result<Outcome<InvoicedAgainst>, String>;

/// What the invoiced-quantity answer is for: the note's kind, its party and the reference typed.
pub(crate) type InvoicedKey = (BaseVoucherKind, LedgerId, String);

/// The balance line's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BalanceShown {
    /// The engine's balance now and after saving.
    Change(BalanceChange),
    /// The engine cannot give it; the reason is not shown as a figure.
    Missing(String),
}

/// The answers held, and the requests in flight.
#[derive(Default)]
pub(crate) struct Extras {
    /// What the number answer is for.
    number_for: Option<(VoucherTypeId, Date)>,
    /// The number the engine will give, when it gives one.
    pub(crate) number: Option<String>,
    number_ticket: Option<((VoucherTypeId, Date), Ticket<NumberReply>)>,
    /// What the balance answer is for.
    balance_for: Option<(LedgerId, Voucher)>,
    /// The balance answer.
    pub(crate) balance: Option<BalanceShown>,
    balance_ticket: Option<((LedgerId, Voucher), Ticket<BalanceReply>)>,
    /// What the open-bills answer is for.
    open_for: Option<(LedgerId, Date)>,
    /// The open bills of that ledger on that date, when the engine gave them.
    pub(crate) open: Option<Vec<OpenBill>>,
    open_ticket: Option<((LedgerId, Date), Ticket<OpenReply>)>,
    /// What the invoiced answer is for.
    invoiced_for: Option<InvoicedKey>,
    /// The invoice the note is against, when the engine found it.
    pub(crate) invoiced: Option<InvoicedAgainst>,
    invoiced_ticket: Option<(InvoicedKey, Ticket<InvoicedReply>)>,
}

impl Extras {
    /// Forgets the number, so the next frame asks again (after a save the next number is a new one).
    pub(crate) fn forget_number(&mut self) {
        self.number_for = None;
        self.number = None;
    }

    /// Hands in finished answers and asks for the number of a voucher of `want` when the held
    /// answer is for another. Returns whether a request is in flight.
    pub(crate) fn drive_number(
        &mut self,
        worker: &ApiWorker,
        want: Option<(VoucherTypeId, Date)>,
    ) -> bool {
        if let Some((key, mut ticket)) = self.number_ticket.take() {
            match ticket.poll() {
                Poll::Pending | Poll::Taken => self.number_ticket = Some((key, ticket)),
                Poll::Ready(reply) => {
                    self.number = match reply {
                        Ok(Outcome::Value(number)) => number,
                        Ok(Outcome::Unsupported(_)) | Err(_) => None,
                    };
                    self.number_for = Some(key);
                }
                Poll::Lost => {
                    self.number = None;
                    self.number_for = Some(key);
                }
            }
        }
        if self.number_ticket.is_none()
            && let Some(key) = want
            && self.number_for != Some(key)
        {
            let (type_id, date) = key;
            let ticket = worker.run(move |api: &mut Api| {
                api.next_voucher_number(type_id, date)
                    .map_err(|e| e.to_string())
            });
            self.number_ticket = Some((key, ticket));
        }
        self.number_ticket.is_some()
    }

    /// As [`Self::drive_number`] for the balance of `party` once `voucher` (with its ledger lines)
    /// is saved. `None` for `want` clears the line.
    pub(crate) fn drive_balance(
        &mut self,
        worker: &ApiWorker,
        want: Option<(LedgerId, Voucher)>,
    ) -> bool {
        if let Some((key, mut ticket)) = self.balance_ticket.take() {
            match ticket.poll() {
                Poll::Pending | Poll::Taken => self.balance_ticket = Some((key, ticket)),
                Poll::Ready(reply) => {
                    self.balance = Some(match reply {
                        Ok(Outcome::Value(change)) => BalanceShown::Change(change),
                        Ok(Outcome::Unsupported(u)) => BalanceShown::Missing(u.reason),
                        Err(e) => BalanceShown::Missing(e),
                    });
                    self.balance_for = Some(key);
                }
                Poll::Lost => {
                    self.balance = Some(BalanceShown::Missing(
                        "the engine stopped before it answered".to_owned(),
                    ));
                    self.balance_for = Some(key);
                }
            }
        }
        let Some(key) = want else {
            self.balance_for = None;
            self.balance = None;
            return self.balance_ticket.is_some();
        };
        if self.balance_ticket.is_none() && self.balance_for.as_ref() != Some(&key) {
            let (party, voucher) = key.clone();
            let ticket = worker.run(move |api: &mut Api| {
                api.balance_change(party, &voucher)
                    .map_err(|e| e.to_string())
            });
            self.balance_ticket = Some((key, ticket));
        }
        self.balance_ticket.is_some()
    }

    /// As [`Self::drive_number`] for the bills of `ledger` open on `date`. `None` for `want`
    /// forgets the answer. Returns whether a request is in flight.
    pub(crate) fn drive_open_bills(
        &mut self,
        worker: &ApiWorker,
        want: Option<(LedgerId, Date)>,
    ) -> bool {
        if let Some((key, mut ticket)) = self.open_ticket.take() {
            match ticket.poll() {
                Poll::Pending | Poll::Taken => self.open_ticket = Some((key, ticket)),
                Poll::Ready(reply) => {
                    self.open = match reply {
                        Ok(Outcome::Value(bills)) => Some(bills),
                        Ok(Outcome::Unsupported(_)) | Err(_) => None,
                    };
                    self.open_for = Some(key);
                }
                Poll::Lost => {
                    self.open = None;
                    self.open_for = Some(key);
                }
            }
        }
        let Some(key) = want else {
            return self.open_ticket.is_some();
        };
        if self.open_ticket.is_none() && self.open_for != Some(key) {
            let (ledger, date) = key;
            let ticket = worker
                .run(move |api: &mut Api| api.open_bills(ledger, date).map_err(|e| e.to_string()));
            self.open_ticket = Some((key, ticket));
        }
        self.open_ticket.is_some()
    }

    /// As [`Self::drive_number`] for the invoice a note is against. `None` for `want` forgets the
    /// answer. Returns whether a request is in flight.
    pub(crate) fn drive_invoiced(&mut self, worker: &ApiWorker, want: Option<InvoicedKey>) -> bool {
        if let Some((key, mut ticket)) = self.invoiced_ticket.take() {
            match ticket.poll() {
                Poll::Pending | Poll::Taken => self.invoiced_ticket = Some((key, ticket)),
                Poll::Ready(reply) => {
                    self.invoiced = match reply {
                        Ok(Outcome::Value(found)) => Some(found),
                        Ok(Outcome::Unsupported(_)) | Err(_) => None,
                    };
                    self.invoiced_for = Some(key);
                }
                Poll::Lost => {
                    self.invoiced = None;
                    self.invoiced_for = Some(key);
                }
            }
        }
        let Some(key) = want else {
            self.invoiced_for = None;
            self.invoiced = None;
            return self.invoiced_ticket.is_some();
        };
        if self.invoiced_ticket.is_none() && self.invoiced_for.as_ref() != Some(&key) {
            let (kind, party, reference) = key.clone();
            let ticket = worker.run(move |api: &mut Api| {
                api.invoiced_against(kind, party, &reference)
                    .map_err(|e| e.to_string())
            });
            self.invoiced_ticket = Some((key, ticket));
        }
        self.invoiced_ticket.is_some()
    }

    /// The invoice found, when the answer held is for exactly `key`.
    pub(crate) fn invoiced_for(&self, key: &InvoicedKey) -> Option<&InvoicedAgainst> {
        match &self.invoiced_for {
            Some(held) if held == key => self.invoiced.as_ref(),
            Some(_) | None => None,
        }
    }

    /// The open bills, when the answer held is for exactly `ledger` on `date`.
    pub(crate) fn open_bills_for(&self, ledger: LedgerId, date: Date) -> Option<&[OpenBill]> {
        match self.open_for {
            Some(key) if key == (ledger, date) => self.open.as_deref(),
            Some(_) | None => None,
        }
    }

    /// Holds `bills` as the answer for `ledger` on `date` (a host or test that already has them).
    pub(crate) fn set_open_bills(&mut self, ledger: LedgerId, date: Date, bills: Vec<OpenBill>) {
        self.open_for = Some((ledger, date));
        self.open = Some(bills);
    }

    /// The balance answer, when it is for exactly `party` and `voucher`.
    pub(crate) fn balance_for(&self, party: LedgerId, voucher: &Voucher) -> Option<&BalanceShown> {
        match &self.balance_for {
            Some((p, v)) if *p == party && v == voucher => self.balance.as_ref(),
            Some(_) | None => None,
        }
    }
}
