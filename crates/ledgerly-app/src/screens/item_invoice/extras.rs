//! What the invoice screen asks the engine for besides tax: the number the voucher will get (the
//! "SI/0418" of the title) and the party's balance now and after saving (the line under the
//! narration). Both come from the API through the worker; the screen shows the answers and adds
//! nothing to them (rule C4).

use ledgerly_api::{Api, BalanceChange};
use ledgerly_core::{Date, LedgerId, Outcome, VoucherTypeId};
use ledgerly_model::Voucher;

use crate::worker::{ApiWorker, Poll, Ticket};

/// The worker's answer for the next voucher number.
type NumberReply = Result<Outcome<Option<String>>, String>;

/// The worker's answer for the balance change.
type BalanceReply = Result<Outcome<BalanceChange>, String>;

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

    /// The balance answer, when it is for exactly `party` and `voucher`.
    pub(crate) fn balance_for(&self, party: LedgerId, voucher: &Voucher) -> Option<&BalanceShown> {
        match &self.balance_for {
            Some((p, v)) if *p == party && v == voucher => self.balance.as_ref(),
            Some(_) | None => None,
        }
    }
}
